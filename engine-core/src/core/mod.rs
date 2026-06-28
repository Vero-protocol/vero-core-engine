pub mod state;
pub mod access;
pub mod guards;

pub use self::state::ProtocolState;
pub use self::state::transition_to as transition_state;
pub use self::state::get_state;
pub use self::state::require_state;
pub use self::access::{initialize, require_role, is_initialized, require_initialized};
pub use self::access::{get_admin, get_operators, get_auditors, ROLE_ADMIN, ROLE_OPERATOR, ROLE_AUDITOR};
pub use self::guards::{with_guard, enter_guard, exit_guard, ReentrancyGuard};

use soroban_sdk::{Address, BytesN, Env};
use crate::audit;
use crate::circuit_breaker;
use crate::governance;
use crate::protocol_fee;
use crate::treasury;
use crate::types::StateCommitment;

pub fn init_full(
    env: &Env,
    admin: &Address,
    operators: soroban_sdk::Vec<Address>,
    auditors: soroban_sdk::Vec<Address>,
    gov_signers: soroban_sdk::Vec<Address>,
    gov_threshold: u32,
    stake_token: Address,
    min_stake: i128,
    cb_guardians: soroban_sdk::Vec<Address>,
    fee_bps: u32,
    fee_recipient: &Address,
) {
    with_guard(env, || {
        access::initialize(env, admin, operators, auditors);
        state::init(env);
        governance::init(env, gov_signers, gov_threshold, stake_token, min_stake);
        circuit_breaker::init(env, cb_guardians);
        treasury::init(env);
        protocol_fee::init(env, fee_bps, fee_recipient);
    });
}

pub fn require_admin(env: &Env, caller: &Address) {
    access::require_role(env, caller, ROLE_ADMIN);
}

pub fn require_operator(env: &Env, caller: &Address) {
    access::require_role(env, caller, ROLE_OPERATOR);
}

pub fn require_auditor(env: &Env, caller: &Address) {
    access::require_role(env, caller, ROLE_AUDITOR);
}

pub fn submit_audit_commitment(env: &Env, commitment: &StateCommitment, payload: &[u8]) {
    with_guard(env, || {
        circuit_breaker::assert_closed(env);
        audit::validate_transition(env, commitment, payload);
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{testutils::Address as _, vec, Env};

    #[soroban_sdk::contract]
    pub struct TestContract;

    #[soroban_sdk::contractimpl]
    impl TestContract {}

    fn make_operators(env: &Env) -> soroban_sdk::Vec<Address> {
        let mut v = vec![env];
        v.push_back(Address::generate(env));
        v
    }

    fn make_auditors(env: &Env) -> soroban_sdk::Vec<Address> {
        let mut v = vec![env];
        v.push_back(Address::generate(env));
        v
    }

    fn make_signers(env: &Env) -> soroban_sdk::Vec<Address> {
        let mut v = vec![env];
        v.push_back(Address::generate(env));
        v
    }

    fn make_guardians(env: &Env) -> soroban_sdk::Vec<Address> {
        let mut v = vec![env];
        v.push_back(Address::generate(env));
        v
    }

    #[test]
    fn full_init_initializes_all_modules() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, TestContract);
        let admin = Address::generate(&env);
        let fee_recipient = Address::generate(&env);
        let stake_token = Address::generate(&env);

        env.as_contract(&contract_id, || {
            init_full(
                &env,
                &admin,
                make_operators(&env),
                make_auditors(&env),
                make_signers(&env),
                1,
                stake_token,
                0,
                make_guardians(&env),
                0,
                &fee_recipient,
            );
            assert!(access::is_initialized(&env));
            assert_eq!(state::get_state(&env), ProtocolState::Pending);
        });
    }

    #[test]
    fn require_admin_calls_succeed() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, TestContract);
        let admin = Address::generate(&env);
        let fee_recipient = Address::generate(&env);
        let stake_token = Address::generate(&env);

        env.as_contract(&contract_id, || {
            init_full(
                &env,
                &admin,
                make_operators(&env),
                make_auditors(&env),
                make_signers(&env),
                1,
                stake_token,
                0,
                make_guardians(&env),
                0,
                &fee_recipient,
            );
            require_admin(&env, &admin);
        });
    }
}
