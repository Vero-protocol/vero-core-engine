//! Regression tests for policy-bundle signature verification (VAG-003).
//!
//! Covers the happy path plus non-happy-path and adversarial-input scenarios:
//! uninitialized use, empty bundles, unauthorized signers, replay, registry
//! management edge cases, and a forged signature from an authorized key.

use super::*;
use ed25519_dalek::{Signer, SigningKey};
use soroban_sdk::{
    testutils::{Address as _, Events},
    Address, Bytes, BytesN, Env,
};

const POLICY: &[u8] = b"vero-policy-bundle:v1:max_fee_bps=50;guardians=3";
const OTHER_POLICY: &[u8] = b"vero-policy-bundle:v1:max_fee_bps=9999;guardians=1";

/// Deterministic ed25519 keypair for tests. Returns (32-byte pubkey, signer).
fn keypair(seed: u8) -> ([u8; 32], SigningKey) {
    let sk = SigningKey::from_bytes(&[seed; 32]);
    (sk.verifying_key().to_bytes(), sk)
}

fn sign(sk: &SigningKey, msg: &[u8]) -> [u8; 64] {
    sk.sign(msg).to_bytes()
}

struct Harness {
    env: Env,
    client: AuditGuardContractClient<'static>,
    admin: Address,
}

fn setup() -> Harness {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, AuditGuardContract);
    let client = AuditGuardContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin);
    Harness { env, client, admin }
}

// --- initialization --------------------------------------------------------

#[test]
fn initialize_then_double_init_rejected() {
    let h = setup();
    // Second initialize must return AlreadyInitialized, not panic.
    assert_eq!(
        h.client.try_initialize(&h.admin),
        Err(Ok(AuditGuardError::AlreadyInitialized))
    );
}

#[test]
fn attest_before_init_is_rejected() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, AuditGuardContract);
    let client = AuditGuardContractClient::new(&env, &contract_id);

    let (pk, sk) = keypair(1);
    let signer = BytesN::from_array(&env, &pk);
    let signature = BytesN::from_array(&env, &sign(&sk, POLICY));
    let bundle = Bytes::from_slice(&env, POLICY);

    assert_eq!(
        client.try_attest_policy_bundle(&bundle, &signer, &signature),
        Err(Ok(AuditGuardError::NotInitialized))
    );
}

// --- signer registry -------------------------------------------------------

#[test]
fn register_and_revoke_signer() {
    let h = setup();
    let (pk, _) = keypair(2);
    let signer = BytesN::from_array(&h.env, &pk);

    assert!(!h.client.is_authorized_signer(&signer));
    h.client.register_signer(&signer);
    assert!(h.client.is_authorized_signer(&signer));

    h.client.revoke_signer(&signer);
    assert!(!h.client.is_authorized_signer(&signer));
}

#[test]
fn register_duplicate_signer_rejected() {
    let h = setup();
    let (pk, _) = keypair(2);
    let signer = BytesN::from_array(&h.env, &pk);

    h.client.register_signer(&signer);
    assert_eq!(
        h.client.try_register_signer(&signer),
        Err(Ok(AuditGuardError::SignerAlreadyRegistered))
    );
}

#[test]
fn revoke_unknown_signer_rejected() {
    let h = setup();
    let (pk, _) = keypair(3);
    let signer = BytesN::from_array(&h.env, &pk);

    assert_eq!(
        h.client.try_revoke_signer(&signer),
        Err(Ok(AuditGuardError::SignerNotFound))
    );
}

// --- attestation: happy path ----------------------------------------------

#[test]
fn attest_valid_bundle_succeeds_and_emits_telemetry() {
    let h = setup();
    let (pk, sk) = keypair(4);
    let signer = BytesN::from_array(&h.env, &pk);
    h.client.register_signer(&signer);

    let bundle = Bytes::from_slice(&h.env, POLICY);
    let signature = BytesN::from_array(&h.env, &sign(&sk, POLICY));

    let hash = h.client.attest_policy_bundle(&bundle, &signer, &signature);

    // Returned hash matches sha256(bundle) and the bundle is now attested.
    let expected: BytesN<32> = h.env.crypto().sha256(&bundle).into();
    assert_eq!(hash, expected);
    assert!(h.client.is_attested(&hash));
    // A telemetry event was surfaced (not a silent success).
    assert!(!h.env.events().all().is_empty());
}

