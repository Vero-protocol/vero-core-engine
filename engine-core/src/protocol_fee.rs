//! Protocol-level fee-on-transfer logic.
//!
//! Every token transfer that flows through the Vero engine passes through
//! [`fee_on_transfer`], which splits the gross amount into a protocol fee
//! (routed to the configured recipient) and the net amount (forwarded to
//! the intended recipient).
//!
//! ## Storage keys
//! | key         | type      | meaning                         |
//! |-------------|-----------|--------------------------------|
//! | `FEE_BPS`   | `u32`     | fee in basis points (0–10 000) |
//! | `FEE_RCP`   | `Address` | fee recipient                  |
//! | `(TOK_EX, token)` | `bool` | per-token fee exemption   |
//!
//! ## Guards
//! * Circuit breaker — all stateful paths call [`assert_closed`].
//! * Reentrancy guard — [`fee_on_transfer`] holds the re-entrancy lock for
//!   its entire execution.

use soroban_sdk::{
    contracterror, panic_with_error, symbol_short, token, Address, BytesN, Env, Symbol,
};

use crate::burn::reject_zero_address;
use crate::circuit_breaker::assert_closed;
use crate::event_struct::{ACT_FEE, ACT_FEE_TRANSFER, MOD_FEE};
use crate::event_utils::publish_event;
use crate::guards::ReentrancyGuard;

// ── Storage keys ──────────────────────────────────────────────────────────────

const KEY_FEE_BPS: Symbol = symbol_short!("FEE_BPS");
const KEY_FEE_RCP: Symbol = symbol_short!("FEE_RCP");
// Per-token exemption keys are (KEY_TOK_EX, token_address) tuples.
const KEY_TOK_EX: Symbol = symbol_short!("TOK_EX");

// ── Errors ────────────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Copy, Clone)]
pub enum FeeError {
    InvalidBasisPoints     = 1,
    InvalidRecipient       = 2,
    FeeCalculationOverflow = 3,
}

const MAX_BPS: u32 = 10_000;

// ── Configuration ─────────────────────────────────────────────────────────────

/// Initialise fee configuration. Must be called once during contract setup.
///
/// Panics with [`FeeError::InvalidBasisPoints`] if `fee_bps > 10 000`.
pub fn init(env: &Env, fee_bps: u32, recipient: &Address) {
    if fee_bps > MAX_BPS {
        panic_with_error!(env, FeeError::InvalidBasisPoints);
    }
    env.storage().instance().set(&KEY_FEE_BPS, &fee_bps);
    env.storage().instance().set(&KEY_FEE_RCP, recipient);
}

/// Update the fee rate. Panics if `fee_bps > 10 000`.
pub fn set_fee_bps(env: &Env, fee_bps: u32) {
    if fee_bps > MAX_BPS {
        panic_with_error!(env, FeeError::InvalidBasisPoints);
    }
    env.storage().instance().set(&KEY_FEE_BPS, &fee_bps);
}

/// Update the fee recipient address.
pub fn set_fee_recipient(env: &Env, recipient: &Address) {
    env.storage().instance().set(&KEY_FEE_RCP, recipient);
}

/// Returns `(fee_bps, recipient)` from storage.
pub fn get_fee_config(env: &Env) -> (u32, Option<Address>) {
    let fee_bps: u32 = env.storage().instance().get(&KEY_FEE_BPS).unwrap_or(0);
    let recipient: Option<Address> = env.storage().instance().get(&KEY_FEE_RCP);
    (fee_bps, recipient)
}

// ── Token exemption ───────────────────────────────────────────────────────────

fn exempt_key(token: &Address) -> (Symbol, Address) {
    (KEY_TOK_EX, token.clone())
}

/// Mark or unmark `token` as exempt from protocol fees.
///
/// Exempt tokens are forwarded in full during [`fee_on_transfer`] with no
/// fee deducted and no fee-recipient transfer.
pub fn set_token_exempt(env: &Env, token: &Address, exempt: bool) {
    let k = exempt_key(token);
    if exempt {
        env.storage().instance().set(&k, &true);
    } else {
        env.storage().instance().remove(&k);
    }
}

