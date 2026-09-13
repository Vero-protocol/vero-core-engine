use super::control_plane::{ControlPlane, ControlPlaneClient};
use super::proxy::{UpgradeableProxy, UpgradeableProxyClient};
use crate::audit::compute_commitment;
use crate::core::zk_hooks;
use crate::types::{Proposal, ProposalState, StateCommitment};
use crate::version::{CONTRACT_VERSION, VERSION_MAJOR, VERSION_MINOR, VERSION_PATCH};
use soroban_sdk::{
    symbol_short,
    testutils::{Address as _, Ledger as _},
    vec, Address, Bytes, BytesN, Env, Map, Symbol,
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

#[allow(dead_code)]
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
    assert_eq!(
        (maj, min, pat),
        (VERSION_MAJOR, VERSION_MINOR, VERSION_PATCH)
    );
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
fn test_update_param_rejects_version_keys() {
    let env = Env::default();
    let (client, admin, _) = initialized_client(&env);

    let keys = [
        symbol_short!("VERSION"),
        symbol_short!("VER_INIT"),
        symbol_short!("ZK_COUNT"),
    ];
    let payload = BytesN::from_array(&env, &[4u8; 32]);
    let commitment = commitment(&env, &admin, 1, &payload);

    for param_key in keys {
        let result = client.try_update_param(&admin, &param_key, &999, &commitment, &payload);
        assert!(result.is_err());
    }
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

#[test]
fn test_batch_update_param_rejects_version_keys() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, ControlPlane);
    let client = ControlPlaneClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let author = Address::generate(&env);

    client.initialize(&admin);

    let keys = [
        symbol_short!("VERSION"),
        symbol_short!("VER_INIT"),
        symbol_short!("ZK_COUNT"),
    ];

    for key in keys {
        let mut params = soroban_sdk::Vec::new(&env);
        params.push_back((key, 999));

        let payload = BytesN::from_array(&env, &[2u8; 32]);
        let hash = compute_commitment(&[0u8; 32], 2, &payload.to_array());

        let commitment = StateCommitment {
            sequence: 2,
            state_hash: BytesN::from_array(&env, &hash),
            ledger: 101,
            author: author.clone(),
        };

        let result = client.try_batch_update_param(&admin, &params, &commitment, &payload);
        assert!(result.is_err());
    }
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

    // Empty Wasm is accepted by the test host as a stand-in for a real
    // uploaded contract (soroban-env-host allows a zero-byte upload under
    // `testutils` for exactly this purpose); `update_current_contract_wasm`
    // requires the hash to correspond to a Wasm actually present in storage.
    let new_wasm_hash = env.deployer().upload_contract_wasm(Bytes::new(&env));
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
        let gap: soroban_sdk::Vec<u64> = soroban_sdk::Vec::from_array(&env, [0u64; 50]);
        env.storage()
            .instance()
            .set(&soroban_sdk::symbol_short!("GAP"), &gap);
    });

    let new_wasm_hash = BytesN::from_array(&env, &[2u8; 32]);
    let result = client.try_upgrade(&new_wasm_hash);
    assert!(result.is_err());
}

/// `update_param` must refuse the RBAC storage slots owned by `core::access`.
///
/// `RESERVED_KEYS` previously omitted them, so an admin could write `C_ADMIN`
/// — replacing an `Address` with a `u64`. Every later `require_role` then fails
/// host conversion, and `access::initialize` refuses to re-run because
/// `C_INIT` is still set, leaving no way to restore the role layer.
#[test]
fn test_update_param_rejects_rbac_storage_keys() {
    let env = Env::default();
    let (client, admin, _) = initialized_client(&env);

    for key in [
        symbol_short!("C_ADMIN"),
        symbol_short!("C_INIT"),
        symbol_short!("C_OPERS"),
        symbol_short!("C_AUDIT"),
        symbol_short!("GAP"),
    ] {
        let payload = BytesN::from_array(&env, &[7u8; 32]);
        let commitment = commitment(&env, &admin, 1, &payload);
        let result = client.try_update_param(&admin, &key, &1u64, &commitment, &payload);
        assert!(
            result.is_err(),
            "update_param must reject reserved RBAC key {:?}",
            key
        );
    }
}

// =========================================================================
// Integration tests for issue #178: ControlPlane entrypoints
// =========================================================================

#[test]
fn test_control_plane_emergency_recovery_flow() {
    let env = Env::default();
    let (client, admin, _) = initialized_client(&env);

    let recovery_admin1 = Address::generate(&env);
    let recovery_admin2 = Address::generate(&env);
    let token = Address::generate(&env);
    let dest = Address::generate(&env);

    // 1. Non-admin cannot initialize recovery
    let non_admin = Address::generate(&env);
    let res = client.try_init_recovery(
        &non_admin,
        &vec![&env, recovery_admin1.clone(), recovery_admin2.clone()],
        &2,
    );
    assert!(res.is_err(), "Non-admin must not initialize recovery");

    // 2. Admin initializes recovery
    client.init_recovery(
        &admin,
        &vec![&env, recovery_admin1.clone(), recovery_admin2.clone()],
        &2,
    );

    // 3. Unauthorized caller cannot request recovery
    let res = client.try_request_recovery(&non_admin, &token, &dest, &1_000);
    assert!(res.is_err(), "Unauthorized caller cannot request recovery");

    // 4. Authorized recovery admin requests recovery
    client.request_recovery(&recovery_admin1, &token, &dest, &1_000);

    // 5. Approvals should list recovery_admin1
    let approvals = client.pending_recovery_approvals();
    assert_eq!(approvals.len(), 1);
    assert_eq!(approvals.get(0).unwrap(), recovery_admin1);

    // 6. Non-recovery-admin cannot approve recovery
    let res = client.try_approve_recovery(&non_admin);
    assert!(res.is_err(), "Non-admin cannot approve recovery");
}

