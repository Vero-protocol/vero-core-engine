//! Contract versioning — tracks the engine-core logic version.
//!
//! Exposes a single source of truth for the deployed contract's logic version
//! so off-chain tooling and on-chain callers can detect version mismatches.
//! The constant is pinned to the crate version declared in `Cargo.toml`.

/// Current engine-core logic version (matches the crate's `Cargo.toml` version).
pub const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Return the current contract logic version.
pub fn version() -> &'static str {
    CONTRACT_VERSION
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returns_crate_version() {
        assert_eq!(version(), env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn version_is_non_empty() {
        assert!(!version().is_empty());
    }
}
