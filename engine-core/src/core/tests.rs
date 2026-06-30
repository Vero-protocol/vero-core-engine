use super::control_plane::{ControlPlane, ControlPlaneClient};
use crate::audit::compute_commitment;
use crate::core::zk_hooks;
use crate::types::StateCommitment;
use soroban_sdk::{
    symbol_short,
    testutils::Address as _,
    Address, Bytes, BytesN, Env, Map, Symbol,
};

fn commitment(env: &Env, author: &Address, sequence: u64, payload: &BytesN<32>) -> StateCommitment {
    let hash = compute_commitment(&[0u8; 32], sequence, &payload.to_array());
    StateCommitment {
        sequence,
        state_hash: BytesN::from_array(env, &hash),
        ledger: env.ledger().sequence(),
        author: author.clone(),
    }
}

fn initialized_client(env: &Env) -> (ControlPlaneClient<'_>, Address, Address) {
    env.mock_all_auths();
    let contract_id = env.register_contract(None, ControlPlane);
    let client = ControlPlaneClient::new(env, &contract_id);
    let admin = Address::generate(env);
    client.initialize(&admin);
    (client, admin, contract_id)
}

#[test]
fn test_initialize() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, ControlPlane);
    let client = ControlPlaneClient::new(&env, &contract_id);
    let admin = Address::generate(&env);

    client.initialize(&admin);
    assert_eq!(client.get_admin(), admin);

    let res = client.try_initialize(&admin);
    assert!(res.is_err());
}

#[test]
#[should_panic]
fn test_get_admin_rejects_uninitialized() {
    let env = Env::default();
    let contract_id = env.register_contract(None, ControlPlane);
    let client = ControlPlaneClient::new(&env, &contract_id);
    client.get_admin(); // should panic: NotInitialized
}

#[test]
fn test_update_param_success() {
    let env = Env::default();
    let (client, admin, _) = initialized_client(&env);

    let param_key = symbol_short!("FEE");
    let param_val = 100u64;
    let payload = BytesN::from_array(&env, &[1u8; 32]);
    let c = commitment(&env, &admin, 1, &payload);

    assert!(client.integrity_check(&c, &payload));
    client.update_param(&admin, &param_key, &param_val, &c, &payload);

    assert_eq!(client.get_param(&param_key), Some(param_val));
    assert_eq!(client.param_update_count(), 1);
    assert_eq!(client.last_audit_sequence(), 1);
    assert_eq!(client.state_hash(), c.state_hash);
}

#[test]
fn test_update_param_rejects_replayed_commitment() {
    let env = Env::default();
    let (client, admin, _) = initialized_client(&env);

    let param_key = symbol_short!("FEE");
    let payload = BytesN::from_array(&env, &[2u8; 32]);
    let c = commitment(&env, &admin, 1, &payload);

    client.update_param(&admin, &param_key, &100, &c, &payload);
    let replay = client.try_update_param(&admin, &param_key, &200, &c, &payload);
    assert!(replay.is_err());
    assert_eq!(client.get_param(&param_key), Some(100));
}

#[test]
fn test_update_param_rejects_bad_hash() {
    let env = Env::default();
    let (client, admin, _) = initialized_client(&env);

    let param_key = symbol_short!("FEE");
    let payload = BytesN::from_array(&env, &[3u8; 32]);
    let mut c = commitment(&env, &admin, 1, &payload);
    c.state_hash = BytesN::from_array(&env, &[9u8; 32]);

    let result = client.try_update_param(&admin, &param_key, &100, &c, &payload);
    assert!(result.is_err());
    assert_eq!(client.get_param(&param_key), None);
}

#[test]
fn test_update_param_requires_admin() {
    let env = Env::default();
    let (client, admin, _) = initialized_client(&env);
    let rogue = Address::generate(&env);

    let param_key = symbol_short!("FEE");
    let payload = BytesN::from_array(&env, &[4u8; 32]);
    let c = commitment(&env, &admin, 1, &payload);

    let result = client.try_update_param(&rogue, &param_key, &100, &c, &payload);
    assert!(result.is_err());
    assert_eq!(client.get_param(&param_key), None);
}

#[test]
fn test_zk_proof_registration_for_current_state_root() {
    let env = Env::default();
    let (client, admin, contract_id) = initialized_client(&env);

    let param_key = symbol_short!("FEE");
    let payload = BytesN::from_array(&env, &[5u8; 32]);
    let c = commitment(&env, &admin, 1, &payload);
    client.update_param(&admin, &param_key, &100, &c, &payload);

    let proof_hash = BytesN::from_array(&env, &[7u8; 32]);
    let metadata: Map<Symbol, Bytes> = Map::new(&env);
    client.register_proof(
        &admin,
        &c.state_hash,
        &proof_hash,
        &env.ledger().sequence(),
        &metadata,
    );

    assert_eq!(client.get_proof(&c.state_hash), Some(proof_hash));
    let proof_count = env.as_contract(&contract_id, || zk_hooks::proof_count(&env));
    assert_eq!(proof_count, 1);
}

#[test]
fn test_zk_proof_rejects_wrong_state_root() {
    let env = Env::default();
    let (client, admin, _) = initialized_client(&env);

    let param_key = symbol_short!("FEE");
    let payload = BytesN::from_array(&env, &[6u8; 32]);
    let c = commitment(&env, &admin, 1, &payload);
    client.update_param(&admin, &param_key, &100, &c, &payload);

    let wrong_root = BytesN::from_array(&env, &[8u8; 32]);
    let proof_hash = BytesN::from_array(&env, &[7u8; 32]);
    let metadata: Map<Symbol, Bytes> = Map::new(&env);

    let result = client.try_register_proof(
        &admin,
        &wrong_root,
        &proof_hash,
        &env.ledger().sequence(),
        &metadata,
    );
    assert!(result.is_err());
}

#[test]
fn test_batch_update_param_success() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, ControlPlane);
    let client = ControlPlaneClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin);

    let param1_key = symbol_short!("FEE1");
    let param2_key = symbol_short!("FEE2");

    let mut params = soroban_sdk::Vec::new(&env);
    params.push_back((param1_key, 100u64));
    params.push_back((param2_key, 200u64));

    let payload = BytesN::from_array(&env, &[2u8; 32]);
    let hash = compute_commitment(&[0u8; 32], 2, &payload.to_array());
    let c = StateCommitment {
        sequence: 2,
        state_hash: BytesN::from_array(&env, &hash),
        ledger: 101,
        author: admin.clone(),
    };

    client.batch_update_param(&admin, &params, &c, &payload);
}
