//! Multi-sig governance with timelock.
//!
//! `Pending → Approved → Executed`
//!
//! Invalid transitions panic with typed `GovError` values.

use soroban_sdk::{
    contracterror, panic_with_error, symbol_short, token, vec, Address, BytesN, Env, Map, Symbol, Vec,
};

use crate::circuit_breaker::assert_closed;
use crate::event_struct::{ACT_APPROVE, ACT_EXECUTE, ACT_PROPOSE, MOD_GOV};
use crate::event_utils::{publish_event, zero_hash};
use crate::types::{Proposal, ProposalState};

const KEY_PROPOSALS: Symbol = symbol_short!("PROPS");
const KEY_SIGNERS:   Symbol = symbol_short!("SIGNERS");
const KEY_THRESH:    Symbol = symbol_short!("THRESH");
const KEY_MIN_STAKE: Symbol = symbol_short!("MINSTAKE");
const KEY_STAKE_TOK: Symbol = symbol_short!("STKTOK");

/// Ledgers to wait after quorum before execution (~1 hour on Stellar).
pub const TIMELOCK_LEDGERS: u32 = 720;
const MAX_THRESHOLD: u32 = 100;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub enum GovError {
    NotASigner             = 1,
    AlreadyApproved        = 2,
    ThresholdNotMet        = 3,
    TimelockActive         = 4,
    InvalidStateTransition = 5,
    ProposalNotFound       = 6,
    InvalidThreshold       = 7,
    InvalidStake           = 8,
    AlreadyInitialized     = 9,
    ProposalAlreadyExists  = 10,
    ArithmeticOverflow     = 11,
}

/// Initialise governance with `signers` and a minimum-approval `threshold`.
pub fn init(env: &Env, signers: Vec<Address>, threshold: u32) {
    init_internal(env, signers, threshold, None, 0);
}

/// Initialise governance with an optional anti-Sybil stake gate.
///
/// Set `min_stake = 0` to disable the gate. When enabled every approver must
/// hold at least `min_stake` units of `stake_token`.
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
    if threshold == 0 || threshold > signers.len() as u32 || threshold > MAX_THRESHOLD {
        panic_with_error!(env, GovError::InvalidThreshold);
    }
    if min_stake < 0 || (min_stake > 0 && stake_token.is_none()) {
        panic_with_error!(env, GovError::InvalidStake);
    }
    let mut seen = Vec::new(env);
    for s in signers.iter() {
        if seen.contains(&s) {
            panic_with_error!(env, GovError::InvalidThreshold);
        }
        seen.push_back(s);
    }
    env.storage().instance().set(&KEY_SIGNERS, &signers);
    env.storage().instance().set(&KEY_THRESH, &threshold);
    env.storage().instance().set(&KEY_MIN_STAKE, &min_stake);
    if let Some(tok) = stake_token {
        env.storage().instance().set(&KEY_STAKE_TOK, &tok);
    }
    let empty: Map<u64, (Proposal, u32)> = Map::new(env);
    env.storage().instance().set(&KEY_PROPOSALS, &empty);
}

/// All proposals keyed by proposal id, value is `(Proposal, unlock_ledger)`.
pub fn load_proposals(env: &Env) -> Map<u64, (Proposal, u32)> {
    env.storage()
        .instance()
        .get(&KEY_PROPOSALS)
        .unwrap_or(Map::new(env))
}

fn save_proposals(env: &Env, m: &Map<u64, (Proposal, u32)>) {
    env.storage().instance().set(&KEY_PROPOSALS, m);
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
    if !load_signers(env).contains(signer) {
        panic_with_error!(env, GovError::NotASigner);
    }
}

