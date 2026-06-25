//! Emergency recovery — multi-sig guarded exit path for bricked contracts.
//!
//! When normal governance is unavailable (e.g. contract bricked, execution path
//! unreachable), this module provides an out-of-band admin recovery path that
//! drains locked funds to a pre-approved destination.
//!
//! ## Safety model
//! * Requires `threshold` distinct admin approvals before the exit executes.
//! * Admins are set at `init` time and stored in instance storage.
//! * Once triggered, a `ER/triggered` event is emitted for off-chain audit.
//! * Only one pending recovery request exists at a time; a new `request` call
//!   resets any prior unapproved request.

use soroban_sdk::{
    contracterror, panic_with_error, symbol_short, token, vec, Address, BytesN, Env, String,
    Symbol, Vec,
};
use crate::event_struct::{MOD_RECOVERY, ACT_REQUEST, ACT_TRIGGERED};
use crate::event_utils::publish_event;
use crate::circuit_breaker::assert_closed;

const KEY_ADMINS:    Symbol = symbol_short!("ER_ADMINS");
const KEY_THRESH:    Symbol = symbol_short!("ER_THRESH");
const KEY_APPROVALS: Symbol = symbol_short!("ER_APPRVS");
const KEY_DEST:      Symbol = symbol_short!("ER_DEST");
const KEY_TOKEN:     Symbol = symbol_short!("ER_TOKEN");
const KEY_AMOUNT:    Symbol = symbol_short!("ER_AMOUNT");

#[contracterror]
#[derive(Copy, Clone)]
pub enum RecoveryError {
    NotAdmin            = 1,
    InvalidAddress      = 6,
    AlreadyApproved     = 2,
    ThresholdNotMet     = 3,
    NoPendingRequest    = 4,
    InvalidThreshold    = 5,
}

/// Initialise the recovery module.
pub fn init(env: &Env, admins: Vec<Address>, threshold: u32) {
    for admin in admins.iter() {
        validate_address(env, &admin);
    }
    if threshold == 0 || threshold > admins.len() {
        panic_with_error!(env, RecoveryError::InvalidThreshold);
    }
    env.storage().instance().set(&KEY_ADMINS, &admins);
    env.storage().instance().set(&KEY_THRESH, &threshold);
    clear_pending(env);
}

/// Submit (or reset) an emergency recovery request.
pub fn request(env: &Env, requester: &Address, token: &Address, dest: &Address, amount: i128) {
    assert_closed(env);
    requester.require_auth();
    require_admin(env, requester);
    validate_address(env, token);
    validate_address(env, dest);

    clear_pending(env);
    env.storage().instance().set(&KEY_TOKEN,  token);
    env.storage().instance().set(&KEY_DEST,   dest);
    env.storage().instance().set(&KEY_AMOUNT, &amount);

    let mut approvals: Vec<Address> = vec![env];
    approvals.push_back(requester.clone());
    env.storage().instance().set(&KEY_APPROVALS, &approvals);

    // Single compact event.
    publish_event(
        env,
        MOD_RECOVERY | ACT_REQUEST,
        amount as u64,
        BytesN::from_array(env, &[0u8; 32]),
    );
}

/// Approve the pending recovery request.
pub fn approve(env: &Env, admin: &Address) {
    assert_closed(env);
    admin.require_auth();
    require_admin(env, admin);

    let dest: Address = env
        .storage()
        .instance()
        .get(&KEY_DEST)
        .unwrap_or_else(|| panic_with_error!(env, RecoveryError::NoPendingRequest));

    let mut approvals: Vec<Address> = env
        .storage()
        .instance()
        .get(&KEY_APPROVALS)
        .unwrap_or(vec![env]);

    if approvals.contains(admin) {
        panic_with_error!(env, RecoveryError::AlreadyApproved);
    }

    approvals.push_back(admin.clone());
    env.storage().instance().set(&KEY_APPROVALS, &approvals);

    let threshold: u32 = env.storage().instance().get(&KEY_THRESH).unwrap_or(1);

    if approvals.len() >= threshold {
        execute_recovery(env, &dest);
    }
}

// ── internal ──────────────────────────────────────────────────────────────────

fn execute_recovery(env: &Env, dest: &Address) {
    assert_closed(env);
    let token:  Address = env.storage().instance().get(&KEY_TOKEN).unwrap();
    let amount: i128    = env.storage().instance().get(&KEY_AMOUNT).unwrap();

    token::Client::new(env, &token).transfer(
        &env.current_contract_address(),
        dest,
        &amount,
    );

    // Single compact event.
    publish_event(
        env,
        MOD_RECOVERY | ACT_TRIGGERED,
        amount as u64,
        BytesN::from_array(env, &[0u8; 32]),
    );

    clear_pending(env);
}

