//! Protocol fee helpers with checked arithmetic.

use crate::circuit_breaker::assert_closed;
use crate::event_struct::{ACT_FEE, MOD_FEE};
use crate::event_utils::{publish_event, zero_hash};
use soroban_sdk::{
    contracterror, panic_with_error, symbol_short, token, Address, Env, String, Symbol,
};

const KEY_FEE_BPS: Symbol = symbol_short!("FEE_BPS");
const KEY_FEE_RECIPIENT: Symbol = symbol_short!("FEE_RCP");
const MAX_BPS: u32 = 10_000;
const ZERO_ADDRESS: &str = "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF";

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub enum FeeError {
    InvalidBasisPoints = 1,
    InvalidRecipient = 2,
    FeeCalculationOverflow = 3,
    InvalidAmount = 4,
}

pub fn init(env: &Env, fee_bps: u32, recipient: &Address) {
    validate_bps(env, fee_bps);
    validate_address(env, recipient);
    env.storage().instance().set(&KEY_FEE_BPS, &fee_bps);
    env.storage().instance().set(&KEY_FEE_RECIPIENT, recipient);
}

pub fn calculate_fee(env: &Env, amount: i128) -> (i128, i128) {
    if amount < 0 {
        panic_with_error!(env, FeeError::InvalidAmount);
    }
    let fee_bps: u32 = env.storage().instance().get(&KEY_FEE_BPS).unwrap_or(0);
    if fee_bps == 0 || amount == 0 {
        return (0, amount);
    }
    let fee = amount
        .checked_mul(fee_bps as i128)
        .and_then(|value| value.checked_div(MAX_BPS as i128))
        .unwrap_or_else(|| panic_with_error!(env, FeeError::FeeCalculationOverflow));
    let net = amount
        .checked_sub(fee)
        .unwrap_or_else(|| panic_with_error!(env, FeeError::FeeCalculationOverflow));
    (fee, net)
}

pub fn deduct_fee(env: &Env, token: &Address, amount: i128) -> i128 {
    crate::non_reentrant!(env);
    assert_closed(env);

    let (fee, net) = calculate_fee(env, amount);
    if fee > 0 {
        let recipient: Address = env
            .storage()
            .instance()
            .get(&KEY_FEE_RECIPIENT)
            .unwrap_or_else(|| panic_with_error!(env, FeeError::InvalidRecipient));

        token::Client::new(env, token).transfer(&env.current_contract_address(), &recipient, &fee);

        if fee > u64::MAX as i128 {
            panic_with_error!(env, FeeError::FeeCalculationOverflow);
        }
        publish_event(env, MOD_FEE | ACT_FEE, fee as u64, zero_hash(env));
    }
    net
}

pub fn get_fee_config(env: &Env) -> (u32, Option<Address>) {
    let fee_bps: u32 = env.storage().instance().get(&KEY_FEE_BPS).unwrap_or(0);
    let recipient: Option<Address> = env.storage().instance().get(&KEY_FEE_RECIPIENT);
    (fee_bps, recipient)
}

pub fn set_fee_bps(env: &Env, fee_bps: u32) {
    validate_bps(env, fee_bps);
    env.storage().instance().set(&KEY_FEE_BPS, &fee_bps);
}

pub fn set_fee_recipient(env: &Env, recipient: &Address) {
    validate_address(env, recipient);
    env.storage().instance().set(&KEY_FEE_RECIPIENT, recipient);
}

fn validate_bps(env: &Env, fee_bps: u32) {
    if fee_bps > MAX_BPS {
        panic_with_error!(env, FeeError::InvalidBasisPoints);
    }
}

fn validate_address(env: &Env, addr: &Address) {
    let zero = String::from_str(env, ZERO_ADDRESS);
    if addr.to_string().is_empty() || addr.to_string() == zero {
        panic_with_error!(env, FeeError::InvalidRecipient);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{testutils::Address as _, Env};

    #[soroban_sdk::contract]
    pub struct TestContract;

    #[soroban_sdk::contractimpl]
    impl TestContract {}

    fn setup(env: &Env, fee_bps: u32) -> (soroban_sdk::Address, Address) {
        let contract_id = env.register_contract(None, TestContract);
        let recipient = Address::generate(env);
        env.as_contract(&contract_id, || init(env, fee_bps, &recipient));
        (contract_id, recipient)
    }

    #[test]
    fn fee_calculation_zero_bps() {
        let env = Env::default();
        let (contract_id, _) = setup(&env, 0);
        env.as_contract(&contract_id, || {
            let (fee, net) = calculate_fee(&env, 1000);
            assert_eq!(fee, 0);
            assert_eq!(net, 1000);
        });
    }

    #[test]
    fn fee_calculation_standard() {
        let env = Env::default();
        let (contract_id, _) = setup(&env, 500);
        env.as_contract(&contract_id, || {
            let (fee, net) = calculate_fee(&env, 1000);
            assert_eq!(fee, 50);
            assert_eq!(net, 950);
        });
    }

    #[test]
    fn fee_calculation_rounds_down() {
        let env = Env::default();
        let (contract_id, _) = setup(&env, 333);
        env.as_contract(&contract_id, || {
            let (fee, net) = calculate_fee(&env, 100);
            assert_eq!(fee, 3);
            assert_eq!(net, 97);
        });
    }

    #[test]
    fn fee_at_full_bps_is_full_amount() {
        let env = Env::default();
        let (contract_id, _) = setup(&env, MAX_BPS);
        env.as_contract(&contract_id, || {
            let (fee, net) = calculate_fee(&env, 1000);
            assert_eq!(fee, 1000);
            assert_eq!(net, 0);
        });
    }

    #[test]
    fn get_fee_config_returns_stored_values() {
        let env = Env::default();
        let (contract_id, recipient) = setup(&env, 250);
        env.as_contract(&contract_id, || {
            let (bps, rec) = get_fee_config(&env);
            assert_eq!(bps, 250);
            assert_eq!(rec.unwrap(), recipient);
        });
    }

    #[test]
    fn set_fee_bps_updates_rate() {
        let env = Env::default();
        let (contract_id, _) = setup(&env, 100);
        env.as_contract(&contract_id, || {
            set_fee_bps(&env, 750);
            let (bps, _) = get_fee_config(&env);
            assert_eq!(bps, 750);
        });
    }

    #[test]
    #[should_panic]
    fn init_rejects_bps_over_max() {
        let env = Env::default();
        let contract_id = env.register_contract(None, TestContract);
        let recipient = Address::generate(&env);
        env.as_contract(&contract_id, || init(&env, MAX_BPS + 1, &recipient));
    }
}
