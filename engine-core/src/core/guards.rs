use soroban_sdk::{contracterror, panic_with_error, symbol_short, Env, Symbol};

const KEY_REENTRY: Symbol = symbol_short!("C_REENTR");

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum GuardError {
    ReentrancyDetected = 1,
}

pub struct ReentrancyGuard;

impl ReentrancyGuard {
    pub fn new(env: &Env) -> Self {
        if env.storage().temporary().has(&KEY_REENTRY) {
            panic_with_error!(env, GuardError::ReentrancyDetected);
        }
        env.storage().temporary().set(&KEY_REENTRY, &true);
        Self
    }
}

impl Drop for ReentrancyGuard {
    fn drop(&mut self) {
    }
}

pub fn with_guard<T>(env: &Env, f: impl FnOnce() -> T) -> T {
    let _guard = ReentrancyGuard::new(env);
    let result = f();
    env.storage().temporary().remove(&KEY_REENTRY);
    result
}

pub fn enter_guard(env: &Env) {
    if env.storage().temporary().has(&KEY_REENTRY) {
        panic_with_error!(env, GuardError::ReentrancyDetected);
    }
    env.storage().temporary().set(&KEY_REENTRY, &true);
}

pub fn exit_guard(env: &Env) {
    env.storage().temporary().remove(&KEY_REENTRY);
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
    fn guard_enter_and_exit() {
        let env = Env::default();
        let contract_id = env.register_contract(None, TestContract);
        env.as_contract(&contract_id, || {
            enter_guard(&env);
            assert!(env.storage().temporary().has(&KEY_REENTRY));
            exit_guard(&env);
            assert!(!env.storage().temporary().has(&KEY_REENTRY));
        });
    }

    #[test]
    #[should_panic]
    fn double_enter_detected() {
        let env = Env::default();
        let contract_id = env.register_contract(None, TestContract);
        env.as_contract(&contract_id, || {
            enter_guard(&env);
            enter_guard(&env);
        });
    }

    #[test]
    fn with_guard_wraps_call() {
        let env = Env::default();
        let contract_id = env.register_contract(None, TestContract);
        env.as_contract(&contract_id, || {
            let result = with_guard(&env, || 42);
            assert_eq!(result, 42);
            assert!(!env.storage().temporary().has(&KEY_REENTRY));
        });
    }

    #[test]
    fn reentrancy_guard_drop_releases() {
        let env = Env::default();
        let contract_id = env.register_contract(None, TestContract);
        env.as_contract(&contract_id, || {
            {
                let _guard = ReentrancyGuard::new(&env);
                assert!(env.storage().temporary().has(&KEY_REENTRY));
            }
            assert!(!env.storage().temporary().has(&KEY_REENTRY));
        });
    }
}
