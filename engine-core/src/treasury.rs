//! Treasury state snapshots and outflow time-locking.


use crate::circuit_breaker::assert_closed;

use crate::event_struct::{ACT_REQUEST, ACT_SNAPSHOT, ACT_TRIGGERED, MOD_TREASURY};
use crate::event_utils::{publish_event, zero_hash};
use crate::types::{TreasurySnapshot, TriggerKind};
use soroban_sdk::{
    contracterror, contracttype, panic_with_error, symbol_short, Address, Bytes, BytesN, Env, Map,
    Symbol, Val, Vec,
};

const KEY_ADMIN: Symbol = symbol_short!("TR_ADMIN");
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
    ArithmeticOverflow = 9,
    Unauthorized = 10,
    NotInitialized = 11,
    AlreadyInitialized = 12,
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

pub fn init(env: &Env, admin: Address) {
    if env.storage().instance().has(&KEY_ADMIN) {
        panic_with_error!(env, TreasuryError::AlreadyInitialized);
    }
    env.storage().instance().set(&KEY_ADMIN, &admin);
    env.storage().instance().set(&KEY_SNAP_COUNTER, &0u64);
    env.storage().instance().set(&KEY_SNAP_LATEST, &0u64);
    env.storage()
        .instance()
        .set(&KEY_OUTFLOWS, &Map::<u64, TimelockedOutflow>::new(env));
}

fn require_admin(env: &Env, caller: &Address) {
    caller.require_auth();
    let admin: Address = env
        .storage()
        .instance()
        .get(&KEY_ADMIN)
        .unwrap_or_else(|| panic_with_error!(env, TreasuryError::NotInitialized));
    if caller != &admin {
        panic_with_error!(env, TreasuryError::Unauthorized);
    }
}

