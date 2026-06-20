//! Burn module — zero-address rejection guard.
//!
//! Prevents burning/transferring funds to the zero address.
//! Wire `reject_zero_address` into any burn or irreversible-transfer entrypoint.

use soroban_sdk::{contracterror, panic_with_error, symbol_short, Address, Env, String, BytesN, Map, Val};

#[contracterror]
#[derive(Copy, Clone)]
pub enum BurnError {
    /// Attempted to burn/transfer funds to the zero address.
    ZeroAddress = 1,
}

/// Stellar well-known zero address (all-A strkey, 56 chars).
const ZERO_ADDRESS: &str = "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF";

/// Panics with `BurnError::ZeroAddress` when `to` is the Stellar zero address.
pub fn reject_zero_address(env: &Env, to: &Address) {
    let zero = String::from_str(env, ZERO_ADDRESS);
    if to.to_string() == zero {
        panic_with_error!(env, BurnError::ZeroAddress);
    }
}

/// Burn-safe transfer wrapper. Validates recipient before emitting event.
pub fn burn_to(env: &Env, to: &Address, amount: i128) {
    reject_zero_address(env, to);
    env.events().publish(
        (symbol_short!("TRE"), symbol_short!("burn_safe")),
        (to.clone(), amount),
    );
    // Emit structured Event for burn safety
    let mut payload = Map::new(env);
    payload.set(Symbol::short("to"), to.clone().into());
    payload.set(Symbol::short("amount"), amount.into());
    publish_event(env, BytesN::from_array(env, & [0u8; 32]), BytesN::from_array(env, & [0u8; 32]), payload);
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