fn require_stake(env: &Env, signer: &Address) {
    let min: i128 = env.storage().instance().get(&KEY_MIN_STAKE).unwrap_or(0);
    if min == 0 {
        return;
    }
    let tok: Address = env
        .storage()
        .instance()
        .get(&KEY_STAKE_TOK)
        .unwrap_or_else(|| panic_with_error!(env, GovError::InvalidStake));
    if token::Client::new(env, &tok).balance(signer) < min {
        panic_with_error!(env, GovError::InvalidStake);
    }
}

/// Submit a new proposal. `proposal.proposer` must be an authorised signer.
pub fn propose(env: &Env, proposal: Proposal) -> u64 {
    crate::non_reentrant!(env);
    require_signer(env, &proposal.proposer);

    let id = proposal.id;
    let mut props = load_proposals(env);
    if props.contains_key(id) {
        panic_with_error!(env, GovError::ProposalAlreadyExists);
    }

    let mut prop = proposal;
    prop.state = ProposalState::Pending;
    props.set(id, (prop, 0u32));
    save_proposals(env, &props);

    publish_event(env, MOD_GOV | ACT_PROPOSE, id, zero_hash(env));
    id
}

/// Record `signer`'s approval. Transitions `Pending → Approved` at threshold.
pub fn approve(env: &Env, signer: &Address, proposal_id: u64) {
    let _guard = crate::guards::ReentrancyGuard::enter(env);
    assert_closed(env);
    signer.require_auth();
    require_signer(env, signer);
    require_stake(env, signer);

    let thresh = threshold(env);
    if thresh == 0 {
        panic_with_error!(env, GovError::InvalidThreshold);
    }

    let mut props = load_proposals(env);
    let (mut prop, mut unlock) = props
        .get(proposal_id)
        .unwrap_or_else(|| panic_with_error!(env, GovError::ProposalNotFound));

    if prop.state != ProposalState::Pending {
        panic_with_error!(env, GovError::InvalidStateTransition);
    }
    if prop.approved_by.contains(signer) {
        panic_with_error!(env, GovError::AlreadyApproved);
    }

    prop.approved_by.push_back(signer.clone());

    if prop.approved_by.len() as u32 >= thresh {
        prop.state = ProposalState::Approved;
        unlock = env
            .ledger()
            .sequence()
            .checked_add(TIMELOCK_LEDGERS)
            .unwrap_or_else(|| panic_with_error!(env, GovError::ArithmeticOverflow));
    }

    props.set(proposal_id, (prop, unlock));
    save_proposals(env, &props);

    publish_event(env, MOD_GOV | ACT_APPROVE, proposal_id, zero_hash(env));
}

/// Execute an approved proposal after the timelock has elapsed.
pub fn execute(env: &Env, proposal_id: u64) -> Proposal {
    let _guard = crate::guards::ReentrancyGuard::enter(env);
    assert_closed(env);

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
    save_proposals(env, &props);

    publish_event(env, MOD_GOV | ACT_EXECUTE, proposal_id, prop.action_hash.clone());
    prop
}

/// Return a proposal or panic with `ProposalNotFound`.
pub fn get_proposal(env: &Env, proposal_id: u64) -> Proposal {
    let (prop, _) = load_proposals(env)
        .get(proposal_id)
        .unwrap_or_else(|| panic_with_error!(env, GovError::ProposalNotFound));
    prop
}

/// Return a proposal's unlock ledger, or `None` if not found.
pub fn get_unlock_ledger(env: &Env, proposal_id: u64) -> Option<u32> {
    load_proposals(env).get(proposal_id).map(|(_, unlock)| unlock)
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
            state: ProposalState::Pending,
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
            let _ = propose(&env, proposal(&env, 1, alice.clone()));
            approve(&env, &alice, 1);
            assert_eq!(get_proposal(&env, 1).state, ProposalState::Pending);
            approve(&env, &bob, 1);
            assert_eq!(get_proposal(&env, 1).state, ProposalState::Approved);
            assert_eq!(get_unlock_ledger(&env, 1), Some(TIMELOCK_LEDGERS));
        });
    }
}
