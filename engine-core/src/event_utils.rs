//! Event publishing helpers.

use crate::event_struct::CompactEvent;
use soroban_sdk::{symbol_short, BytesN, Env};

/// Publish a deterministic compact event under a single topic for indexing.
pub fn publish_event(env: &Env, flags: u32, value: u64, hash: BytesN<32>) {
    let ev = CompactEvent { flags, value, hash };
    env.events()
        .publish((symbol_short!("EVENT"), symbol_short!("LOG")), ev);
}
