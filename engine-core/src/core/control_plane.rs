//! Vero Protocol Control Plane Foundation
//!
//! Orchestrates administrative functionality and enforces ZK-ready integrity checks
//! via the `audit::validate_transition` hook.
//!
//! Security properties (audit-ready):
//! - Zero-address protection on admin init / transfer (#112)
//! - Administrative parameter sanitization with bounds checking (#110)
//! - Circuit-breaker pause integration (#118)
//! - 2-step admin transfer to prevent lockout
//! - Semantic versioning constants with on-chain tracking (#115)
//! - Re-entrancy guarded state mutations
//! - ZK integrity commitment validation on every param update
//! - Event emission for all state changes

use soroban_sdk::{
    contract, contracterror, contractimpl, panic_with_error, symbol_short,
    Address, BytesN, Env, String, Symbol,
};

use crate::audit::validate_transition;
use crate::circuit_breaker::{assert_closed, state as breaker_state};
use crate::types::{BreakerState, StateCommitment};
use crate::version::{
    self, contract_version, get_stored_version, init_version, version as get_version_tuple,
    version_string,
};

const KEY_ADMIN: Symbol = symbol_short!("ADMIN");
const KEY_PENDING_ADMIN: Symbol = symbol_short!("PEND_ADM");

// Parameter keys (whitelist for sanitization)
const PARAM_FEE: Symbol = symbol_short!("FEE");
const PARAM_THRESH: Symbol = symbol_short!("THRESH");
const PARAM_TLCK: Symbol = symbol_short!("TLCK");
const PARAM_LIMIT: Symbol = symbol_short!("LIMIT");

// Parameter bounds (sanitization - Issue #110)
const FEE_MIN: u64 = 0;
const FEE_MAX: u64 = 10_000; // basis points, 100%

const THRESH_MIN: u64 = 1;
const THRESH_MAX: u64 = 100;

const TLCK_MIN: u64 = 1;
const TLCK_MAX: u64 = 1_000_000; // ~ 5 days of ledgers

const LIMIT_MIN: u64 = 0;
const LIMIT_MAX: u64 = u64::MAX / 2;

// Events
const EVT_PARAM_UPDATED: Symbol = symbol_short!("PRM_UPD");
const EVT_ADMIN_XFER_INIT: Symbol = symbol_short!("ADM_XFR");
const EVT_ADMIN_ACCEPTED: Symbol = symbol_short!("ADM_ACC");
const EVT_INITIALIZED: Symbol = symbol_short!("INIT_CP");

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum ControlPlaneError {
    AlreadyInitialized = 1,
    Unauthorized = 2,
    NotInitialized = 3,
    InvalidParam = 4,
    ParamOutOfBounds = 5,
    ZeroAddress = 6,
    AdminTransferNotPending = 7,
    InvalidAdmin = 8,
}

#[contract]
pub struct ControlPlane;

#[contractimpl]
impl ControlPlane {
    /// Initialize the control plane with a master admin.
    ///
    /// Security:
    /// - Zero-address protection (#112)
    /// - Rejects double-initialization
    /// - Initialises on-chain version tracking (#115)
    /// - Emits Initialized event
    pub fn initialize(env: Env, admin: Address) {
        crate::non_reentrant!(&env);

        if env.storage().instance().has(&KEY_ADMIN) {
            panic_with_error!(&env, ControlPlaneError::AlreadyInitialized);
        }

        // Zero-address protection — reject the contract itself as admin
        // (Soroban has no null address, but self-admin is a footgun)
        if admin == env.current_contract_address() {
            panic_with_error!(&env, ControlPlaneError::ZeroAddress);
        }

        env.storage().instance().set(&KEY_ADMIN, &admin);

        // Initialise version tracking (ignore AlreadyInitialized – contract may be upgraded)
        if get_stored_version(&env).is_none() {
            init_version(&env);
        }

        env.events()
            .publish((EVT_INITIALIZED, ), admin);
    }

    /// Mutate a protocol parameter securely.
    ///
    /// Security checklist:
    /// - Admin auth required
    /// - Circuit breaker must be closed (#118)
    /// - ZK-ready integrity check via validate_transition
    /// - Parameter sanitization with bounds (#110)
    /// - Re-entrancy guarded
    /// - Event emission
    pub fn update_param(
        env: Env,
        caller: Address,
        param_key: Symbol,
        param_val: u64,
        commitment: StateCommitment,
        payload: BytesN<32>,
    ) {
        crate::non_reentrant!(&env);
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

        // Parameter sanitization (#110)
        sanitize_param(&env, &param_key, param_val);

        // Update the parameter
        env.storage().instance().set(&param_key, &param_val);

        env.events().publish(
            (EVT_PARAM_UPDATED, param_key.clone()),
            (admin, param_val),
        );
    }

    /// Get a protocol parameter.
    pub fn get_param(env: Env, param_key: Symbol) -> Option<u64> {
        env.storage().instance().get(&param_key)
    }

