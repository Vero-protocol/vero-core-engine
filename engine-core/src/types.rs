use soroban_sdk::{contracttype, Address, BytesN};

/// Canonical state snapshot committed to a ZK audit cycle.
#[contracttype]
#[derive(Clone, Debug)]
pub struct StateCommitment {
    /// SHA-256 of serialised state payload (32 bytes).
    pub state_hash: BytesN<32>,
    /// Sequence number — monotonically increasing, prevents replay.
    pub sequence:   u64,
    /// Ledger at which this commitment was recorded.
    pub ledger:     u32,
    /// Signer that produced this commitment.
    pub author:     Address,
}

/// Treasury snapshot for audit history — captures state at point in time.
/// Recorded after every state-changing operation (deposit, withdrawal, governance action).
#[contracttype]
#[derive(Clone, Debug)]
pub struct TreasurySnapshot {
    /// Unique snapshot ID (incremental, monotonic)
    pub id: u64,
    /// Total treasury balance at snapshot time
    pub total_balance: i128,
    /// Number of accounts in treasury
    pub account_count: u32,
    /// Ledger sequence at snapshot time
    pub ledger: u32,
    /// Timestamp (ISO 8601 string)
    pub timestamp: soroban_sdk::String,
    /// Hash of snapshot data (SHA-256, 32 bytes) for integrity verification
    pub state_hash: BytesN<32>,
    /// Operation that triggered snapshot (e.g., "deposit", "withdrawal", "proposal_executed")
    pub triggered_by: soroban_sdk::String,
    /// Optional context data (e.g., {"proposal_id": "42", "amount": "1000"})
    pub context: soroban_sdk::Map<soroban_sdk::Symbol, soroban_sdk::Val>,
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
    pub id:          u64,
    pub action_hash: BytesN<32>,
    pub proposer:    Address,
    pub approved_by: soroban_sdk::Vec<Address>,
    pub state:       ProposalState,
}

/// Circuit-breaker state persisted in contract storage.
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum BreakerState {
    Closed,  // normal operation
    Open,    // halted — no state transitions allowed
}
