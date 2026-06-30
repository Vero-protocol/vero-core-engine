//! Event publishing helpers — emit a `CompactEvent` via the Soroban event log.

use soroban_sdk::{symbol_short, BytesN, Env};

use crate::event_struct::CompactEvent;

pub fn publish_event(env: &Env, flags: u32, value: u64, hash: BytesN<32>) {
    let ev = CompactEvent { flags, value, hash };
    env.events()
        .publish((symbol_short!("EVENT"), symbol_short!("LOG")), ev);
}

/// Canonical all-zero 32-byte hash for events that carry no hash payload.
pub fn zero_hash(env: &Env) -> BytesN<32> {
    BytesN::from_array(env, &[0u8; 32])
}
