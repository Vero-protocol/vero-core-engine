#![no_std]
//! vero-audit-guard — Policy Bundle Signature Verification (VAG-003)
//!
//! An **observational / telemetry-only** control in the Vero audit-guard layer.
//! PR review reduces, but does not eliminate, the risk of a compromised CI
//! runner pushing an unauthorized policy change. This module adds a second,
//! independent control: a policy bundle is only attested when it carries a
//! valid ed25519 signature produced by a key held in the guard's authorized
//! signer registry.
//!
//! ## Observational-only invariant
//! This contract has **no on-chain halt authority**. It only ever writes to its
//! own storage (the signer registry and the attestation log) and emits
//! telemetry events. It never calls into `engine-core`, never mutates protocol
//! state, and never reverts a protocol operation. A rejected bundle surfaces as
//! an `ALERT` event plus a typed [`AuditGuardError`] — not a silent drop.
//!
//! ## Error propagation
//! Every failure the contract can evaluate to a boolean (malformed input,
//! unauthorized signer, replay) is returned as a typed [`AuditGuardError`] and
//! accompanied by an alert event — the guard never panics on these paths. The
//! sole trapping path is the host primitive `env.crypto().ed25519_verify`,
//! which the Soroban host implements as a trap-on-failure operation; there is no
//! non-trapping ed25519 verifier on-chain. It is reached only after the signer
//! has already been authorized, so it guards against a corrupted/forged
//! signature from an otherwise-registered key. The primary threat — an
//! unauthorized signer (a compromised CI runner without an authorized key) — is
//! caught earlier on the non-trapping, alert-emitting path.
//!
//! > On `thiserror`: the tracking issue suggested `thiserror` for error
//! > propagation. `thiserror` is a `std`-only crate and cannot compile in this
//! > `#![no_std]` wasm contract; Soroban's idiomatic equivalent is the
//! > `#[contracterror]` enum used below, which propagates typed errors across
//! > the contract boundary instead of panicking.

#[cfg(test)]
extern crate std;

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, Bytes, BytesN, Env,
};

/// Typed errors propagated across the contract boundary in place of panics.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub enum AuditGuardError {
    /// `initialize` was called on an already-initialized contract.
    AlreadyInitialized = 1,
    /// A method requiring an admin ran before `initialize`.
    NotInitialized = 2,
    /// The submitted policy bundle was empty.
    EmptyBundle = 3,
    /// The signing key is not in the authorized signer registry.
    UnauthorizedSigner = 4,
    /// This exact bundle has already been attested (replay).
    ReplayedBundle = 5,
    /// `register_signer` called with a key already registered.
    SignerAlreadyRegistered = 6,
    /// `revoke_signer` called with a key that is not registered.
    SignerNotFound = 7,
}

/// Alert reason codes carried in emitted telemetry events.
const REASON_EMPTY_BUNDLE: u32 = 1;
const REASON_UNAUTHORIZED_SIGNER: u32 = 2;
const REASON_REPLAY: u32 = 3;
const REASON_SIGNER_REGISTERED: u32 = 10;
const REASON_SIGNER_REVOKED: u32 = 11;

#[contracttype]
enum DataKey {
    /// Marker set once the contract is initialized.
    Init,
    /// The admin authorized to manage the signer registry.
    Admin,
    /// Authorized signer public key -> registered flag.
    Signer(BytesN<32>),
    /// Attested bundle hash -> attested flag (replay protection).
    Bundle(BytesN<32>),
}

/// Telemetry payload emitted when a bundle is rejected.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AlertEvent {
    pub reason: u32,
    pub signer: BytesN<32>,
    pub bundle_hash: BytesN<32>,
}

/// Telemetry payload emitted when a bundle is successfully attested.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedEvent {
    pub signer: BytesN<32>,
    pub bundle_hash: BytesN<32>,
}

/// Telemetry payload emitted when the signer registry changes.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SignerEvent {
    pub reason: u32,
    pub signer: BytesN<32>,
}

#[contract]
pub struct AuditGuardContract;

#[contractimpl]
impl AuditGuardContract {
    /// Initialize the guard with the admin that manages the signer registry.
    pub fn initialize(env: Env, admin: Address) -> Result<(), AuditGuardError> {
        if env.storage().instance().has(&DataKey::Init) {
            return Err(AuditGuardError::AlreadyInitialized);
        }
        admin.require_auth();
        env.storage().instance().set(&DataKey::Init, &true);
        env.storage().instance().set(&DataKey::Admin, &admin);
        Ok(())
    }

    /// Register an ed25519 public key as an authorized policy-bundle signer.
    pub fn register_signer(env: Env, signer: BytesN<32>) -> Result<(), AuditGuardError> {
        let admin = read_admin(&env)?;
        admin.require_auth();
        if signer_authorized(&env, &signer) {
            return Err(AuditGuardError::SignerAlreadyRegistered);
        }
        env.storage()
            .persistent()
            .set(&DataKey::Signer(signer.clone()), &true);
        emit_signer_event(&env, REASON_SIGNER_REGISTERED, &signer);
        Ok(())
    }

