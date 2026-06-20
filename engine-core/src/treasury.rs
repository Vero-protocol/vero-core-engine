//! Treasury state snapshots for audit history.
//!
//! Records treasury state at critical points (deposits, withdrawals, governance actions)
//! to enable audit trails and historical analysis. Snapshots are immutable once recorded.
//!
//! Storage Layout:
//!   SNAP_COUNTER    → Current snapshot ID (incremental)
//!   SNAP:<id>       → TreasurySnapshot indexed by ID
//!   SNAP:LATEST     → Most recent snapshot ID

use soroban_sdk::{contracterror, panic_with_error, symbol_short, BytesN, Bytes, Env, Map, String, Symbol, Vec, Val};
use crate::event_utils::publish_event;

use crate::types::TreasurySnapshot;

const KEY_SNAP_COUNTER: Symbol = symbol_short!("SNAPC");
const KEY_SNAP_LATEST:  Symbol = symbol_short!("SNAPL");

#[contracterror]
#[derive(Copy, Clone)]
pub enum TreasuryError {
    SnapshotNotFound = 1,
    InvalidBalance   = 2,
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
    if total_balance < 0 {
        panic_with_error!(env, TreasuryError::InvalidBalance);
    }

    let counter: u64 = env.storage().instance().get(&KEY_SNAP_COUNTER).unwrap_or(0);
    let snapshot_id = counter + 1;

    let state_hash = compute_hash(env, total_balance, account_count, env.ledger().sequence());

    // Store ledger timestamp as u64; soroban_sdk::String is used for triggered_by label.
    let ts_str = String::from_str(env, &format!("{}", env.ledger().timestamp()));

    let snapshot = TreasurySnapshot {
        id: snapshot_id,
        total_balance,
        account_count,
        ledger: env.ledger().sequence(),
        timestamp: ts_str,
        state_hash,
        triggered_by,
        context,
    };

    let snapshot_key = make_snap_key(env, snapshot_id);
    env.storage().instance().set(&snapshot_key, &snapshot);
    env.storage().instance().set(&KEY_SNAP_COUNTER, &snapshot_id);
    env.storage().instance().set(&KEY_SNAP_LATEST, &snapshot_id);

    env.events().publish(
        (symbol_short!("TRE"), symbol_short!("snapshot")),
        snapshot_id,
    );
    // Emit structured Event for treasury snapshot
    let mut payload = Map::new(env);
    payload.set(Symbol::short("id"), snapshot_id.into());
    payload.set(Symbol::short("balance"), total_balance.into());
    payload.set(Symbol::short("accounts"), account_count.into());
    payload.set(Symbol::short("ledger"), env.ledger().sequence().into());
    publish_event(env, BytesN::from_array(env, &[0u8; 32]), BytesN::from_array(env, &[0u8; 32]), payload);

    snapshot_id
}

/// Retrieve a snapshot by ID.
pub fn get_snapshot(env: &Env, snapshot_id: u64) -> Option<TreasurySnapshot> {
    let key = make_snap_key(env, snapshot_id);
    env.storage().instance().get(&key)
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
    // Encode snapshot id into a short symbol: prefix "S" + id as decimal.
    // Symbol is limited to 32 chars; u64 max is 20 digits, safe.
    Symbol::new(env, &format!("S{}", id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{testutils::Address as _, vec, Env, Map, String, Symbol};

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
            let ctx: Map<Symbol, soroban_sdk::Val> = Map::new(&env);
            let id = record_snapshot(&env, 1000, 5, String::from_str(&env, "deposit"), ctx);
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
            let ctx: Map<Symbol, soroban_sdk::Val> = Map::new(&env);
            record_snapshot(&env, 500, 2, String::from_str(&env, "withdrawal"), ctx);
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
            let ctx: Map<Symbol, soroban_sdk::Val> = Map::new(&env);
            record_snapshot(&env, -1, 0, String::from_str(&env, "bad"), ctx);
        });
    }
}
