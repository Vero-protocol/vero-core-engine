use super::control_plane::{ControlPlane, ControlPlaneClient};
use super::proxy::{UpgradeableProxy, UpgradeableProxyClient};
use crate::audit::compute_commitment;
use crate::core::zk_hooks;
use crate::types::StateCommitment;
use crate::version::{CONTRACT_VERSION, VERSION_MAJOR, VERSION_MINOR, VERSION_PATCH};
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

fn setup_client(env: &Env) -> (Address, ControlPlaneClient<'_>) {
    let contract_id = env.register_contract(None, ControlPlane);
    let client = ControlPlaneClient::new(env, &contract_id);
    (contract_id, client)
}

fn make_commitment(env: &Env, admin: &Address, seq: u32) -> (StateCommitment, BytesN<32>) {
    let payload = BytesN::from_array(env, &[seq as u8; 32]);
    let hash = compute_commitment(&[0u8; 32], seq as u64, &payload.to_array());
    let commitment = StateCommitment {
        sequence: seq as u64,
        state_hash: BytesN::from_array(env, &hash),
        ledger: 100,
        author: admin.clone(),
    };
    (commitment, payload)
}

#[test]
fn test_initialize() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, ControlPlane);
    let client = ControlPlaneClient::new(&env, &contract_id);
    let admin = Address::generate(&env);

    client.initialize(&admin);
    assert_eq!(client.admin(), Some(admin.clone()));

    assert_eq!(client.get_admin(), admin);

    let res = client.try_initialize(&admin);
    assert!(res.is_err());

    // Version tracking is initialised
    assert_eq!(client.contract_version(), CONTRACT_VERSION);
    let (maj, min, pat) = client.version();
    assert_eq!((maj, min, pat), (VERSION_MAJOR, VERSION_MINOR, VERSION_PATCH));
}

#[test]
fn test_initialize_rejects_contract_as_its_own_admin() {
    let env = Env::default();
    let (contract_id, client) = setup_client(&env);
    // The contract can never authorize on its own behalf, so this would be an
    // unusable admin; reject it explicitly instead of bricking the contract.
    let res = client.try_initialize(&contract_id);
    assert!(res.is_err());
}

#[test]
#[should_panic]
fn test_get_admin_rejects_uninitialized() {
    let env = Env::default();
    let (_, client) = setup_client(&env);
    client.get_admin();
}

#[test]
fn test_update_param_full_flow() {
    let env = Env::default();
    let (client, admin, _) = initialized_client(&env);

    let param_key = symbol_short!("FEE");
    let param_val = 100u64;
    let payload = BytesN::from_array(&env, &[1u8; 32]);
    let commitment = commitment(&env, &admin, 1, &payload);

    assert!(client.integrity_check(&commitment, &payload));
    client.update_param(&admin, &param_key, &param_val, &commitment, &payload);

    assert_eq!(client.get_param(&param_key), Some(param_val));
    assert_eq!(client.param_update_count(), 1);
    assert_eq!(client.last_audit_sequence(), 1);
    assert_eq!(client.state_hash(), commitment.state_hash);
}

#[test]
fn test_update_param_rejects_replayed_commitment() {
    let env = Env::default();
    let (client, admin, _) = initialized_client(&env);

    let param_key = symbol_short!("FEE");
    let payload = BytesN::from_array(&env, &[2u8; 32]);
    let commitment = commitment(&env, &admin, 1, &payload);

    client.update_param(&admin, &param_key, &100, &commitment, &payload);
    let replay = client.try_update_param(&admin, &param_key, &200, &commitment, &payload);
    assert!(replay.is_err());
    assert_eq!(client.get_param(&param_key), Some(100));
}

#[test]
fn test_update_param_rejects_bad_hash() {
    let env = Env::default();
    let (client, admin, _) = initialized_client(&env);

    let param_key = symbol_short!("FEE");
    let payload = BytesN::from_array(&env, &[3u8; 32]);
    let mut commitment = commitment(&env, &admin, 1, &payload);
    commitment.state_hash = BytesN::from_array(&env, &[9u8; 32]);

    let result = client.try_update_param(&admin, &param_key, &100, &commitment, &payload);
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
    let commitment = commitment(&env, &admin, 1, &payload);

    let result = client.try_update_param(&rogue, &param_key, &100, &commitment, &payload);
    assert!(result.is_err());
    assert_eq!(client.get_param(&param_key), None);
}

