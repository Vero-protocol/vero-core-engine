//! Multi-sig governance with timelock.
//!
//! ## Storage layout (optimised)
//!
//! ## Proposal State Machine
//! ```text
//! Pending ─ (on approve, threshold met) → Approved ─ (on execute, timelock elapsed) → Executed
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
#[derive(Copy, Clone)]
pub enum GovError {
    NotASigner = 1,
    AlreadyApproved = 2,
    ThresholdNotMet = 3,
    TimelockActive = 4,
    InvalidStateTransition = 5,
    ProposalNotFound = 6,
}

/// Initialise governance with an ordered signer set and approval threshold.
pub fn init(env: &Env, signers: Vec<Address>, threshold: u32) {
    assert!(threshold <= (signers.len() as u32), "threshold > signer count");
    env.storage().instance().set(&KEY_SIGNERS, &signers);
    env.storage().instance().set(&KEY_THRESH, &threshold);
    let empty: Map<u64, (Proposal, u32)> = Map::new(env);
    env.storage().instance().set(&KEY_PROPOSALS, &empty);
}

pub fn load_proposals(env: &Env) -> Map<u64, (Proposal, u32)> {
    env.storage().instance().get(&KEY_PROPOSALS).unwrap_or(Map::new(env))
}

/// Submit a new proposal. Returns the assigned proposal id.
pub fn propose(env: &Env, mut proposal: Proposal) -> u64 {
    crate::non_reentrant!(env);
    let signers: Vec<Address> = env.storage().instance().get(&KEY_SIGNERS).unwrap_or(vec![env]);
    if !signers.contains(&proposal.proposer) {
        panic_with_error!(env, GovError::NotASigner);
    }

    let current_ledger = env.ledger().sequence();
    let voting_deadline = current_ledger + duration_ledgers;

    let mut props: Map<u64, (Proposal, u32)> = env
        .storage()
        .instance()
        .get(&KEY_PROPOSALS)
        .unwrap_or(Map::new(env))
}

/// Record a signer's approval for `proposal_id`.
/// Transitions state from Pending → Approved when threshold is met.
pub fn approve(env: &Env, signer: &Address, proposal_id: u64) {
    crate::non_reentrant!(env);
    signer.require_auth();
    let signers: Vec<Address> = env.storage().instance().get(&KEY_SIGNERS).unwrap_or(vec![env]);
    if !signers.contains(signer) {
        panic_with_error!(env, GovError::NotASigner);
    }
    let threshold: u32 = env.storage().instance().get(&KEY_THRESH).unwrap_or(1);
    let min_stake: i128 = env.storage().instance().get(&KEY_MIN_STAKE).unwrap_or(0);
    let stake_token: Address = env.storage().instance().get(&KEY_STAKE_TOK).unwrap();

    let (mut prop, unlock) = props.get(proposal_id).unwrap_or_else(|| {
        panic_with_error!(env, GovError::ProposalNotFound)
    });

    if prop.state != ProposalState::Pending {
        panic_with_error!(env, GovError::InvalidStateTransition);
    }
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

    if min_stake > 0 {
        let balance = token::Client::new(env, &stake_token).balance(voter);
        if balance < min_stake {
            panic_with_error!(env, GovError::InsufficientStake);
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
    })
    }

    props.set(proposal_id, (prop.clone(), unlock));
    env.storage().instance().set(&KEY_PROPOSALS, &props);

    }

    env.storage().persistent().set(&key, &(prop.clone(), unlock));
    extend_proposal_ttl(env, &key);

    if prop.state == ProposalState::Approved && env.ledger().sequence() >= unlock {
        execute(env, proposal_id);
    }
}

pub fn execute(env: &Env, proposal_id: u64) -> Proposal {
    crate::non_reentrant!(env);
    let mut props = load_proposals(env);
    let (mut prop, unlock) = props.get(proposal_id).unwrap_or_else(|| {
        panic_with_error!(env, GovError::ProposalNotFound)
    });

    if prop.state != ProposalState::Approved {
        panic_with_error!(env, GovError::InvalidStateTransition);
    }
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

/// Record a signer's approval for `proposal_id`.
/// Transitions state from Pending → Approved when threshold is met.
pub fn approve(env: &Env, signer: &Address, proposal_id: u64) {
    crate::non_reentrant!(env, {
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
    });
}

pub fn execute(env: &Env, proposal_id: u64) -> Proposal {
    crate::non_reentrant!(env, {
        let mut props = load_proposals(env);
        let (mut prop, unlock) = props
            .get(proposal_id)
            .unwrap_or_else(|| panic_with_error!(env, GovError::ProposalNotFound));

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
    })
}
