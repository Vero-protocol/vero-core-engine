//! Multi-sig governance hooks — treasury and upgrade decision gating.
//!
//! A `Proposal` requires `threshold` distinct approvals before `execute`
//! can be called. The time-lock window enforces a mandatory delay between
//! full approval and execution, giving stakeholders a veto window.
//!
//! ## Proposal State Machine
//! ```text
//! Pending -- (on approve, threshold met) -> Approved -- (on execute, timelock elapsed) -> Executed
//! ```
//! Invalid transitions trigger contract panics.

use soroban_sdk::{
    contracterror, panic_with_error, symbol_short, token, vec, Address, Env, Map, Symbol, Vec,
};

use crate::types::{Proposal, ProposalState};

const KEY_PROPOSALS:  Symbol = symbol_short!("PROPS");
const KEY_SIGNERS:    Symbol = symbol_short!("SIGNERS");
const KEY_THRESH:     Symbol = symbol_short!("THRESH");
const KEY_MIN_STAKE:  Symbol = symbol_short!("MINSTAKE");
const KEY_STAKE_TOK:  Symbol = symbol_short!("STKTOK");
/// Ledgers to wait after full approval before execution (~1 hour on Stellar).
const TIMELOCK_LEDGERS: u32 = 720;
const SCALING_FACTOR: u64 = 10_000;

#[contracterror]
#[derive(Copy, Clone)]
pub enum GovError {
    NotASigner             = 1,
    AlreadyApproved        = 2,
    ThresholdNotMet        = 3,
    TimelockActive         = 4,
    InvalidStateTransition = 5,
    ProposalNotFound       = 6,
    InsufficientStake      = 7,
}

/// Initialise governance with an ordered signer set, approval threshold, and
/// anti-Sybil stake parameters.
///
/// * `stake_token`  – SAC/token contract address whose balance is checked at vote time.
/// * `min_stake`    – Minimum balance (in token's smallest unit) a signer must hold to vote.
///                    Pass `0` to disable the stake gate.
pub fn init(
    env: &Env,
    signers: Vec<Address>,
    threshold: u32,
    stake_token: Address,
    min_stake: i128,
) {
    assert!(threshold <= signers.len(), "threshold > signer count");
    env.storage().instance().set(&KEY_SIGNERS, &signers);
    env.storage().instance().set(&KEY_WEIGHTS, &weights);
    env.storage().instance().set(&KEY_THRESH, &threshold);
    env.storage().instance().set(&KEY_STAKE_TOK, &stake_token);
    env.storage().instance().set(&KEY_MIN_STAKE, &min_stake);
    let empty: Map<u64, (Proposal, u32)> = Map::new(env);
    env.storage().instance().set(&KEY_PROPOSALS, &empty);
}

fn load_proposals(env: &Env) -> Map<u64, (Proposal, u32)> {
    env.storage().instance().get(&KEY_PROPOSALS).unwrap_or(Map::new(env))
}

/// Submit a new proposal. Returns the assigned proposal id.
pub fn propose(env: &Env, mut proposal: Proposal) -> u64 {
    let signers: Vec<Address> = env.storage().instance().get(&KEY_SIGNERS).unwrap_or(vec![env]);
    if !signers.contains(&proposal.proposer) {
        panic_with_error!(env, GovError::NotASigner);
    }
    // Initialize state to Pending
    proposal.state = ProposalState::Pending;
    
    let mut props = load_proposals(env);
    let unlock_ledger = env.ledger().sequence() + TIMELOCK_LEDGERS;
    props.set(proposal.id, (proposal.clone(), unlock_ledger));
    env.storage().instance().set(&KEY_PROPOSALS, &props);
    env.events().publish(
        (symbol_short!("GOV"), symbol_short!("propose")),
        proposal.id,
    );
    proposal.id
}

