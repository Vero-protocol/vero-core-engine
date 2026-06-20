use soroban_sdk::{
    contracterror, panic_with_error, symbol_short, token, Address, Env, Symbol,
};

const KEY_FEE_BPS: Symbol = symbol_short!("FEE_BPS");
const KEY_FEE_RECIPIENT: Symbol = symbol_short!("FEE_RCP");

#[contracterror]
#[derive(Copy, Clone)]
pub enum FeeError {
    InvalidBasisPoints = 1,
    InvalidRecipient = 2,
    FeeCalculationOverflow = 3,
}

const MAX_BPS: u32 = 10000;

pub fn init(env: &Env, fee_bps: u32, recipient: &Address) {
    if fee_bps > MAX_BPS {
        panic_with_error!(env, FeeError::InvalidBasisPoints);
    }
    validate_address(env, recipient);
    env.storage().instance().set(&KEY_FEE_BPS, &fee_bps);
    env.storage().instance().set(&KEY_FEE_RECIPIENT, recipient);
}

pub fn calculate_fee(env: &Env, amount: i128) -> (i128, i128) {
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

pub fn deduct_fee(env: &Env, token: &Address, amount: i128) -> i128 {
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

        env.events().publish(
            (symbol_short!("FEE"), symbol_short!("deduct")),
            (recipient, fee, net),
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
    use soroban_sdk::{testutils::Address as _, Env};

    #[soroban_sdk::contract]
    pub struct TestContract;

    #[soroban_sdk::contractimpl]
    impl TestContract {}

    fn setup(env: &Env, fee_bps: u32) -> (soroban_sdk::Address, Address) {
        let contract_id = env.register_contract(None, TestContract);
        let recipient = Address::generate(env);
        env.as_contract(&contract_id, || {
            init(env, fee_bps, &recipient);
        });
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
        let (contract_id, _) = setup(&env, 10000);
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
            init(&env, 10001, &recipient);
        });
    }
}
