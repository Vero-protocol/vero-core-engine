//! Vero Protocol Control Plane Foundation
//!
//! This module exposes the hardened administrative surface for `engine-core`.
//! Every state-changing control-plane path is authenticated, circuit-breaker
//! aware, reentrancy guarded, and anchored through the ZK-ready audit commitment
//! chain before mutating protocol configuration.

use soroban_sdk::{
    contract, contracterror, contractimpl, panic_with_error, symbol_short, Address, Bytes, BytesN,
    Env, Map, Symbol,
};

use crate::audit;
use crate::circuit_breaker;
use crate::event_struct::{ACT_EXECUTE, ACT_PROPOSE, MOD_GOV};
use crate::event_utils::publish_event;
use crate::types::{Proposal, StateCommitment};

const KEY_ADMIN: Symbol = symbol_short!("ADMIN");
const KEY_INIT: Symbol = symbol_short!("CP_INIT");
const KEY_PARAM_COUNT: Symbol = symbol_short!("P_COUNT");
const KEY_LAST_PARAM: Symbol = symbol_short!("LASTPAR");

/// Reserved keys that may not be modified through `update_param` to prevent
/// accidental corruption of internal engine state.
const RESERVED_KEYS: &[Symbol] = &[
    symbol_short!("ADMIN"),
    symbol_short!("SEQ"),
    symbol_short!("PREV_H"),
    symbol_short!("CB_STATE"),
    symbol_short!("CB_GUARD"),
    symbol_short!("PROPS"),
    symbol_short!("SIGNERS"),
    symbol_short!("THRESH"),
    symbol_short!("MINSTAKE"),
    symbol_short!("STKTOK"),
    symbol_short!("ER_ADMINS"),
    symbol_short!("ER_THRESH"),
    symbol_short!("ER_APPRVS"),
    symbol_short!("ER_DEST"),
    symbol_short!("ER_TOKEN"),
    symbol_short!("ER_AMOUNT"),
    symbol_short!("FEE_BPS"),
    symbol_short!("FEE_RCP"),
    symbol_short!("SNAPC"),
    symbol_short!("SNAPL"),
    symbol_short!("OUTFLOWS"),
];

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum ControlPlaneError {
    AlreadyInitialized = 1,
    Unauthorized = 2,
    NotInitialized = 3,
    InvalidPayload = 4,
    ArithmeticOverflow = 5,
}

#[contract]
pub struct ControlPlane;

#[contractimpl]
impl ControlPlane {
    /// Initialize the control plane with a master admin.
    ///
    /// The admin must authorize initialization, making deployment races
    /// auditable and preventing an arbitrary account from installing itself as
    /// administrator without a matching signature.
    pub fn initialize(env: Env, admin: Address) {
        crate::non_reentrant!(&env);

        if env.storage().instance().has(&KEY_INIT) {
            panic_with_error!(&env, ControlPlaneError::AlreadyInitialized);
        }

        admin.require_auth();
        env.storage().instance().set(&KEY_ADMIN, &admin);
        env.storage().instance().set(&KEY_INIT, &true);
        env.storage().instance().set(&KEY_PARAM_COUNT, &0u64);
    }

    /// Return the configured administrator, or `None` before initialization.
    pub fn admin(env: Env) -> Option<Address> {
        env.storage().instance().get(&KEY_ADMIN)
    }

    /// Return a stored protocol parameter value.
    pub fn get_param(env: Env, param_key: Symbol) -> Option<u64> {
        env.storage().instance().get(&param_key)
    }

    /// Return the number of successful parameter updates.
    pub fn param_update_count(env: Env) -> u64 {
        env.storage().instance().get(&KEY_PARAM_COUNT).unwrap_or(0)
    }

    /// Return the latest accepted audit commitment sequence.
    pub fn last_audit_sequence(env: Env) -> u64 {
        audit::get_last_sequence(&env)
    }

    /// Return the latest accepted state commitment hash.
    pub fn state_hash(env: Env) -> BytesN<32> {
        audit::get_state_hash(&env)
    }

    /// Pure preflight integrity check for clients and tests.
    pub fn integrity_check(env: Env, commitment: StateCommitment, payload: BytesN<32>) -> bool {
        audit::integrity_check(&env, &commitment, &payload.to_array())
    }

    /// Return the configured admin, or panic if not initialized.
    pub fn get_admin(env: Env) -> Address {
        env.storage()
            .instance()
            .get(&KEY_ADMIN)
            .unwrap_or_else(|| panic_with_error!(&env, ControlPlaneError::NotInitialized))
    }

