//! Defensive guards used by state-changing entrypoints.

use soroban_sdk::{contracterror, panic_with_error, symbol_short, Env, Symbol};

const KEY_GUARD: Symbol = symbol_short!("RE_GUARD");

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub enum GuardError {
    ReentrancyDetected = 100,
}

/// Sets the reentrancy lock. Panics if the current invocation already holds it.
pub fn enter_reentrancy_guard(env: &Env) {
    if env.storage().temporary().has(&KEY_GUARD) {
        panic_with_error!(env, GuardError::ReentrancyDetected);
    }
    env.storage().temporary().set(&KEY_GUARD, &true);
}

/// Clears the reentrancy lock.
pub fn exit_reentrancy_guard(env: &Env) {
    env.storage().temporary().remove(&KEY_GUARD);
}

/// RAII guard for reentrancy protection.
pub struct ReentrancyGuard<'a> {
    env: &'a Env,
}

impl<'a> ReentrancyGuard<'a> {
    pub fn enter(env: &'a Env) -> Self {
        enter_reentrancy_guard(env);
        Self { env }
    }
}

impl Drop for ReentrancyGuard<'_> {
    fn drop(&mut self) {
        exit_reentrancy_guard(self.env);
    }
}

/// Wrap a state-changing function or block with the reentrancy guard.
#[macro_export]
macro_rules! non_reentrant {
    ($env:expr) => {
        let _reentrancy_guard = $crate::guards::ReentrancyGuard::enter($env);
    };
    ($env:expr, $body:block) => {{
        let _reentrancy_guard = $crate::guards::ReentrancyGuard::enter($env);
        $body
    }};
}
