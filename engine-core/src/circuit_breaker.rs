use soroban_sdk::{contracterror, panic_with_error, symbol_short, vec, Address, Env, IntoVal, Symbol, Vec, BytesN, Map};
//! Emergency circuit-breaker — halts all state transitions when tripped.
//!
//! Only authorised guardians may open or close the breaker.
//! All stateful entry-points must call `assert_closed` before proceeding.

use soroban_sdk::{contracterror, panic_with_error, symbol_short, vec, Address, Env, Symbol, Vec};

use crate::event_struct::{MOD_CB, ACT_TRIP, ACT_RESET};
use crate::event_utils::publish_event;
use crate::types::BreakerState;

const KEY_STATE: Symbol = symbol_short!("CB_STATE");
const KEY_GUARDIAN: Symbol = symbol_short!("CB_GUARD");

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum BreakerError {
    CircuitOpen = 1,
    NotGuardian = 2,
    AlreadyInState = 3,
}

pub fn init(env: &Env, guardians: Vec<Address>) {
    env.storage()
        .instance()
        .set(&KEY_STATE, &BreakerState::Closed);
    env.storage().instance().set(&KEY_GUARDIAN, &guardians);
}

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

pub fn trip(env: &Env, guardian: &Address) {
    require_guardian(env, guardian);
    set_state(env, BreakerState::Open);
    // Single compact event — replaces previous double-emit.
    publish_event(
        env,
        MOD_CB | ACT_TRIP,
        0,
        BytesN::from_array(env, &[0u8; 32]),
    );
    let mut payload = Map::new(env);
    payload.set(symbol_short!("guardian"), guardian.clone().into_val(env));
    publish_event(env, BytesN::from_array(env, & [0u8; 32]), BytesN::from_array(env, & [0u8; 32]), payload);
}

pub fn reset(env: &Env, guardian: &Address) {
    require_guardian(env, guardian);
    set_state(env, BreakerState::Closed);
    // Single compact event — replaces previous double-emit.
    publish_event(
        env,
        MOD_CB | ACT_RESET,
        0,
        BytesN::from_array(env, &[0u8; 32]),
    );
    let mut payload = Map::new(env);
    payload.set(symbol_short!("guardian"), guardian.clone().into_val(env));
    publish_event(env, BytesN::from_array(env, & [0u8; 32]), BytesN::from_array(env, & [0u8; 32]), payload);
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
    use crate::VeroCore;
    use crate::VeroCoreClient;
    use soroban_sdk::{testutils::Address as _, vec, Address, Env};

    #[soroban_sdk::contract]
    pub struct TestContract;

    #[soroban_sdk::contractimpl]
    impl TestContract {}

    #[test]
    fn trip_and_reset() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, VeroCore);
        let client = VeroCoreClient::new(&env, &contract_id);

        let g = Address::generate(&env);
        let signers = vec![&env, Address::generate(&env)];
        client.init(&signers, &1, &vec![&env, g.clone()]);

        client.trip(&g);
        client.reset(&g);
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #2)")] // NotGuardian
    fn non_guardian_cannot_trip() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, VeroCore);
        let client = VeroCoreClient::new(&env, &contract_id);

        let g = Address::generate(&env);
        let rogue = Address::generate(&env);
        let signers = vec![&env, Address::generate(&env)];
        client.init(&signers, &1, &vec![&env, g.clone()]);
        client.trip(&rogue);
    }
}
