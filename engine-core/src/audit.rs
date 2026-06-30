//! ZK-audit layer — state-commitment validation.
//!
//! `state_hash = SHA256(previous_state_hash || sequence || payload)`

use sha2::{Digest, Sha256};
use soroban_sdk::{contracterror, panic_with_error, symbol_short, BytesN, Env, Symbol};

use crate::circuit_breaker::assert_closed;
use crate::event_struct::{ACT_COMMIT, MOD_AUDIT};
use crate::event_utils::publish_event;
use crate::types::StateCommitment;

const KEY_SEQ:  Symbol = symbol_short!("SEQ");
const KEY_PREV: Symbol = symbol_short!("PREV_H");

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub enum AuditError {
    ReplayedSequence   = 1,
    HashMismatch       = 2,
    AuthorUnauthorised = 3,
}

/// Compute `SHA256(prev_hash || sequence || payload)`.
pub fn compute_commitment(prev_hash: &[u8; 32], sequence: u64, payload: &[u8]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(prev_hash);
    h.update(sequence.to_be_bytes());
    h.update(payload);
    h.finalize().into()
}

pub fn get_last_sequence(env: &Env) -> u64 {
    env.storage().instance().get(&KEY_SEQ).unwrap_or(0)
}

pub fn get_previous_hash_raw(env: &Env) -> [u8; 32] {
    env.storage()
        .instance()
        .get::<Symbol, [u8; 32]>(&KEY_PREV)
        .unwrap_or([0u8; 32])
}

pub fn get_state_hash(env: &Env) -> BytesN<32> {
    BytesN::from_array(env, &get_previous_hash_raw(env))
}

pub fn integrity_check(env: &Env, commitment: &StateCommitment, payload: &[u8]) -> bool {
    if commitment.sequence <= get_last_sequence(env) {
        return false;
    }
    let expected = compute_commitment(&get_previous_hash_raw(env), commitment.sequence, payload);
    expected == commitment.state_hash.to_array()
}

/// Inner check-and-persist: no reentrancy guard or circuit-breaker. Use when
/// the caller already holds the guard (e.g. from within a contract method).
pub fn validate_transition_inner(env: &Env, commitment: &StateCommitment, payload: &[u8]) {
    if commitment.sequence <= get_last_sequence(env) {
        panic_with_error!(env, AuditError::ReplayedSequence);
    }
    let expected = compute_commitment(&get_previous_hash_raw(env), commitment.sequence, payload);
    if expected != commitment.state_hash.to_array() {
        panic_with_error!(env, AuditError::HashMismatch);
    }
    env.storage().instance().set(&KEY_SEQ, &commitment.sequence);
    env.storage().instance().set(&KEY_PREV, &commitment.state_hash.to_array());
    publish_event(env, MOD_AUDIT | ACT_COMMIT, commitment.sequence, commitment.state_hash.clone());
}

/// Validate and persist a new `StateCommitment`. Enforces circuit-breaker,
/// replay protection, and hash chaining.
pub fn validate_transition(env: &Env, commitment: &StateCommitment, payload: &[u8]) {
    crate::non_reentrant!(env);
    assert_closed(env);

    if commitment.sequence <= get_last_sequence(env) {
        panic_with_error!(env, AuditError::ReplayedSequence);
    }
    let expected = compute_commitment(&get_previous_hash_raw(env), commitment.sequence, payload);
    if expected != commitment.state_hash.to_array() {
        panic_with_error!(env, AuditError::HashMismatch);
    }

    env.storage().instance().set(&KEY_SEQ, &commitment.sequence);
    env.storage().instance().set(&KEY_PREV, &commitment.state_hash.to_array());

    publish_event(env, MOD_AUDIT | ACT_COMMIT, commitment.sequence, commitment.state_hash.clone());
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{testutils::Address as _, Address, BytesN, Env};

    #[soroban_sdk::contract]
    pub struct TestContract;

    #[soroban_sdk::contractimpl]
    impl TestContract {}

    fn make_commitment(env: &Env, author: Address, sequence: u64, payload: &[u8]) -> StateCommitment {
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
            let c = make_commitment(&env, author, 1, payload);
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
            let c = make_commitment(&env, author, 1, payload);
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
            let mut c = make_commitment(&env, author, 1, payload);
            c.state_hash = BytesN::from_array(&env, &[9u8; 32]);
            validate_transition(&env, &c, payload);
        });
    }
}
