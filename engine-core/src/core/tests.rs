use super::control_plane::{ControlPlane, ControlPlaneClient};
use crate::audit::compute_commitment;
use crate::types::StateCommitment;
use soroban_sdk::{testutils::Address as _, Address, BytesN, Env, symbol_short};

#[test]
fn test_initialize() {
    let env = Env::default();
    let contract_id = env.register_contract(None, ControlPlane);
    let client = ControlPlaneClient::new(&env, &contract_id);
    let admin = Address::generate(&env);

    client.initialize(&admin);

    let res = client.try_initialize(&admin);
    assert!(res.is_err());
}

#[test]
fn test_update_param_success() {
    let env = Env::default();
    let contract_id = env.register_contract(None, ControlPlane);
    let client = ControlPlaneClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let author = Address::generate(&env);

    client.initialize(&admin);

    let param_key = symbol_short!("FEE");
    let param_val = 100;
    let payload = BytesN::from_array(&env, &[1u8; 32]);
    let hash = compute_commitment(&[0u8; 32], 1, &payload.to_array());

    let commitment = StateCommitment {
        sequence: 1,
        state_hash: BytesN::from_array(&env, &hash),
        ledger: 100,
        author: author.clone(),
    };

    env.mock_all_auths();
    client.update_param(&admin, &param_key, &param_val, &commitment, &payload);
}

#[test]
fn test_batch_update_param_success() {
    let env = Env::default();
    let contract_id = env.register_contract(None, ControlPlane);
    let client = ControlPlaneClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let author = Address::generate(&env);

    client.initialize(&admin);

    let param1_key = symbol_short!("FEE1");
    let param1_val = 100;
    let param2_key = symbol_short!("FEE2");
    let param2_val = 200;
    
    let mut params = soroban_sdk::Vec::new(&env);
    params.push_back((param1_key, param1_val));
    params.push_back((param2_key, param2_val));

    let payload = BytesN::from_array(&env, &[2u8; 32]);
    let hash = compute_commitment(&[0u8; 32], 2, &payload.to_array());

    let commitment = StateCommitment {
        sequence: 2,
        state_hash: BytesN::from_array(&env, &hash),
        ledger: 101,
        author: author.clone(),
    };

    env.mock_all_auths();
    client.batch_update_param(&admin, &params, &commitment, &payload);
}
