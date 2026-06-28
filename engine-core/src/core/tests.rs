use super::control_plane::{ControlPlane, ControlPlaneClient};
use crate::audit::compute_commitment;
use crate::types::StateCommitment;
use crate::version::{CONTRACT_VERSION, VERSION_MAJOR, VERSION_MINOR, VERSION_PATCH};
use soroban_sdk::{testutils::Address as _, Address, BytesN, Env, symbol_short};

fn setup_client(env: &Env) -> (Address, ControlPlaneClient<'_>) {
    let contract_id = env.register(ControlPlane, ());
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
    let (_, client) = setup_client(&env);
    let admin = Address::generate(&env);

    client.initialize(&admin);

    let res = client.try_initialize(&admin);
    assert!(res.is_err());

    // Version tracking is initialised
    assert_eq!(client.contract_version(), CONTRACT_VERSION);
    let (maj, min, pat) = client.version();
    assert_eq!((maj, min, pat), (VERSION_MAJOR, VERSION_MINOR, VERSION_PATCH));
}

#[test]
#[should_panic(expected = "ZeroAddress")]
fn test_initialize_zero_address_protection() {
    let env = Env::default();
    let (contract_id, client) = setup_client(&env);
    // Trying to set the contract itself as admin should fail
    let admin = Address::from_string(&"CDLZFC3SYJYDZT7K67VZ75HPJVIEUVNIXF47ZG2FB2RMQQVU2HHGCYSC"
        .parse()
        .unwrap_or(contract_id.clone()));
    // Simpler: use contract_id directly (Soroban allows this, our check catches it)
    let res = client.try_initialize(&contract_id);
    assert!(res.is_err());
}

#[test]
fn test_update_param_success() {
    let env = Env::default();
    let (_, client) = setup_client(&env);
    let admin = Address::generate(&env);

    client.initialize(&admin);

    let param_key = symbol_short!("FEE");
    let param_val = 100;
    let (commitment, payload) = make_commitment(&env, &admin, 1);

    env.mock_all_auths();
    client.update_param(&admin, &param_key, &param_val, &commitment, &payload);

    assert_eq!(client.get_param(&param_key), Some(param_val));
}

#[test]
fn test_param_sanitization_valid_keys() {
    let env = Env::default();
    let (_, client) = setup_client(&env);
    let admin = Address::generate(&env);
    client.initialize(&admin);
    env.mock_all_auths();

    // Test all whitelisted params at boundary values
    let tests = [
        (symbol_short!("FEE"), 0u64),
        (symbol_short!("FEE"), 10_000u64),
        (symbol_short!("THRESH"), 1u64),
        (symbol_short!("THRESH"), 100u64),
        (symbol_short!("TLCK"), 1u64),
        (symbol_short!("TLCK"), 1_000_000u64),
        (symbol_short!("LIMIT"), 0u64),
    ];

    let mut seq = 1u32;
    for (key, val) in tests {
        let (commitment, payload) = make_commitment(&env, &admin, seq);
        client.update_param(&admin, &key, &val, &commitment, &payload);
        assert_eq!(client.get_param(&key), Some(val));
        seq += 1;
    }
}

#[test]
fn test_param_sanitization_rejects_invalid_key() {
    let env = Env::default();
    let (_, client) = setup_client(&env);
    let admin = Address::generate(&env);
    client.initialize(&admin);
    env.mock_all_auths();

    let param_key = symbol_short!("BADKEY");
    let param_val = 100;
    let (commitment, payload) = make_commitment(&env, &admin, 1);

    let res = client.try_update_param(&admin, &param_key, &param_val, &commitment, &payload);
    assert!(res.is_err());
}

#[test]
fn test_param_sanitization_rejects_out_of_bounds() {
    let env = Env::default();
    let (_, client) = setup_client(&env);
    let admin = Address::generate(&env);
    client.initialize(&admin);
    env.mock_all_auths();

    // FEE > 10_000 should fail
    let param_key = symbol_short!("FEE");
    let param_val = 20_000;
    let (commitment, payload) = make_commitment(&env, &admin, 1);

    let res = client.try_update_param(&admin, &param_key, &param_val, &commitment, &payload);
    assert!(res.is_err());
}

#[test]
fn test_admin_2step_transfer() {
    let env = Env::default();
    let (_, client) = setup_client(&env);
    let admin = Address::generate(&env);
    let new_admin = Address::generate(&env);

    client.initialize(&admin);
    env.mock_all_auths();

    // Initiate transfer
    client.transfer_admin(&admin, &new_admin);
    assert_eq!(client.get_pending_admin(), Some(new_admin.clone()));
    assert_eq!(client.get_admin(), Some(admin.clone()));

    // Accept
    client.accept_admin(&new_admin);
    assert_eq!(client.get_admin(), Some(new_admin.clone()));
    assert_eq!(client.get_pending_admin(), None);
}

#[test]
fn test_admin_transfer_zero_address_protection() {
    let env = Env::default();
    let (contract_id, client) = setup_client(&env);
    let admin = Address::generate(&env);

    client.initialize(&admin);
    env.mock_all_auths();

    // Cannot transfer to self
    let res = client.try_transfer_admin(&admin, &admin);
    assert!(res.is_err());

    // Cannot transfer to contract itself
    let res = client.try_transfer_admin(&admin, &contract_id);
    assert!(res.is_err());
}

#[test]
fn test_admin_transfer_cancel() {
    let env = Env::default();
    let (_, client) = setup_client(&env);
    let admin = Address::generate(&env);
    let new_admin = Address::generate(&env);

    client.initialize(&admin);
    env.mock_all_auths();

    client.transfer_admin(&admin, &new_admin);
    assert!(client.get_pending_admin().is_some());

    client.cancel_admin_transfer(&admin);
    assert_eq!(client.get_pending_admin(), None);
    assert_eq!(client.get_admin(), Some(admin));
}

#[test]
fn test_version_endpoints() {
    let env = Env::default();
    let (_, client) = setup_client(&env);
    let admin = Address::generate(&env);

    client.initialize(&admin);

    let (maj, min, pat) = client.version();
    assert_eq!((maj, min, pat), (VERSION_MAJOR, VERSION_MINOR, VERSION_PATCH));

    let vs = client.version_string();
    assert!(vs.len() > 0);

    assert_eq!(client.contract_version(), CONTRACT_VERSION);
    assert_eq!(client.get_stored_version(), Some(CONTRACT_VERSION));
}

#[test]
fn test_pause_blocks_param_update() {
    use crate::circuit_breaker;
    let env = Env::default();
    let contract_id = env.register(ControlPlane, ());
    let client = ControlPlaneClient::new(&env, &contract_id);
    let admin = Address::generate(&env);

    client.initialize(&admin);
    env.mock_all_auths();

    // Trip breaker
    env.as_contract(&contract_id, || {
        circuit_breaker::init(&env, soroban_sdk::vec![&env, admin.clone()]);
        circuit_breaker::trip(&env, &admin);
    });

    assert!(client.is_paused());

    let param_key = symbol_short!("FEE");
    let param_val = 100;
    let (commitment, payload) = make_commitment(&env, &admin, 1);

    let res = client.try_update_param(&admin, &param_key, &param_val, &commitment, &payload);
    assert!(res.is_err(), "param update should fail when paused");
}
