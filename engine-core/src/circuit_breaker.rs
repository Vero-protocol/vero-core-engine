
//! Emergency circuit breaker.
//!
//! Guardians can open the breaker to halt state transitions. Stateful modules
//! call `assert_closed` before mutating protected state.

use crate::event_struct::{ACT_RESET, ACT_TRIP, MOD_CB};
use crate::event_utils::publish_event;
use crate::types::BreakerState;
use soroban_sdk::{contracterror, panic_with_error, symbol_short, vec, Address, BytesN, Env, Symbol, Vec};

//! Emergency circuit-breaker — halts state transitions when tripped.

use crate::event_struct::{ACT_RESET, ACT_TRIP, MOD_CB};
use crate::event_utils::{publish_event, zero_hash};

use crate::types::BreakerState;
use soroban_sdk::{contracterror, panic_with_error, symbol_short, vec, Address, Env, Symbol, Vec};


const KEY_STATE: Symbol = symbol_short!("CB_STATE");
const KEY_GUARDIAN: Symbol = symbol_short!("CB_GUARD");

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub enum BreakerError {
    CircuitOpen = 1,
    NotGuardian = 2,
    AlreadyInState = 3,
    InvalidGuardianSet = 4,

    AlreadyInitialized = 5,

}

/// Initialise the circuit breaker in the closed state.
pub fn init(env: &Env, guardians: Vec<Address>) {

    if guardians.len() == 0 {
        panic_with_error!(env, BreakerError::InvalidGuardianSet);
    }
    env.storage().instance().set(&KEY_STATE, &BreakerState::Closed);
    env.storage().instance().set(&KEY_GUARDIAN, &guardians);
}


    if env.storage().instance().has(&KEY_GUARDIAN) {
        panic_with_error!(env, BreakerError::AlreadyInitialized);
    }
    if guardians.is_empty() {
        panic_with_error!(env, BreakerError::InvalidGuardianSet);
    }

    let mut seen = Vec::new(env);
    for guardian in guardians.iter() {
        if seen.contains(&guardian) {
            panic_with_error!(env, BreakerError::InvalidGuardianSet);
        }
        seen.push_back(guardian);
    }

    env.storage()
        .instance()
        .set(&KEY_STATE, &BreakerState::Closed);
    env.storage().instance().set(&KEY_GUARDIAN, &guardians);
}

/// Return the current breaker state.

pub fn state(env: &Env) -> BreakerState {
    env.storage()
        .instance()
        .get(&KEY_STATE)
        .unwrap_or(BreakerState::Closed)
}


/// Panics with `CircuitOpen` when the breaker is tripped.

pub fn assert_closed(env: &Env) {
    if state(env) == BreakerState::Open {
        panic_with_error!(env, BreakerError::CircuitOpen);
    }
}

/// Trip the breaker — halts guarded state transitions. Requires guardian auth.
pub fn trip(env: &Env, guardian: &Address) {
    crate::non_reentrant!(env);
    guardian.require_auth();
    require_guardian(env, guardian);
    set_state(env, BreakerState::Open);

    publish_event(env, MOD_CB | ACT_TRIP, 0, BytesN::from_array(env, &[0u8; 32]));

    publish_event(env, MOD_CB | ACT_TRIP, 0, zero_hash(env));

}

/// Reset the breaker — resumes guarded state transitions. Requires guardian auth.
pub fn reset(env: &Env, guardian: &Address) {
    crate::non_reentrant!(env);
    guardian.require_auth();
    require_guardian(env, guardian);
    set_state(env, BreakerState::Closed);

    publish_event(env, MOD_CB | ACT_RESET, 0, BytesN::from_array(env, &[0u8; 32]));
}

fn set_state(env: &Env, new_state: BreakerState) {
    if state(env) == new_state {
        panic_with_error!(env, BreakerError::AlreadyInState);
    }
    env.storage().instance().set(&KEY_STATE, &new_state);

    publish_event(env, MOD_CB | ACT_RESET, 0, zero_hash(env));

}

fn set_state(env: &Env, next: BreakerState) {
    let current = state(env);
    if current == next {
        panic_with_error!(env, BreakerError::AlreadyInState);
    }
    env.storage().instance().set(&KEY_STATE, &next);

}

fn require_guardian(env: &Env, caller: &Address) {
    let guardians: Vec<Address> = env
        .storage()
        .instance()
        .get(&KEY_GUARDIAN)
        .unwrap_or(vec![env]);
    if !guardians.contains(caller) {
        panic_with_error!(env, BreakerError::NotGuardian);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{testutils::Address as _, vec, Env, contract, contractimpl};

    #[contract]
    pub struct TestContract;

    #[contractimpl]
    impl TestContract {}

    #[soroban_sdk::contract]
    pub struct TestContract;

    #[soroban_sdk::contractimpl]
    impl TestContract {}




    #[soroban_sdk::contract]
    pub struct TestContract;

    #[soroban_sdk::contractimpl]
    impl TestContract {}

    #[test]
    fn trip_and_reset() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, TestContract);

        let g = Address::generate(&env);
        env.as_contract(&contract_id, || {
            init(&env, vec![&env, g.clone()]);
            assert_closed(&env);
        });
        env.mock_all_auths();
        env.as_contract(&contract_id, || {
            trip(&env, &g);

        let guardian = Address::generate(&env);

        env.as_contract(&contract_id, || {

            init(&env, vec![&env, g.clone()]);
            assert_closed(&env);
            trip(&env, &g);
            assert_eq!(state(&env), BreakerState::Open);
            reset(&env, &g);

            init(&env, vec![&env, guardian.clone()]);
            assert_closed(&env);
        });
        env.as_contract(&contract_id, || {
            trip(&env, &guardian);
            assert_eq!(state(&env), BreakerState::Open);

        });
        env.mock_all_auths();
        env.as_contract(&contract_id, || {

            reset(&env, &g);

            reset(&env, &guardian);

            assert_closed(&env);
        });
    }

    #[test]
    #[should_panic]
    fn non_guardian_cannot_trip() {
        let env = Env::default();
        env.mock_all_auths_allowing_non_root_auth();
        let contract_id = env.register_contract(None, TestContract);

        let g = Address::generate(&env);
        let rogue = Address::generate(&env);
        env.as_contract(&contract_id, || {
            init(&env, vec![&env, g]);

        let guardian = Address::generate(&env);
        let rogue = Address::generate(&env);
        env.as_contract(&contract_id, || {

            let g = Address::generate(&env);
            let rogue = Address::generate(&env);
            init(&env, vec![&env, g]);

            init(&env, vec![&env, guardian]);

            trip(&env, &rogue);
        });
    }
}
