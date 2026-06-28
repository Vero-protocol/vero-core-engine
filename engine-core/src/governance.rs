//! Multi-signature governance with timelock.
//!
//! Proposals are stored individually in persistent storage to keep the instance
//! footprint bounded. A proposal moves through exactly this state machine:
//!
//! `Pending -> Approved -> Executed`.

use crate::circuit_breaker::assert_closed;
use crate::event_struct::{ACT_APPROVE, ACT_EXECUTE, ACT_PROPOSE, MOD_GOV};
use crate::event_utils::publish_event;
use crate::types::{Proposal, ProposalState};
use soroban_sdk::{contracterror, contracttype, panic_with_error, symbol_short, vec, Address, BytesN, Env, Symbol, Vec};

const KEY_SIGNERS: Symbol = symbol_short!("SIGNERS");
const KEY_THRESH: Symbol = symbol_short!("THRESH");

/// Ledgers to wait after quorum before execution (~1 hour on Stellar).
pub const TIMELOCK_LEDGERS: u32 = 720;
const PROPOSAL_TTL_THRESHOLD: u32 = 17_280;
const PROPOSAL_TTL_EXTEND_TO: u32 = 17_280 * 30;

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GovKey {
    Proposal(u64),
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub enum GovError {
    NotASigner = 1,
    AlreadyApproved = 2,
    ThresholdNotMet = 3,
    TimelockActive = 4,
    InvalidStateTransition = 5,
    ProposalNotFound = 6,
    InvalidThreshold = 7,
    DuplicateSigner = 8,
    DuplicateProposal = 9,
}

/// Initialise governance with a signer set and approval threshold.
pub fn init(env: &Env, signers: Vec<Address>, threshold: u32) {
    if threshold == 0 || threshold > signers.len() {
        panic_with_error!(env, GovError::InvalidThreshold);
    }
    ensure_unique_signers(env, &signers);
    env.storage().instance().set(&KEY_SIGNERS, &signers);
    env.storage().instance().set(&KEY_THRESH, &threshold);
}

/// Return configured signers.
pub fn signers(env: &Env) -> Vec<Address> {
    env.storage().instance().get(&KEY_SIGNERS).unwrap_or(vec![env])
}

/// Return configured approval threshold.
pub fn threshold(env: &Env) -> u32 {
    env.storage().instance().get(&KEY_THRESH).unwrap_or(1)
}

/// Submit a new proposal. Returns the proposal id.
pub fn propose(env: &Env, proposal: Proposal) -> u64 {
    crate::non_reentrant!(env);
    assert_closed(env);
    let signers = signers(env);
    if !signers.contains(&proposal.proposer) {
        panic_with_error!(env, GovError::NotASigner);
    }
    proposal.proposer.require_auth();

    let key = proposal_key(proposal.id);
    if env.storage().persistent().has(&key) {
        panic_with_error!(env, GovError::DuplicateProposal);
    }

    let id = proposal.id;
    let mut prop = proposal;
    prop.state = ProposalState::Pending;
    prop.approved_by = vec![env];
    let unlock_ledger = env.ledger().sequence() + TIMELOCK_LEDGERS;

    env.storage().persistent().set(&key, &(prop, unlock_ledger));
    extend_proposal_ttl(env, &key);

    publish_event(env, MOD_GOV | ACT_PROPOSE, id, BytesN::from_array(env, &[0u8; 32]));
    id
}

/// Record one signer approval. When quorum is reached, state becomes Approved.
pub fn approve(env: &Env, signer: &Address, proposal_id: u64) {
    crate::non_reentrant!(env);
    assert_closed(env);
    signer.require_auth();
    let configured = signers(env);
    if !configured.contains(signer) {
        panic_with_error!(env, GovError::NotASigner);
    }

    let key = proposal_key(proposal_id);
    let (mut prop, unlock): (Proposal, u32) = env
        .storage()
        .persistent()
        .get(&key)
        .unwrap_or_else(|| panic_with_error!(env, GovError::ProposalNotFound));

    if prop.state != ProposalState::Pending {
        panic_with_error!(env, GovError::InvalidStateTransition);
    }
    if prop.approved_by.contains(signer) {
        panic_with_error!(env, GovError::AlreadyApproved);
    }

    prop.approved_by.push_back(signer.clone());
    if prop.approved_by.len() >= threshold(env) {
        prop.state = ProposalState::Approved;
    }

    env.storage().persistent().set(&key, &(prop.clone(), unlock));
    extend_proposal_ttl(env, &key);

    publish_event(env, MOD_GOV | ACT_APPROVE, proposal_id, prop.action_hash.clone());
}

/// Execute an approved proposal after the timelock expires.
pub fn execute(env: &Env, proposal_id: u64) -> Proposal {
    crate::non_reentrant!(env);
    assert_closed(env);
    let key = proposal_key(proposal_id);
    let (mut prop, unlock): (Proposal, u32) = env
        .storage()
        .persistent()
        .get(&key)
        .unwrap_or_else(|| panic_with_error!(env, GovError::ProposalNotFound));

    if prop.state != ProposalState::Approved {
        panic_with_error!(env, GovError::InvalidStateTransition);
    }
    if env.ledger().sequence() < unlock {
        panic_with_error!(env, GovError::TimelockActive);
    }

    prop.state = ProposalState::Executed;
    env.storage().persistent().set(&key, &(prop.clone(), unlock));
    extend_proposal_ttl(env, &key);

    publish_event(env, MOD_GOV | ACT_EXECUTE, proposal_id, prop.action_hash.clone());
    prop
}

pub fn get_proposal(env: &Env, proposal_id: u64) -> Proposal {
    let key = proposal_key(proposal_id);
    let (prop, _): (Proposal, u32) = env
        .storage()
        .persistent()
        .get(&key)
        .unwrap_or_else(|| panic_with_error!(env, GovError::ProposalNotFound));
    prop
}

pub fn get_proposal_with_unlock(env: &Env, proposal_id: u64) -> (Proposal, u32) {
    let key = proposal_key(proposal_id);
    env.storage()
        .persistent()
        .get(&key)
        .unwrap_or_else(|| panic_with_error!(env, GovError::ProposalNotFound))
}

fn proposal_key(id: u64) -> GovKey {
    GovKey::Proposal(id)
}

fn extend_proposal_ttl(env: &Env, key: &GovKey) {
    env.storage()
        .persistent()
        .extend_ttl(key, PROPOSAL_TTL_THRESHOLD, PROPOSAL_TTL_EXTEND_TO);
}

fn ensure_unique_signers(env: &Env, signers: &Vec<Address>) {
    let mut seen = vec![env];
    for signer in signers.iter() {
        if seen.contains(&signer) {
            panic_with_error!(env, GovError::DuplicateSigner);
        }
        seen.push_back(signer);
    }
}
