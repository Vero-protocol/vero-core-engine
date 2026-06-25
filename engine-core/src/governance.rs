//! Multi-sig governance with timelock.
//!
//! ## Storage layout (optimised)
//!
//! ## Proposal State Machine
//! ```text
//! Pending -> (on approve, threshold met) -> Approved -> (on execute, timelock elapsed) -> Executed
//! ```
//! Invalid transitions trigger contract panics.

use crate::event_struct::{MOD_GOV, ACT_PROPOSE, ACT_APPROVE, ACT_EXECUTE};
use crate::event_utils::publish_event;
use crate::types::{Proposal, ProposalState};
use soroban_sdk::{
    contracterror, panic_with_error, symbol_short, vec, Address, Env, Map, Symbol, Vec,
};

const KEY_PROPOSALS:  Symbol = symbol_short!("PROPS");
const KEY_SIGNERS:    Symbol = symbol_short!("SIGNERS");
const KEY_THRESH:     Symbol = symbol_short!("THRESH");
const KEY_MIN_STAKE:  Symbol = symbol_short!("MINSTAKE");
const KEY_STAKE_TOK:  Symbol = symbol_short!("STKTOK");
    Symbol, Val, Vec,
};

const KEY_PROPOSALS: Symbol = symbol_short!("PROPS");
const KEY_SIGNERS: Symbol = symbol_short!("SIGNERS");
const KEY_THRESH: Symbol = symbol_short!("THRESH");
const KEY_NEXT_ID: Symbol = symbol_short!("NEXT_ID");
/// Ledgers to wait after full approval before execution (~1 hour on Stellar).
const TIMELOCK_LEDGERS: u32 = 720;
const MAX_THRESHOLD: u32 = 100;
const MAX_DURATION_LEDGERS: u32 = 5256000;
const MIN_DURATION_LEDGERS: u32 = 1;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum GovError {
    NotASigner = 1,
    AlreadyApproved = 2,
    ProposalNotFound = 3,
    InvalidStateTransition = 4,
    TimelockActive = 5,
    InsufficientStake = 6,
    InvalidThreshold = 7,
    InvalidStake = 8,
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum GovError {
    NotASigner = 1,
    AlreadyApproved = 2,
    ThresholdNotMet = 3,
    TimelockActive = 4,
    InvalidStateTransition = 5,
    ProposalNotFound = 6,
    InvalidThreshold = 7,
}

/// Initialise governance with an ordered signer set and approval threshold.
pub fn init(env: &Env, signers: Vec<Address>, threshold: u32) {
    if threshold == 0 || threshold > signers.len() {
        panic_with_error!(env, GovError::InvalidThreshold);
    }
    env.storage().instance().set(&KEY_SIGNERS, &signers);
    env.storage().instance().set(&KEY_THRESH, &threshold);
    env.storage().instance().set(&KEY_NEXT_ID, &1u64);
    let empty: Map<u64, (Proposal, u32)> = Map::new(env);
    env.storage().instance().set(&KEY_PROPOSALS, &empty);
}

fn load_proposals(env: &Env) -> Map<u64, (Proposal, u32)> {
    env.storage()
        .instance()
        .get(&KEY_PROPOSALS)
        .unwrap_or(Map::new(env))
}

/// Submit a new proposal. Returns the assigned proposal id.
pub fn propose(env: &Env, mut proposal: Proposal) -> u64 {
    let signers: Vec<Address> = env
        .storage()
        .instance()
        .get(&KEY_SIGNERS)
        .unwrap_or(vec![env]);
    if !signers.contains(&proposal.proposer) {
        panic_with_error!(env, GovError::NotASigner);
    }

    let id: u64 = env.storage().instance().get(&KEY_NEXT_ID).unwrap_or(1);
    proposal.id = id;
    env.storage().instance().set(&KEY_NEXT_ID, &(id + 1));

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
    let mut payload = Map::new(env);
    payload.set(Symbol::new(env, "proposal_id"), proposal.id.into_val(env));
    publish_event(
        env,
        BytesN::from_array(env, &[0u8; 32]),
        BytesN::from_array(env, &[0u8; 32]),
        payload,
    );
    proposal.id
pub fn propose(env: &Env, proposal: Proposal) -> u64 {
    let unlock_ledger = env.ledger().sequence() + TIMELOCK_LEDGERS;
    let id = proposal.id;

    let mut prop = proposal;
    prop.state = ProposalState::Pending;

    let key = proposal_key(env, id);
    env.storage().persistent().set(&key, &(prop, unlock_ledger));
    extend_proposal_ttl(env, &key);

    // Single compact event.
    publish_event(
        env,
        MOD_GOV | ACT_PROPOSE,
        id,
        BytesN::from_array(env, &[0u8; 32]),
    );

    id
}

/// Record a signer's approval for `proposal_id`.
/// Transitions state from Pending → Approved when threshold is met.
pub fn approve(env: &Env, signer: &Address, proposal_id: u64) {
    let signers: Vec<Address> = env
        .storage()
        .instance()
        .get(&KEY_SIGNERS)
        .unwrap_or(vec![env]);
    if !signers.contains(signer) {
        panic_with_error!(env, GovError::NotASigner);
    }
    let threshold: u32 = env.storage().instance().get(&KEY_THRESH).unwrap_or(1);
    let min_stake: i128 = env.storage().instance().get(&KEY_MIN_STAKE).unwrap_or(0);
    let stake_token: Address = env.storage().instance().get(&KEY_STAKE_TOK).unwrap();

    let mut props = load_proposals(env);
    let (mut prop, unlock) = props
        .get(proposal_id)
        .unwrap_or_else(|| panic_with_error!(env, GovError::ProposalNotFound));

    // Only pending proposals can receive approvals
    if prop.state != ProposalState::Pending {
        panic_with_error!(env, GovError::InvalidStateTransition);
    }

    if prop.approved_by.contains(signer) {
        panic_with_error!(env, GovError::AlreadyApproved);
    }
    prop.approved_by.push_back(signer.clone());

    // Transition to Approved when threshold is met
    if prop.approved_by.len() >= threshold {
        prop.state = ProposalState::Approved;
        env.events().publish(
            (symbol_short!("GOV"), symbol_short!("approved")),
    prop.approved_by.push_back(voter.clone());

    // Audit log: record every vote, not only the one that meets threshold.
    // Topics carry the voter so logs can be filtered per address; the data and
    // structured payload carry the proposal id and the running approval tally.
    let votes_cast = prop.approved_by.len();

    env.events().publish(
        (symbol_short!("GOV"), symbol_short!("vote"), voter.clone()),
        (proposal_id, votes_cast),
    );

    let mut vote_payload = Map::new(env);
    vote_payload.set(Symbol::short("proposal_id"), proposal_id.into());
    vote_payload.set(Symbol::short("voter"), voter.clone().into_val(env));
    vote_payload.set(Symbol::short("votes"), votes_cast.into());
    publish_event(
        env,
        BytesN::from_array(env, &[0u8; 32]),
        BytesN::from_array(env, &[0u8; 32]),
        vote_payload,
    );

    let threshold: u32 = env.storage().instance().get(&KEY_THRESH).unwrap_or(1);

    if prop.approved_by.len() >= threshold {
        prop.state = ProposalState::Approved;

        // Single compact event for approval.
        publish_event(
            env,
            MOD_GOV | ACT_APPROVE,
            proposal_id,
            BytesN::from_array(env, &[0u8; 32]),
        );
        let mut payload = Map::new(env);
        payload.set(Symbol::new(env, "proposal_id"), proposal_id.into_val(env));
        publish_event(
            env,
            BytesN::from_array(env, &[0u8; 32]),
            BytesN::from_array(env, &[0u8; 32]),
            payload,
        );
    }

    props.set(proposal_id, (prop, unlock));
    env.storage().instance().set(&KEY_PROPOSALS, &props);

    }

    env.storage().persistent().set(&key, &(prop.clone(), unlock));
    extend_proposal_ttl(env, &key);

    if prop.state == ProposalState::Approved && env.ledger().sequence() >= unlock {
        execute(env, proposal_id);
    }
}

pub fn execute(env: &Env, proposal_id: u64) -> Proposal {
    let mut props = load_proposals(env);
    let (mut prop, unlock) = props
        .get(proposal_id)
        .unwrap_or_else(|| panic_with_error!(env, GovError::ProposalNotFound));
    let threshold: u32 = env.storage().instance().get(&KEY_THRESH).unwrap_or(1);
    if prop.approved_by.len() < threshold {
        panic_with_error!(env, GovError::ThresholdNotMet);
    }

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
    env.storage().persistent().set(&key, &(prop.clone(), unlock));
    // Extend TTL so the executed record remains accessible for the audit window.
    extend_proposal_ttl(env, &key);

    // Single compact event.
    publish_event(
        env,
        MOD_GOV | ACT_EXECUTE,
        proposal_id,
        prop.action_hash.clone(),
    );

    prop
}

/// Cancel (roll back) a proposal that has not yet been executed.
///
/// Reverts the proposal to the terminal `Cancelled` state so it can no longer
/// be approved or executed. Only a governance signer may cancel, and only while
/// the proposal is still in a non-terminal state — an already executed proposal
/// cannot be undone, and an already cancelled proposal cannot be cancelled again.
pub fn cancel(env: &Env, caller: &Address, proposal_id: u64) -> Proposal {
    caller.require_auth();
    require_signer(env, caller);

    let key = proposal_key(env, proposal_id);
    let (mut prop, unlock): (Proposal, u32) = env
        .storage()
        .persistent()
        .get(&key)
        .unwrap_or_else(|| panic_with_error!(env, GovError::ProposalNotFound));

    // An executed proposal is terminal and cannot be rolled back.
    if prop.state == ProposalState::Executed {
        panic_with_error!(env, GovError::AlreadyExecuted);
    }

    // Reject any other invalid transition (e.g. cancelling an already
    // cancelled proposal).
    if prop.state == ProposalState::Cancelled {
        panic_with_error!(env, GovError::InvalidStateTransition);
    }

    prop.state = ProposalState::Cancelled;
    env.storage().persistent().set(&key, &(prop.clone(), unlock));
    extend_proposal_ttl(env, &key);

    env.events().publish(
        (symbol_short!("GOV"), symbol_short!("cancel")),
        proposal_id,
    );
    let mut payload = Map::new(env);
    payload.set(Symbol::new(env, "proposal_id"), proposal_id.into_val(env));

    let mut payload = Map::new(env);
    payload.set(Symbol::short("proposal_id"), proposal_id.into());
    publish_event(
        env,
        BytesN::from_array(env, &[0u8; 32]),
        BytesN::from_array(env, &[0u8; 32]),
        payload,
    );
    prop
}
