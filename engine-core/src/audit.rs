//! ZK-audit layer: ordered state-commitment validation.
//!
//! State-changing code can call `validate_transition` with an off-chain produced
//! commitment. The module enforces circuit-breaker status, author authentication,
//! monotonic sequencing, and deterministic hash chaining:
//!
//! `state_hash = sha256(previous_state_hash || sequence || payload)`.

use sha2::{Digest, Sha256};
use soroban_sdk::{contracterror, panic_with_error, symbol_short, BytesN, Env, Symbol};

use crate::circuit_breaker::assert_closed;
use crate::event_struct::{ACT_COMMIT, MOD_AUDIT};
use crate::event_utils::publish_event;
use crate::types::StateCommitment;

const KEY_SEQ: Symbol = symbol_short!("SEQ");
const KEY_PREV: Symbol = symbol_short!("PREV_H");

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub enum AuditError {
    ReplayedSequence = 1,
    HashMismatch = 2,
}

/// Compute the SHA-256 commitment hash over `(prev_hash || sequence || payload)`.
pub fn compute_commitment(prev_hash: &[u8; 32], sequence: u64, payload: &[u8]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(prev_hash);
    h.update(sequence.to_be_bytes());
    h.update(payload);
    h.finalize().into()
}

/// Return the last accepted sequence number.
pub fn last_sequence(env: &Env) -> u64 {
    env.storage().instance().get(&KEY_SEQ).unwrap_or(0)
}

/// Return the last accepted state hash.
pub fn previous_hash(env: &Env) -> BytesN<32> {
    env.storage()
        .instance()
        .get(&KEY_PREV)
        .unwrap_or_else(|| BytesN::from_array(env, &[0u8; 32]))
}

/// Validate and persist a new state commitment.
pub fn validate_transition(env: &Env, commitment: &StateCommitment, payload: &[u8]) {
    crate::non_reentrant!(env);
    assert_closed(env);
    commitment.author.require_auth();

    let last_seq = last_sequence(env);
    if commitment.sequence <= last_seq {
        panic_with_error!(env, AuditError::ReplayedSequence);
    }

    let prev_hash = previous_hash(env).to_array();
    let expected = compute_commitment(&prev_hash, commitment.sequence, payload);
    let actual = commitment.state_hash.to_array();
    if expected != actual {
        panic_with_error!(env, AuditError::HashMismatch);
    }

    env.storage().instance().set(&KEY_SEQ, &commitment.sequence);
    env.storage().instance().set(&KEY_PREV, &commitment.state_hash);

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
    use soroban_sdk::{contract, contractimpl, testutils::Address as _, Address, BytesN, Env};

    #[contract]
    pub struct TestContract;

    #[contractimpl]
    impl TestContract {}

    #[test]
    fn valid_first_commitment() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, TestContract);
        env.as_contract(&contract_id, || {
            let payload = b"state_payload_v1";
            let hash = compute_commitment(&[0u8; 32], 1, payload);
            let c = StateCommitment {
                state_hash: BytesN::from_array(&env, &hash),
                sequence: 1,
                ledger: 100,
                author: Address::generate(&env),
            };
            validate_transition(&env, &c, payload);
            assert_eq!(last_sequence(&env), 1);
        });
    }

    #[test]
    #[should_panic]
    fn replay_is_rejected() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, TestContract);
        env.as_contract(&contract_id, || {
            let payload = b"payload";
            let hash = compute_commitment(&[0u8; 32], 1, payload);
            let c = StateCommitment {
                state_hash: BytesN::from_array(&env, &hash),
                sequence: 1,
                ledger: 100,
                author: Address::generate(&env),
            };
            validate_transition(&env, &c, payload);
            validate_transition(&env, &c, payload);
        });
    }
}
