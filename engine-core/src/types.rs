use soroban_sdk::{contracttype, contracterror};

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

#[contracttype]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ProposalState {
    Pending = 0,
    Approved = 1,
    Executed = 2,
    Expired = 3,
}

#[contracttype]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum BreakerState {
    Closed = 0,
    Open = 1,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StateCommitment {
    pub sequence: u64,
    pub state_hash: soroban_sdk::BytesN<32>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Proposal {
    pub id: u64,
    pub proposer: soroban_sdk::Address,
    pub action_hash: soroban_sdk::BytesN<32>,
    pub approved_by: soroban_sdk::Vec<soroban_sdk::Address>,
    pub state: u32, 
    pub voting_deadline: u32, // Absolute ledger sequence where voting window closes
}
