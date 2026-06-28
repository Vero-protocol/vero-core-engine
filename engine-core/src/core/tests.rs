use super::control_plane::{ControlPlane, ControlPlaneClient};
use crate::audit::compute_commitment;
use crate::circuit_breaker;
use crate::types::StateCommitment;
use soroban_sdk::{
    testutils::{Address as _, Events},
    symbol_short, vec, Address, BytesN, Env,
};

fn setup() -> (Env, Address, ControlPlaneClient<'static>, Address) {
    let env = Env::default();
    let contract_id = env.register_contract(None, ControlPlane);
    let client = ControlPlaneClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin);
    (env, contract_id, client, admin)
}

fn make_commitment(
    env: &Env,
    admin: &Address,
    sequence: u64,
    payload: &BytesN<32>,
) -> StateCommitment {
    let hash = compute_commitment(&[0u8; 32], sequence, &payload.to_array());
    StateCommitment {
        sequence,
        state_hash: BytesN::from_array(env, &hash),
        ledger: 100,
        author: admin.clone(),
    }
}

#[test]
fn test_initialize() {
    let env = Env::default();
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

    client.get_admin();
}

#[test]
fn test_update_param_success() {
    let (env, _contract_id, client, admin) = setup();

    let param_key = symbol_short!("FEE");
    let param_val = 100u64;
    let payload = BytesN::from_array(&env, &[1u8; 32]);
    let commitment = make_commitment(&env, &admin, 1, &payload);

    env.mock_all_auths();
    client.update_param(&admin, &param_key, &param_val, &commitment, &payload);

    assert_eq!(client.get_param(&param_key), Some(param_val));
}

#[test]
#[should_panic]
fn test_update_param_rejects_non_admin() {
    let (env, _contract_id, client, _admin) = setup();

    let rogue = Address::generate(&env);
    let param_key = symbol_short!("FEE");
    let param_val = 100u64;
    let payload = BytesN::from_array(&env, &[1u8; 32]);
    let commitment = make_commitment(&env, &rogue, 1, &payload);

    env.mock_all_auths();
    client.update_param(&rogue, &param_key, &param_val, &commitment, &payload);
}

#[test]
#[should_panic]
fn test_update_param_rejects_author_mismatch() {
    let (env, _contract_id, client, admin) = setup();

    let other = Address::generate(&env);
    let param_key = symbol_short!("FEE");
    let param_val = 100u64;
    let payload = BytesN::from_array(&env, &[1u8; 32]);
    let commitment = make_commitment(&env, &other, 1, &payload);

    env.mock_all_auths();
    client.update_param(&admin, &param_key, &param_val, &commitment, &payload);
}

#[test]
#[should_panic]
fn test_update_param_rejects_reserved_key() {
    let (env, _contract_id, client, admin) = setup();

    let param_key = symbol_short!("ADMIN");
    let param_val = 100u64;
    let payload = BytesN::from_array(&env, &[1u8; 32]);
    let commitment = make_commitment(&env, &admin, 1, &payload);

    env.mock_all_auths();
    client.update_param(&admin, &param_key, &param_val, &commitment, &payload);
}

#[test]
#[should_panic]
fn test_update_param_rejects_when_circuit_open() {
    let (env, contract_id, client, admin) = setup();

    let guardian = Address::generate(&env);
    env.as_contract(&contract_id, || {
        circuit_breaker::init(&env, vec![&env, guardian.clone()]);
        circuit_breaker::trip(&env, &guardian);
    });

    let param_key = symbol_short!("FEE");
    let param_val = 100u64;
    let payload = BytesN::from_array(&env, &[1u8; 32]);
    let commitment = make_commitment(&env, &admin, 1, &payload);

    env.mock_all_auths();
    client.update_param(&admin, &param_key, &param_val, &commitment, &payload);
}

#[test]
#[should_panic]
fn test_update_param_rejects_replayed_commitment() {
    let (env, _contract_id, client, admin) = setup();

    let param_key = symbol_short!("FEE");
    let param_val = 100u64;
    let payload = BytesN::from_array(&env, &[1u8; 32]);
    let commitment = make_commitment(&env, &admin, 1, &payload);

    env.mock_all_auths();
    client.update_param(&admin, &param_key, &param_val, &commitment, &payload);
    client.update_param(&admin, &param_key, &param_val, &commitment, &payload);
}

#[test]
fn test_update_param_emits_event() {
    let (env, _contract_id, client, admin) = setup();

    let param_key = symbol_short!("FEE");
    let param_val = 100u64;
    let payload = BytesN::from_array(&env, &[1u8; 32]);
    let commitment = make_commitment(&env, &admin, 1, &payload);

    env.mock_all_auths();
    client.update_param(&admin, &param_key, &param_val, &commitment, &payload);

    let events = env.events().all();
    assert!(
        !events.is_empty(),
        "expected at least one structured event to be emitted"
    );
}

#[test]
fn test_update_param_records_auth_for_admin() {
    let (env, _contract_id, client, admin) = setup();

    let param_key = symbol_short!("FEE");
    let param_val = 100u64;
    let payload = BytesN::from_array(&env, &[1u8; 32]);
    let commitment = make_commitment(&env, &admin, 1, &payload);

    env.mock_all_auths();
    client.update_param(&admin, &param_key, &param_val, &commitment, &payload);

    let auths = env.auths();
    assert!(
        auths.iter().any(|(id, _)| *id == admin),
        "admin must be recorded as authorizer"
    );
}
