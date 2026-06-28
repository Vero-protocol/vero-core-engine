//! Event publishing helpers.

use crate::event_struct::CompactEvent;
use soroban_sdk::{symbol_short, BytesN, Env};

/// Publish a compact, structured event.
///
/// `flags` is built by OR-ing a `MOD_*` constant with an `ACT_*` constant from
/// [`crate::event_struct`]. The event topic is intentionally stable so the
/// off-chain bridge can consume every engine-core event with one subscription.
pub fn publish_event(env: &Env, flags: u32, value: u64, hash: BytesN<32>) {
    let event = CompactEvent { flags, value, hash };
    env.events()
        .publish((symbol_short!("EVENT"), symbol_short!("LOG")), event);
}

/// Return the canonical all-zero hash used when an event has no hash payload.
pub fn zero_hash(env: &Env) -> BytesN<32> {
    BytesN::from_array(env, &[0u8; 32])
}
