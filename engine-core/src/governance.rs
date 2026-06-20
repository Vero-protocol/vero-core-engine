use crate::types::Proposal;
use soroban_sdk::{contract, contractimpl, Address, Env, BytesN, Symbol, Vec};

use soroban_sdk::{
    contracterror, panic_with_error, symbol_short, token, vec, Address, Env, Map, Symbol, BytesN, Vec, Val,
};
use crate::event_utils::publish_event;

#[contractimpl]
impl Governance {
    pub fn propose(
        env: Env,
        proposer: Address,
        action_hash: BytesN<32>,
    ) -> u64 {
        proposer.require_auth();

const KEY_PROPOSALS:  Symbol = symbol_short!("PROPS");
const KEY_SIGNERS:    Symbol = symbol_short!("SIGNERS");
const KEY_THRESH:     Symbol = symbol_short!("THRESH");
const KEY_MIN_STAKE:  Symbol = symbol_short!("MINSTAKE");
const KEY_STAKE_TOK:  Symbol = symbol_short!("STKTOK");
/// Ledgers to wait after full approval before execution (~1 hour on Stellar).
const TIMELOCK_LEDGERS: u32 = 720;

        let proposal = Proposal {
            id: next_id,
            proposer,
            action_hash,
            approved_by: Vec::new(&env),
            state: 0, 
        };

/// Initialise governance with an ordered signer set, approval threshold, and
/// anti-Sybil stake parameters.
///
/// * `stake_token`  – SAC/token contract address whose balance is checked at vote time.
/// * `min_stake`    – Minimum balance (in token's smallest unit) a signer must hold to vote.
///                    Pass `0` to disable the stake gate.
pub fn init(
    env: &Env,
    signers: Vec<Address>,
    threshold: u32,
    stake_token: Address,
    min_stake: i128,
) {
    assert!(threshold <= signers.len(), "threshold > signer count");
    env.storage().instance().set(&KEY_SIGNERS, &signers);
    env.storage().instance().set(&KEY_THRESH, &threshold);
    env.storage().instance().set(&KEY_STAKE_TOK, &stake_token);
    env.storage().instance().set(&KEY_MIN_STAKE, &min_stake);
    let empty: Map<u64, (Proposal, u32)> = Map::new(env);
    env.storage().instance().set(&KEY_PROPOSALS, &empty);
}

        next_id
    }
    // Initialize state to Pending
    proposal.state = ProposalState::Pending;
    
    let mut props = load_proposals(env);
    let unlock_ledger = env.ledger().sequence() + TIMELOCK_LEDGERS;
    props.set(proposal.id, (proposal.clone(), unlock_ledger));
    env.storage().instance().set(&KEY_PROPOSALS, &props);
    // Emit raw event for proposal creation
    env.events().publish(
        (symbol_short!("GOV"), symbol_short!("propose")),
        proposal.id,
    );
    // Emit structured event for proposal creation
    let mut payload = Map::new(env);
    payload.set(Symbol::short("proposal_id"), proposal.id.into());
    publish_event(env, BytesN::from_array(env, &[0u8; 32]), BytesN::from_array(env, &[0u8; 32]), payload);
    proposal.id
}

    pub fn approve(env: Env, voter: Address, proposal_id: u64) {
        voter.require_auth();

        let mut proposal: Proposal = env
            .storage()
            .persistent()
            .get(&proposal_id)
            .expect("Proposal not found");

    let mut props = load_proposals(env);
    let threshold: u32 = env.storage().instance().get(&KEY_THRESH).unwrap_or(1);
    let (mut prop, unlock) = props.get(proposal_id).unwrap_or_else(|| {
        panic_with_error!(env, GovError::ProposalNotFound)
    });
    
    // Only pending proposals can receive approvals
    if prop.state != ProposalState::Pending {
        panic_with_error!(env, GovError::InvalidStateTransition);
    }
    
    if prop.approved_by.contains(signer) {
        panic_with_error!(env, GovError::AlreadyApproved);
    }
    prop.approved_by.push_back(signer.clone());

    // Transition to Approved when count threshold is met
    if prop.approved_by.len() >= threshold {
        prop.state = ProposalState::Approved;
        // Emit raw event for proposal approval
        env.events().publish(
            (symbol_short!("GOV"), symbol_short!("approved")),
            proposal_id,
        );
        // Emit structured event for proposal approval
        let mut payload = Map::new(env);
        payload.set(Symbol::short("proposal_id"), proposal_id.into());
        publish_event(env, BytesN::from_array(env, &[0u8; 32]), BytesN::from_array(env, &[0u8; 32]), payload);
    }
    
    props.set(proposal_id, (prop.clone(), unlock));
    env.storage().instance().set(&KEY_PROPOSALS, &props);

    // Auto-execute hook: if the proposal just reached Approved state and the
    // timelock has already elapsed, execute immediately to avoid manual step.
    if prop.state == ProposalState::Approved && env.ledger().sequence() >= unlock {
        // Best-effort: calling `execute` will update storage and emit execute event.
        // Any failure will panic as per existing execute guards.
        let _ = execute(env, proposal_id);
    }
}

        proposal.state = 2; 
        env.storage().persistent().set(&proposal_id, &proposal);
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
    // Emit structured event for proposal execution
    let mut payload = Map::new(env);
    payload.set(Symbol::short("proposal_id"), proposal_id.into());
    publish_event(env, BytesN::from_array(env, &[0u8; 32]), BytesN::from_array(env, &[0u8; 32]), payload);
    prop
}
