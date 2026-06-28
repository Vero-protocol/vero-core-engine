//! Protocol fee helpers with checked arithmetic.

use crate::burn::reject_zero_address;
use crate::circuit_breaker::assert_closed;
use crate::event_struct::{ACT_FEE, ACT_FEE_TRANSFER, MOD_FEE};
use crate::event_utils::{publish_event, zero_hash};
use crate::guards::ReentrancyGuard;
use soroban_sdk::{
    contracterror, panic_with_error, symbol_short, token, Address, Env, Symbol,
};

const KEY_FEE_BPS:       Symbol = symbol_short!("FEE_BPS");
const KEY_FEE_RECIPIENT: Symbol = symbol_short!("FEE_RCP");
const KEY_TOK_EX:        Symbol = symbol_short!("TOK_EX");

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub enum FeeError {
    InvalidBasisPoints     = 1,
    InvalidRecipient       = 2,
    FeeCalculationOverflow = 3,
    InvalidAmount          = 4,
}

const MAX_BPS: u32 = 10_000;

const ZERO_ADDRESS: &str = "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF";

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

// ── Token exemption ───────────────────────────────────────────────────────────

fn exempt_key(token: &Address) -> (Symbol, Address) {
    (KEY_TOK_EX, token.clone())
}

pub fn set_token_exempt(env: &Env, token: &Address, exempt: bool) {
    let k = exempt_key(token);
    if exempt {
        env.storage().instance().set(&k, &true);
    } else {
        env.storage().instance().remove(&k);
    }
}

pub fn is_token_exempt(env: &Env, token: &Address) -> bool {
    env.storage().instance().has(&exempt_key(token))
}

// ── Fee calculation ───────────────────────────────────────────────────────────

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
/// 3. Rejects `to` = zero address.
/// 4. If `token` is fee-exempt, transfers `amount` from `from` → `to` unchanged.
/// 5. Otherwise splits the transfer: `fee` → fee recipient, `net` → `to`.
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
            .get(&KEY_FEE_RECIPIENT)
            .unwrap_or_else(|| panic_with_error!(env, FeeError::InvalidRecipient));
        tkn.transfer(from, &recipient, &fee);
    }

    if net > 0 {
        tkn.transfer(from, to, &net);
    }

    publish_event(env, MOD_FEE | ACT_FEE_TRANSFER, fee as u64, zero_hash(env));

    net
}

/// Low-level fee deduction from the contract's own token balance.
///
/// Transfers the fee portion from `current_contract_address()` to the
/// configured recipient. Returns the net amount.
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
        token::Client::new(env, token).transfer(
            &env.current_contract_address(),
            &recipient,
            &fee,
        );
        publish_event(env, MOD_FEE | ACT_FEE, fee as u64, zero_hash(env));
    }
    net
}

fn validate_address(env: &Env, addr: &Address) {
    use soroban_sdk::String;
    let zero = String::from_str(env, ZERO_ADDRESS);
    if addr.to_string() == zero {
        panic_with_error!(env, FeeError::InvalidRecipient);
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{testutils::Address as _, token, Env};

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
            use soroban_sdk::vec;
            circuit_breaker::init(&env, vec![&env, guardian.clone()]);
            circuit_breaker::trip(&env, &guardian);
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
        assert_eq!(tkn.balance(&fee_recipient), 0);
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
        assert_eq!(tkn.balance(&fee_recipient), 200);
        assert_eq!(tkn.balance(&from), 0);
    }
}
