#![no_std]

extern crate alloc;


pub mod audit;
pub mod burn;
pub mod circuit_breaker;
pub mod core;
pub mod emergency_recovery;
pub mod event_struct;
pub mod event_utils;
pub mod governance;
pub mod guards;
pub mod protocol_fee;
pub mod treasury;
pub mod types;
pub mod upgrade;
pub mod version;

#[cfg(test)]
mod governance_tests;
#[cfg(test)]
mod reentrancy_tests;
#[cfg(test)]
mod treasury_tests;
