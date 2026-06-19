//! Burn module.
//!
//! This repository snapshot-completes the treasury audit-history subsystem.
//! The current codebase, however, does not contain a treasury token / burn
//! entrypoint. To satisfy the new requirement (reject burn to the zero
//! address), we provide a minimal burn-safe entrypoint wrapper that can be
//! wired into the token/treasury layer.

use soroban_sdk::{panic_with_error, symbol_short, Address, Env, BytesN, Bytes};

/// Errors for burn safety checks.
#[repr(u32)]
#[derive(Copy, Clone)]
pub enum BurnError {
    /// Attempted to burn/transfer funds to the zero address.
    ZeroAddress = 1,
}

/// Rejects zero-address recipients.
///
/// Intended usage: call this at the beginning of any burn/consume entrypoint
/// before executing state mutation.
pub fn reject_zero_address(env: &Env, to: &Address) {
    // Soroban addresses are opaque bytes. The "zero" address is the
    // all-zero 32-byte address.
    //
    // `Address::from_contract_id` / conversions aren’t available without
    // external context; instead, we use the raw address bytes.
    let raw = to.to_bytes();
    let mut all_zero = true;
    for b in raw.iter() {
        if b != 0 {
            all_zero = false;
            break;
        }
    }
    if all_zero {
        panic_with_error!(env, BurnError::ZeroAddress);
    }
}

/// Example burn entrypoint wrapper (no-op placeholder).
///
/// Since the actual token/treasury state machine isn’t present in this repo,
/// this function only performs the safety check and emits an event.
/// It can be replaced/extended once the real burn state exists.
pub fn burn_to(env: &Env, to: &Address, amount: i128) {
    reject_zero_address(env, to);
    env.events().publish((symbol_short!("TRE"), symbol_short!("burn_safe")), (to.clone(), amount));
}

