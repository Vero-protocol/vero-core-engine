//! Vero Protocol Control Plane Foundation
//!
//! Orchestrates administrative functionality and enforces ZK-ready integrity checks
//! via the `audit::validate_transition` hook. This module is intentionally minimal,
//! deterministic, and audit-ready: every state-changing path is guarded by auth,
//! the circuit breaker, and a validated commitment chain.

use soroban_sdk::{
    contract, contracterror, contractimpl, panic_with_error, symbol_short, Address, BytesN, Env, Symbol,
};

use crate::audit::validate_transition;
use crate::circuit_breaker::assert_closed;
use crate::event_struct::{ACT_UPDATE, MOD_GOV};
use crate::event_utils::publish_event;
use crate::types::StateCommitment;

const KEY_ADMIN: Symbol = symbol_short!("ADMIN");

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
pub enum ControlPlaneError {
    AlreadyInitialized = 1,
    Unauthorized = 2,
    NotInitialized = 3,
    ReservedKey = 4,
    CommitmentAuthorMismatch = 5,
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
    /// Requires administrative authorization, asserts the circuit breaker is closed,
    /// and invokes the ZK-ready `validate_transition` hook to ensure state integrity.
    /// The `caller` must match the commitment `author` so the off-chain audit anchor
    /// is signed by the same privileged identity that authorised the on-chain write.
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

        if caller != commitment.author {
            panic_with_error!(&env, ControlPlaneError::CommitmentAuthorMismatch);
        }

        if is_reserved_key(&param_key) {
            panic_with_error!(&env, ControlPlaneError::ReservedKey);
        }

        // Ensure the protocol isn't paused
        assert_closed(&env);

        // ZK-ready integrity check (enforces no replays and valid hash)
        validate_transition(&env, &commitment, &payload.to_array());

        // Update the parameter
        env.storage().instance().set(&param_key, &param_val);

        // Emit a structured audit event
        publish_event(
            &env,
            MOD_GOV | ACT_UPDATE,
            param_val,
            commitment.state_hash,
        );
    }
}

fn is_reserved_key(key: &Symbol) -> bool {
    RESERVED_KEYS.iter().any(|reserved| reserved == key)
}
