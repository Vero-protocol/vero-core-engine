use soroban_sdk::{contracttype, Address, BytesN};

/// Canonical state snapshot committed to a ZK audit cycle.
#[contracttype]
#[derive(Clone, Debug)]
pub struct StateCommitment {
    /// SHA-256 of serialised state payload (32 bytes).
    pub state_hash: BytesN<32>,
    /// Sequence number — monotonically increasing, prevents replay.
    pub sequence: u64,
    /// Ledger at which this commitment was recorded.
    pub ledger: u32,
    /// Signer that produced this commitment.
    pub author: Address,
}

/// Proposal state machine for multi-sig governance.
/// Valid transitions: Pending → Approved → Executed
#[contracttype]
#[derive(Clone, Debug, Copy, PartialEq, Eq)]
pub enum ProposalState {
    /// Awaiting approvals (default state at proposal creation)
    Pending = 0,
    /// Threshold met; time-lock window active before execution
    Approved = 1,
    /// Executed; terminal state
    Executed = 2,
}

/// Governance proposal passed to multi-sig hooks.
#[contracttype]
#[derive(Clone, Debug)]
pub struct Proposal {
    pub id: u64,
    pub action_hash: BytesN<32>,
    pub proposer: Address,
    pub approved_by: soroban_sdk::Vec<Address>,
    pub state: ProposalState,
}

/// Circuit-breaker state persisted in contract storage.
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum BreakerState {
    Closed, // normal operation
    Open,   // halted — no state transitions allowed
}