fn clear_pending(env: &Env) {
    let empty: Vec<Address> = vec![env];
    env.storage().instance().set(&KEY_APPROVALS, &empty);
    env.storage().instance().remove(&KEY_DEST);
    env.storage().instance().remove(&KEY_TOKEN);
    env.storage().instance().remove(&KEY_AMOUNT);
}

fn require_admin(env: &Env, caller: &Address) {
    let admins: Vec<Address> = env
        .storage()
        .instance()
        .get(&KEY_ADMINS)
        .unwrap_or(vec![env]);
    if !admins.contains(caller) {
        panic_with_error!(env, RecoveryError::NotAdmin);
    }
}

/// Helper to validate that an address is not obviously invalid (empty string).
fn validate_address(env: &Env, addr: &Address) {
    let s = addr.to_string();
    if s.is_empty() {
        panic_with_error!(env, RecoveryError::InvalidAddress);
    }
    // Guard against all-zero (zero address) via string comparison.
    let zero = String::from_str(env, "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF");
    if addr.to_string() == zero {
        panic_with_error!(env, RecoveryError::InvalidAddress);
    }
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{testutils::Address as _, vec, Env};

    #[soroban_sdk::contract]
    pub struct TestContract;

    #[soroban_sdk::contractimpl]
    impl TestContract {}

    fn setup(env: &Env, n: u32, threshold: u32) -> (Address, Vec<Address>) {
        let contract_id = env.register_contract(None, TestContract);
        let mut admins = vec![env];
        for _ in 0..n {
            admins.push_back(Address::generate(env));
        }
        let admins: Vec<Address> = (0..n)
            .map(|_| Address::generate(env))
            .collect::<std::vec::Vec<_>>()
            .iter()
            .fold(vec![env], |mut v, a| { v.push_back(a.clone()); v });
        env.as_contract(&contract_id, || init(env, admins.clone(), threshold));
        (contract_id, admins)
    }

    #[test]
    #[should_panic]
    fn init_rejects_zero_threshold() {
        let env = Env::default();
        env.mock_all_auths();
        let a = Address::generate(&env);
        let contract_id = env.register_contract(None, TestContract);
        env.as_contract(&contract_id, || {
            init(&env, vec![&env, a.clone()], 0);
        });
    }

    #[test]
    #[should_panic]
    fn init_rejects_threshold_exceeding_admin_count() {
        let env = Env::default();
        env.mock_all_auths();
        let a = Address::generate(&env);
        let contract_id = env.register_contract(None, TestContract);
        env.as_contract(&contract_id, || {
            init(&env, vec![&env, a.clone()], 2);
        });
    }

    #[test]
    #[should_panic]
    fn non_admin_cannot_request() {
        let env = Env::default();
        env.mock_all_auths();
        let (contract_id, _) = setup(&env, 2, 2);
        let rogue = Address::generate(&env);
        let token = Address::generate(&env);
        let dest  = Address::generate(&env);
        env.as_contract(&contract_id, || {
            request(&env, &rogue, &token, &dest, 1000);
        });
    }

    #[test]
    #[should_panic]
    fn non_admin_cannot_approve() {
        let env = Env::default();
        env.mock_all_auths();
        let (contract_id, admins) = setup(&env, 2, 2);
        let token = Address::generate(&env);
        let dest  = Address::generate(&env);
        let rogue = Address::generate(&env);
        env.as_contract(&contract_id, || {
            request(&env, &admins.get(0).unwrap(), &token, &dest, 1000);
            approve(&env, &rogue);
        });
    }

    #[test]
    #[should_panic]
    fn same_admin_cannot_double_approve() {
        let env = Env::default();
        env.mock_all_auths();
        let (contract_id, admins) = setup(&env, 2, 2);
        let token = Address::generate(&env);
        let dest  = Address::generate(&env);
        let a0 = admins.get(0).unwrap();
        env.as_contract(&contract_id, || {
            request(&env, &a0, &token, &dest, 1000);
            approve(&env, &a0);
        });
    }

    #[test]
    fn single_approval_does_not_execute_when_threshold_is_two() {
        let env = Env::default();
        env.mock_all_auths();
        let (contract_id, admins) = setup(&env, 3, 2);
        let token = Address::generate(&env);
        let dest  = Address::generate(&env);
        let a0 = admins.get(0).unwrap();
        env.as_contract(&contract_id, || {
            request(&env, &a0, &token, &dest, 500);
            let stored_dest: Option<Address> = env.storage().instance().get(&KEY_DEST);
            assert!(stored_dest.is_some());
        });
    }
}
