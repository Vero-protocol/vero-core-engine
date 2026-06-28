//! Emergency recovery — multi-sig guarded exit path for bricked contracts.

use crate::circuit_breaker::assert_closed;
use crate::event_struct::{ACT_REQUEST, ACT_TRIGGERED, MOD_RECOVERY};
use crate::event_utils::{publish_event, zero_hash};
use soroban_sdk::{
    contracterror, panic_with_error, symbol_short, token, vec, Address, Env, String, Symbol, Vec,
};

const KEY_ADMINS: Symbol = symbol_short!("ER_ADMINS");
const KEY_THRESH: Symbol = symbol_short!("ER_THRESH");
const KEY_APPROVALS: Symbol = symbol_short!("ER_APPRVS");
const KEY_DEST: Symbol = symbol_short!("ER_DEST");
const KEY_TOKEN: Symbol = symbol_short!("ER_TOKEN");
const KEY_AMOUNT: Symbol = symbol_short!("ER_AMOUNT");
const ZERO_ADDRESS: &str = "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF";

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub enum RecoveryError {
    NotAdmin = 1,
    AlreadyApproved = 2,
    ThresholdNotMet = 3,
    NoPendingRequest = 4,
    InvalidThreshold = 5,
    InvalidAddress = 6,
    InvalidAmount = 7,
    AlreadyInitialized = 8,
}

/// Initialise the recovery module.
pub fn init(env: &Env, admins: Vec<Address>, threshold: u32) {
    if env.storage().instance().has(&KEY_ADMINS) {
        panic_with_error!(env, RecoveryError::AlreadyInitialized);
    }
    if threshold == 0 || threshold > admins.len() {
        panic_with_error!(env, RecoveryError::InvalidThreshold);
    }

    let mut seen = Vec::new(env);
    for admin in admins.iter() {
        validate_address(env, &admin);
        if seen.contains(&admin) {
            panic_with_error!(env, RecoveryError::InvalidThreshold);
        }
        seen.push_back(admin);
    }

    env.storage().instance().set(&KEY_ADMINS, &admins);
    env.storage().instance().set(&KEY_THRESH, &threshold);
    clear_pending(env);
}

/// Submit (or reset) an emergency recovery request.
pub fn request(env: &Env, requester: &Address, token: &Address, dest: &Address, amount: i128) {
    crate::non_reentrant!(env);
    assert_closed(env);

    requester.require_auth();
    require_admin(env, requester);
    validate_address(env, token);
    validate_address(env, dest);
    if amount <= 0 {
        panic_with_error!(env, RecoveryError::InvalidAmount);
    }

    clear_pending(env);
    env.storage().instance().set(&KEY_TOKEN, token);
    env.storage().instance().set(&KEY_DEST, dest);
    env.storage().instance().set(&KEY_AMOUNT, &amount);

    let mut approvals: Vec<Address> = vec![env];
    approvals.push_back(requester.clone());
    env.storage().instance().set(&KEY_APPROVALS, &approvals);

    publish_event(
        env,
        MOD_RECOVERY | ACT_REQUEST,
        event_amount(env, amount),
        zero_hash(env),
    );

    let threshold: u32 = env.storage().instance().get(&KEY_THRESH).unwrap_or(1);
    if approvals.len() >= threshold {
        execute_recovery(env, dest);
    }
}

/// Approve the pending recovery request.
pub fn approve(env: &Env, admin: &Address) {
    crate::non_reentrant!(env);
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

pub fn pending_approvals(env: &Env) -> Vec<Address> {
    env.storage()
        .instance()
        .get(&KEY_APPROVALS)
        .unwrap_or(vec![env])
}

fn execute_recovery(env: &Env, dest: &Address) {
    let token: Address = env
        .storage()
        .instance()
        .get(&KEY_TOKEN)
        .unwrap_or_else(|| panic_with_error!(env, RecoveryError::NoPendingRequest));
    let amount: i128 = env
        .storage()
        .instance()
        .get(&KEY_AMOUNT)
        .unwrap_or_else(|| panic_with_error!(env, RecoveryError::NoPendingRequest));

    token::Client::new(env, &token).transfer(&env.current_contract_address(), dest, &amount);
    publish_event(
        env,
        MOD_RECOVERY | ACT_TRIGGERED,
        event_amount(env, amount),
        zero_hash(env),
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

fn validate_address(env: &Env, addr: &Address) {
    let zero = String::from_str(env, ZERO_ADDRESS);
    if addr.to_string().is_empty() || addr.to_string() == zero {
        panic_with_error!(env, RecoveryError::InvalidAddress);
    }
}

fn event_amount(env: &Env, amount: i128) -> u64 {
    if amount <= 0 || amount > u64::MAX as i128 {
        panic_with_error!(env, RecoveryError::InvalidAmount);
    }
    amount as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{testutils::Address as _, vec, Env};

    #[soroban_sdk::contract]
    pub struct TestContract;

    #[soroban_sdk::contractimpl]
    impl TestContract {}

    fn setup(env: &Env, admin_count: u32, threshold: u32) -> (soroban_sdk::Address, Vec<Address>) {
        let contract_id = env.register_contract(None, TestContract);
        let mut admins = vec![env];
        for _ in 0..admin_count {
            admins.push_back(Address::generate(env));
        }
        env.as_contract(&contract_id, || init(env, admins.clone(), threshold));
        (contract_id, admins)
    }

    #[test]
    #[should_panic]
    fn init_rejects_zero_threshold() {
        let env = Env::default();
        let a = Address::generate(&env);
        let contract_id = env.register_contract(None, TestContract);
        env.as_contract(&contract_id, || init(&env, vec![&env, a], 0));
    }

    #[test]
    #[should_panic]
    fn init_rejects_threshold_exceeding_admin_count() {
        let env = Env::default();
        let a = Address::generate(&env);
        let contract_id = env.register_contract(None, TestContract);
        env.as_contract(&contract_id, || init(&env, vec![&env, a], 2));
    }

    #[test]
    #[should_panic]
    fn non_admin_cannot_request() {
        let env = Env::default();
        env.mock_all_auths();
        let (contract_id, _) = setup(&env, 2, 2);
        let rogue = Address::generate(&env);
        let token = Address::generate(&env);
        let dest = Address::generate(&env);
        env.as_contract(&contract_id, || request(&env, &rogue, &token, &dest, 1000));
    }

    #[test]
    #[should_panic]
    fn same_admin_cannot_double_approve() {
        let env = Env::default();
        env.mock_all_auths();
        let (contract_id, admins) = setup(&env, 2, 2);
        let token = Address::generate(&env);
        let dest = Address::generate(&env);
        let admin = admins.get(0).unwrap();
        env.as_contract(&contract_id, || {
            request(&env, &admin, &token, &dest, 1000);
            approve(&env, &admin);
        });
    }

    #[test]
    fn single_approval_does_not_execute_when_threshold_is_two() {
        let env = Env::default();
        env.mock_all_auths();
        let (contract_id, admins) = setup(&env, 3, 2);
        let token = Address::generate(&env);
        let dest = Address::generate(&env);
        let admin = admins.get(0).unwrap();
        env.as_contract(&contract_id, || {
            request(&env, &admin, &token, &dest, 500);
            assert_eq!(pending_approvals(&env).len(), 1);
        });
    }
}
