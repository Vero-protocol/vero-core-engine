use soroban_sdk::{contracterror, panic_with_error, symbol_short, BytesN, Env, Symbol};
use crate::event_struct::{MOD_CORE, ACT_TRANSITION};
use crate::event_utils::publish_event;

const KEY_STATE: Symbol = symbol_short!("PROTO_ST");

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub enum StateError {
    InvalidTransition = 1,
    AlreadyInState    = 2,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub enum ProtocolState {
    Pending = 0,
    Active  = 1,
    Settled = 2,
    Closed  = 3,
}

impl ProtocolState {
    fn from_u32(v: u32) -> Self {
        match v {
            0 => ProtocolState::Pending,
            1 => ProtocolState::Active,
            2 => ProtocolState::Settled,
            3 => ProtocolState::Closed,
            _ => ProtocolState::Pending,
        }
    }
}

fn allowed_transition(from: ProtocolState, to: ProtocolState) -> bool {
    match (from, to) {
        (ProtocolState::Pending, ProtocolState::Active) => true,
        (ProtocolState::Pending, ProtocolState::Closed) => true,
        (ProtocolState::Active,  ProtocolState::Settled) => true,
        (ProtocolState::Active,  ProtocolState::Closed) => true,
        (ProtocolState::Settled, ProtocolState::Closed) => true,
        _ => false,
    }
}

pub fn init(env: &Env) {
    env.storage().instance().set(&KEY_STATE, &ProtocolState::Pending);
}

pub fn get_state(env: &Env) -> ProtocolState {
    let raw: u32 = env.storage().instance().get(&KEY_STATE).unwrap_or(0);
    ProtocolState::from_u32(raw)
}

pub fn transition_to(env: &Env, next: ProtocolState) {
    let current = get_state(env);
    if current == next {
        panic_with_error!(env, StateError::AlreadyInState);
    }
    if !allowed_transition(current, next) {
        panic_with_error!(env, StateError::InvalidTransition);
    }
    env.storage().instance().set(&KEY_STATE, &next);
    publish_event(
        env,
        MOD_CORE | ACT_TRANSITION,
        current as u64,
        BytesN::from_array(env, &[0u8; 32]),
    );
}

pub fn require_state(env: &Env, expected: ProtocolState) {
    let current = get_state(env);
    if current != expected {
        panic_with_error!(env, StateError::InvalidTransition);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::Env;

    #[soroban_sdk::contract]
    pub struct TestContract;

    #[soroban_sdk::contractimpl]
    impl TestContract {}

    #[test]
    fn initial_state_is_pending() {
        let env = Env::default();
        let contract_id = env.register_contract(None, TestContract);
        env.as_contract(&contract_id, || {
            init(&env);
            assert_eq!(get_state(&env), ProtocolState::Pending);
        });
    }

    #[test]
    fn transition_pending_to_active() {
        let env = Env::default();
        let contract_id = env.register_contract(None, TestContract);
        env.as_contract(&contract_id, || {
            init(&env);
            transition_to(&env, ProtocolState::Active);
            assert_eq!(get_state(&env), ProtocolState::Active);
        });
    }

    #[test]
    fn full_lifecycle() {
        let env = Env::default();
        let contract_id = env.register_contract(None, TestContract);
        env.as_contract(&contract_id, || {
            init(&env);
            transition_to(&env, ProtocolState::Active);
            transition_to(&env, ProtocolState::Settled);
            transition_to(&env, ProtocolState::Closed);
            assert_eq!(get_state(&env), ProtocolState::Closed);
        });
    }

    #[test]
    #[should_panic]
    fn invalid_pending_to_settled_rejected() {
        let env = Env::default();
        let contract_id = env.register_contract(None, TestContract);
        env.as_contract(&contract_id, || {
            init(&env);
            transition_to(&env, ProtocolState::Settled);
        });
    }

    #[test]
    #[should_panic]
    fn require_state_panics_on_mismatch() {
        let env = Env::default();
        let contract_id = env.register_contract(None, TestContract);
        env.as_contract(&contract_id, || {
            init(&env);
            require_state(&env, ProtocolState::Active);
        });
    }
}