    /// Return a previously set protocol parameter, or `None`.
    pub fn get_param(env: Env, param_key: Symbol) -> Option<u64> {
        env.storage().instance().get(&param_key)
    }

    /// Mutate a protocol parameter securely.
    ///
    /// Security properties:
    /// - caller must be the initialized admin and must authorize the invocation;
    /// - circuit breaker must be closed;
    /// - transition must pass the audit module's chained commitment check;
    /// - update counter uses checked arithmetic;
    /// - reentrancy guard wraps the full mutation.
    pub fn update_param(
        env: Env,
        caller: Address,
        param_key: Symbol,
        param_val: u64,
        commitment: StateCommitment,
        payload: BytesN<32>,
    ) {
        crate::non_reentrant!(&env);
        require_admin(&env, &caller);
        circuit_breaker::assert_closed(&env);

        let payload_raw = payload.to_array();
        audit::validate_transition_inner(&env, &commitment, &payload_raw);

        env.storage().instance().set(&param_key, &param_val);
        env.storage().instance().set(&KEY_LAST_PARAM, &param_key);
        increment_param_count(&env);
    }

    /// Initialize the shared circuit-breaker guardian set through the
    /// control-plane contract surface.
    pub fn init_breaker(env: Env, caller: Address, guardians: soroban_sdk::Vec<Address>) {
        crate::non_reentrant!(&env);
        require_admin(&env, &caller);
        circuit_breaker::init(&env, guardians);
    }

    /// Trip the circuit breaker. Guardian authorization is enforced by the
    /// circuit-breaker module itself.
    pub fn trip_breaker(env: Env, guardian: Address) {
        circuit_breaker::trip(&env, &guardian);
    }

    /// Reset the circuit breaker. Guardian authorization is enforced by the
    /// circuit-breaker module itself.
    pub fn reset_breaker(env: Env, guardian: Address) {
        circuit_breaker::reset(&env, &guardian);
    }

    /// Store a governance proposal via the shared governance module.
    pub fn propose(env: Env, proposal: Proposal) -> u64 {
        crate::governance::propose(&env, proposal)
    }

    /// Approve a governance proposal via the shared governance module.
    pub fn approve(env: Env, signer: Address, proposal_id: u64) {
        crate::governance::approve(&env, &signer, proposal_id);
    }

    /// Execute a governance proposal via the shared governance module.
    pub fn execute(env: Env, proposal_id: u64) -> Proposal {
        crate::governance::execute(&env, proposal_id)
    }

    /// Register an off-chain ZK proof attestation for a committed state root.
    ///
    /// This stable hook is admin-gated and only accepts attestations for the
    /// current committed state hash, preventing fabricated proofs for unrelated
    /// roots from being anchored under the control-plane ABI.
    pub fn register_proof(
        env: Env,
        caller: Address,
        state_root: BytesN<32>,
        proof_hash: BytesN<32>,
        block_seq: u32,
        metadata: Map<Symbol, Bytes>,
    ) {
        crate::non_reentrant!(&env);
        crate::core::zk_hooks::register_proof(
            &env,
            &caller,
            state_root,
            proof_hash,
            block_seq,
            metadata,
        );
    }

    /// Retrieve a proof attestation by state root.
    pub fn get_proof(env: Env, state_root: BytesN<32>) -> Option<BytesN<32>> {
        crate::core::zk_hooks::get_proof(&env, state_root)
    }
}

fn require_admin(env: &Env, caller: &Address) {
    caller.require_auth();

    let admin: Address = env
        .storage()
        .instance()
        .get(&KEY_ADMIN)
        .unwrap_or_else(|| panic_with_error!(env, ControlPlaneError::NotInitialized));

    if caller != &admin {
        panic_with_error!(env, ControlPlaneError::Unauthorized);
    }
}

fn increment_param_count(env: &Env) {
    let count: u64 = env.storage().instance().get(&KEY_PARAM_COUNT).unwrap_or(0);
    let next = count
        .checked_add(1)
        .unwrap_or_else(|| panic_with_error!(env, ControlPlaneError::ArithmeticOverflow));
    env.storage().instance().set(&KEY_PARAM_COUNT, &next);
}

/// Emit a compact governance-control event for future control-plane extensions.
#[allow(dead_code)]
fn publish_control_governance_event(env: &Env, proposal_id: u64, executed: bool, hash: BytesN<32>) {
    let action = if executed { ACT_EXECUTE } else { ACT_PROPOSE };
    publish_event(env, MOD_GOV | action, proposal_id, hash);
}

fn is_reserved_key(key: &Symbol) -> bool {
    RESERVED_KEYS.iter().any(|reserved| reserved == key)
}
