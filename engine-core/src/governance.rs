//! Multi-sig governance with timelock.
//!
//! Typical lifecycle: `init` → `propose` → `approve` (≥ threshold) → `execute`.
//! Execution is blocked until `TIMELOCK_LEDGERS` ledgers have passed since
//! the proposal was submitted.

use soroban_sdk::{contracterror, panic_with_error, symbol_short, vec, Address, BytesN, Env, Map, Symbol, Vec};

use crate::event_struct::{ACT_APPROVE, ACT_EXECUTE, ACT_PROPOSE, MOD_GOV};
use crate::event_utils::publish_event;
use crate::types::{Proposal, ProposalState};

const KEY_PROPOSALS: Symbol = symbol_short!("PROPS");
const KEY_SIGNERS:   Symbol = symbol_short!("SIGNERS");
const KEY_THRESH:    Symbol = symbol_short!("THRESH");

/// Ledgers to wait before an approved proposal may be executed (~1 hour on Stellar).
const TIMELOCK_LEDGERS: u32 = 720;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum GovError {
    NotASigner             = 1,
    AlreadyApproved        = 2,
    ThresholdNotMet        = 3,
    TimelockActive         = 4,
    InvalidStateTransition = 5,
    ProposalNotFound       = 6,
    InsufficientStake      = 7,
    InvalidThreshold       = 8,
}

/// Initialise governance with `signers` and a minimum-approval `threshold`.
pub fn init(env: &Env, signers: Vec<Address>, threshold: u32) {
    if threshold == 0 {
        panic_with_error!(env, GovError::InvalidThreshold);
    }
    env.storage().instance().set(&KEY_SIGNERS, &signers);
    env.storage().instance().set(&KEY_THRESH, &threshold);
    let empty: Map<u64, (Proposal, u32)> = Map::new(env);
    env.storage().instance().set(&KEY_PROPOSALS, &empty);
}

/// Returns all proposals as `Map<proposal_id, (Proposal, unlock_ledger)>`.
pub fn load_proposals(env: &Env) -> Map<u64, (Proposal, u32)> {
    env.storage()
        .instance()
        .get(&KEY_PROPOSALS)
        .unwrap_or(Map::new(env))
}

/// Submit a new proposal. `proposal.proposer` must be an authorised signer.
/// Returns the proposal id.
pub fn propose(env: &Env, proposal: Proposal) -> u64 {
    let signers: Vec<Address> = env
        .storage()
        .instance()
        .get(&KEY_SIGNERS)
        .unwrap_or(vec![env]);
    if !signers.contains(&proposal.proposer) {
        panic_with_error!(env, GovError::NotASigner);
    }

    let mut prop = proposal;
    prop.state = ProposalState::Pending;
    let id = prop.id;
    let unlock_ledger = env.ledger().sequence() + TIMELOCK_LEDGERS;

    let mut props = load_proposals(env);
    props.set(id, (prop, unlock_ledger));
    env.storage().instance().set(&KEY_PROPOSALS, &props);

    publish_event(env, MOD_GOV | ACT_PROPOSE, id, BytesN::from_array(env, &[0u8; 32]));

    id
}

/// Record `signer`'s approval. Transitions Pending → Approved when threshold is met.
pub fn approve(env: &Env, signer: &Address, proposal_id: u64) {
    let _guard = crate::guards::ReentrancyGuard::enter(env);
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

    if prop.state != ProposalState::Pending {
        panic_with_error!(env, GovError::InvalidStateTransition);
    }
    if prop.approved_by.contains(signer) {
        panic_with_error!(env, GovError::AlreadyApproved);
    }

    prop.approved_by.push_back(signer.clone());

    if prop.approved_by.len() as u32 >= threshold {
        prop.state = ProposalState::Approved;
        publish_event(env, MOD_GOV | ACT_APPROVE, proposal_id, BytesN::from_array(env, &[0u8; 32]));
    }

    props.set(proposal_id, (prop, unlock));
    env.storage().instance().set(&KEY_PROPOSALS, &props);
}

/// Execute an approved proposal after the timelock. Returns the executed `Proposal`.
pub fn execute(env: &Env, proposal_id: u64) -> Proposal {
    let _guard = crate::guards::ReentrancyGuard::enter(env);

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

    publish_event(env, MOD_GOV | ACT_EXECUTE, proposal_id, prop.action_hash.clone());

    prop
}