#[test]
fn test_control_plane_treasury_outflow_and_snapshot_flow() {
    let env = Env::default();
    let (client, admin, _) = initialized_client(&env);

    let treasury_admin = Address::generate(&env);
    let non_admin = Address::generate(&env);

    // 1. Non-admin cannot init treasury
    let res = client.try_init_treasury(&non_admin, &treasury_admin);
    assert!(res.is_err(), "Non-admin cannot init treasury");

    // 2. Admin inits treasury
    client.init_treasury(&admin, &treasury_admin);

    // 3. Unauthorized caller cannot schedule outflow
    let res = client.try_schedule_outflow(&non_admin, &101, &500);
    assert!(res.is_err(), "Non-treasury admin cannot schedule outflow");

    // 4. Treasury admin schedules outflow
    let unlock_at = client.schedule_outflow(&treasury_admin, &101, &500);
    assert!(unlock_at > env.ledger().timestamp());

    let outflow = client.get_outflow(&101).expect("Outflow must exist");
    assert_eq!(outflow.id, 101);
    assert_eq!(outflow.amount, 500);
    assert!(!outflow.executed);

    // 5. Outflow execution fails before timelock
    let res = client.try_execute_outflow(&treasury_admin, &101);
    assert!(res.is_err(), "Outflow execution must fail before timelock");

    // 6. Record snapshot by treasury admin
    let context = Map::new(&env);
    let snap_id = client.record_snapshot(
        &treasury_admin,
        &10_000_000,
        &150,
        &crate::types::TriggerKind::Manual,
        &context,
    );
    assert_eq!(snap_id, 1);

    let snap = client.get_snapshot(&snap_id).expect("Snapshot must exist");
    assert_eq!(snap.total_balance, 10_000_000);
    assert_eq!(snap.account_count, 150);

    let latest_snap = client
        .get_latest_snapshot()
        .expect("Latest snapshot must exist");
    assert_eq!(latest_snap.id, snap_id);
}

#[test]
fn test_control_plane_burn_flow() {
    let env = Env::default();
    let (client, admin, _) = initialized_client(&env);

    let non_admin = Address::generate(&env);
    let valid_dest = Address::generate(&env);

    // 1. Non-admin cannot burn
    let res = client.try_burn_to(&non_admin, &valid_dest, &1_000);
    assert!(res.is_err(), "Non-admin must not be permitted to burn");

    // 2. Zero address rejection view check
    let zero_addr = Address::from_string(&soroban_sdk::String::from_str(
        &env,
        "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF",
    ));
    let res = client.try_reject_zero_address(&zero_addr);
    assert!(res.is_err(), "Zero address must be rejected");

    let res = client.try_reject_zero_address(&valid_dest);
    assert!(res.is_ok(), "Valid address must pass reject_zero_address");

    // 3. Admin burn with zero address is rejected
    let res = client.try_burn_to(&admin, &zero_addr, &1_000);
    assert!(res.is_err(), "Burn to zero address must be rejected");

    // 4. Admin burn with invalid amount (<= 0) is rejected
    let res = client.try_burn_to(&admin, &valid_dest, &0);
    assert!(res.is_err(), "Burn with 0 amount must be rejected");

    // 5. Admin burn with valid destination and amount succeeds
    client.burn_to(&admin, &valid_dest, &1_000);
}

#[test]
fn test_control_plane_governed_upgrade_flow() {
    let env = Env::default();
    let (client, _admin, contract_id) = initialized_client(&env);

    let alice = Address::generate(&env);
    let bob = Address::generate(&env);
    let wasm_hash = env.deployer().upload_contract_wasm(Bytes::new(&env));

    // 1. Unapproved proposal upgrade fails
    let res = client.try_governed_upgrade(&42, &wasm_hash);
    assert!(res.is_err(), "Upgrade must fail on unapproved proposal");

    // 2. Setup governance multi-sig
    env.as_contract(&contract_id, || {
        crate::governance::init(&env, vec![&env, alice.clone(), bob.clone()], 2);
    });

    let proposal = Proposal {
        id: 42,
        action_hash: wasm_hash.clone(),
        proposer: alice.clone(),
        approved_by: vec![&env],
        state: ProposalState::Pending,
    };
    client.propose(&proposal);

    // 3. Single approval - still not approved (quorum is 2)
    client.approve(&alice, &42);
    let res = client.try_governed_upgrade(&42, &wasm_hash);
    assert!(res.is_err(), "Upgrade must fail before quorum reached");

    // 4. Second approval reaches quorum -> moves to Approved
    client.approve(&bob, &42);

    // 5. Wrong wasm hash rejected even if proposal approved
    let wrong_hash = BytesN::from_array(&env, &[8u8; 32]);
    let res = client.try_governed_upgrade(&42, &wrong_hash);
    assert!(
        res.is_err(),
        "Upgrade with mismatched hash must be rejected"
    );

    // 6. Timelock must pass before execute in upgrade
    env.ledger()
        .set_sequence_number(crate::governance::TIMELOCK_LEDGERS + 10);
    client.governed_upgrade(&42, &wasm_hash);
}
