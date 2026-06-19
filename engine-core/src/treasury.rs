//! Treasury state snapshots for audit history.
//!
//! Records treasury state at critical points (deposits, withdrawals, governance actions)
//! to enable audit trails and historical analysis. Snapshots are immutable once recorded.
//!
//! Storage Layout:
//!   SNAP_COUNTER    → Current snapshot ID (incremental)
//!   SNAP:<id>       → TreasurySnapshot indexed by ID
//!   SNAP:LATEST     → Most recent snapshot ID

use soroban_sdk::{contracttype, panic_with_error, symbol_short, Address, Bytes, BytesN, Env, Map, String, Symbol, Vec};
use sha2::{Sha256, Digest};

use crate::types::TreasurySnapshot;

const KEY_SNAP_COUNTER: Symbol = symbol_short!("SNAPC");
const KEY_SNAP_PREFIX:  Symbol = symbol_short!("SNAP");
const KEY_SNAP_LATEST:  Symbol = symbol_short!("SNAPL");

#[contracttype]
#[derive(Copy, Clone)]
pub enum TreasuryError {
    SnapshotNotFound = 1,
    InvalidBalance  = 2,
}

/// Initialize treasury snapshot system. Called once at contract deployment.
pub fn init(env: &Env) {
    env.storage().instance().set(&KEY_SNAP_COUNTER, &0u64);
    env.storage().instance().set(&KEY_SNAP_LATEST, &0u64);
}

/// Record a treasury snapshot. Called after state-changing operations.
///
/// Returns the snapshot ID for reference.
pub fn record_snapshot(
    env: &Env,
    total_balance: i128,
    account_count: u32,
    triggered_by: String,
    context: Map<Symbol, soroban_sdk::Val>,
) -> u64 {
    // Validate balance is non-negative
    if total_balance < 0 {
        panic_with_error!(env, TreasuryError::InvalidBalance);
    }

    // Get next snapshot ID
    let counter: u64 = env.storage().instance().get(&KEY_SNAP_COUNTER).unwrap_or(0);
    let snapshot_id = counter + 1;

    // Build snapshot data for hashing
    let snapshot_data = format!(
        "{:064x}{:08x}{:010x}{}",
        total_balance as u64,  // total_balance (simplified hash)
        account_count,          // account_count
        env.ledger().sequence(), // ledger
        triggered_by            // triggered_by
    );

    // Compute SHA-256 hash of snapshot data
    let mut hasher = Sha256::new();
    hasher.update(snapshot_data.as_bytes());
    let hash_bytes = hasher.finalize();
    let mut hash_array: [u8; 32] = [0; 32];
    hash_array.copy_from_slice(&hash_bytes);
    let state_hash = BytesN::<32>::from_array(env, &hash_array);

    // Create snapshot
    let snapshot = TreasurySnapshot {
        id: snapshot_id,
        total_balance,
        account_count,
        ledger: env.ledger().sequence(),
        timestamp: env.ledger().timestamp().to_string(),
        state_hash,
        triggered_by,
        context,
    };

    // Store snapshot
    let snapshot_key = Symbol::new(env, &format!("SNAP:{}", snapshot_id));
    env.storage().instance().set(&snapshot_key, &snapshot);

    // Update counter and latest snapshot ID
    env.storage().instance().set(&KEY_SNAP_COUNTER, &snapshot_id);
    env.storage().instance().set(&KEY_SNAP_LATEST, &snapshot_id);

    // Emit snapshot recorded event
    env.events().publish(
        (symbol_short!("TRE"), symbol_short!("snapshot")),
        snapshot_id,
    );

    snapshot_id
}

/// Retrieve a snapshot by ID.
pub fn get_snapshot(env: &Env, snapshot_id: u64) -> Option<TreasurySnapshot> {
    let snapshot_key = Symbol::new(env, &format!("SNAP:{}", snapshot_id));
    env.storage().instance().get(&snapshot_key)
}

/// Get the most recent snapshot.
pub fn get_latest_snapshot(env: &Env) -> Option<TreasurySnapshot> {
    let latest_id: u64 = env.storage().instance().get(&KEY_SNAP_LATEST).unwrap_or(0);
    if latest_id == 0 {
        return None;
    }
    get_snapshot(env, latest_id)
}

/// Get snapshot count (total number of snapshots recorded).
pub fn snapshot_count(env: &Env) -> u64 {
    env.storage().instance().get(&KEY_SNAP_COUNTER).unwrap_or(0)
}

/// Get snapshot range (e.g., last N snapshots for audit report).
/// Returns IDs in descending order (newest first).
pub fn get_recent_snapshots(env: &Env, count: u32) -> Vec<u64> {
    let total = snapshot_count(env);
    let mut result = Vec::new(env);

    let start = if total as u32 > count {
        (total as u32) - count + 1
    } else {
        1
    };

    for id in (start as u64..=total).rev() {
        result.push_back(id);
    }

    result
}

/// Verify snapshot integrity (hash verification).
/// Returns true if the snapshot's state_hash matches recomputed hash.
pub fn verify_snapshot(
    env: &Env,
    snapshot: &TreasurySnapshot,
) -> bool {
    // Recompute hash from snapshot fields
    let snapshot_data = format!(
        "{:064x}{:08x}{:010x}{}",
        snapshot.total_balance as u64,
        snapshot.account_count,
        snapshot.ledger,
        &snapshot.triggered_by
    );

    let mut hasher = Sha256::new();
    hasher.update(snapshot_data.as_bytes());
    let hash_bytes = hasher.finalize();
    let mut hash_array: [u8; 32] = [0; 32];
    hash_array.copy_from_slice(&hash_bytes);
    let recomputed = BytesN::<32>::from_array(env, &hash_array);

    snapshot.state_hash == recomputed
}

/// Audit report: retrieve all snapshots since a given ID.
/// Useful for compliance and historical analysis.
pub fn audit_trail(env: &Env, from_id: u64) -> Vec<TreasurySnapshot> {
    let total = snapshot_count(env);
    let mut result = Vec::new(env);

    for id in from_id..=total {
        if let Some(snap) = get_snapshot(env, id) {
            result.push_back(snap);
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_snapshot_creation() {
        // Note: Full integration test requires Soroban test harness
        // This is a placeholder for test structure
    }

    #[test]
    fn test_snapshot_hash_verification() {
        // Verify hash computation is deterministic
    }

    #[test]
    fn test_snapshot_retrieval() {
        // Verify snapshots can be retrieved by ID
    }
}
