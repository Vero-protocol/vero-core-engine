use soroban_sdk::{contracterror, panic_with_error, symbol_short, BytesN, Bytes, Env, IntoVal, Map, String, Symbol, Vec};
use crate::event_utils::publish_event;
use crate::types::TreasurySnapshot;
use crate::circuit_breaker;

const KEY_SNAP_COUNTER: Symbol = symbol_short!("SNAPC");
const KEY_SNAP_LATEST:  Symbol = symbol_short!("SNAPL");
const MAX_BALANCE: i128 = 1_000_000_000_000_000_000;
const MAX_ACCOUNT_COUNT: u32 = 10_000_000;

// Safety cap to prevent unbounded iteration / excessive gas use in audit queries
const MAX_AUDIT_RESULTS: u32 = 100;

#[contracttype]
#[contracterror]
#[derive(Copy, Clone)]
pub enum TreasuryError {
    SnapshotNotFound = 1,
    InvalidBalance   = 2,
    InvalidAccountCount = 3,
}
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
    circuit_breaker::assert_closed(env);
    // Validate balance is non-negative
    if total_balance < 0 {
        panic_with_error!(env, TreasuryError::InvalidBalance);
    }
    let total_balance = total_balance.min(MAX_BALANCE);
    let account_count = account_count.min(MAX_ACCOUNT_COUNT);

    let counter: u64 = env.storage().instance().get(&KEY_SNAP_COUNTER).unwrap_or(0);
    let snapshot_id = counter + 1;

    let state_hash = compute_hash(env, total_balance, account_count, env.ledger().sequence());

    let ts_str = String::from_str(env, &alloc::format!("{}", env.ledger().timestamp()));

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
    env.storage().instance().set(&KEY_SNAP_LATEST, &snapshot_id);

    env.events().publish(
        (symbol_short!("TRE"), symbol_short!("snapshot")),
        snapshot_id,
    );
    let mut payload = Map::new(env);
    payload.set(symbol_short!("id"), snapshot_id.into_val(env));
    payload.set(symbol_short!("balance"), total_balance.into_val(env));
    payload.set(symbol_short!("accounts"), account_count.into_val(env));
    payload.set(symbol_short!("ledger"), env.ledger().sequence().into_val(env));
    publish_event(env, BytesN::from_array(env, &[0u8; 32]), BytesN::from_array(env, &[0u8; 32]), payload);
    env.storage().instance().set(&KEY_SNAP_LATEST,  &snapshot_id);

    // Single compact event — snapshot id in value, state_hash in hash field.
    publish_event(env, MOD_TREASURY | ACT_SNAPSHOT, snapshot_id, state_hash);

    snapshot_id
}

pub fn get_snapshot(env: &Env, snapshot_id: u64) -> Option<TreasurySnapshot> {
    let key = make_snap_key(env, snapshot_id);
    env.storage().temporary().get(&key)
}

pub fn get_latest_snapshot(env: &Env) -> Option<TreasurySnapshot> {
    let latest_id: u64 = env.storage().instance().get(&KEY_SNAP_LATEST).unwrap_or(0);
    if latest_id == 0 { return None; }
    get_snapshot(env, latest_id)
}

pub fn snapshot_count(env: &Env) -> u64 {
    env.storage().instance().get(&KEY_SNAP_COUNTER).unwrap_or(0)
}

pub fn get_recent_snapshots(env: &Env, count: u32) -> Vec<u64> {
    let count = count.min(MAX_ACCOUNT_COUNT);
    let total = snapshot_count(env);
    let mut result = Vec::new(env);

    let requested = if count > MAX_AUDIT_RESULTS { MAX_AUDIT_RESULTS } else { count };
    let start = if total as u32 > requested {
        (total as u32) - requested + 1
    } else {
        1
    };

    if total == 0 { return result; }
    let start = if total as u32 > count { (total as u32) - count + 1 } else { 1 };
    for id in (start as u64..=total).rev() {
        result.push_back(id);
    }
    result
}

pub fn verify_snapshot(env: &Env, snapshot: &TreasurySnapshot) -> bool {
    let recomputed = compute_hash(env, snapshot.total_balance, snapshot.account_count, snapshot.ledger);
    snapshot.state_hash == recomputed
}

pub fn audit_trail(env: &Env, from_id: u64) -> Vec<TreasurySnapshot> {
    // Delegate to a bounded audit function to avoid unbounded gas usage.
    audit_trail_limited(env, from_id, MAX_AUDIT_RESULTS)
}

/// Audit report (limited): retrieve up to `max_results` snapshots since a given ID.
/// Returns snapshots in ascending ID order. This bounded variant prevents callers
/// from triggering unbounded iteration and excessive gas consumption.
pub fn audit_trail_limited(env: &Env, from_id: u64, max_results: u32) -> Vec<TreasurySnapshot> {
    let total = snapshot_count(env);
    let from_id = from_id.min(total);
    let mut result = Vec::new(env);

    let requested = if max_results > MAX_AUDIT_RESULTS { MAX_AUDIT_RESULTS } else { max_results };
    let mut collected: u32 = 0;

    let mut id = from_id;
    while id <= total && collected < requested {
    for id in from_id..=total {
        if let Some(snap) = get_snapshot(env, id) {
            result.push_back(snap);
            collected += 1;
        }
        id += 1;
    }
    result
}

fn compute_hash(env: &Env, balance: i128, account_count: u32, ledger: u32) -> BytesN<32> {
    let mut raw = [0u8; 24];
    raw[..16].copy_from_slice(&balance.to_be_bytes());
    raw[16..20].copy_from_slice(&account_count.to_be_bytes());
    raw[20..24].copy_from_slice(&ledger.to_be_bytes());
    env.crypto().sha256(&Bytes::from_slice(env, &raw)).into()
}

fn make_snap_key(env: &Env, id: u64) -> Symbol {
    Symbol::new(env, &alloc::format!("S{}", id))
    Symbol::new(env, &format!("S{}", id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{Env, Map, String, Symbol};
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
