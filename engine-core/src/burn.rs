use soroban_sdk::{contracterror, panic_with_error, symbol_short, Address, Env, IntoVal, String, BytesN, Map};
//! Burn module — zero-address rejection guard.
//!
//! Prevents burning/transferring funds to the zero address.
//! Wire `reject_zero_address` into any burn or irreversible-transfer entrypoint.

use soroban_sdk::{contracterror, panic_with_error, Address, BytesN, Env, String};
use crate::event_struct::{MOD_BURN, ACT_BURN_SAFE};
use crate::event_utils::publish_event;

#[contracterror]
#[derive(Copy, Clone)]
pub enum BurnError {
    ZeroAddress = 1,
}

const ZERO_ADDRESS: &str = "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF";

pub fn reject_zero_address(env: &Env, to: &Address) {
    let zero = String::from_str(env, ZERO_ADDRESS);
    if to.to_string() == zero {
        panic_with_error!(env, BurnError::ZeroAddress);
    }
}

pub fn burn_to(env: &Env, to: &Address, amount: i128) {
    reject_zero_address(env, to);
    if amount <= 0 {
        panic_with_error!(env, BurnError::ZeroAddress);
    }
    env.events().publish(
        (symbol_short!("TRE"), symbol_short!("burn_safe")),
        (to.clone(), amount),
    );
    let mut payload = Map::new(env);
    payload.set(symbol_short!("to"), to.clone().into_val(env));
    payload.set(symbol_short!("amount"), amount.into_val(env));
    publish_event(env, BytesN::from_array(env, & [0u8; 32]), BytesN::from_array(env, & [0u8; 32]), payload);
/// Burn-safe transfer wrapper. Validates recipient before emitting a single compact event.
pub fn burn_to(env: &Env, _to: &Address, amount: i128) {
    reject_zero_address(env, _to);
    // Single compact event — module=BURN, action=BURN_SAFE, value=amount as u64 (safe for normal amounts).
    publish_event(
        env,
        MOD_BURN | ACT_BURN_SAFE,
        amount as u64,
        BytesN::from_array(env, &[0u8; 32]),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{testutils::Address as _, Env};

    #[soroban_sdk::contract]
    pub struct TestContract;

    #[soroban_sdk::contractimpl]
    impl TestContract {}

    #[test]
    fn valid_address_passes() {
        let env = Env::default();
        let contract_id = env.register_contract(None, TestContract);
        let addr = Address::generate(&env);
        env.as_contract(&contract_id, || {
            reject_zero_address(&env, &addr);
        });
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
}
