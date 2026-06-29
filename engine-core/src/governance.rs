//! Multi-sig governance with explicit proposal-state transitions and timelock.
//!
//! State machine:
//!
//! ```text
//! Pending ──(threshold approvals)──> Approved ──(timelock elapsed)──> Executed
//! ```
//!
//! Invalid transitions panic with typed `GovError` values for auditability.

use crate::circuit_breaker::assert_closed;
use crate::event_struct::{ACT_APPROVE, ACT_EXECUTE, ACT_PROPOSE, MOD_GOV};
use crate::event_utils::{publish_event, zero_hash};
use crate::types::{Proposal, ProposalState};
use soroban_sdk::{
    contracterror, panic_with_error, symbol_short, token, vec, Address, Env, Map, Symbol, Vec, BytesN,
};

const KEY_PROPOSALS: Symbol = symbol_short!("PROPS");
const KEY_SIGNERS: Symbol = symbol_short!("SIGNERS");
const KEY_THRESH: Symbol = symbol_short!("THRESH");
const KEY_MIN_STAKE: Symbol = symbol_short!("MINSTAKE");
const KEY_STAKE_TOK: Symbol = symbol_short!("STKTOK");

/// Ledgers to wait after threshold approval before execution (~1 hour).
pub const TIMELOCK_LEDGERS: u32 = 720;
const MAX_THRESHOLD: u32 = 100;

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
    InvalidStake = 8,
    AlreadyInitialized = 9,
    ProposalAlreadyExists = 10,
    InvalidProposal = 11,
    ArithmeticOverflow = 12,
}

pub fn init(env: &Env, signers: Vec<Address>, threshold: u32) {
    init_internal(env, signers, threshold, None, 0);
}

/// Initialise governance with an optional anti-Sybil stake gate.
///
/// `min_stake == 0` disables the stake gate. When enabled, every approving
/// signer must hold at least `min_stake` units of `stake_token`.
pub fn init_with_stake(
    env: &Env,
    signers: Vec<Address>,
    threshold: u32,
    stake_token: Address,
    min_stake: i128,
) {
    init_internal(env, signers, threshold, Some(stake_token), min_stake);
}

fn init_internal(
    env: &Env,
    signers: Vec<Address>,
    threshold: u32,
    stake_token: Option<Address>,
    min_stake: i128,
) {
    crate::non_reentrant!(env);

    if env.storage().instance().has(&KEY_SIGNERS) {
        panic_with_error!(env, GovError::AlreadyInitialized);
    }
    if threshold == 0 || threshold > signers.len() || threshold > MAX_THRESHOLD {
        panic_with_error!(env, GovError::InvalidThreshold);
    }
    if min_stake < 0 || (min_stake > 0 && stake_token.is_none()) {
        panic_with_error!(env, GovError::InvalidStake);
    }

    let mut seen = Vec::new(env);
    for signer in signers.iter() {
        if seen.contains(&signer) {
            panic_with_error!(env, GovError::InvalidThreshold);
        }
        seen.push_back(signer);
    }

    env.storage().instance().set(&KEY_SIGNERS, &signers);
    env.storage().instance().set(&KEY_THRESH, &threshold);
    env.storage().instance().set(&KEY_MIN_STAKE, &min_stake);
    if let Some(token) = stake_token {
        env.storage().instance().set(&KEY_STAKE_TOK, &token);
    }

    let empty: Map<u64, (Proposal, u32)> = Map::new(env);
    env.storage().instance().set(&KEY_PROPOSALS, &empty);
}

/// Load the proposal map. Public for tests and read-only off-chain simulation.
pub fn load_proposals(env: &Env) -> Map<u64, (Proposal, u32)> {
    env.storage()
        .instance()
        .get(&KEY_PROPOSALS)
        .unwrap_or(Map::new(env))
}

fn save_proposals(env: &Env, proposals: &Map<u64, (Proposal, u32)>) {
    env.storage().instance().set(&KEY_PROPOSALS, proposals);
}

fn load_signers(env: &Env) -> Vec<Address> {
    env.storage()
        .instance()
        .get(&KEY_SIGNERS)
        .unwrap_or(vec![env])
}

fn threshold(env: &Env) -> u32 {
    env.storage().instance().get(&KEY_THRESH).unwrap_or(0)
}

fn require_signer(env: &Env, signer: &Address) {
    let signers = load_signers(env);
    if !signers.contains(signer) {
        panic_with_error!(env, GovError::NotASigner);
    }
}

fn require_stake(env: &Env, signer: &Address) {
    let min_stake: i128 = env.storage().instance().get(&KEY_MIN_STAKE).unwrap_or(0);
    if min_stake == 0 {
        return;
    }
    let stake_token: Address = env
        .storage()
        .instance()
        .get(&KEY_STAKE_TOK)
        .unwrap_or_else(|| panic_with_error!(env, GovError::InvalidStake));
    let balance = token::Client::new(env, &stake_token).balance(signer);
    if balance < min_stake {
        panic_with_error!(env, GovError::InvalidStake);
    }
}

