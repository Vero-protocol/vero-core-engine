//! Vero Protocol Control Plane Foundation
//!
//! Orchestrates administrative functionality and enforces ZK-ready integrity checks
//! via the `audit::validate_transition` hook.

use soroban_sdk::{
    contract, contracterror, contractimpl, panic_with_error, symbol_short, Address, BytesN, Env, Symbol,
};

use crate::audit::validate_transition;
use crate::circuit_breaker::assert_closed;
use crate::types::StateCommitment;

const KEY_ADMIN: Symbol = symbol_short!("ADMIN");

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub enum ControlPlaneError {
    AlreadyInitialized = 1,
    Unauthorized = 2,
    NotInitialized = 3,
}

#[contract]
pub struct ControlPlane;

#[contractimpl]
impl ControlPlane {
    /// Initialize the control plane with a master admin.
    pub fn initialize(env: Env, admin: Address) {
        if env.storage().instance().has(&KEY_ADMIN) {
            panic_with_error!(&env, ControlPlaneError::AlreadyInitialized);
        }
        env.storage().instance().set(&KEY_ADMIN, &admin);
    }

    /// Mutate a protocol parameter securely.
    ///
    /// Requires administrative authorization, asserts the circuit breaker is closed,
    /// and invokes the ZK-ready `validate_transition` hook to ensure state integrity.
    pub fn update_param(
        env: Env,
        caller: Address,
        param_key: Symbol,
        param_val: u64,
        commitment: StateCommitment,
        payload: BytesN<32>,
    ) {
        caller.require_auth();

        let admin: Address = env
            .storage()
            .instance()
            .get(&KEY_ADMIN)
            .unwrap_or_else(|| panic_with_error!(&env, ControlPlaneError::NotInitialized));

        if caller != admin {
            panic_with_error!(&env, ControlPlaneError::Unauthorized);
        }

        // Ensure the protocol isn't paused
        assert_closed(&env);

        // ZK-ready integrity check (enforces no replays and valid hash)
        validate_transition(&env, &commitment, &payload.to_array());

        // Update the parameter
        env.storage().instance().set(&param_key, &param_val);
    }
}
