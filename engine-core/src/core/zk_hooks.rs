//! Stable ZK proof attestation hook for the control plane.
//!
//! The audit module validates state-transition commitments. This module anchors
//! proof hashes against those committed state roots, creating a small and stable
//! ABI for off-chain ZK workers and indexers.

use soroban_sdk::{
    contracterror, contracttype, panic_with_error, symbol_short, Address, Bytes, BytesN, Env, Map,
    Symbol,
};

use crate::audit;
use crate::event_struct::{ACT_COMMIT, MOD_AUDIT};
use crate::event_utils::publish_event;

const KEY_PROOF_COUNT: Symbol = symbol_short!("ZK_COUNT");
const MAX_METADATA_ENTRIES: u32 = 16;
const MAX_METADATA_VALUE_BYTES: u32 = 256;

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ZkProofKey {
    Proof(BytesN<32>),
    Attestation(BytesN<32>),
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProofAttestation {
    pub state_root: BytesN<32>,
    pub proof_hash: BytesN<32>,
    pub block_seq: u32,
    pub registered_at_ledger: u32,
    pub metadata: Map<Symbol, Bytes>,
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub enum ZkHookError {
    Unauthorized = 1,
    StateRootMismatch = 2,
    InvalidProofHash = 3,
    MetadataTooLarge = 4,
    NotInitialized = 5,
    ArithmeticOverflow = 6,
}

/// Register or update the proof hash associated with the latest committed state root.
pub fn register_proof(
    env: &Env,
    caller: &Address,
    state_root: BytesN<32>,
    proof_hash: BytesN<32>,
    block_seq: u32,
    metadata: Map<Symbol, Bytes>,
) {
    require_control_plane_admin(env, caller);
    validate_current_state_root(env, &state_root);
    validate_proof_hash(env, &proof_hash);
    validate_metadata(env, &metadata);

    let attestation = ProofAttestation {
        state_root: state_root.clone(),
        proof_hash: proof_hash.clone(),
        block_seq,
        registered_at_ledger: env.ledger().sequence(),
        metadata,
    };

    env.storage()
        .instance()
        .set(&ZkProofKey::Proof(state_root.clone()), &proof_hash);
    env.storage()
        .instance()
        .set(&ZkProofKey::Attestation(state_root.clone()), &attestation);
    increment_proof_count(env);

    publish_event(env, MOD_AUDIT | ACT_COMMIT, block_seq as u64, proof_hash);
}

pub fn get_proof(env: &Env, state_root: BytesN<32>) -> Option<BytesN<32>> {
    env.storage().instance().get(&ZkProofKey::Proof(state_root))
}

pub fn get_attestation(env: &Env, state_root: BytesN<32>) -> Option<ProofAttestation> {
    env.storage()
        .instance()
        .get(&ZkProofKey::Attestation(state_root))
}

pub fn proof_count(env: &Env) -> u64 {
    env.storage().instance().get(&KEY_PROOF_COUNT).unwrap_or(0)
}

fn require_control_plane_admin(env: &Env, caller: &Address) {
    // Keep this in sync with control_plane::KEY_ADMIN without exposing the raw
    // key publicly. Storage is shared because zk_hooks is invoked inside the
    // same contract instance.
    const KEY_ADMIN: Symbol = symbol_short!("ADMIN");

    caller.require_auth();
    let admin: Address = env
        .storage()
        .instance()
        .get(&KEY_ADMIN)
        .unwrap_or_else(|| panic_with_error!(env, ZkHookError::NotInitialized));

    if caller != &admin {
        panic_with_error!(env, ZkHookError::Unauthorized);
    }
}

fn validate_current_state_root(env: &Env, state_root: &BytesN<32>) {
    if audit::get_last_sequence(env) == 0 || audit::get_state_hash(env) != *state_root {
        panic_with_error!(env, ZkHookError::StateRootMismatch);
    }
}

fn validate_proof_hash(env: &Env, proof_hash: &BytesN<32>) {
    if proof_hash.to_array() == [0u8; 32] {
        panic_with_error!(env, ZkHookError::InvalidProofHash);
    }
}

fn validate_metadata(env: &Env, metadata: &Map<Symbol, Bytes>) {
    if metadata.len() > MAX_METADATA_ENTRIES {
        panic_with_error!(env, ZkHookError::MetadataTooLarge);
    }

    for (_, value) in metadata.iter() {
        if value.len() > MAX_METADATA_VALUE_BYTES {
            panic_with_error!(env, ZkHookError::MetadataTooLarge);
        }
    }
}

fn increment_proof_count(env: &Env) {
    let count = proof_count(env);
    let next = count
        .checked_add(1)
        .unwrap_or_else(|| panic_with_error!(env, ZkHookError::ArithmeticOverflow));
    env.storage().instance().set(&KEY_PROOF_COUNT, &next);
}
