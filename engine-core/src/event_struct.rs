//! Compact event encoding for audit-ready Soroban logs.
//!
//! `CompactEvent` keeps on-chain events small and deterministic while still
//! giving the relayer/dashboard enough data to reconstruct the control-plane
//! audit trail. `flags` packs a module id and an action id; `value` carries the
//! primary numeric datum; `hash` carries a state/action hash when present.

use soroban_sdk::{contracttype, BytesN};

// Compact event encoding — bitmask-based event struct.
//
// Replaces the previous fat `Event { event_type: BytesN<32>, action: BytesN<32>, payload: Map }`
// which wasted 64 bytes of zeroed data and allocated an expensive `Map<Symbol,Val>` on every call.
//
// ## Encoding
//
// `flags` packs module id (bits 0–7) and action id (bits 8–15) into a single `u32`:
//
// ```text
// bits  0– 7 : module id  (MOD_*)
// bits  8–15 : action id  (ACT_*)
// bits 16–31 : reserved for future use / version
// ```
//
// `value` carries a u64 primary value (sequence, proposal id, amount, …).
// `hash`  carries an optional 32-byte hash (state_hash, action_hash). Zero if unused.

// ── module ids ────────────────────────────────────────────────────────────────

pub const MOD_AUDIT: u32 = 0x01;
pub const MOD_GOV: u32 = 0x02;
pub const MOD_TREASURY: u32 = 0x03;
pub const MOD_CB: u32 = 0x04;
pub const MOD_BURN: u32 = 0x05;
pub const MOD_RECOVERY: u32 = 0x06;
pub const MOD_FEE: u32 = 0x07;
pub const MOD_UPGRADE: u32 = 0x08;

// ── action ids ────────────────────────────────────────────────────────────────

pub const ACT_COMMIT: u32 = 0x01 << 8;
pub const ACT_SNAPSHOT: u32 = 0x02 << 8;
pub const ACT_PROPOSE: u32 = 0x03 << 8;
pub const ACT_APPROVE: u32 = 0x04 << 8;
pub const ACT_EXECUTE: u32 = 0x05 << 8;
pub const ACT_TRIP: u32 = 0x06 << 8;
pub const ACT_RESET: u32 = 0x07 << 8;
pub const ACT_BURN_SAFE: u32 = 0x08 << 8;
pub const ACT_REQUEST: u32 = 0x09 << 8;
pub const ACT_TRIGGERED: u32 = 0x0A << 8;
pub const ACT_FEE: u32 = 0x0B << 8;
pub const ACT_UPGRADE: u32 = 0x0C << 8;

/// Compact event struct emitted by all engine-core modules.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompactEvent {
    /// Packed module + action bitmask. Use `MOD_* | ACT_*` constants.
    pub flags: u32,
    /// Primary numeric value — sequence number, proposal id, amount, etc.
    pub value: u64,
    /// Optional 32-byte hash (all-zero when unused).
    pub hash: BytesN<32>,
}
