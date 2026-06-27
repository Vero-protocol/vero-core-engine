//! ZK-audit layer — state-commitment validation for the Vero Protocol.
//!
//! Every state-changing control-plane path can anchor its transition here. The
//! commitment chain is deliberately simple and audit-friendly:
//!
//! `state_hash = SHA256(previous_state_hash || sequence || payload)`
//!
//! The module enforces signer authentication, replay protection, circuit-breaker
//! safety and deterministic event emission.

use crate::circuit_breaker::assert_closed;
use crate::event_struct::{ACT_COMMIT, MOD_AUDIT};
use crate::event_utils::publish_event;
use crate::types::StateCommitment;
use sha2::{Digest, Sha256};
use soroban_sdk::{contracterror, panic_with_error, symbol_short, BytesN, Env, Symbol};

const KEY_SEQ: Symbol = symbol_short!("SEQ");
const KEY_PREV: Symbol = symbol_short!("PREV_H");

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub enum AuditError {
    ReplayedSequence = 1,
    HashMismatch = 2,
    AuthorUnauthorised = 3,
}

/// Compute the SHA-256 commitment hash over `(prev_hash || sequence || payload)`.
pub fn compute_commitment(prev_hash: &[u8; 32], sequence: u64, payload: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(prev_hash);
    hasher.update(sequence.to_be_bytes());
    hasher.update(payload);
    hasher.finalize().into()
}

/// Return the latest accepted commitment sequence.
pub fn get_last_sequence(env: &Env) -> u64 {
    env.storage().instance().get(&KEY_SEQ).unwrap_or(0)
}

/// Return the latest accepted state hash as raw bytes.
pub fn get_previous_hash_raw(env: &Env) -> [u8; 32] {
    env.storage()
        .instance()
        .get::<Symbol, [u8; 32]>(&KEY_PREV)
        .unwrap_or([0u8; 32])
}

/// Return the latest accepted state hash as `BytesN<32>`.
pub fn get_state_hash(env: &Env) -> BytesN<32> {
    BytesN::from_array(env, &get_previous_hash_raw(env))
}

/// Pure integrity check used by tests/off-chain simulation before committing.
pub fn integrity_check(env: &Env, commitment: &StateCommitment, payload: &[u8]) -> bool {
    if commitment.sequence <= get_last_sequence(env) {
        return false;
    }
    let expected = compute_commitment(&get_previous_hash_raw(env), commitment.sequence, payload);
    expected == commitment.state_hash.to_array()
}

/// Validate and record a new `StateCommitment`.
///
/// Panics if:
/// - the circuit breaker is open,
/// - the author did not sign the invocation,
/// - `commitment.sequence` is replayed or stale,
/// - `commitment.state_hash` does not match the expected chain derivation.
pub fn validate_transition(env: &Env, commitment: &StateCommitment, payload: &[u8]) {
    crate::non_reentrant!(env);
    assert_closed(env);

    // The author field must be authenticated; otherwise the commitment would be
    // an unauthenticated hint rather than an auditable proof anchor.
    commitment.author.require_auth();

    if !integrity_check(env, commitment, payload) {
        if commitment.sequence <= get_last_sequence(env) {
            panic_with_error!(env, AuditError::ReplayedSequence);
        }
        panic_with_error!(env, AuditError::HashMismatch);
    }

    let actual = commitment.state_hash.to_array();
    env.storage().instance().set(&KEY_SEQ, &commitment.sequence);
    env.storage().instance().set(&KEY_PREV, &actual);

    publish_event(
        env,
        MOD_AUDIT | ACT_COMMIT,
        commitment.sequence,
        commitment.state_hash.clone(),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{testutils::Address as _, Address, BytesN, Env};

    #[soroban_sdk::contract]
    pub struct TestContract;

    #[soroban_sdk::contractimpl]
    impl TestContract {}

    fn commitment(env: &Env, author: Address, sequence: u64, payload: &[u8]) -> StateCommitment {
        let prev = get_previous_hash_raw(env);
        let hash = compute_commitment(&prev, sequence, payload);
        StateCommitment {
            state_hash: BytesN::from_array(env, &hash),
            sequence,
            ledger: env.ledger().sequence(),
            author,
        }
    }

    #[test]
    fn valid_first_commitment() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, TestContract);
        let author = Address::generate(&env);
        env.as_contract(&contract_id, || {
            let payload = b"state_payload_v1";
            let c = commitment(&env, author, 1, payload);
            assert!(integrity_check(&env, &c, payload));
            validate_transition(&env, &c, payload);
            assert_eq!(get_last_sequence(&env), 1);
            assert_eq!(get_state_hash(&env), c.state_hash);
        });
    }

    #[test]
    #[should_panic]
    fn replay_is_rejected() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, TestContract);
        let author = Address::generate(&env);
        env.as_contract(&contract_id, || {
            let payload = b"payload";
            let c = commitment(&env, author, 1, payload);
            validate_transition(&env, &c, payload);
            validate_transition(&env, &c, payload);
        });
    }

    #[test]
    #[should_panic]
    fn hash_mismatch_is_rejected() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, TestContract);
        let author = Address::generate(&env);
        env.as_contract(&contract_id, || {
            let payload = b"payload";
            let mut c = commitment(&env, author, 1, payload);
            c.state_hash = BytesN::from_array(&env, &[9u8; 32]);
            validate_transition(&env, &c, payload);
        });
    }
}
