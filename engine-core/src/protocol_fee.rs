//! Protocol fee helpers with checked arithmetic.

use crate::circuit_breaker::assert_closed;
use crate::event_struct::{ACT_FEE, MOD_FEE};
use crate::event_utils::{publish_event, zero_hash};
use soroban_sdk::{
    contracterror, panic_with_error, symbol_short, token, Address, Env, String, Symbol,
};
use crate::event_struct::{MOD_FEE, ACT_FEE};
use crate::event_utils::publish_event;
use crate::circuit_breaker::assert_closed;

const KEY_FEE_BPS:       Symbol = symbol_short!("FEE_BPS");
const KEY_FEE_RECIPIENT: Symbol = symbol_short!("FEE_RCP");

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub enum FeeError {
    InvalidBasisPoints      = 1,
    InvalidRecipient        = 2,
    FeeCalculationOverflow  = 3,
}

const MAX_BPS: u32 = 10000;

// ── Configuration ─────────────────────────────────────────────────────────────

/// Initialise fee configuration. Must be called once during contract setup.
///
/// Panics with [`FeeError::InvalidBasisPoints`] if `fee_bps > 10 000`.
pub fn init(env: &Env, fee_bps: u32, recipient: &Address) {
    if fee_bps > MAX_BPS {
        panic_with_error!(env, FeeError::InvalidBasisPoints);
    }
    validate_address(env, recipient);
    env.storage().instance().set(&KEY_FEE_BPS,       &fee_bps);
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
        .and_then(|v| v.checked_div(10000))
        .unwrap_or_else(|| panic_with_error!(env, FeeError::FeeCalculationOverflow));
    let net = amount
        .checked_sub(fee)
        .unwrap_or_else(|| panic_with_error!(env, FeeError::FeeCalculationOverflow));
    (fee, net)
}

// ── Transfer hooks ────────────────────────────────────────────────────────────

/// Protocol-level fee-on-transfer hook.
///
/// Intercepts a transfer of `amount` tokens from `from` to `to` and:
///
/// 1. Asserts the circuit breaker is `Closed`.
/// 2. Acquires the reentrancy guard.
/// 3. Rejects `to` = zero address ([`BurnError::ZeroAddress`]).
/// 4. If `token` is fee-exempt, transfers `amount` from `from` → `to` unchanged.
/// 5. Otherwise:
///    - Computes `(fee, net) = amount * fee_bps / 10 000`.
///    - Transfers `fee` from `from` → fee recipient (skipped when fee is 0).
///    - Transfers `net` from `from` → `to`.
///    - Emits `MOD_FEE | ACT_FEE_TRANSFER` event carrying the fee amount.
///
/// Returns the net amount delivered to `to`.
pub fn fee_on_transfer(
    env: &Env,
    token: &Address,
    from: &Address,
    to: &Address,
    amount: i128,
) -> i128 {
    assert_closed(env);
    let _guard = ReentrancyGuard::enter(env);
    reject_zero_address(env, to);

    let tkn = token::Client::new(env, token);

    if is_token_exempt(env, token) {
        tkn.transfer(from, to, &amount);
        return amount;
    }

    let (fee, net) = calculate_fee(env, amount);

    if fee > 0 {
        let recipient: Address = env
            .storage()
            .instance()
            .get(&KEY_FEE_RCP)
            .unwrap_or_else(|| panic_with_error!(env, FeeError::InvalidRecipient));
        tkn.transfer(from, &recipient, &fee);
    }

    if net > 0 {
        tkn.transfer(from, to, &net);
    }

    publish_event(
        env,
        MOD_FEE | ACT_FEE_TRANSFER,
        fee as u64,
        BytesN::from_array(env, &[0u8; 32]),
    );

    net
}

/// Low-level fee deduction from the contract's own token balance.
///
/// Transfers the fee portion from `current_contract_address()` to the
/// configured recipient. Returns the net amount.
///
/// Use [`fee_on_transfer`] when intercepting a third-party transfer end-to-end.
pub fn deduct_fee(env: &Env, token: &Address, amount: i128) -> i128 {
    crate::non_reentrant!(env);
    assert_closed(env);

    let (fee, net) = calculate_fee(env, amount);
    if fee > 0 {
        let recipient: Address = env
            .storage()
            .instance()
            .get(&KEY_FEE_RCP)
            .unwrap_or_else(|| panic_with_error!(env, FeeError::InvalidRecipient));

        token::Client::new(env, token).transfer(
            &env.current_contract_address(),
            &recipient,
            &fee,
        );

        // Single compact event — fee amount in value field.
        publish_event(
            env,
            MOD_FEE | ACT_FEE,
            fee as u64,
            BytesN::from_array(env, &[0u8; 32]),
        );
    }
    net
}

pub fn get_fee_config(env: &Env) -> (u32, Option<Address>) {
    let fee_bps: u32 = env.storage().instance().get(&KEY_FEE_BPS).unwrap_or(0);
    let recipient: Option<Address> = env.storage().instance().get(&KEY_FEE_RECIPIENT);
    (fee_bps, recipient)
}

pub fn set_fee_bps(env: &Env, fee_bps: u32) {
    if fee_bps > MAX_BPS {
        panic_with_error!(env, FeeError::InvalidBasisPoints);
    }
    env.storage().instance().set(&KEY_FEE_BPS, &fee_bps);
}

pub fn set_fee_recipient(env: &Env, recipient: &Address) {
    validate_address(env, recipient);
    env.storage().instance().set(&KEY_FEE_RECIPIENT, recipient);
}

fn validate_address(env: &Env, addr: &Address) {
    let s = addr.to_string();
    if s.is_empty() {
        panic_with_error!(env, FeeError::InvalidRecipient);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{testutils::Address as _, token, vec, Env};

    use crate::circuit_breaker;

    #[soroban_sdk::contract]
    pub struct TestContract;

    #[soroban_sdk::contractimpl]
    impl TestContract {}

    fn setup(env: &Env, fee_bps: u32) -> (Address, Address) {
        let contract_id = env.register_contract(None, TestContract);
        let recipient = Address::generate(env);
        env.as_contract(&contract_id, || init(env, fee_bps, &recipient));
        (contract_id, recipient)
    }

    fn make_token(env: &Env) -> (Address, token::StellarAssetClient) {
        let admin = Address::generate(env);
        let addr = env.register_stellar_asset_contract_v2(admin).address();
        let client = token::StellarAssetClient::new(env, &addr);
        (addr, client)
    }

    // ── calculate_fee ─────────────────────────────────────────────────────────

    #[test]
    fn fee_calculation_zero_bps() {
        let env = Env::default();
        let (contract_id, _) = setup(&env, 0);
        env.as_contract(&contract_id, || {
            let (fee, net) = calculate_fee(&env, 1_000);
            assert_eq!(fee, 0);
            assert_eq!(net, 1_000);
        });
    }

    #[test]
    fn fee_calculation_standard() {
        let env = Env::default();
        let (contract_id, _) = setup(&env, 500);
        env.as_contract(&contract_id, || {
            let (fee, net) = calculate_fee(&env, 1_000);
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
        let (contract_id, _) = setup(&env, 10000);
        env.as_contract(&contract_id, || {
            let (fee, net) = calculate_fee(&env, 1_000);
            assert_eq!(fee, 1_000);
            assert_eq!(net, 0);
        });
    }

    // ── get/set config ────────────────────────────────────────────────────────

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
        env.as_contract(&contract_id, || {
            init(&env, 10001, &recipient);
        });
    }
}
