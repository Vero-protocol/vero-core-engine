//! Emergency circuit-breaker — halts all state transitions when tripped.
//!
//! Guardians can open the breaker to halt all guarded state mutations.
//! Stateful modules call `assert_closed` before mutating protected state.

use soroban_sdk::{contracterror, panic_with_error, symbol_short, vec, Address, Env, Symbol, Vec};

use crate::event_struct::{ACT_RESET, ACT_TRIP, MOD_CB};
use crate::event_utils::{publish_event, zero_hash};
use crate::types::BreakerState;

const KEY_STATE:    Symbol = symbol_short!("CB_STATE");
const KEY_GUARDIAN: Symbol = symbol_short!("CB_GUARD");

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub enum BreakerError {
    CircuitOpen        = 1,
    NotGuardian        = 2,
    AlreadyInState     = 3,
    InvalidGuardianSet = 4,
    AlreadyInitialized = 5,
}

/// Initialise the circuit breaker in the closed state.
///
/// Panics if `guardians` is empty or if the breaker has already been initialised.
pub fn init(env: &Env, guardians: Vec<Address>) {
    if env.storage().instance().has(&KEY_GUARDIAN) {
        panic_with_error!(env, BreakerError::AlreadyInitialized);
    }
    if guardians.is_empty() {
        panic_with_error!(env, BreakerError::InvalidGuardianSet);
    }
    let mut seen = Vec::new(env);
    for g in guardians.iter() {
        if seen.contains(&g) {
            panic_with_error!(env, BreakerError::InvalidGuardianSet);
        }
        seen.push_back(g);
    }
    env.storage().instance().set(&KEY_STATE, &BreakerState::Closed);
    env.storage().instance().set(&KEY_GUARDIAN, &guardians);
}

/// Return the current breaker state (defaults to `Closed` before `init`).
pub fn state(env: &Env) -> BreakerState {
    env.storage()
        .instance()
        .get(&KEY_STATE)
        .unwrap_or(BreakerState::Closed)
}

/// Panics with [`BreakerError::CircuitOpen`] when the breaker is open.
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
    publish_event(env, MOD_CB | ACT_TRIP, 0, zero_hash(env));
}

/// Reset the breaker — resumes guarded state transitions. Requires guardian auth.
pub fn reset(env: &Env, guardian: &Address) {
    crate::non_reentrant!(env);
    guardian.require_auth();
    require_guardian(env, guardian);
    set_state(env, BreakerState::Closed);
    publish_event(env, MOD_CB | ACT_RESET, 0, zero_hash(env));
}

fn set_state(env: &Env, next: BreakerState) {
    if state(env) == next {
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
    use soroban_sdk::{testutils::Address as _, vec, Env};

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
            assert_eq!(state(&env), BreakerState::Closed);
        });
        env.as_contract(&contract_id, || {
            trip(&env, &g);
            assert_eq!(state(&env), BreakerState::Open);
        });
        env.as_contract(&contract_id, || {
            reset(&env, &g);
            assert_closed(&env);
        });
    }

    #[test]
    #[should_panic]
    fn non_guardian_cannot_trip() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, TestContract);
        let guardian = Address::generate(&env);
        let rogue = Address::generate(&env);
        env.as_contract(&contract_id, || {
            init(&env, vec![&env, guardian]);
            trip(&env, &rogue);
        });
    }

    #[test]
    #[should_panic]
    fn empty_guardian_set_rejected() {
        let env = Env::default();
        let contract_id = env.register_contract(None, TestContract);
        env.as_contract(&contract_id, || {
            init(&env, vec![&env]);
        });
    }
}
