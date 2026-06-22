//! Treasury state snapshots for audit history.
//!
//! ## Storage layout (optimised)
//!
//! | Key       | Storage     | Type             | Notes                          |
//! |-----------|-------------|------------------|--------------------------------|
//! | `SNAPC`   | instance    | `u64`            | monotonic snapshot counter     |
//! | `SNAPL`   | instance    | `u64`            | id of most recent snapshot     |
//! | `S{id}`   | temporary   | `TreasurySnapshot` | individual snapshot record   |
//!
//! Individual snapshot records are stored in **temporary** storage so they accrue
//! no ledger-entry rent.  The counter and latest-id remain in instance storage
//! because they are accessed on every write path.

use soroban_sdk::{contracterror, panic_with_error, BytesN, Bytes, Env, Map, Symbol, Val, Vec};
use crate::event_struct::{MOD_TREASURY, ACT_SNAPSHOT};
use crate::event_utils::publish_event;

use crate::types::{TreasurySnapshot, TriggerKind, TreasuryError};

const KEY_SNAP_COUNTER: Symbol = soroban_sdk::symbol_short!("SNAPC");
const KEY_SNAP_LATEST:  Symbol = soroban_sdk::symbol_short!("SNAPL");

/// Temporary storage TTL constants (ledgers).
/// ~7 days at 5-second ledger time — sufficient for off-chain indexer pickup.
const SNAP_TTL_THRESHOLD: u32 = 17_280;
const SNAP_TTL_EXTEND_TO: u32 = 17_280 * 7;

/// Initialize treasury snapshot system. Called once at contract deployment.
pub fn init(env: &Env) {
    env.storage().instance().set(&KEY_SNAP_COUNTER, &0u64);
    env.storage().instance().set(&KEY_SNAP_LATEST,  &0u64);
}

/// Record a treasury snapshot. Called after state-changing operations.
///
/// Returns the snapshot ID for reference.
///
/// `trigger` replaces the previous freeform `triggered_by: String` to eliminate
/// heap-allocated Soroban Strings.  Pass an empty `Map::new(env)` for `context`
/// when no extra metadata is needed.
pub fn record_snapshot(
    env: &Env,
    total_balance: i128,
    account_count: u32,
    trigger: TriggerKind,
    context: Map<Symbol, Val>,
) -> u64 {
    if total_balance < 0 {
        panic_with_error!(env, TreasuryError::InvalidBalance);
    }

    let counter: u64 = env.storage().instance().get(&KEY_SNAP_COUNTER).unwrap_or(0);
    let snapshot_id = counter + 1;

    let state_hash = compute_hash(env, total_balance, account_count, env.ledger().sequence());

    let snapshot = TreasurySnapshot {
        id:             snapshot_id,
        total_balance,
        account_count,
        ledger:         env.ledger().sequence(),
        timestamp_unix: env.ledger().timestamp(),
        state_hash:     state_hash.clone(),
        trigger,
        context,
    };

    let snapshot_key = make_snap_key(env, snapshot_id);

    // Store in temporary storage — no rent accrual.
    env.storage().temporary().set(&snapshot_key, &snapshot);
    env.storage()
        .temporary()
        .extend_ttl(&snapshot_key, SNAP_TTL_THRESHOLD, SNAP_TTL_EXTEND_TO);

    env.storage().instance().set(&KEY_SNAP_COUNTER, &snapshot_id);
    env.storage().instance().set(&KEY_SNAP_LATEST,  &snapshot_id);

    // Single compact event — snapshot id in value, state_hash in hash field.
    publish_event(env, MOD_TREASURY | ACT_SNAPSHOT, snapshot_id, state_hash);

    snapshot_id
}

/// Retrieve a snapshot by ID.
pub fn get_snapshot(env: &Env, snapshot_id: u64) -> Option<TreasurySnapshot> {
    let key = make_snap_key(env, snapshot_id);
    env.storage().temporary().get(&key)
}

/// Get the most recent snapshot.
pub fn get_latest_snapshot(env: &Env) -> Option<TreasurySnapshot> {
    let latest_id: u64 = env.storage().instance().get(&KEY_SNAP_LATEST).unwrap_or(0);
    if latest_id == 0 { return None; }
    get_snapshot(env, latest_id)
}

/// Get snapshot count.
pub fn snapshot_count(env: &Env) -> u64 {
    env.storage().instance().get(&KEY_SNAP_COUNTER).unwrap_or(0)
}

/// Get IDs of the most recent `count` snapshots (newest first).
pub fn get_recent_snapshots(env: &Env, count: u32) -> Vec<u64> {
    let total = snapshot_count(env);
    let mut result = Vec::new(env);
    let start = if total as u32 > count { (total as u32) - count + 1 } else { 1 };
    for id in (start as u64..=total).rev() {
        result.push_back(id);
    }
    result
}

/// Verify snapshot integrity by recomputing the hash.
pub fn verify_snapshot(env: &Env, snapshot: &TreasurySnapshot) -> bool {
    let recomputed = compute_hash(env, snapshot.total_balance, snapshot.account_count, snapshot.ledger);
    snapshot.state_hash == recomputed
}

/// Retrieve all snapshots from `from_id` onward (audit trail).
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

// ── internal ──────────────────────────────────────────────────────────────────

fn compute_hash(env: &Env, balance: i128, account_count: u32, ledger: u32) -> BytesN<32> {
    // Pack fields into a fixed-size byte buffer for deterministic hashing.
    // Layout: balance(16) | account_count(4) | ledger(4) = 24 bytes
    let mut raw = [0u8; 24];
    raw[..16].copy_from_slice(&balance.to_be_bytes());
    raw[16..20].copy_from_slice(&account_count.to_be_bytes());
    raw[20..24].copy_from_slice(&ledger.to_be_bytes());
    env.crypto().sha256(&Bytes::from_slice(env, &raw)).into()
}

fn make_snap_key(env: &Env, id: u64) -> Symbol {
    Symbol::new(env, &format!("S{}", id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{Env, Map, Symbol};

    #[soroban_sdk::contract]
    pub struct TestContract;

    #[soroban_sdk::contractimpl]
    impl TestContract {}

    #[test]
    fn snapshot_creation_and_retrieval() {
        let env = Env::default();
        let contract_id = env.register_contract(None, TestContract);
        env.as_contract(&contract_id, || {
            init(&env);
            let ctx: Map<Symbol, Val> = Map::new(&env);
            let id = record_snapshot(&env, 1000, 5, TriggerKind::Deposit, ctx);
            assert_eq!(id, 1);
            let snap = get_snapshot(&env, 1).unwrap();
            assert_eq!(snap.total_balance, 1000);
            assert_eq!(snap.account_count, 5);
            assert_eq!(snapshot_count(&env), 1);
        });
    }

    #[test]
    fn snapshot_hash_verification() {
        let env = Env::default();
        let contract_id = env.register_contract(None, TestContract);
        env.as_contract(&contract_id, || {
            init(&env);
            let ctx: Map<Symbol, Val> = Map::new(&env);
            record_snapshot(&env, 500, 2, TriggerKind::Withdrawal, ctx);
            let snap = get_snapshot(&env, 1).unwrap();
            assert!(verify_snapshot(&env, &snap));
        });
    }

    #[test]
    #[should_panic]
    fn negative_balance_rejected() {
        let env = Env::default();
        let contract_id = env.register_contract(None, TestContract);
        env.as_contract(&contract_id, || {
            init(&env);
            let ctx: Map<Symbol, Val> = Map::new(&env);
            record_snapshot(&env, -1, 0, TriggerKind::Other, ctx);
        });
    }
}
