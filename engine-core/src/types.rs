use soroban_sdk::{contracttype, contracterror, Address, BytesN, Map, Symbol, Val};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub enum TreasuryError {
    InvalidBalance = 1,
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub enum BurnError {
    ZeroAddress = 1,
}

/// Proposal lifecycle states — stored as u32 bitmask-friendly variant.
#[contracttype]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ProposalState {
    Pending  = 0,
    Approved = 1,
    Executed = 2,
    Expired = 3,
    Cancelled = 4,
}

#[contracttype]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum BreakerState {
    Closed = 0,
    Open   = 1,
}

/// What action triggered a treasury snapshot.
/// Encoded as u32 to avoid heap-allocated `String` fields.
#[contracttype]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum TriggerKind {
    Deposit           = 0,
    Withdrawal        = 1,
    ProposalExecuted  = 2,
    GovernanceUpdate  = 3,
    Manual            = 4,
    BurnSafe          = 5,
    RecoveryExecuted  = 6,
    Other             = 7,
}

/// State commitment submitted by off-chain ZK provers.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StateCommitment {
    pub sequence:   u64,
    pub state_hash: BytesN<32>,
    /// Ledger sequence at which this commitment was submitted.
    pub ledger:     u32,
    /// Address of the off-chain prover that submitted this commitment.
    pub author:     Address,
}

/// Compact governance proposal.
///
/// `approved_by` retains the full `Vec<Address>` so that Soroban auth
/// can validate unique signers.  The `state` field now uses the typed enum.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Proposal {
    pub id:              u64,
    pub proposer:        Address,
    pub action_hash:     BytesN<32>,
    /// Accumulated approvals — bounded by the signer set size.
    pub approved_by:     soroban_sdk::Vec<Address>,
    pub state:           ProposalState,
}

/// Treasury snapshot — compact representation for audit history.
///
/// Replaced heap-allocated `String` fields with:
/// - `timestamp_ledger: u32`  (ledger seq at record time — already available)
/// - `timestamp_unix: u64`    (UNIX timestamp from `env.ledger().timestamp()`)
/// - `trigger: TriggerKind`   (enum instead of freeform String)
///
/// `context` is kept as a `Map<Symbol,Val>` for extensibility but callers
/// should pass minimal maps to limit storage cost.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TreasurySnapshot {
    pub id:               u64,
    pub total_balance:    i128,
    pub account_count:    u32,
    /// Ledger sequence at snapshot time.
    pub ledger:           u32,
    /// UNIX timestamp at snapshot time (seconds since epoch).
    pub timestamp_unix:   u64,
    pub state_hash:       BytesN<32>,
    /// Trigger classification — replaces the freeform `triggered_by: String`.
    pub trigger:          TriggerKind,
    /// Optional small context map. Prefer empty maps when not needed.
    pub context:          Map<Symbol, Val>,
}
