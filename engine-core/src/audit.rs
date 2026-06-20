//! ZK-audit layer — state-commitment validation for the V-Zero Protocol.
//!
//! Each contract call that mutates state must pass through `validate_transition`.
//! Off-chain provers submit `StateCommitment`s; this module verifies ordering
//! and hash integrity before they are persisted.

use soroban_sdk::{contracterror, panic_with_error, symbol_short, Env, Symbol, BytesN, Map, Val};
use crate::event_utils::publish_event;
use sha2::{Digest, Sha256};

use crate::types::StateCommitment;

const KEY_SEQ:  Symbol = symbol_short!("SEQ");
const KEY_PREV: Symbol = symbol_short!("PREV_H");

#[contracterror]
#[derive(Copy, Clone)]
pub enum AuditError {
    ReplayedSequence  = 1,
    HashMismatch      = 2,
    AuthorUnauthorised = 3,
}

/// Compute the SHA-256 commitment hash over (prev_hash ‖ sequence ‖ payload).
pub fn compute_commitment(prev_hash: &[u8; 32], sequence: u64, payload: &[u8]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(prev_hash);
    h.update(sequence.to_be_bytes());
    h.update(payload);
    h.finalize().into()
}

/// Validate and record a new `StateCommitment`.
///
/// Panics if:
/// - `commitment.sequence` ≤ last recorded sequence (replay guard)
/// - `commitment.state_hash` doesn't match the expected derivation
pub fn validate_transition(env: &Env, commitment: &StateCommitment, payload: &[u8]) {
    let last_seq: u64 = env.storage().instance().get(&KEY_SEQ).unwrap_or(0);
    if commitment.sequence <= last_seq {
        panic_with_error!(env, AuditError::ReplayedSequence);
    }

    let prev_hash: [u8; 32] = env
        .storage()
        .instance()
        .get::<Symbol, [u8; 32]>(&KEY_PREV)
        .unwrap_or([0u8; 32]);

    let expected = compute_commitment(&prev_hash, commitment.sequence, payload);
    let actual: [u8; 32] = commitment.state_hash.to_array();
    if expected != actual {
        panic_with_error!(env, AuditError::HashMismatch);
    }

    env.storage().instance().set(&KEY_SEQ, &commitment.sequence);
    env.storage().instance().set(&KEY_PREV, &actual);

    env.events().publish(
        (symbol_short!("AUDIT"), symbol_short!("commit")),
        (commitment.sequence, commitment.state_hash.clone()),
    );
    // Emit structured Event for audit logs
    let mut payload = Map::new(env);
    payload.set(Symbol::short("seq"), commitment.sequence.into());
    payload.set(Symbol::short("hash"), commitment.state_hash.clone().into());
    publish_event(env, BytesN::from_array(env, &[0u8; 32]), BytesN::from_array(env, &[0u8; 32]), payload);
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{testutils::Address as _, Address, BytesN, Env};

    #[soroban_sdk::contract]
    pub struct TestContract;

    #[soroban_sdk::contractimpl]
    impl TestContract {}

    #[test]
    fn valid_first_commitment() {
        let env = Env::default();
        let payload = b"state_payload_v1";
        let hash = compute_commitment(&[0u8; 32], 1, payload);

        let c = StateCommitment {
            state_hash: BytesN::from_array(&env, &hash),
            sequence:   1,
            ledger:     100,
            author:     Address::generate(&env),
        };
        let contract_id = env.register_contract(None, TestContract);
        env.as_contract(&contract_id, || {
            validate_transition(&env, &c, payload); // must not panic
        });
    }

    #[test]
    #[should_panic]
    fn replay_is_rejected() {
        let env = Env::default();
        let payload = b"payload";
        let hash = compute_commitment(&[0u8; 32], 1, payload);
        let c = StateCommitment {
            state_hash: BytesN::from_array(&env, &hash),
            sequence:   1,
            ledger:     100,
            author:     Address::generate(&env),
        };
        let contract_id = env.register_contract(None, TestContract);
        env.as_contract(&contract_id, || {
            validate_transition(&env, &c, payload);
            validate_transition(&env, &c, payload); // second call must panic
        });
    }
}
