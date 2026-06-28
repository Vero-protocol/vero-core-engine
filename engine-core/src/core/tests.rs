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

    client.initialize(&admin);

    let param_key = symbol_short!("FEE");
    let param_val = 100;
    let payload = BytesN::from_array(&env, &[1u8; 32]);
    let hash = compute_commitment(&[0u8; 32], 1, &payload.to_array());

    let commitment = StateCommitment {
        sequence: 1,
        state_hash: BytesN::from_array(&env, &hash),
        ledger: 100,
        author: admin.clone(),
    };

    env.mock_all_auths();
    client.update_param(&admin, &param_key, &param_val, &commitment, &payload);
}
