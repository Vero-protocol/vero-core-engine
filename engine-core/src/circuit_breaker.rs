//! Emergency circuit-breaker — halts all state transitions when tripped.
//!
//! Only authorised guardians may open or close the breaker.
//! All stateful entry-points must call `assert_closed` before proceeding.

use soroban_sdk::{contracterror, panic_with_error, symbol_short, vec, Address, Env, Symbol, Vec, Map};

use crate::types::BreakerState;
use crate::access;

const KEY_STATE:    Symbol = symbol_short!("CB_STATE");
const KEY_GUARDIAN: Symbol = symbol_short!("CB_GUARD");
const KEY_GUARDIAN_MAP: Symbol = symbol_short!("CB_GUARD_MAP");

#[contracterror]
#[derive(Copy, Clone)]
pub enum BreakerError {
    CircuitOpen      = 1,
    NotGuardian      = 2,
    AlreadyInState   = 3,
}

pub fn init(env: &Env, guardians: Vec<Address>) {
    env.storage().instance().set(&KEY_STATE, &BreakerState::Closed);
    env.storage().instance().set(&KEY_GUARDIAN, &guardians);
    // Build a Map index for guardian membership checks to avoid linear scans
    let mut guard_map: Map<Address, bool> = Map::new(env);
    let len = guardians.len();
    let mut i = 0usize;
    while i < len {
        let a = guardians.get(i).unwrap();
        guard_map.set(a, true);
        i += 1;
    }
    env.storage().instance().set(&KEY_GUARDIAN_MAP, &guard_map);
}

/// Panics with `BreakerError::CircuitOpen` when the breaker is tripped.
pub fn assert_closed(env: &Env) {
    let state: BreakerState = env
        .storage()
        .instance()
        .get(&KEY_STATE)
        .unwrap_or(BreakerState::Closed);
    if state == BreakerState::Open {
        panic_with_error!(env, BreakerError::CircuitOpen);
    }
}

/// Trip the breaker — halts the engine. Requires guardian auth.
pub fn trip(env: &Env, guardian: &Address) {
    guardian.require_auth();
    // Backwards-compatible: allow either the stored guardian set or an on-chain PAUSE_GUARDIAN role
    let guard_map: Map<Address, bool> = env
        .storage()
        .instance()
        .get(&KEY_GUARDIAN_MAP)
        .unwrap_or(Map::new(env));
    if !guard_map.get(guardian).unwrap_or(false) && !access::has_role(env, guardian, access::ROLE_PAUSE_GUARDIAN) {
        panic_with_error!(env, BreakerError::NotGuardian);
    }
    set_state(env, BreakerState::Open);
    env.events().publish(
        (symbol_short!("CB"), symbol_short!("tripped")),
        guardian.clone(),
    );
}

/// Reset the breaker — resumes normal operation. Requires guardian auth.
pub fn reset(env: &Env, guardian: &Address) {
    guardian.require_auth();
    // Only ADMIN may reset the breaker.
    access::require_role(env, guardian, access::ROLE_ADMIN);
    set_state(env, BreakerState::Closed);
    env.events().publish(
        (symbol_short!("CB"), symbol_short!("reset")),
        guardian.clone(),
    );
}

fn set_state(env: &Env, state: BreakerState) {
    let current: BreakerState = env
        .storage()
        .instance()
        .get(&KEY_STATE)
        .unwrap_or(BreakerState::Closed);
    if current == state {
        panic_with_error!(env, BreakerError::AlreadyInState);
    }
    env.storage().instance().set(&KEY_STATE, &state);
}

fn require_guardian(env: &Env, caller: &Address) {
    // Prefer indexed guardian lookup when available to avoid O(n) scans
    let guard_map: Map<Address, bool> = env
        .storage()
        .instance()
        .get(&KEY_GUARDIAN_MAP)
        .unwrap_or(Map::new(env));
    if !guard_map.get(caller).unwrap_or(false) {
        panic_with_error!(env, BreakerError::NotGuardian);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{testutils::Address as _, vec, Env};

    #[test]
    fn trip_and_reset() {
        let env = Env::default();
        env.mock_all_auths();
        let g = Address::generate(&env);
        init(&env, vec![&env, g.clone()]);

        assert_closed(&env); // should not panic
        trip(&env, &g);

        let state: BreakerState = env.storage().instance().get(&KEY_STATE).unwrap();
        assert_eq!(state, BreakerState::Open);

        reset(&env, &g);
        assert_closed(&env); // back to closed — no panic
    }

    #[test]
    #[should_panic]
    fn non_guardian_cannot_trip() {
        let env = Env::default();
        env.mock_all_auths();
        let g = Address::generate(&env);
        let rogue = Address::generate(&env);
        init(&env, vec![&env, g.clone()]);
        trip(&env, &rogue);
    }
}
