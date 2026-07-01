//! Example: Deterministic proposal hashing for multi-sig consensus
//!
//! This example demonstrates how to use the canonical serializer to ensure
//! all signers calculate the same hash for a proposal.

use vero_engine::serialization::{CanonicalSerializer, Proposal, ProposalState, Transaction};
use serde_json::json;

fn main() {
    println!("=== Deterministic Proposal Hashing Example ===\n");

    // Example 1: Basic proposal hashing
    basic_proposal_hashing();

    // Example 2: Multi-sig consensus scenario
    multisig_consensus();

    // Example 3: Transaction hashing
    transaction_hashing();

    // Example 4: Custom data hashing
    custom_data_hashing();

    // Example 5: Key ordering demonstration
    key_ordering_demo();
}

fn basic_proposal_hashing() {
    println!("1. Basic Proposal Hashing");
    println!("   ----------------------");

    let proposal = Proposal {
        id: 42,
        action: "transfer_funds".to_string(),
        proposer: "GABC123...XYZ".to_string(),
        approved_by: vec![
            "GDEF456...ABC".to_string(),
            "GHIJ789...DEF".to_string(),
        ],
        state: ProposalState::Pending,
        created_at: 1700000000,
        expires_at: 1700086400,
        metadata: None,
    };

    match CanonicalSerializer::hash_proposal(&proposal) {
        Ok(hash) => {
            let hex = CanonicalSerializer::hash_to_hex(&hash);
            println!("   Proposal ID: {}", proposal.id);
            println!("   Action: {}", proposal.action);
            println!("   Hash: {}", hex);
            println!("   ✓ Hash generated successfully\n");
        }
        Err(e) => {
            println!("   ✗ Error: {}\n", e);
        }
    }
}

fn multisig_consensus() {
    println!("2. Multi-Sig Consensus Scenario");
    println!("   -----------------------------");

    // Simulate 3 different signers creating the same proposal
    // (but potentially with different JSON key ordering internally)

    println!("   Signer 1: Creating proposal...");
    let signer1_proposal = Proposal {
        id: 100,
        action: "upgrade_contract".to_string(),
        proposer: "GSIGNER1...ABC".to_string(),
        approved_by: vec![],
        state: ProposalState::Pending,
        created_at: 1700000000,
        expires_at: 1700086400,
        metadata: None,
    };
    let hash1 = CanonicalSerializer::hash_proposal(&signer1_proposal)
        .expect("Failed to hash signer1 proposal");

    println!("   Signer 2: Creating same proposal...");
    let signer2_proposal = Proposal {
        id: 100,
        action: "upgrade_contract".to_string(),
        proposer: "GSIGNER1...ABC".to_string(),
        approved_by: vec![],
        state: ProposalState::Pending,
        created_at: 1700000000,
        expires_at: 1700086400,
        metadata: None,
    };
    let hash2 = CanonicalSerializer::hash_proposal(&signer2_proposal)
        .expect("Failed to hash signer2 proposal");

    println!("   Signer 3: Creating same proposal...");
    let signer3_proposal = Proposal {
        id: 100,
        action: "upgrade_contract".to_string(),
        proposer: "GSIGNER1...ABC".to_string(),
        approved_by: vec![],
        state: ProposalState::Pending,
        created_at: 1700000000,
        expires_at: 1700086400,
        metadata: None,
    };
    let hash3 = CanonicalSerializer::hash_proposal(&signer3_proposal)
        .expect("Failed to hash signer3 proposal");

    // Verify all hashes match
    if hash1 == hash2 && hash2 == hash3 {
        println!("   ✓ All signers produced identical hash!");
        println!("   Hash: {}", CanonicalSerializer::hash_to_hex(&hash1));
        println!("   ✓ Multi-sig consensus achieved\n");
    } else {
        println!("   ✗ Hashes don't match - consensus failed!\n");
    }
}

fn transaction_hashing() {
    println!("3. Transaction Hashing");
    println!("   -------------------");

    let tx = Transaction {
        from: "GSENDER...XYZ".to_string(),
        to: "GRECIPIENT...ABC".to_string(),
        amount: "1000.5000000".to_string(), // Use string for precision
        nonce: 12345,
        timestamp: 1700000000,
        data: None,
    };

    match CanonicalSerializer::hash_transaction(&tx) {
        Ok(hash) => {
            let hex = CanonicalSerializer::hash_to_hex(&hash);
            println!("   From: {}", tx.from);
            println!("   To: {}", tx.to);
            println!("   Amount: {} XLM", tx.amount);
            println!("   Hash: {}", hex);
            println!("   ✓ Transaction hash generated\n");
        }
        Err(e) => {
            println!("   ✗ Error: {}\n", e);
        }
    }
}

fn custom_data_hashing() {
    println!("4. Custom Data Hashing");
    println!("   -------------------");

    let data = json!({
        "protocol_version": 1,
        "network": "testnet",
        "upgrade_params": {
            "new_fee": 100,
            "activation_ledger": 1000000
        }
    });

    match CanonicalSerializer::hash_data(&data) {
        Ok(hash) => {
            let hex = CanonicalSerializer::hash_to_hex(&hash);
            println!("   Data: {}", serde_json::to_string_pretty(&data).unwrap());
            println!("   Hash: {}", hex);
            println!("   ✓ Custom data hashed successfully\n");
        }
        Err(e) => {
            println!("   ✗ Error: {}\n", e);
        }
    }
}

fn key_ordering_demo() {
    println!("5. Key Ordering Independence");
    println!("   -------------------------");

    // Same data with different key orders
    let data1 = json!({
        "zebra": 3,
        "apple": 1,
        "middle": 2,
        "nested": {
            "z_key": "last",
            "a_key": "first"
        }
    });

    let data2 = json!({
        "apple": 1,
        "middle": 2,
        "zebra": 3,
        "nested": {
            "a_key": "first",
            "z_key": "last"
        }
    });

    let hash1 = CanonicalSerializer::hash_data(&data1)
        .expect("Failed to hash data1");
    let hash2 = CanonicalSerializer::hash_data(&data2)
        .expect("Failed to hash data2");

    println!("   Original order: [zebra, apple, middle, ...]");
    println!("   Hash 1: {}", CanonicalSerializer::hash_to_hex(&hash1));
    println!();
    println!("   Different order: [apple, middle, zebra, ...]");
    println!("   Hash 2: {}", CanonicalSerializer::hash_to_hex(&hash2));
    println!();

    if hash1 == hash2 {
        println!("   ✓ Hashes match despite different key ordering!");
        println!("   ✓ Deterministic serialization working correctly\n");
    } else {
        println!("   ✗ Hashes don't match - unexpected behavior!\n");
    }
}
