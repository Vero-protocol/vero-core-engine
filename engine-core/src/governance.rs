//! Multi-sig governance with timelock.
//!
//! ## Proposal State Machine
//! ```text
//! Pending ─ (on approve, threshold met) → Approved ─ (on execute, timelock elapsed) → Executed
//! ```
//! Invalid transitions trigger contract panics.

use crate::event_struct::{ACT_APPROVE, ACT_EXECUTE, ACT_PROPOSE, MOD_GOV};
use crate::event_utils::publish_event;
use crate::types::{Proposal, ProposalState};
use soroban_sdk::{
    contracterror, panic_with_error, symbol_short, vec, Address, Env, Map, Symbol, Vec,
};

const KEY_PROPOSALS: Symbol = symbol_short!("PROPS");
const KEY_SIGNERS:   Symbol = symbol_short!("SIGNERS");
const KEY_THRESH:    Symbol = symbol_short!("THRESH");

/// Ledgers to wait after full approval before execution (~1 hour on Stellar).
const TIMELOCK_LEDGERS: u32 = 720;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub enum GovError {
    NotASigner = 1,
    AlreadyApproved = 2,
    ProposalNotFound = 3,
    TimelockActive = 4,
    InvalidStateTransition = 5,
    InvalidThreshold = 6,
}

/// Initialise governance with an ordered signer set and approval threshold.
pub fn init(env: &Env, signers: Vec<Address>, threshold: u32) {
    if threshold == 0 || threshold > signers.len() {
        panic_with_error!(env, GovError::InvalidThreshold);
    }
    env.storage().instance().set(&KEY_SIGNERS, &signers);
    env.storage().instance().set(&KEY_THRESH, &threshold);
    let empty: Map<u64, (Proposal, u32)> = Map::new(env);
    env.storage().instance().set(&KEY_PROPOSALS, &empty);
}

/// Internal helper — load the proposal map.
pub fn load_proposals(env: &Env) -> Map<u64, (Proposal, u32)> {
    env.storage()
        .instance()
        .get(&KEY_PROPOSALS)
        .unwrap_or(Map::new(env))
}

/// Fetch a proposal by id; panics with `ProposalNotFound` if missing.
pub fn get_proposal(env: &Env, proposal_id: u64) -> Proposal {
    let props = load_proposals(env);
    let (prop, _) = props
        .get(proposal_id)
        .unwrap_or_else(|| panic_with_error!(env, GovError::ProposalNotFound));
    prop
}

/// Submit a new proposal. Returns the assigned proposal id.
///
/// The proposal's `proposer` must be a registered signer, and the proposer must
/// authorise the call. State is always initialised to `Pending`.
pub fn propose(env: &Env, mut proposal: Proposal) -> u64 {
    let signers: Vec<Address> = env
        .storage()
        .instance()
        .get(&KEY_SIGNERS)
        .unwrap_or(vec![env]);
    if !signers.contains(&proposal.proposer) {
        panic_with_error!(env, GovError::NotASigner);
    }

    proposal.state = ProposalState::Pending;

    let mut props = load_proposals(env);
    let unlock_ledger = env.ledger().sequence() + TIMELOCK_LEDGERS;
    props.set(proposal.id, (proposal.clone(), unlock_ledger));
    env.storage().instance().set(&KEY_PROPOSALS, &props);

    // Single compact event.
    publish_event(
        env,
        MOD_GOV | ACT_PROPOSE,
        proposal.id,
        proposal.action_hash.clone(),
    );

    proposal.id
}

/// Record a signer's approval for `proposal_id`.
/// Transitions state from Pending → Approved when threshold is met.
pub fn approve(env: &Env, signer: &Address, proposal_id: u64) {
    crate::non_reentrant!(env);
    signer.require_auth();

    let signers: Vec<Address> = env
        .storage()
        .instance()
        .get(&KEY_SIGNERS)
        .unwrap_or(vec![env]);
    if !signers.contains(signer) {
        panic_with_error!(env, GovError::NotASigner);
    }
    let threshold: u32 = env.storage().instance().get(&KEY_THRESH).unwrap_or(1);

    let mut props = load_proposals(env);
    let (mut prop, unlock) = props
        .get(proposal_id)
        .unwrap_or_else(|| panic_with_error!(env, GovError::ProposalNotFound));

    // Only pending proposals can receive approvals.
    if prop.state != ProposalState::Pending {
        panic_with_error!(env, GovError::InvalidStateTransition);
    }

    if prop.approved_by.contains(signer) {
        panic_with_error!(env, GovError::AlreadyApproved);
    }
    prop.approved_by.push_back(signer.clone());

    // Transition to Approved when threshold is met.
    if (prop.approved_by.len() as u32) >= threshold {
        prop.state = ProposalState::Approved;
        publish_event(
            env,
            MOD_GOV | ACT_APPROVE,
            proposal_id,
            prop.action_hash.clone(),
        );
    }

    props.set(proposal_id, (prop, unlock));
    env.storage().instance().set(&KEY_PROPOSALS, &props);
}

/// Execute an approved proposal whose timelock has elapsed.
pub fn execute(env: &Env, proposal_id: u64) -> Proposal {
    crate::non_reentrant!(env);

    let mut props = load_proposals(env);
    let (mut prop, unlock) = props
        .get(proposal_id)
        .unwrap_or_else(|| panic_with_error!(env, GovError::ProposalNotFound));

    if prop.state != ProposalState::Approved {
        panic_with_error!(env, GovError::InvalidStateTransition);
    }
    if env.ledger().sequence() < unlock {
        panic_with_error!(env, GovError::TimelockActive);
    }

    prop.state = ProposalState::Executed;
    props.set(proposal_id, (prop.clone(), unlock));
    env.storage().instance().set(&KEY_PROPOSALS, &props);

    // Single compact event.
    publish_event(
        env,
        MOD_GOV | ACT_EXECUTE,
        proposal_id,
        prop.action_hash.clone(),
    );

    prop
}
