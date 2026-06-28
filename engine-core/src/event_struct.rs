//! Compact event encoding for audit-friendly Soroban logs.
//!
//! `CompactEvent` replaces heap-heavy, loosely typed event payloads with a flat
//! structure that is deterministic, cheap to emit, and easy for indexers to
//! verify. The `flags` field packs a module id and action id into one `u32`.

use soroban_sdk::{contracttype, BytesN};

// Module ids (bits 0..=7).
pub const MOD_AUDIT: u32 = 0x01;
pub const MOD_GOV: u32 = 0x02;
pub const MOD_TREASURY: u32 = 0x03;
pub const MOD_CB: u32 = 0x04;
pub const MOD_BURN: u32 = 0x05;
pub const MOD_RECOVERY: u32 = 0x06;
pub const MOD_FEE: u32 = 0x07;

// Action ids (bits 8..=15).
pub const ACT_COMMIT: u32 = 0x01 << 8;
pub const ACT_SNAPSHOT: u32 = 0x02 << 8;
pub const ACT_PROPOSE: u32 = 0x03 << 8;
pub const ACT_APPROVE: u32 = 0x04 << 8;
pub const ACT_EXECUTE: u32 = 0x05 << 8;
pub const ACT_TRIP: u32 = 0x06 << 8;
pub const ACT_RESET: u32 = 0x07 << 8;
pub const ACT_BURN_SAFE: u32 = 0x08 << 8;
pub const ACT_REQUEST: u32 = 0x09 << 8;
pub const ACT_TRIGGERED: u32 = 0x0a << 8;
pub const ACT_FEE: u32 = 0x0b << 8;

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompactEvent {
    pub flags: u32,
    pub value: u64,
    pub hash: BytesN<32>,
}
