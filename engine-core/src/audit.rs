//! ZK-audit layer — state-commitment validation for the V-Zero Protocol.
//!
//! Each contract call that mutates state must pass through `validate_transition`.
//! Off-chain provers submit `StateCommitment`s; this module verifies ordering
//! and hash integrity before they are persisted.

use soroban_sdk::{contracterror, panic_with_error, symbol_short, Bytes, Env, Symbol};

use crate::types::StateCommitment;

const KEY_SEQ:  Symbol = symbol_short!("SEQ");
const KEY_PREV: Symbol = symbol_short!("PREV_H");

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum AuditError {
    ReplayedSequence  = 1,
    HashMismatch      = 2,
    AuthorUnauthorised = 3,
}

/// Compute the SHA-256 commitment hash over (prev_hash ‖ sequence ‖ payload).
pub fn compute_commitment(env: &Env, prev_hash: &[u8; 32], sequence: u64, payload: &Bytes) -> [u8; 32] {
    let mut data = Bytes::new(env);
    data.append(&Bytes::from_array(env, prev_hash));
    data.append(&Bytes::from_array(env, &sequence.to_be_bytes()));
    data.append(payload);
    env.crypto().sha256(&data).to_array()
}

/// Validate and record a new `StateCommitment`.
///
/// Panics if:
/// - `commitment.sequence` ≤ last recorded sequence (replay guard)
/// - `commitment.state_hash` doesn't match the expected derivation
pub fn validate_transition(env: &Env, commitment: &StateCommitment, payload: &Bytes) {
    let last_seq: u64 = env.storage().instance().get(&KEY_SEQ).unwrap_or(0);
    if commitment.sequence <= last_seq {
        panic_with_error!(env, AuditError::ReplayedSequence);
    }

    let prev_hash: [u8; 32] = env
        .storage()
        .instance()
        .get::<Symbol, [u8; 32]>(&KEY_PREV)
        .unwrap_or([0u8; 32]);

    let expected = compute_commitment(env, &prev_hash, commitment.sequence, payload);
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
}

#[cfg(test)]
mod tests {
    use crate::VeroCore;
    use crate::VeroCoreClient;
    use crate::types::StateCommitment;
    use soroban_sdk::{testutils::Address as _, Address, Bytes, BytesN, Env, vec};
    use super::compute_commitment;

    #[test]
    fn valid_first_commitment() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, VeroCore);
        let client = VeroCoreClient::new(&env, &contract_id);

        let author = Address::generate(&env);
        let signers = vec![&env, Address::generate(&env)];
        client.init(&signers, &1, &vec![&env]);

        let payload = Bytes::from_slice(&env, b"state_payload_v1");
        let hash = compute_commitment(&env, &[0u8; 32], 1, &payload);

        let c = StateCommitment {
            state_hash: BytesN::from_array(&env, &hash),
            sequence:   1,
            ledger:     100,
            author:     author.clone(),
        };
        client.commit(&c, &payload); // must not panic
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #1)")] // ReplayedSequence
    fn replay_is_rejected() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, VeroCore);
        let client = VeroCoreClient::new(&env, &contract_id);

        let author = Address::generate(&env);
        let signers = vec![&env, Address::generate(&env)];
        client.init(&signers, &1, &vec![&env]);

        let payload = Bytes::from_slice(&env, b"payload");
        let hash = compute_commitment(&env, &[0u8; 32], 1, &payload);
        let c = StateCommitment {
            state_hash: BytesN::from_array(&env, &hash),
            sequence:   1,
            ledger:     100,
            author:     author.clone(),
        };
        client.commit(&c, &payload);
        client.commit(&c, &payload); // second call must panic
    }
}
