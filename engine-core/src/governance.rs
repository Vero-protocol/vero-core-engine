use crate::event_utils::publish_event;
use crate::types::{Proposal, ProposalState};
use soroban_sdk::{
    contracterror, panic_with_error, symbol_short, token, vec, Address, BytesN, Env, Map, Symbol,
    Val, Vec,
};

const KEY_PROPOSALS: Symbol = symbol_short!("PROPS");
const KEY_SIGNERS: Symbol = symbol_short!("SIGNERS");
const KEY_THRESH: Symbol = symbol_short!("THRESH");
const KEY_MIN_STAKE: Symbol = symbol_short!("MINSTAKE");
const KEY_STAKE_TOK: Symbol = symbol_short!("STKTOK");
const TIMELOCK_LEDGERS: u32 = 720;

#[contracterror]
#[derive(Copy, Clone)]
pub enum GovError {
    NotASigner = 1,
    AlreadyApproved = 2,
    ProposalNotFound = 3,
    TimelockActive = 4,
    InvalidStateTransition = 5,
    AlreadyExecuted = 6,
    InsufficientStake = 7,
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
    env.storage().instance().set(&KEY_SIGNERS, &signers);
    env.storage().instance().set(&KEY_THRESH, &threshold);
    env.storage().instance().set(&KEY_STAKE_TOK, &stake_token);
    env.storage().instance().set(&KEY_MIN_STAKE, &min_stake);
    let empty: Map<u64, (Proposal, u32)> = Map::new(env);
    env.storage().instance().set(&KEY_PROPOSALS, &empty);
}

pub fn propose(env: &Env, proposal: Proposal) -> u64 {
    let mut props: Map<u64, (Proposal, u32)> = env
        .storage()
        .instance()
        .get(&KEY_PROPOSALS)
        .unwrap_or(Map::new(env));

    let unlock_ledger = env.ledger().sequence() + TIMELOCK_LEDGERS;
    let id = proposal.id;

    let mut prop = proposal;
    prop.state = ProposalState::Pending;

    props.set(id, (prop, unlock_ledger));
    env.storage().instance().set(&KEY_PROPOSALS, &props);

    env.events().publish(
        (symbol_short!("GOV"), symbol_short!("propose")),
        id,
    );

    let mut payload = Map::new(env);
    payload.set(Symbol::short("proposal_id"), id.into());
    publish_event(
        env,
        BytesN::from_array(env, &[0u8; 32]),
        BytesN::from_array(env, &[0u8; 32]),
        payload,
    );

    id
}

pub fn approve(env: &Env, voter: &Address, proposal_id: u64) {
    voter.require_auth();
    require_signer(env, voter);
    require_min_stake(env, voter);

    let mut props: Map<u64, (Proposal, u32)> = env
        .storage()
        .instance()
        .get(&KEY_PROPOSALS)
        .unwrap_or_else(|| panic_with_error!(env, GovError::ProposalNotFound));

    let (mut prop, unlock) = props
        .get(proposal_id)
        .unwrap_or_else(|| panic_with_error!(env, GovError::ProposalNotFound));

    if prop.state != ProposalState::Pending {
        panic_with_error!(env, GovError::InvalidStateTransition);
    }

    if prop.approved_by.contains(voter) {
        panic_with_error!(env, GovError::AlreadyApproved);
    }

    prop.approved_by.push_back(voter.clone());

    let threshold: u32 = env.storage().instance().get(&KEY_THRESH).unwrap_or(1);

    if prop.approved_by.len() >= threshold {
        prop.state = ProposalState::Approved;

        env.events().publish(
            (symbol_short!("GOV"), symbol_short!("approved")),
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
    }

    props.set(proposal_id, (prop.clone(), unlock));
    env.storage().instance().set(&KEY_PROPOSALS, &props);

    if prop.state == ProposalState::Approved && env.ledger().sequence() >= unlock {
        execute(env, proposal_id);
    }
}

pub fn execute(env: &Env, proposal_id: u64) -> Proposal {
    let mut props: Map<u64, (Proposal, u32)> = env
        .storage()
        .instance()
        .get(&KEY_PROPOSALS)
        .unwrap_or_else(|| panic_with_error!(env, GovError::ProposalNotFound));

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

    env.events().publish(
        (symbol_short!("GOV"), symbol_short!("execute")),
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
