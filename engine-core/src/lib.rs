use soroban_sdk::{contract, contractimpl, symbol_short, Address, Bytes, BytesN, Env, Symbol, Vec};

pub mod audit;
pub mod circuit_breaker;
pub mod governance;
pub mod types;

#[cfg(test)]
mod governance_tests;

const KEY_INIT: Symbol = symbol_short!("INIT");

use crate::types::{Proposal, StateCommitment};

#[contract]
pub struct VeroCore;

#[contractimpl]
impl VeroCore {
    /// Initialise the Vero Protocol engine.
    pub fn init(env: Env, signers: Vec<Address>, threshold: u32, guardians: Vec<Address>) {
        if env.storage().instance().has(&KEY_INIT) {
            panic!("Already initialized");
        }
        env.storage().instance().set(&KEY_INIT, &true);

        governance::init(&env, signers, threshold);
        circuit_breaker::init(&env, guardians);
    }

    /// Trip the breaker — halts the engine. Requires guardian auth.
    pub fn trip(env: Env, guardian: Address) {
        guardian.require_auth();
        circuit_breaker::trip(&env, &guardian);
    }

    /// Reset the breaker — resumes normal operation. Requires guardian auth.
    pub fn reset(env: Env, guardian: Address) {
        guardian.require_auth();
        circuit_breaker::reset(&env, &guardian);
    }

    /// Submit a new governance proposal.
    pub fn propose(env: Env, proposer: Address, action_hash: BytesN<32>) -> u64 {
        circuit_breaker::assert_closed(&env);
        proposer.require_auth();
        let proposal = Proposal {
            id: 0, // Assigned by governance module
            action_hash,
            proposer,
            approved_by: Vec::new(&env),
            state: types::ProposalState::Pending,
        };
        governance::propose(&env, proposal)
    }

    /// Record a signer's approval for `proposal_id`.
    pub fn approve(env: Env, signer: Address, proposal_id: u64) {
        circuit_breaker::assert_closed(&env);
        signer.require_auth();
        governance::approve(&env, &signer, proposal_id);
    }

    /// Execute a contract upgrade proposal.
    pub fn upgrade(env: Env, proposal_id: u64) {
        circuit_breaker::assert_closed(&env);
        let prop = governance::execute(&env, proposal_id);
        env.deployer()
            .update_current_contract_wasm(prop.action_hash);
    }

    /// Validate and record a new state transition.
    pub fn commit(env: Env, commitment: StateCommitment, payload: Bytes) {
        circuit_breaker::assert_closed(&env);
        commitment.author.require_auth();
        audit::validate_transition(&env, &commitment, &payload);
    }
}
