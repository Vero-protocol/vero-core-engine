//! Event publishing helpers — emit a `CompactEvent` via the Soroban event log.

use soroban_sdk::{symbol_short, BytesN, Env};

use crate::event_struct::CompactEvent;

/// Publish a compact event.
///
/// `flags` — OR of `MOD_*` and `ACT_*` constants from `event_struct`.
/// `value` — primary numeric datum; 0 when unused.
/// `hash`  — 32-byte hash; all-zero when unused.
pub fn publish_event(env: &Env, flags: u32, value: u64, hash: BytesN<32>) {
    let ev = CompactEvent { flags, value, hash };
    env.events()
        .publish((symbol_short!("EVENT"), symbol_short!("LOG")), ev);
}
