//! Multi-sig governance hooks — treasury and upgrade decision gating.
//!
//! A `Proposal` requires `threshold` distinct approvals before `execute`
//! can be called. The time-lock window enforces a mandatory delay between
//! full approval and execution, giving stakeholders a veto window.
//!
//! ## Proposal State Machine
//! ```
//! Pending ─ (on approve, threshold met) → Approved ─ (on execute, timelock elapsed) → Executed
//! ```
//! Invalid transitions trigger contract panics.

use soroban_sdk::{
    contracterror, panic_with_error, symbol_short, token, vec, Address, Env, Map, Symbol, Vec,
};

use crate::types::{Proposal, ProposalState};
use crate::circuit_breaker;
use crate::access;

const KEY_PROPOSALS:  Symbol = symbol_short!("PROPS");
const KEY_SIGNERS:    Symbol = symbol_short!("SIGNERS");
const KEY_SIGNER_MAP: Symbol = symbol_short!("SIGNMAP");
const KEY_THRESH:     Symbol = symbol_short!("THRESH");
const KEY_MIN_STAKE:  Symbol = symbol_short!("MINSTAKE");
const KEY_STAKE_TOK:  Symbol = symbol_short!("STKTOK");
/// Ledgers to wait after full approval before execution (~1 hour on Stellar).
const TIMELOCK_LEDGERS: u32 = 720;

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
    // Build a map index for O(1) signer membership checks
    let mut signers_map: Map<Address, bool> = Map::new(env);
    let len = signers.len();
    let mut i = 0usize;
    while i < len {
        let a = signers.get(i).unwrap();
        signers_map.set(a, true);
        i += 1;
    }
    env.storage().instance().set(&KEY_SIGNER_MAP, &signers_map);
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
    circuit_breaker::assert_closed(env);
    // Prefer indexed signer lookup when available to avoid O(n) scans
    let signer_map: Map<Address, bool> = env
        .storage()
        .instance()
        .get(&KEY_SIGNER_MAP)
        .unwrap_or(Map::new(env));
    // Allow either a signer from the configured signer set, or an on-chain roleed OP/ADMIN
    if !signer_map.get(&proposal.proposer).unwrap_or(false)
        && !access::has_role(env, &proposal.proposer, access::ROLE_OPERATOR)
        && !access::has_role(env, &proposal.proposer, access::ROLE_ADMIN)
    {
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
    circuit_breaker::assert_closed(env);
    signer.require_auth();
    // Prefer indexed signer lookup when available to avoid O(n) scans
    let signer_map: Map<Address, bool> = env
        .storage()
        .instance()
        .get(&KEY_SIGNER_MAP)
        .unwrap_or(Map::new(env));
    if !signer_map.get(signer).unwrap_or(false)
        && !access::has_role(env, signer, access::ROLE_OPERATOR)
        && !access::has_role(env, signer, access::ROLE_ADMIN)
    {
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
    
    // Transition to Approved when threshold is met
    if (prop.approved_by.len() as u32) >= threshold {
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
    circuit_breaker::assert_closed(env);
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

