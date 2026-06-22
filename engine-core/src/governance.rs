//! Multi-sig governance with timelock.
//!
//! ## Storage layout (optimised)
//!
//! | Key          | Storage    | Type                    | Notes                        |
//! |--------------|------------|-------------------------|------------------------------|
//! | `SIGNERS`    | instance   | `Vec<Address>`          | signer set                   |
//! | `THRESH`     | instance   | `u32`                   | approval threshold           |
//! | `MINSTAKE`   | instance   | `i128`                  | optional min stake           |
//! | `STKTOK`     | instance   | `Address`               | stake token contract         |
//! | `P{id}`      | persistent | `(Proposal, u32)`       | per-proposal entry + unlock  |
//!
//! Moving from a single `Map<u64,(Proposal,u32)>` in instance storage to per-key
//! persistent entries means each `approve`/`execute` call only deserialises the
//! single proposal being acted on, not the entire proposal set.

use crate::event_struct::{MOD_GOV, ACT_PROPOSE, ACT_APPROVE, ACT_EXECUTE};
use crate::event_utils::publish_event;
use crate::types::{Proposal, ProposalState};
use soroban_sdk::{
    contracterror, panic_with_error, symbol_short, token, vec, Address, BytesN, Env, IntoVal, Map,
    Symbol, Val, Vec,
};

const KEY_SIGNERS:   Symbol = symbol_short!("SIGNERS");
const KEY_THRESH:    Symbol = symbol_short!("THRESH");
const KEY_MIN_STAKE: Symbol = symbol_short!("MINSTAKE");
const KEY_STAKE_TOK: Symbol = symbol_short!("STKTOK");

/// Persistent TTL constants (in ledgers).  ~30 days at 5-second ledger time.
const PROPOSAL_TTL_THRESHOLD: u32 = 17_280;
const PROPOSAL_TTL_EXTEND_TO: u32 = 17_280 * 30;

const TIMELOCK_LEDGERS: u32 = 720;

#[contracterror]
#[derive(Copy, Clone)]
pub enum GovError {
    NotASigner              = 1,
    AlreadyApproved         = 2,
    ProposalNotFound        = 3,
    TimelockActive          = 4,
    InvalidStateTransition  = 5,
    AlreadyExecuted         = 6,
    InsufficientStake       = 7,
}

pub fn init(
    env: &Env,
    signers: Vec<Address>,
    threshold: u32,
    stake_token: Address,
    min_stake: i128,
) {
    if threshold == 0 || threshold > signers.len() {
        panic_with_error!(env, GovError::NotASigner);
    }
    env.storage().instance().set(&KEY_SIGNERS,   &signers);
    env.storage().instance().set(&KEY_THRESH,    &threshold);
    env.storage().instance().set(&KEY_STAKE_TOK, &stake_token);
    env.storage().instance().set(&KEY_MIN_STAKE, &min_stake);
}

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

pub fn approve(env: &Env, voter: &Address, proposal_id: u64) {
    voter.require_auth();
    require_signer(env, voter);
    require_min_stake(env, voter);

    let key = proposal_key(env, proposal_id);
    let (mut prop, unlock): (Proposal, u32) = env
        .storage()
        .persistent()
        .get(&key)
        .unwrap_or_else(|| panic_with_error!(env, GovError::ProposalNotFound));

    if prop.state != ProposalState::Pending {
        panic_with_error!(env, GovError::InvalidStateTransition);
    }

    if prop.approved_by.contains(voter) {
        panic_with_error!(env, GovError::AlreadyApproved);
    }

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
    }

    env.storage().persistent().set(&key, &(prop.clone(), unlock));
    extend_proposal_ttl(env, &key);

    if prop.state == ProposalState::Approved && env.ledger().sequence() >= unlock {
        execute(env, proposal_id);
    }
}

pub fn execute(env: &Env, proposal_id: u64) -> Proposal {
    let key = proposal_key(env, proposal_id);
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

    let mut props: Map<u64, (Proposal, u32)> = env
        .storage()
        .instance()
        .get(&KEY_PROPOSALS)
        .unwrap_or_else(|| panic_with_error!(env, GovError::ProposalNotFound));

    let (mut prop, unlock) = props
        .get(proposal_id)
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
    props.set(proposal_id, (prop.clone(), unlock));
    env.storage().instance().set(&KEY_PROPOSALS, &props);

    env.events().publish(
        (symbol_short!("GOV"), symbol_short!("cancel")),
        proposal_id,
    );

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

fn require_signer(env: &Env, voter: &Address) {
    let signers: Vec<Address> = env
        .storage()
        .instance()
        .get(&KEY_SIGNERS)
        .unwrap_or(vec![env]);
    if !signers.contains(voter) {
        panic_with_error!(env, GovError::NotASigner);
    }
}

fn require_min_stake(env: &Env, voter: &Address) {
    let min_stake: i128 = env.storage().instance().get(&KEY_MIN_STAKE).unwrap_or(0);
    if min_stake == 0 {
        return;
    }
    let stake_token: Address = env
        .storage()
        .instance()
        .get(&KEY_STAKE_TOK)
        .unwrap_or_else(|| panic_with_error!(env, GovError::NotASigner));
    let balance = token::Client::new(env, &stake_token).balance(voter);
    if balance < min_stake {
        panic_with_error!(env, GovError::InsufficientStake);
    }
}
