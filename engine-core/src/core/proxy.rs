//! Upgradeable Proxy Pattern Entry Point
//!
//! Provides a hardened, audit-ready foundation for the Vero Protocol control plane.
//! Includes storage gap to prevent storage collisions, admin controls, and
//! adheres to Soroban/Rust security standards.

use soroban_sdk::{contract, contractimpl, contracterror, panic_with_error, Address, BytesN, Env, Symbol, Bytes};
extern crate alloc;

use crate::{audit, types::StateCommitment};

const ADMIN_KEY: Symbol = soroban_sdk::symbol_short!("ADMIN");
const GAP_KEY: Symbol = soroban_sdk::symbol_short!("GAP");

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub enum ProxyError {
    AlreadyInitialized = 1,
}

#[contract]
pub struct UpgradeableProxy;

#[contractimpl]
impl UpgradeableProxy {
    /// Initialize the proxy with an admin address and a storage gap.
    pub fn init(env: Env, admin: Address) {
        crate::non_reentrant!(&env);

        if env.storage().instance().has(&ADMIN_KEY) {
            panic_with_error!(&env, ProxyError::AlreadyInitialized);
        }
        
        admin.require_auth();
        env.storage().instance().set(&ADMIN_KEY, &admin);
        
        // Storage gap to reserve slots and prevent collisions in future upgrades
        let gap: soroban_sdk::Vec<u64> = soroban_sdk::Vec::from_array(&env, [0u64; 50]);
        env.storage().instance().set(&GAP_KEY, &gap);
    }

    /// Upgrade the contract's WASM code. Routes through the governance-gated flow.
    pub fn upgrade(env: Env, proposal_id: u64, new_wasm_hash: BytesN<32>) {
        crate::non_reentrant!(&env);
        crate::upgrade::upgrade(&env, proposal_id, new_wasm_hash);
    }
    
    /// ZK-ready integrity check invoked via the audit layer
    pub fn verify_integrity(env: Env, commitment: StateCommitment, payload: Bytes) {
        crate::non_reentrant!(&env);

        // Copy bytes to verify transition
        let mut payload_buf = alloc::vec::Vec::new();
        payload_buf.resize(payload.len() as usize, 0);
        payload.copy_into_slice(&mut payload_buf);
        
        audit::validate_transition(&env, &commitment, &payload_buf);
    }
}