// --- attestation: adversarial / non-happy paths ---------------------------

#[test]
fn attest_empty_bundle_rejected_with_alert() {
    let h = setup();
    let (pk, sk) = keypair(5);
    let signer = BytesN::from_array(&h.env, &pk);
    h.client.register_signer(&signer);

    let empty = Bytes::new(&h.env);
    let signature = BytesN::from_array(&h.env, &sign(&sk, b""));

    assert_eq!(
        h.client.try_attest_policy_bundle(&empty, &signer, &signature),
        Err(Ok(AuditGuardError::EmptyBundle))
    );
    // Failure surfaced as an alert event, not a silent drop.
    assert!(!h.env.events().all().is_empty());
}

#[test]
fn attest_unauthorized_signer_rejected_with_alert() {
    let h = setup();
    // Signer produces a cryptographically valid signature but is NOT
    // registered — the compromised-CI-runner threat model.
    let (pk, sk) = keypair(6);
    let signer = BytesN::from_array(&h.env, &pk);
    let bundle = Bytes::from_slice(&h.env, POLICY);
    let signature = BytesN::from_array(&h.env, &sign(&sk, POLICY));

    assert_eq!(
        h.client.try_attest_policy_bundle(&bundle, &signer, &signature),
        Err(Ok(AuditGuardError::UnauthorizedSigner))
    );
    assert!(!h.env.events().all().is_empty());
    // Nothing was recorded — the bundle is not attested.
    let hash: BytesN<32> = h.env.crypto().sha256(&bundle).into();
    assert!(!h.client.is_attested(&hash));
}

#[test]
fn attest_replayed_bundle_rejected() {
    let h = setup();
    let (pk, sk) = keypair(7);
    let signer = BytesN::from_array(&h.env, &pk);
    h.client.register_signer(&signer);

    let bundle = Bytes::from_slice(&h.env, POLICY);
    let signature = BytesN::from_array(&h.env, &sign(&sk, POLICY));

    // First attestation succeeds.
    h.client.attest_policy_bundle(&bundle, &signer, &signature);
    // Replaying the identical bundle is rejected.
    assert_eq!(
        h.client.try_attest_policy_bundle(&bundle, &signer, &signature),
        Err(Ok(AuditGuardError::ReplayedBundle))
    );
}

#[test]
fn distinct_bundles_from_same_signer_both_attest() {
    let h = setup();
    let (pk, sk) = keypair(8);
    let signer = BytesN::from_array(&h.env, &pk);
    h.client.register_signer(&signer);

    let b1 = Bytes::from_slice(&h.env, POLICY);
    let s1 = BytesN::from_array(&h.env, &sign(&sk, POLICY));
    let b2 = Bytes::from_slice(&h.env, OTHER_POLICY);
    let s2 = BytesN::from_array(&h.env, &sign(&sk, OTHER_POLICY));

    let h1 = h.client.attest_policy_bundle(&b1, &signer, &s1);
    let h2 = h.client.attest_policy_bundle(&b2, &signer, &s2);
    assert_ne!(h1, h2);
    assert!(h.client.is_attested(&h1));
    assert!(h.client.is_attested(&h2));
}

#[test]
fn revoked_signer_can_no_longer_attest() {
    let h = setup();
    let (pk, sk) = keypair(9);
    let signer = BytesN::from_array(&h.env, &pk);
    h.client.register_signer(&signer);
    h.client.revoke_signer(&signer);

    let bundle = Bytes::from_slice(&h.env, POLICY);
    let signature = BytesN::from_array(&h.env, &sign(&sk, POLICY));

    assert_eq!(
        h.client.try_attest_policy_bundle(&bundle, &signer, &signature),
        Err(Ok(AuditGuardError::UnauthorizedSigner))
    );
}

#[test]
#[should_panic]
fn attest_forged_signature_from_authorized_key_traps() {
    let h = setup();
    // Register a key, then present a signature that does not match the bundle.
    let (pk, _sk) = keypair(10);
    let signer = BytesN::from_array(&h.env, &pk);
    h.client.register_signer(&signer);

    let bundle = Bytes::from_slice(&h.env, POLICY);
    // Signature is all-zero bytes — a forgery. The host ed25519_verify traps.
    let forged = BytesN::from_array(&h.env, &[0u8; 64]);
    h.client.attest_policy_bundle(&bundle, &signer, &forged);
}
