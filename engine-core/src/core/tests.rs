#![cfg(test)]

use super::engine::{CoreEngine, CoreEngineClient, CoreError, EngineRole};
use soroban_sdk::{testutils::Address as _, Address, Env, vec};

#[test]
fn test_initialize_and_require_role() {
    let env = Env::default();
    let contract_id = env.register_contract(None, CoreEngine);
    let client = CoreEngineClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let operator = Address::generate(&env);
    
    // Mock authentication
    env.mock_all_auths();

    // Initialize
    client.initialize(&admin, &vec![&env, operator.clone()]);

    // Test require_role success
    client.require_role(&admin, &EngineRole::Admin);
    client.require_role(&operator, &EngineRole::Operator);
}

#[test]
#[should_panic(expected = "Error(Contract, #1)")]
fn test_initialize_twice_panics() {
    let env = Env::default();
    let contract_id = env.register_contract(None, CoreEngine);
    let client = CoreEngineClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    
    env.mock_all_auths();

    client.initialize(&admin, &vec![&env]);
    client.initialize(&admin, &vec![&env]);
}

#[test]
#[should_panic(expected = "Error(Contract, #2)")]
fn test_require_role_unauthorized() {
    let env = Env::default();
    let contract_id = env.register_contract(None, CoreEngine);
    let client = CoreEngineClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let unauthorized = Address::generate(&env);
    
    env.mock_all_auths();

    client.initialize(&admin, &vec![&env]);
    client.require_role(&unauthorized, &EngineRole::Admin);
}
