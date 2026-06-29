//! Treasury state snapshots and outflow time-locking.


use crate::circuit_breaker::assert_closed;

use crate::event_struct::{ACT_REQUEST, ACT_SNAPSHOT, ACT_TRIGGERED, MOD_TREASURY};
use crate::event_utils::{publish_event, zero_hash};
use crate::types::{TreasurySnapshot, TriggerKind};
use soroban_sdk::{
    contracterror, contracttype, panic_with_error, symbol_short, Bytes, BytesN, Env, Map, Symbol,
    Val, Vec,
};

const KEY_SNAP_COUNTER: Symbol = symbol_short!("SNAPC");
const KEY_SNAP_LATEST: Symbol = symbol_short!("SNAPL");
const KEY_OUTFLOWS: Symbol = symbol_short!("OUTFLOWS");

const MAX_BALANCE: i128 = 1_000_000_000_000_000_000;
const MAX_ACCOUNT_COUNT: u32 = 10_000_000;
pub const OUTFLOW_TIMELOCK_SECONDS: u64 = 24 * 60 * 60;


/// About 7 days at 5-second ledger time, enough for off-chain indexer pickup.

/// Temporary storage TTL constants (ledgers).

const SNAP_TTL_THRESHOLD: u32 = 17_280;
const SNAP_TTL_EXTEND_TO: u32 = 17_280 * 7;

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]

pub enum TreasuryKey {

enum TreasuryKey {