    /// Revoke a previously-authorized signer.
    pub fn revoke_signer(env: Env, signer: BytesN<32>) -> Result<(), AuditGuardError> {
        let admin = read_admin(&env)?;
        admin.require_auth();
        if !signer_authorized(&env, &signer) {
            return Err(AuditGuardError::SignerNotFound);
        }
        env.storage()
            .persistent()
            .remove(&DataKey::Signer(signer.clone()));
        emit_signer_event(&env, REASON_SIGNER_REVOKED, &signer);
        Ok(())
    }

    /// View: is `signer` in the authorized registry?
    pub fn is_authorized_signer(env: Env, signer: BytesN<32>) -> bool {
        signer_authorized(&env, &signer)
    }

    /// View: has this bundle hash already been attested?
    pub fn is_attested(env: Env, bundle_hash: BytesN<32>) -> bool {
        bundle_attested(&env, &bundle_hash)
    }

    /// Attest a policy bundle by verifying its ed25519 signature against an
    /// authorized signer. Returns the sha256 hash of the bundle on success.
    ///
    /// Rejections on the non-trapping paths (empty bundle, unauthorized signer,
    /// replay) emit an `ALERT` telemetry event and return a typed error. A
    /// cryptographically invalid signature from an authorized key traps in the
    /// host `ed25519_verify` primitive (see module docs).
    pub fn attest_policy_bundle(
        env: Env,
        bundle: Bytes,
        signer: BytesN<32>,
        signature: BytesN<64>,
    ) -> Result<BytesN<32>, AuditGuardError> {
        require_initialized(&env)?;

        // 1. Structural check — reject empty bundles with an alert.
        if bundle.is_empty() {
            emit_alert(&env, REASON_EMPTY_BUNDLE, &signer, &zero_hash(&env));
            return Err(AuditGuardError::EmptyBundle);
        }

        let bundle_hash: BytesN<32> = env.crypto().sha256(&bundle).into();

        // 2. Independent control — reject bundles from unauthorized signers.
        //    This is the primary defense against a compromised CI runner: it
        //    lacks an authorized signing key, so its bundle is alerted and
        //    rejected here without ever reaching the trapping crypto path.
        if !signer_authorized(&env, &signer) {
            emit_alert(&env, REASON_UNAUTHORIZED_SIGNER, &signer, &bundle_hash);
            return Err(AuditGuardError::UnauthorizedSigner);
        }

        // 3. Replay protection — a given bundle is attested at most once.
        if bundle_attested(&env, &bundle_hash) {
            emit_alert(&env, REASON_REPLAY, &signer, &bundle_hash);
            return Err(AuditGuardError::ReplayedBundle);
        }

        // 4. Cryptographic gate — host traps if the signature is invalid.
        env.crypto().ed25519_verify(&signer, &bundle, &signature);

        // 5. Record the attestation and surface success as telemetry.
        env.storage()
            .persistent()
            .set(&DataKey::Bundle(bundle_hash.clone()), &true);
        emit_verified(&env, &signer, &bundle_hash);

        Ok(bundle_hash)
    }
}

// --- internal helpers ------------------------------------------------------

fn read_admin(env: &Env) -> Result<Address, AuditGuardError> {
    env.storage()
        .instance()
        .get(&DataKey::Admin)
        .ok_or(AuditGuardError::NotInitialized)
}

fn require_initialized(env: &Env) -> Result<(), AuditGuardError> {
    if env.storage().instance().has(&DataKey::Init) {
        Ok(())
    } else {
        Err(AuditGuardError::NotInitialized)
    }
}

fn signer_authorized(env: &Env, signer: &BytesN<32>) -> bool {
    env.storage()
        .persistent()
        .get(&DataKey::Signer(signer.clone()))
        .unwrap_or(false)
}

fn bundle_attested(env: &Env, bundle_hash: &BytesN<32>) -> bool {
    env.storage()
        .persistent()
        .get(&DataKey::Bundle(bundle_hash.clone()))
        .unwrap_or(false)
}

fn zero_hash(env: &Env) -> BytesN<32> {
    BytesN::from_array(env, &[0u8; 32])
}

fn emit_alert(env: &Env, reason: u32, signer: &BytesN<32>, bundle_hash: &BytesN<32>) {
    env.events().publish(
        (symbol_short!("AGUARD"), symbol_short!("ALERT")),
        AlertEvent {
            reason,
            signer: signer.clone(),
            bundle_hash: bundle_hash.clone(),
        },
    );
}

fn emit_verified(env: &Env, signer: &BytesN<32>, bundle_hash: &BytesN<32>) {
    env.events().publish(
        (symbol_short!("AGUARD"), symbol_short!("VERIFY")),
        VerifiedEvent {
            signer: signer.clone(),
            bundle_hash: bundle_hash.clone(),
        },
    );
}

fn emit_signer_event(env: &Env, reason: u32, signer: &BytesN<32>) {
    env.events().publish(
        (symbol_short!("AGUARD"), symbol_short!("SIGNER")),
        SignerEvent {
            reason,
            signer: signer.clone(),
        },
    );
}

#[cfg(test)]
mod test;
