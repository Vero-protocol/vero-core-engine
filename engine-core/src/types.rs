use soroban_sdk::{contracttype, Address, BytesN, Map, Symbol, Val};

/// Canonical state commitment submitted by an off-chain ZK/audit worker.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StateCommitment {
    /// Strictly increasing commitment sequence.
    pub sequence: u64,
    /// Hash over `(previous_hash || sequence || payload)`.
    pub state_hash: BytesN<32>,
    /// Ledger sequence at which the transition was observed/submitted.
    pub ledger: u32,
    /// Authenticated prover/auditor submitting the commitment.
    pub author: Address,
}

/// Governance proposal lifecycle.
#[contracttype]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ProposalState {
    Pending = 0,
    Approved = 1,
    Executed = 2,
    Expired = 3,
    Cancelled = 4,
}

/// Circuit-breaker state.
#[contracttype]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum BreakerState {
    Closed = 0,
    Open = 1,
}

/// What action triggered a treasury snapshot.
#[contracttype]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum TriggerKind {
    Deposit = 0,
    Withdrawal = 1,
    ProposalExecuted = 2,
    GovernanceUpdate = 3,
    Manual = 4,
    BurnSafe = 5,
    RecoveryExecuted = 6,
    Other = 7,
}

/// Compact treasury snapshot for integrity checks and off-chain audit trails.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TreasurySnapshot {
    pub id: u64,
    pub total_balance: i128,
    pub account_count: u32,
    pub ledger: u32,
    pub timestamp_unix: u64,
    pub state_hash: BytesN<32>,
    pub trigger: TriggerKind,
    pub context: Map<Symbol, Val>,
}

/// Compact governance proposal.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Proposal {
    pub id: u64,
    pub action_hash: BytesN<32>,
    pub proposer: Address,
    pub approved_by: soroban_sdk::Vec<Address>,
    pub state: ProposalState,
}
