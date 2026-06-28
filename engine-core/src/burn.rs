
//! Burn safety helpers.

use crate::event_struct::{ACT_BURN_SAFE, MOD_BURN};
use crate::event_utils::publish_event;
use soroban_sdk::{contracterror, panic_with_error, Address, BytesN, Env, String};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]

//! Burn module — zero-address and invalid-amount guards.

use crate::event_struct::{ACT_BURN_SAFE, MOD_BURN};
use crate::event_utils::{publish_event, zero_hash};
use soroban_sdk::{contracterror, panic_with_error, Address, Env, String};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]

pub enum BurnError {
    ZeroAddress = 1,
    InvalidAmount = 2,
}

const ZERO_ADDRESS: &str = "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF";

/// Panics when `to` is the Stellar zero address.
pub fn reject_zero_address(env: &Env, to: &Address) {
    let zero = String::from_str(env, ZERO_ADDRESS);
    if to.to_string() == zero {
        panic_with_error!(env, BurnError::ZeroAddress);
    }
}


/// Validate a burn recipient and emit a compact audit event.

fn amount_to_event_value(env: &Env, amount: i128) -> u64 {
    if amount <= 0 || amount > u64::MAX as i128 {
        panic_with_error!(env, BurnError::InvalidAmount);
    }
    amount as u64
}

/// Burn-safe transfer wrapper. Validates recipient/amount before emitting.

pub fn burn_to(env: &Env, to: &Address, amount: i128) {
    crate::circuit_breaker::assert_closed(env);
    reject_zero_address(env, to);

    if amount <= 0 {
        panic_with_error!(env, BurnError::InvalidAmount);
    }
    publish_event(
        env,
        MOD_BURN | ACT_BURN_SAFE,
        amount as u64,
        BytesN::from_array(env, &[0u8; 32]),
    );

    let value = amount_to_event_value(env, amount);
    publish_event(env, MOD_BURN | ACT_BURN_SAFE, value, zero_hash(env));

}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{contract, contractimpl, testutils::Address as _, Env};

    #[contract]
    pub struct TestContract;

    #[contractimpl]
    impl TestContract {}

    #[test]
    fn valid_address_passes() {
        let env = Env::default();
        let contract_id = env.register_contract(None, TestContract);
        let addr = Address::generate(&env);
        env.as_contract(&contract_id, || reject_zero_address(&env, &addr));
    }

    #[test]
    #[should_panic]
    fn zero_address_rejected() {
        let env = Env::default();
        let contract_id = env.register_contract(None, TestContract);
        env.as_contract(&contract_id, || {
            let zero = Address::from_string(&String::from_str(&env, ZERO_ADDRESS));
            reject_zero_address(&env, &zero);
        });
    }

    #[test]
    #[should_panic]
    fn invalid_amount_rejected() {
        let env = Env::default();
        let contract_id = env.register_contract(None, TestContract);
        let addr = Address::generate(&env);
        env.as_contract(&contract_id, || burn_to(&env, &addr, 0));
    }
}
