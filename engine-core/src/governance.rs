//! Multi-sig governance hooks — treasury and upgrade decision gating.
//!
//! A `Proposal` requires `threshold` distinct approvals before `execute`
//! can be called. The time-lock window enforces a mandatory delay between
//! full approval and execution, giving stakeholders a veto window.

use soroban_sdk::{
    contracttype, panic_with_error, symbol_short, vec, Address, Env, Map, Symbol, Vec,
};

use crate::types::Proposal;

const KEY_PROPOSALS: Symbol = symbol_short!("PROPS");
const KEY_SIGNERS:   Symbol = symbol_short!("SIGNERS");
const KEY_THRESH:    Symbol = symbol_short!("THRESH");
/// Ledgers to wait after full approval before execution (~1 hour on Stellar).
const TIMELOCK_LEDGERS: u32 = 720;

#[contracttype]
#[derive(Copy, Clone)]
pub enum GovError {
    NotASigner       = 1,
    AlreadyApproved  = 2,
    ThresholdNotMet  = 3,
    TimelockActive   = 4,
    AlreadyExecuted  = 5,
    ProposalNotFound = 6,
}

/// Initialise governance with an ordered signer set and approval threshold.
pub fn init(env: &Env, signers: Vec<Address>, threshold: u32) {
    assert!(threshold as usize <= signers.len(), "threshold > signer count");
    env.storage().instance().set(&KEY_SIGNERS, &signers);
    env.storage().instance().set(&KEY_THRESH, &threshold);
    let empty: Map<u64, (Proposal, u32)> = Map::new(env);
    env.storage().instance().set(&KEY_PROPOSALS, &empty);
}

fn load_proposals(env: &Env) -> Map<u64, (Proposal, u32)> {
    env.storage().instance().get(&KEY_PROPOSALS).unwrap_or(Map::new(env))
}

/// Submit a new proposal. Returns the assigned proposal id.
pub fn propose(env: &Env, proposal: Proposal) -> u64 {
    let signers: Vec<Address> = env.storage().instance().get(&KEY_SIGNERS).unwrap_or(vec![env]);
    if !signers.contains(&proposal.proposer) {
        panic_with_error!(env, GovError::NotASigner);
    }
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
pub fn approve(env: &Env, signer: &Address, proposal_id: u64) {
    signer.require_auth();
    let signers: Vec<Address> = env.storage().instance().get(&KEY_SIGNERS).unwrap_or(vec![env]);
    if !signers.contains(signer) {
        panic_with_error!(env, GovError::NotASigner);
    }

    let mut props = load_proposals(env);
    let (mut prop, unlock) = props.get(proposal_id).unwrap_or_else(|| {
        panic_with_error!(env, GovError::ProposalNotFound)
    });
    if prop.approved_by.contains(signer) {
        panic_with_error!(env, GovError::AlreadyApproved);
    }
    prop.approved_by.push_back(signer.clone());
    props.set(proposal_id, (prop, unlock));
    env.storage().instance().set(&KEY_PROPOSALS, &props);
}

/// Execute a proposal after threshold approvals and time-lock expiry.
pub fn execute(env: &Env, proposal_id: u64) -> Proposal {
    let threshold: u32 = env.storage().instance().get(&KEY_THRESH).unwrap_or(1);
    let mut props = load_proposals(env);
    let (mut prop, unlock) = props.get(proposal_id).unwrap_or_else(|| {
        panic_with_error!(env, GovError::ProposalNotFound)
    });
    if prop.executed {
        panic_with_error!(env, GovError::AlreadyExecuted);
    }
    if (prop.approved_by.len() as u32) < threshold {
        panic_with_error!(env, GovError::ThresholdNotMet);
    }
    if env.ledger().sequence() < unlock {
        panic_with_error!(env, GovError::TimelockActive);
    }
    prop.executed = true;
    props.set(proposal_id, (prop.clone(), unlock));
    env.storage().instance().set(&KEY_PROPOSALS, &props);
    env.events().publish(
        (symbol_short!("GOV"), symbol_short!("execute")),
        proposal_id,
    );
    prop
}