/// Record a signer's approval for `proposal_id`.
/// The signer must hold at least `min_stake` tokens to prevent Sybil voting.
/// Transitions state from Pending → Approved when threshold is met.
pub fn approve(env: &Env, signer: &Address, proposal_id: u64) {
    signer.require_auth();
    let signers: Vec<Address> = env.storage().instance().get(&KEY_SIGNERS).unwrap_or(vec![env]);
    if !signers.contains(signer) {
        panic_with_error!(env, GovError::NotASigner);
    }

    // Anti-Sybil: verify the signer holds the required stake at vote time.
    let min_stake: i128 = env.storage().instance().get(&KEY_MIN_STAKE).unwrap_or(0);
    if min_stake > 0 {
        let stake_token: Address = env.storage().instance().get(&KEY_STAKE_TOK).unwrap();
        let balance = token::Client::new(env, &stake_token).balance(signer);
        if balance < min_stake {
            panic_with_error!(env, GovError::InsufficientStake);
        }
    }

    let mut props = load_proposals(env);
    let threshold: u32 = env.storage().instance().get(&KEY_THRESH).unwrap_or(1);
    let (mut prop, unlock) = props.get(proposal_id).unwrap_or_else(|| {
        panic_with_error!(env, GovError::ProposalNotFound)
    });
    
    // Only pending proposals can receive approvals
    if prop.state != ProposalState::Pending {
        panic_with_error!(env, GovError::InvalidStateTransition);
    }
    
    if prop.approved_by.contains(signer) {
        panic_with_error!(env, GovError::AlreadyApproved);
    }
    prop.approved_by.push_back(signer.clone());
    
    // Calculate total weight and approved weight
    let mut total_weight: u64 = 0;
    for i in 0..weights.len() {
        let w = weights.get(i).unwrap();
        total_weight = total_weight.checked_add(w as u64).unwrap_or_else(|| panic!("Total weight overflow"));
    }

    let mut approved_weight: u64 = 0;
    for i in 0..signers.len() {
        let s = signers.get(i).unwrap();
        if prop.approved_by.contains(&s) {
            let w = weights.get(i).unwrap();
            approved_weight = approved_weight.checked_add(w as u64).unwrap_or_else(|| panic!("Approved weight overflow"));
        }
    }

    // Perform strict fixed-point quorum verification
    // approved_weight / total_weight >= threshold / SCALING_FACTOR
    // => approved_weight * SCALING_FACTOR >= threshold * total_weight
    let approved_weight_scaled = approved_weight
        .checked_mul(SCALING_FACTOR)
        .unwrap_or_else(|| panic!("Approved weight multiplication overflow"));
    
    let required_weight = (threshold as u64)
        .checked_mul(total_weight)
        .unwrap_or_else(|| panic!("Required weight multiplication overflow"));

    // Transition to Approved when threshold is met
    if approved_weight_scaled >= required_weight {
        prop.state = ProposalState::Approved;
        env.events().publish(
            (symbol_short!("GOV"), symbol_short!("approved")),
            proposal_id,
        );
    }
    
    props.set(proposal_id, (prop, unlock));
    env.storage().instance().set(&KEY_PROPOSALS, &props);
}

/// Execute a proposal after threshold approvals and time-lock expiry.
/// Transitions state from Approved → Executed.
pub fn execute(env: &Env, proposal_id: u64) -> Proposal {
    let mut props = load_proposals(env);
    let (mut prop, unlock) = props.get(proposal_id).unwrap_or_else(|| {
        panic_with_error!(env, GovError::ProposalNotFound)
    });
    
    // Only approved proposals can be executed
    if prop.state != ProposalState::Approved {
        panic_with_error!(env, GovError::InvalidStateTransition);
    }
    
    if env.ledger().sequence() < unlock {
        panic_with_error!(env, GovError::TimelockActive);
    }
    
    prop.state = ProposalState::Executed;
    props.set(proposal_id, (prop.clone(), unlock));
    env.storage().instance().set(&KEY_PROPOSALS, &props);
    env.events().publish(
        (symbol_short!("GOV"), symbol_short!("execute")),
        proposal_id,
    );
    prop
}