    /// Get the current admin.
    pub fn get_admin(env: Env) -> Option<Address> {
        env.storage().instance().get(&KEY_ADMIN)
    }

    /// Initiate a 2-step admin transfer (zero-address protected).
    ///
    /// Prevents accidental lockout by requiring the new admin to accept.
    pub fn transfer_admin(env: Env, caller: Address, new_admin: Address) {
        crate::non_reentrant!(&env);
        caller.require_auth();

        let admin: Address = env
            .storage()
            .instance()
            .get(&KEY_ADMIN)
            .unwrap_or_else(|| panic_with_error!(&env, ControlPlaneError::NotInitialized));

        if caller != admin {
            panic_with_error!(&env, ControlPlaneError::Unauthorized);
        }

        // Zero-address protection (#112)
        if new_admin == admin {
            panic_with_error!(&env, ControlPlaneError::InvalidAdmin);
        }
        if new_admin == env.current_contract_address() {
            panic_with_error!(&env, ControlPlaneError::ZeroAddress);
        }

        env.storage().instance().set(&KEY_PENDING_ADMIN, &new_admin);

        env.events().publish(
            (EVT_ADMIN_XFER_INIT, ),
            (admin, new_admin),
        );
    }

    /// Accept a pending admin transfer. Must be called by pending_admin.
    pub fn accept_admin(env: Env, caller: Address) {
        crate::non_reentrant!(&env);
        caller.require_auth();

        let pending: Address = env
            .storage()
            .instance()
            .get(&KEY_PENDING_ADMIN)
            .unwrap_or_else(|| panic_with_error!(&env, ControlPlaneError::AdminTransferNotPending));

        if caller != pending {
            panic_with_error!(&env, ControlPlaneError::Unauthorized);
        }

        env.storage().instance().set(&KEY_ADMIN, &caller);
        env.storage().instance().remove(&KEY_PENDING_ADMIN);

        env.events().publish((EVT_ADMIN_ACCEPTED, ), caller);
    }

    /// Get pending admin, if a transfer is in progress.
    pub fn get_pending_admin(env: Env) -> Option<Address> {
        env.storage().instance().get(&KEY_PENDING_ADMIN)
    }

    /// Cancel a pending admin transfer. Only current admin can cancel.
    pub fn cancel_admin_transfer(env: Env, caller: Address) {
        crate::non_reentrant!(&env);
        caller.require_auth();

        let admin: Address = env
            .storage()
            .instance()
            .get(&KEY_ADMIN)
            .unwrap_or_else(|| panic_with_error!(&env, ControlPlaneError::NotInitialized));

        if caller != admin {
            panic_with_error!(&env, ControlPlaneError::Unauthorized);
        }

        if !env.storage().instance().has(&KEY_PENDING_ADMIN) {
            panic_with_error!(&env, ControlPlaneError::AdminTransferNotPending);
        }

        env.storage().instance().remove(&KEY_PENDING_ADMIN);
    }

    // --- Versioning endpoints (#115) ---

    /// Get semantic version tuple (major, minor, patch).
    pub fn version(env: Env) -> (u32, u32, u32) {
        let _ = env;
        get_version_tuple()
    }

    /// Get version string, e.g. "0.1.0".
    pub fn version_string(env: Env) -> String {
        version_string(&env)
    }

    /// Get storage schema contract version.
    pub fn contract_version(env: Env) -> u32 {
        let _ = env;
        contract_version()
    }

    /// Get stored on-chain version, if initialised.
    pub fn get_stored_version(env: Env) -> Option<u32> {
        get_stored_version(&env)
    }

    // --- Pause / circuit-breaker integration (#118) ---

    /// Check if the control plane is paused (circuit breaker open).
    pub fn is_paused(env: Env) -> bool {
        breaker_state(&env) == BreakerState::Open
    }
}

/// Sanitize administrative parameters.
///
/// Whitelist-only keys with strict bounds checking to prevent
/// misconfiguration attacks (#110).
fn sanitize_param(env: &Env, key: &Symbol, val: u64) {
    let allowed = is_allowed_param(key);
    if !allowed {
        panic_with_error!(env, ControlPlaneError::InvalidParam);
    }

    let in_bounds = match key {
        k if *k == PARAM_FEE => (FEE_MIN..=FEE_MAX).contains(&val),
        k if *k == PARAM_THRESH => (THRESH_MIN..=THRESH_MAX).contains(&val),
        k if *k == PARAM_TLCK => (TLCK_MIN..=TLCK_MAX).contains(&val),
        k if *k == PARAM_LIMIT => (LIMIT_MIN..=LIMIT_MAX).contains(&val),
        _ => false,
    };

    if !in_bounds {
        panic_with_error!(env, ControlPlaneError::ParamOutOfBounds);
    }
}

fn is_allowed_param(key: &Symbol) -> bool {
    *key == PARAM_FEE
        || *key == PARAM_THRESH
        || *key == PARAM_TLCK
        || *key == PARAM_LIMIT
}