#[test]
fn test_zk_proof_registration_for_current_state_root() {
    let env = Env::default();
    let (client, admin, contract_id) = initialized_client(&env);

    let param_key = symbol_short!("FEE");
    let payload = BytesN::from_array(&env, &[5u8; 32]);
    let commitment = commitment(&env, &admin, 1, &payload);
    client.update_param(&admin, &param_key, &100, &commitment, &payload);

    let proof_hash = BytesN::from_array(&env, &[7u8; 32]);
    let metadata: Map<Symbol, Bytes> = Map::new(&env);
    client.register_proof(
        &admin,
        &commitment.state_hash,
        &proof_hash,
        &env.ledger().sequence(),
        &metadata,
    );

    assert_eq!(client.get_proof(&commitment.state_hash), Some(proof_hash));
    let proof_count = env.as_contract(&contract_id, || zk_hooks::proof_count(&env));
    assert_eq!(proof_count, 1);
}

#[test]
fn test_zk_proof_rejects_wrong_state_root() {
    let env = Env::default();
    let (client, admin, _) = initialized_client(&env);

    let param_key = symbol_short!("FEE");
    let payload = BytesN::from_array(&env, &[6u8; 32]);
    let commitment = commitment(&env, &admin, 1, &payload);
    client.update_param(&admin, &param_key, &100, &commitment, &payload);

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

    client.batch_update_param(&admin, &params, &commitment, &payload);
}

fn proxy_initialized_client(env: &Env) -> (UpgradeableProxyClient<'_>, Address) {
    env.mock_all_auths();
    let contract_id = env.register_contract(None, UpgradeableProxy);
    let client = UpgradeableProxyClient::new(env, &contract_id);
    let admin = Address::generate(env);
    client.init(&admin);
    (client, admin)
}

fn proxy_setup_client(env: &Env) -> UpgradeableProxyClient<'_> {
    let contract_id = env.register_contract(None, UpgradeableProxy);
    UpgradeableProxyClient::new(env, &contract_id)
}

#[test]
fn test_proxy_upgrade_success() {
    let env = Env::default();
    let (client, _admin) = proxy_initialized_client(&env);

    let new_wasm_hash = BytesN::from_array(&env, &[1u8; 32]);
    client.upgrade(&new_wasm_hash);
}

#[test]
fn test_proxy_upgrade_rejects_zero_hash() {
    let env = Env::default();
    let (client, _admin) = proxy_initialized_client(&env);

    let zero_hash = BytesN::from_array(&env, &[0u8; 32]);
    let result = client.try_upgrade(&zero_hash);
    assert!(result.is_err());
}

#[test]
#[should_panic]
fn test_proxy_upgrade_rejects_pre_init() {
    let env = Env::default();
    env.mock_all_auths();
    let client = proxy_setup_client(&env);

    let new_wasm_hash = BytesN::from_array(&env, &[1u8; 32]);
    client.upgrade(&new_wasm_hash);
}

#[test]
#[should_panic]
fn test_proxy_verify_integrity_rejects_pre_init() {
    let env = Env::default();
    env.mock_all_auths();
    let client = proxy_setup_client(&env);

    let author = Address::generate(&env);
    let payload_bytes = Bytes::from_array(&env, &[1u8; 32]);
    let payload_hash = BytesN::from_array(&env, &[1u8; 32]);
    let hash = compute_commitment(&[0u8; 32], 0, &payload_hash.to_array());
    let commitment = StateCommitment {
        sequence: 0,
        state_hash: BytesN::from_array(&env, &hash),
        ledger: env.ledger().sequence(),
        author: author.clone(),
    };
    client.verify_integrity(&commitment, &payload_bytes);
}

#[test]
fn test_proxy_upgrade_rejects_non_admin() {
    let env = Env::default();
    let contract_id = env.register_contract(None, UpgradeableProxy);
    let client = UpgradeableProxyClient::new(&env, &contract_id);
    let admin = Address::generate(&env);

    env.as_contract(&contract_id, || {
        env.storage()
            .instance()
            .set(&soroban_sdk::symbol_short!("ADMIN"), &admin);
        let gap: soroban_sdk::Vec<u64> =
            soroban_sdk::Vec::from_array(&env, [0u64; 50]);
        env.storage()
            .instance()
            .set(&soroban_sdk::symbol_short!("GAP"), &gap);
    });

    let new_wasm_hash = BytesN::from_array(&env, &[2u8; 32]);
    let result = client.try_upgrade(&new_wasm_hash);
    assert!(result.is_err());
}
