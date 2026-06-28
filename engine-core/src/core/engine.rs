//! Foundation for the Vero Protocol control plane.
//! Provides initialization and basic access control mechanisms.

use soroban_sdk::{contract, contractimpl, contracttype, contracterror, panic_with_error, Address, Env, Symbol, Vec, symbol_short};

pub const ADMIN_KEY: Symbol = symbol_short!("ADMIN");
pub const INIT_FLAG: Symbol = symbol_short!("INIT");

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum CoreError {
    AlreadyInitialized = 1,
    Unauthorized = 2,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EngineRole {
    Admin,
    Operator,
}

#[contract]
pub struct CoreEngine;

#[contractimpl]
impl CoreEngine {
    /// Initialize the core engine with an admin and an optional list of operators.
    pub fn initialize(env: Env, admin: Address, operators: Vec<Address>) {
        if env.storage().persistent().has(&INIT_FLAG) {
            panic_with_error!(&env, CoreError::AlreadyInitialized);
        }

        admin.require_auth();

        env.storage().persistent().set(&ADMIN_KEY, &admin);
        env.storage().persistent().set(&INIT_FLAG, &true);

        for operator in operators.iter() {
            env.storage().persistent().set(&operator, &EngineRole::Operator);
        }
        
        env.storage().persistent().set(&admin, &EngineRole::Admin);

        env.events().publish(
            (symbol_short!("CORE"), symbol_short!("init")),
            admin.clone(),
        );
    }

    /// Check if an address has a specific role
    pub fn require_role(env: &Env, caller: Address, role: EngineRole) {
        caller.require_auth();
        let stored_role = env.storage().persistent().get::<_, EngineRole>(&caller);
        
        if stored_role != Some(role) {
            panic_with_error!(env, CoreError::Unauthorized);
        }
    }
}