/// Queue a treasury outflow behind the mandatory 24-hour delay.
///
/// Authorization is enforced in this module, not only at a future ControlPlane
/// wrapper (see issue #178). Wrappers must forward `caller`; they cannot omit
/// the check because `require_admin` lives here.
pub fn schedule_outflow(env: &Env, caller: &Address, outflow_id: u64, amount: i128) -> u64 {
    crate::non_reentrant!(env);

    assert_closed(env);
    require_admin(env, caller);

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
///
/// Authorization is enforced in this module, not only at a future ControlPlane
/// wrapper (see issue #178). Wrappers must forward `caller`; they cannot omit
/// the check because `require_admin` lives here.
pub fn execute_outflow(env: &Env, caller: &Address, outflow_id: u64) -> TimelockedOutflow {
    crate::non_reentrant!(env);

    assert_closed(env);
    require_admin(env, caller);

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


/// Record a treasury snapshot and return its monotonic snapshot id.
///
/// Authorization is enforced in this module, not only at a future ControlPlane
/// wrapper (see issue #178). Wrappers must forward `caller`; they cannot omit
/// the check because `require_admin` lives here.
pub fn record_snapshot(
    env: &Env,
    caller: &Address,
    total_balance: i128,
    account_count: u32,
    trigger: TriggerKind,
    context: Map<Symbol, Val>,
) -> u64 {
    crate::non_reentrant!(env);

    assert_closed(env);
    require_admin(env, caller);

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
    use soroban_sdk::{
        contract, contractimpl,
        testutils::{Address as _, Ledger as _},
        Env, Map, Symbol,
    };

    #[contract]
    pub struct TestContract;

    #[contractimpl]
    impl TestContract {}

    fn setup() -> (Env, Address, Address) {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, TestContract);
        let admin = Address::generate(&env);
        env.as_contract(&contract_id, || init(&env, admin.clone()));
        (env, contract_id, admin)
    }

    fn setup_without_auth_mock() -> (Env, Address, Address) {
        let env = Env::default();
        let contract_id = env.register_contract(None, TestContract);
        let admin = Address::generate(&env);
        env.as_contract(&contract_id, || init(&env, admin.clone()));
        (env, contract_id, admin)
    }

    #[test]
    fn snapshot_creation_and_retrieval() {
        let (env, contract_id, admin) = setup();
        env.as_contract(&contract_id, || {
            let ctx: Map<Symbol, Val> = Map::new(&env);
            let id = record_snapshot(&env, &admin, 1000, 5, TriggerKind::Deposit, ctx);
            assert_eq!(id, 1);
            let snap = get_snapshot(&env, 1).unwrap();
            assert_eq!(snap.total_balance, 1000);
            assert_eq!(snapshot_count(&env), 1);
        });
    }

    #[test]
    fn snapshot_hash_verification() {
        let (env, contract_id, admin) = setup();
        env.as_contract(&contract_id, || {
            let ctx: Map<Symbol, Val> = Map::new(&env);
            record_snapshot(&env, &admin, 500, 2, TriggerKind::Withdrawal, ctx);
            let snap = get_snapshot(&env, 1).unwrap();
            assert!(verify_snapshot(&env, &snap));
        });
    }

    #[test]
    fn latest_snapshot_is_none_when_empty() {
        let (env, contract_id, _admin) = setup();
        env.as_contract(&contract_id, || {
            assert!(get_latest_snapshot(&env).is_none());
            assert_eq!(snapshot_count(&env), 0);
        });
    }

    #[test]
    #[should_panic]
    fn negative_balance_rejected() {
        let (env, contract_id, admin) = setup();
        env.as_contract(&contract_id, || {
            let ctx: Map<Symbol, Val> = Map::new(&env);
            record_snapshot(&env, &admin, -1, 0, TriggerKind::Other, ctx);
        });
    }

    #[test]
    #[should_panic]
    fn withdrawal_blocked_before_time_lock_expires() {
        let (env, contract_id, admin) = setup();
        env.as_contract(&contract_id, || {
            schedule_outflow(&env, &admin, 7, 1_000);
        });
        env.as_contract(&contract_id, || {
            execute_outflow(&env, &admin, 7);
        });
    }

    #[test]
    fn withdrawal_executes_after_time_lock_expires() {
        let (env, contract_id, admin) = setup();
        let unlock_at = env.as_contract(&contract_id, || schedule_outflow(&env, &admin, 7, 1_000));
        env.ledger().set_timestamp(unlock_at);
        env.as_contract(&contract_id, || {
            let outflow = execute_outflow(&env, &admin, 7);
            assert!(outflow.executed);
            assert_eq!(outflow.executable_at, unlock_at);
        });
    }

    #[test]
    #[should_panic]
    fn schedule_outflow_rejects_unauthorized_caller() {
        let (env, contract_id, _admin) = setup();
        let rogue = Address::generate(&env);
        env.as_contract(&contract_id, || {
            schedule_outflow(&env, &rogue, 1, 1_000);
        });
    }

    #[test]
    #[should_panic]
    fn execute_outflow_rejects_unauthorized_caller() {
        let (env, contract_id, admin) = setup();
        env.as_contract(&contract_id, || {
            schedule_outflow(&env, &admin, 1, 1_000);
        });
        let rogue = Address::generate(&env);
        env.as_contract(&contract_id, || {
            execute_outflow(&env, &rogue, 1);
        });
    }

    #[test]
    #[should_panic]
    fn record_snapshot_rejects_unauthorized_caller() {
        let (env, contract_id, _admin) = setup();
        let rogue = Address::generate(&env);
        env.as_contract(&contract_id, || {
            let ctx: Map<Symbol, Val> = Map::new(&env);
            record_snapshot(&env, &rogue, 1000, 1, TriggerKind::Manual, ctx);
        });
    }

    #[test]
    #[should_panic]
    fn schedule_outflow_rejects_unauthenticated_caller() {
        let (env, contract_id, admin) = setup_without_auth_mock();
        env.as_contract(&contract_id, || {
            schedule_outflow(&env, &admin, 1, 1_000);
        });
    }

    #[test]
    #[should_panic]
    fn execute_outflow_rejects_unauthenticated_caller() {
        let (env, contract_id, admin) = setup_without_auth_mock();
        env.as_contract(&contract_id, || {
            execute_outflow(&env, &admin, 1);
        });
    }

    #[test]
    #[should_panic]
    fn record_snapshot_rejects_unauthenticated_caller() {
        let (env, contract_id, admin) = setup_without_auth_mock();
        env.as_contract(&contract_id, || {
            let ctx: Map<Symbol, Val> = Map::new(&env);
            record_snapshot(&env, &admin, 1000, 1, TriggerKind::Manual, ctx);
        });
    }
}