    Snapshot(u64),
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub enum TreasuryError {
    SnapshotNotFound = 1,
    InvalidBalance = 2,
    InvalidAccountCount = 3,
    InvalidOutflowAmount = 4,
    OutflowNotFound = 5,
    OutflowAlreadyExecuted = 6,
    TimelockActive = 7,

    DuplicateOutflow = 8,

    ArithmeticOverflow = 8,

}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TimelockedOutflow {
    pub id: u64,
    pub amount: i128,
    pub requested_at: u64,
    pub executable_at: u64,
    pub executed: bool,
}

pub fn init(env: &Env) {
    env.storage().instance().set(&KEY_SNAP_COUNTER, &0u64);
    env.storage().instance().set(&KEY_SNAP_LATEST, &0u64);
    env.storage()
        .instance()
        .set(&KEY_OUTFLOWS, &Map::<u64, TimelockedOutflow>::new(env));
}

/// Queue a treasury outflow behind the mandatory 24-hour delay.
pub fn schedule_outflow(env: &Env, outflow_id: u64, amount: i128) -> u64 {
    crate::non_reentrant!(env);

    assert_closed(env);

    if amount <= 0 {
        panic_with_error!(env, TreasuryError::InvalidOutflowAmount);
    }

    let now = env.ledger().timestamp();
    let unlock_at = now
        .checked_add(OUTFLOW_TIMELOCK_SECONDS)
        .unwrap_or_else(|| panic_with_error!(env, TreasuryError::ArithmeticOverflow));
    let mut outflows = load_outflows(env);
    if outflows.contains_key(outflow_id) {
        panic_with_error!(env, TreasuryError::DuplicateOutflow);
    }
    let outflow = TimelockedOutflow {
        id: outflow_id,
        amount,
        requested_at: now,
        executable_at: unlock_at,
        executed: false,
    };

    outflows.set(outflow_id, outflow);
    env.storage().instance().set(&KEY_OUTFLOWS, &outflows);

    publish_event(env, MOD_TREASURY | ACT_REQUEST, outflow_id, zero_hash(env));
    unlock_at
}

/// Mark an outflow executable only after its 24-hour time-lock has expired.
pub fn execute_outflow(env: &Env, outflow_id: u64) -> TimelockedOutflow {
    crate::non_reentrant!(env);

    assert_closed(env);

    let mut outflows = load_outflows(env);
    let mut outflow = outflows
        .get(outflow_id)
        .unwrap_or_else(|| panic_with_error!(env, TreasuryError::OutflowNotFound));

    if outflow.executed {
        panic_with_error!(env, TreasuryError::OutflowAlreadyExecuted);
    }
    if env.ledger().timestamp() < outflow.executable_at {
        panic_with_error!(env, TreasuryError::TimelockActive);
    }

    outflow.executed = true;
    outflows.set(outflow_id, outflow.clone());
    env.storage().instance().set(&KEY_OUTFLOWS, &outflows);

    publish_event(
        env,
        MOD_TREASURY | ACT_TRIGGERED,
        outflow_id,
        zero_hash(env),
    );
    outflow
}

pub fn get_outflow(env: &Env, outflow_id: u64) -> Option<TimelockedOutflow> {
    load_outflows(env).get(outflow_id)
}


/// Record a treasury snapshot and return its ID.

/// Record a treasury snapshot and return its monotonic snapshot id.

pub fn record_snapshot(
    env: &Env,
    total_balance: i128,
    account_count: u32,
    trigger: TriggerKind,
    context: Map<Symbol, Val>,
) -> u64 {
    crate::non_reentrant!(env);

    assert_closed(env);

    if total_balance < 0 {
        panic_with_error!(env, TreasuryError::InvalidBalance);
    }

    // Preserve the repository's prior numeric-clamping behaviour while keeping
    // arithmetic checked around counters and timestamps.
    let total_balance = total_balance.min(MAX_BALANCE);
    let account_count = account_count.min(MAX_ACCOUNT_COUNT);

    let counter: u64 = env.storage().instance().get(&KEY_SNAP_COUNTER).unwrap_or(0);
    let snapshot_id = counter
        .checked_add(1)
        .unwrap_or_else(|| panic_with_error!(env, TreasuryError::ArithmeticOverflow));
    let ledger = env.ledger().sequence();
    let state_hash = compute_hash(env, total_balance, account_count, ledger);

    let snapshot = TreasurySnapshot {
        id: snapshot_id,
        total_balance,
        account_count,
        ledger,
        timestamp_unix: env.ledger().timestamp(),
        state_hash: state_hash.clone(),
        trigger,
        context,
    };


    let snapshot_key = make_snap_key(snapshot_id);

    let snapshot_key = TreasuryKey::Snapshot(snapshot_id);

    env.storage().temporary().set(&snapshot_key, &snapshot);
    env.storage()
        .temporary()
        .extend_ttl(&snapshot_key, SNAP_TTL_THRESHOLD, SNAP_TTL_EXTEND_TO);

    env.storage()
        .instance()
        .set(&KEY_SNAP_COUNTER, &snapshot_id);
    env.storage().instance().set(&KEY_SNAP_LATEST, &snapshot_id);

    publish_event(env, MOD_TREASURY | ACT_SNAPSHOT, snapshot_id, state_hash);
    snapshot_id
}

pub fn get_snapshot(env: &Env, snapshot_id: u64) -> Option<TreasurySnapshot> {

    let key = make_snap_key(snapshot_id);
    env.storage().temporary().get(&key)

    env.storage()
        .temporary()
        .get(&TreasuryKey::Snapshot(snapshot_id))

}

pub fn get_latest_snapshot(env: &Env) -> Option<TreasurySnapshot> {
    let latest_id: u64 = env.storage().instance().get(&KEY_SNAP_LATEST).unwrap_or(0);
    if latest_id == 0 {
        return None;
    }
    get_snapshot(env, latest_id)
}

pub fn snapshot_count(env: &Env) -> u64 {
    env.storage().instance().get(&KEY_SNAP_COUNTER).unwrap_or(0)
}

pub fn get_recent_snapshots(env: &Env, count: u32) -> Vec<u64> {
    let count = count.min(MAX_ACCOUNT_COUNT);
    let total = snapshot_count(env);
    let mut result = Vec::new(env);

    if total == 0 || count == 0 {

    if count == 0 || total == 0 {

        return result;
    }

    let start = if total as u32 > count {
        total - count as u64 + 1
    } else {
        1
    };
    for id in (start..=total).rev() {
        result.push_back(id);
    }
    result
}

pub fn verify_snapshot(env: &Env, snapshot: &TreasurySnapshot) -> bool {
    compute_hash(
        env,
        snapshot.total_balance,
        snapshot.account_count,
        snapshot.ledger,
    ) == snapshot.state_hash
}

pub fn audit_trail(env: &Env, from_id: u64) -> Vec<TreasurySnapshot> {
    let total = snapshot_count(env);
    let mut result = Vec::new(env);
    if total == 0 {
        return result;
    }

    let start = from_id.max(1).min(total);
    for id in start..=total {
        if let Some(snap) = get_snapshot(env, id) {
            result.push_back(snap);

    let start = if from_id == 0 { 1 } else { from_id.min(total) };
    for id in start..=total {
        if let Some(snapshot) = get_snapshot(env, id) {
            result.push_back(snapshot);

        }
    }
    result
}

fn load_outflows(env: &Env) -> Map<u64, TimelockedOutflow> {
    env.storage()
        .instance()
        .get(&KEY_OUTFLOWS)
        .unwrap_or(Map::new(env))
}

fn compute_hash(env: &Env, balance: i128, account_count: u32, ledger: u32) -> BytesN<32> {
    let mut raw = [0u8; 24];
    raw[..16].copy_from_slice(&balance.to_be_bytes());
    raw[16..20].copy_from_slice(&account_count.to_be_bytes());
    raw[20..24].copy_from_slice(&ledger.to_be_bytes());
    env.crypto().sha256(&Bytes::from_slice(env, &raw)).into()
}


fn make_snap_key(id: u64) -> TreasuryKey {
    TreasuryKey::Snapshot(id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{contract, contractimpl, testutils::Ledger as _, Env, Map, Symbol};

#[cfg(test)]
mod tests {
    use super::*;

    use soroban_sdk::{testutils::Ledger as _, Env, Map, Symbol, testutils::Address as _};

    use soroban_sdk::{testutils::Ledger as _, Env, Map, Symbol};


    #[contract]
    pub struct TestContract;

    #[contractimpl]
    impl TestContract {}

    fn with_treasury_env(run: impl FnOnce(&Env)) {
        let env = Env::default();
        let contract_id = env.register_contract(None, TestContract);
        env.as_contract(&contract_id, || {
            init(&env);
            run(&env);
        });
    }

    #[test]
    fn snapshot_creation_and_retrieval() {
        with_treasury_env(|env| {
            let ctx: Map<Symbol, Val> = Map::new(env);
            let id = record_snapshot(env, 1000, 5, TriggerKind::Deposit, ctx);
            assert_eq!(id, 1);
            let snap = get_snapshot(env, 1).unwrap();
            assert_eq!(snap.total_balance, 1000);
            assert_eq!(snapshot_count(env), 1);
        });
    }

    #[test]
    fn snapshot_hash_verification() {
        with_treasury_env(|env| {
            let ctx: Map<Symbol, Val> = Map::new(env);
            record_snapshot(env, 500, 2, TriggerKind::Withdrawal, ctx);
            let snap = get_snapshot(env, 1).unwrap();
            assert!(verify_snapshot(env, &snap));
        });
    }

    #[test]
fn latest_snapshot_is_none_when_empty() {
    with_treasury_env(|env| {
        assert!(get_latest_snapshot(env).is_none());
        assert_eq!(snapshot_count(env), 0);
    });
}

    #[test]
    #[should_panic]
    fn negative_balance_rejected() {
        with_treasury_env(|env| {
            let ctx: Map<Symbol, Val> = Map::new(env);
            record_snapshot(env, -1, 0, TriggerKind::Other, ctx);
        });
    }

    #[test]
    #[should_panic]
    fn withdrawal_blocked_before_time_lock_expires() {
        with_treasury_env(|env| {
            schedule_outflow(env, 7, 1_000);
            execute_outflow(env, 7);
        });
    }

    #[test]
    fn withdrawal_executes_after_time_lock_expires() {
        with_treasury_env(|env| {
            let unlock_at = schedule_outflow(env, 7, 1_000);
            env.ledger().set_timestamp(unlock_at);
            let outflow = execute_outflow(env, 7);
            assert!(outflow.executed);
            assert_eq!(outflow.executable_at, unlock_at);
        });
    }
}