/// Returns `true` if `token` is currently exempt from protocol fees.
pub fn is_token_exempt(env: &Env, token: &Address) -> bool {
    env.storage().instance().has(&exempt_key(token))
}

// ── Fee arithmetic ────────────────────────────────────────────────────────────

/// Compute `(fee, net)` for a gross `amount` using the stored `fee_bps`.
///
/// Returns `(0, amount)` when `fee_bps` is 0 or `amount` is 0.
/// Fee is rounded down (floor division).
pub fn calculate_fee(env: &Env, amount: i128) -> (i128, i128) {
    let fee_bps: u32 = env.storage().instance().get(&KEY_FEE_BPS).unwrap_or(0);
    if fee_bps == 0 || amount == 0 {
        return (0, amount);
    }
    let fee = amount
        .checked_mul(fee_bps as i128)
        .and_then(|v| v.checked_div(10_000))
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
        publish_event(
            env,
            MOD_FEE | ACT_FEE,
            fee as u64,
            BytesN::from_array(env, &[0u8; 32]),
        );
    }
    net
}

// ── Tests ─────────────────────────────────────────────────────────────────────

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
        env.as_contract(&contract_id, || {
            init(env, fee_bps, &recipient);
        });
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
    fn zero_amount_no_fee() {
        let env = Env::default();
        let (contract_id, _) = setup(&env, 500);
        env.as_contract(&contract_id, || {
            let (fee, net) = calculate_fee(&env, 0);
            assert_eq!(fee, 0);
            assert_eq!(net, 0);
        });
    }

    #[test]
    fn fee_at_full_bps_is_full_amount() {
        let env = Env::default();
        let (contract_id, _) = setup(&env, 10_000);
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
    fn set_fee_recipient_updates_address() {
        let env = Env::default();
        let (contract_id, _) = setup(&env, 100);
        let new_recipient = Address::generate(&env);
        env.as_contract(&contract_id, || {
            set_fee_recipient(&env, &new_recipient);
            let (_, rec) = get_fee_config(&env);
            assert_eq!(rec.unwrap(), new_recipient);
        });
    }

    #[test]
    #[should_panic]
    fn init_rejects_bps_over_max() {
        let env = Env::default();
        let contract_id = env.register_contract(None, TestContract);
        let recipient = Address::generate(&env);
        env.as_contract(&contract_id, || {
            init(&env, 10_001, &recipient);
        });
    }

    // ── token exemption ───────────────────────────────────────────────────────

    #[test]
    fn token_exempt_false_by_default() {
        let env = Env::default();
        let (contract_id, _) = setup(&env, 500);
        let token = Address::generate(&env);
        env.as_contract(&contract_id, || {
            assert!(!is_token_exempt(&env, &token));
        });
    }

    #[test]
    fn token_exempt_set_and_check() {
        let env = Env::default();
        let (contract_id, _) = setup(&env, 500);
        let token = Address::generate(&env);
        env.as_contract(&contract_id, || {
            set_token_exempt(&env, &token, true);
            assert!(is_token_exempt(&env, &token));
        });
    }

    #[test]
    fn token_exempt_cleared() {
        let env = Env::default();
        let (contract_id, _) = setup(&env, 500);
        let token = Address::generate(&env);
        env.as_contract(&contract_id, || {
            set_token_exempt(&env, &token, true);
            set_token_exempt(&env, &token, false);
            assert!(!is_token_exempt(&env, &token));
        });
    }

    #[test]
    fn multiple_tokens_exempt_independently() {
        let env = Env::default();
        let (contract_id, _) = setup(&env, 500);
        let token_a = Address::generate(&env);
        let token_b = Address::generate(&env);
        env.as_contract(&contract_id, || {
            set_token_exempt(&env, &token_a, true);
            assert!(is_token_exempt(&env, &token_a));
            assert!(!is_token_exempt(&env, &token_b));
        });
    }

    // ── fee_on_transfer ───────────────────────────────────────────────────────

    #[test]
    #[should_panic]
    fn fee_on_transfer_circuit_open_panics() {
        let env = Env::default();
        env.mock_all_auths();
        let (contract_id, _) = setup(&env, 500);
        let token = Address::generate(&env);
        let from = Address::generate(&env);
        let to = Address::generate(&env);
        let guardian = Address::generate(&env);

        env.as_contract(&contract_id, || {
            circuit_breaker::init(&env, vec![&env, guardian.clone()]);
            circuit_breaker::trip(&env, &guardian);
            // Circuit is now open — fee_on_transfer must panic.
            fee_on_transfer(&env, &token, &from, &to, 1_000);
        });
    }

    #[test]
    fn fee_on_transfer_standard_fee_splits_correctly() {
        let env = Env::default();
        env.mock_all_auths_allowing_non_root_auth();

        let (token_addr, stellar) = make_token(&env);
        let (contract_id, fee_recipient) = setup(&env, 500); // 5 %

        let from = Address::generate(&env);
        let to = Address::generate(&env);
        stellar.mint(&from, &1_000i128);

        let net = env.as_contract(&contract_id, || {
            fee_on_transfer(&env, &token_addr, &from, &to, 1_000)
        });

        let tkn = token::Client::new(&env, &token_addr);
        assert_eq!(net, 950);
        assert_eq!(tkn.balance(&to), 950);
        assert_eq!(tkn.balance(&fee_recipient), 50);
        assert_eq!(tkn.balance(&from), 0);
    }

    #[test]
    fn fee_on_transfer_zero_bps_passes_full_amount() {
        let env = Env::default();
        env.mock_all_auths_allowing_non_root_auth();

        let (token_addr, stellar) = make_token(&env);
        let (contract_id, _) = setup(&env, 0);

        let from = Address::generate(&env);
        let to = Address::generate(&env);
        stellar.mint(&from, &500i128);

        let net = env.as_contract(&contract_id, || {
            fee_on_transfer(&env, &token_addr, &from, &to, 500)
        });

        let tkn = token::Client::new(&env, &token_addr);
        assert_eq!(net, 500);
        assert_eq!(tkn.balance(&to), 500);
        assert_eq!(tkn.balance(&from), 0);
    }

    #[test]
    fn fee_on_transfer_exempt_token_no_fee_deducted() {
        let env = Env::default();
        env.mock_all_auths_allowing_non_root_auth();

        let (token_addr, stellar) = make_token(&env);
        let (contract_id, fee_recipient) = setup(&env, 500); // 5 % — but token is exempt

        let from = Address::generate(&env);
        let to = Address::generate(&env);
        stellar.mint(&from, &1_000i128);

        env.as_contract(&contract_id, || {
            set_token_exempt(&env, &token_addr, true);
        });

        let net = env.as_contract(&contract_id, || {
            fee_on_transfer(&env, &token_addr, &from, &to, 1_000)
        });

        let tkn = token::Client::new(&env, &token_addr);
        assert_eq!(net, 1_000);
        assert_eq!(tkn.balance(&to), 1_000);
        assert_eq!(tkn.balance(&fee_recipient), 0); // no fee taken
        assert_eq!(tkn.balance(&from), 0);
    }

    #[test]
    fn fee_on_transfer_full_bps_all_goes_to_recipient() {
        let env = Env::default();
        env.mock_all_auths_allowing_non_root_auth();

        let (token_addr, stellar) = make_token(&env);
        let (contract_id, fee_recipient) = setup(&env, 10_000); // 100 %

        let from = Address::generate(&env);
        let to = Address::generate(&env);
        stellar.mint(&from, &200i128);

        let net = env.as_contract(&contract_id, || {
            fee_on_transfer(&env, &token_addr, &from, &to, 200)
        });

        let tkn = token::Client::new(&env, &token_addr);
        assert_eq!(net, 0);
        assert_eq!(tkn.balance(&to), 0);
        assert_eq!(tkn.balance(&fee_recipient), 200);
    }
}