pub fn propose(env: &Env, mut proposal: Proposal) -> u64 {
    let signers: Vec<Address> = env
        .storage()
        .instance()
        .get(&KEY_SIGNERS)
        .unwrap_or(vec![env]);
    if !signers.contains(&proposal.proposer) {
        panic_with_error!(env, GovError::NotASigner);
    }
    proposal.proposer.require_auth();
    
    // Initialize state to Pending
    proposal.state = ProposalState::Pending;

    let mut props = load_proposals(env);
    let unlock_ledger = env.ledger().sequence() + TIMELOCK_LEDGERS;
    let id = proposal.id;
    props.set(id, (proposal.clone(), unlock_ledger));
    env.storage().instance().set(&KEY_PROPOSALS, &props);

    publish_event(
        env,
        MOD_GOV | ACT_PROPOSE,
        proposal.id,
        BytesN::from_array(env, &[0u8; 32]),
    );
    proposal.id
}

pub fn approve(env: &Env, signer: &Address, proposal_id: u64) {
    crate::circuit_breaker::assert_closed(env);
    crate::non_reentrant!(env);
    assert_closed(env);

    signer.require_auth();
    require_signer(env, signer);
    require_stake(env, signer);

    let mut proposals = load_proposals(env);
    let (mut proposal, mut unlock_ledger) = proposals
        .get(proposal_id)
        .unwrap_or_else(|| panic_with_error!(env, GovError::ProposalNotFound));

    if proposal.state != ProposalState::Pending {
        panic_with_error!(env, GovError::InvalidStateTransition);
    }
    if proposal.approved_by.contains(signer) {
        panic_with_error!(env, GovError::AlreadyApproved);
    }
    proposal.approved_by.push_back(signer.clone());

    let approval_threshold = threshold(env);
    if approval_threshold == 0 {
        panic_with_error!(env, GovError::InvalidThreshold);
    }

    if proposal.approved_by.len() >= approval_threshold {
        proposal.state = ProposalState::Approved;
        unlock_ledger = env
            .ledger()
            .sequence()
            .checked_add(TIMELOCK_LEDGERS)
            .unwrap_or_else(|| panic_with_error!(env, GovError::ArithmeticOverflow));
    }

    proposals.set(proposal_id, (proposal, unlock_ledger));
    save_proposals(env, &proposals);

    publish_event(env, MOD_GOV | ACT_APPROVE, proposal_id, zero_hash(env));
}

/// Execute an approved proposal after its timelock has elapsed.
pub fn execute(env: &Env, proposal_id: u64) -> Proposal {
    crate::circuit_breaker::assert_closed(env);
    crate::non_reentrant!(env);
    assert_closed(env);

    let mut proposals = load_proposals(env);
    let (mut proposal, unlock_ledger) = proposals
        .get(proposal_id)
        .unwrap_or_else(|| panic_with_error!(env, GovError::ProposalNotFound));

    if proposal.state != ProposalState::Approved {
        panic_with_error!(env, GovError::InvalidStateTransition);
    }
    if proposal.approved_by.len() < threshold(env) {
        panic_with_error!(env, GovError::ThresholdNotMet);
    }
    if env.ledger().sequence() < unlock_ledger {
        panic_with_error!(env, GovError::TimelockActive);
    }

    proposal.state = ProposalState::Executed;
    proposals.set(proposal_id, (proposal.clone(), unlock_ledger));
    save_proposals(env, &proposals);

    publish_event(
        env,
        MOD_GOV | ACT_EXECUTE,
        proposal_id,
        proposal.action_hash.clone(),
    );
    proposal
}

/// Return a proposal or panic with `ProposalNotFound`.
pub fn get_proposal(env: &Env, proposal_id: u64) -> Proposal {
    let (proposal, _) = load_proposals(env)
        .get(proposal_id)
        .unwrap_or_else(|| panic_with_error!(env, GovError::ProposalNotFound));
    proposal
}

/// Return a proposal's unlock ledger, if it exists.
pub fn get_unlock_ledger(env: &Env, proposal_id: u64) -> Option<u32> {
    load_proposals(env)
        .get(proposal_id)
        .map(|(_, unlock)| unlock)
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{testutils::Address as _, vec, BytesN, Env};

    #[soroban_sdk::contract]
    pub struct TestContract;

    #[soroban_sdk::contractimpl]
    impl TestContract {}

    fn proposal(env: &Env, id: u64, proposer: Address) -> Proposal {
        Proposal {
            id,
            action_hash: BytesN::from_array(env, &[7u8; 32]),
            proposer,
            approved_by: vec![env],
            state: ProposalState::Executed,
        }
    }

    #[test]
    fn threshold_transition_sets_unlock() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, TestContract);
        let alice = Address::generate(&env);
        let bob = Address::generate(&env);

        env.as_contract(&contract_id, || {
            init(&env, vec![&env, alice.clone(), bob.clone()], 2);
        });
        env.as_contract(&contract_id, || {
            propose(&env, proposal(&env, 1, alice.clone()));
        });
        env.as_contract(&contract_id, || {
            approve(&env, &alice, 1);
            assert_eq!(get_proposal(&env, 1).state, ProposalState::Pending);
        });
        env.as_contract(&contract_id, || {
            approve(&env, &bob, 1);
            assert_eq!(get_proposal(&env, 1).state, ProposalState::Approved);
            assert_eq!(get_unlock_ledger(&env, 1), Some(TIMELOCK_LEDGERS));
        });
    }
}
