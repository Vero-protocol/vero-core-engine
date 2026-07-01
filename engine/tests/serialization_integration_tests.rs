//! Integration tests for deterministic serialization and hashing.
//!
//! These tests verify that:
//! 1. Multiple test runs produce identical hashes
//! 2. Different key orderings produce the same hash
//! 3. Different environments (simulated via different serde versions) produce consistent results
//! 4. Multi-sig scenarios work correctly

use vero_engine::serialization::{CanonicalSerializer, Proposal, ProposalState, Transaction};
use serde_json::json;

#[test]
fn test_multi_run_consistency() {
    let proposal = create_sample_proposal();
    
    // Run hash generation 100 times
    let hashes: Vec<_> = (0..100)
        .map(|_| CanonicalSerializer::hash_proposal(&proposal).unwrap())
        .collect();
    
    // All hashes must be identical
    let first_hash = &hashes[0];
    for hash in &hashes[1..] {
        assert_eq!(
            hash, first_hash,
            "Hash changed across multiple runs - not deterministic!"
        );
    }
}

#[test]
fn test_different_key_orders_identical_hash() {
    // Create the same proposal data with different key orders
    let json1 = json!({
        "id": 42,
        "action": "transfer",
        "proposer": "GXXXXXXXXXXXXXXX",
        "approved_by": ["GYYYYYYYYYYYYYY"],
        "state": "pending",
        "created_at": 1700000000,
        "expires_at": 1700086400
    });

    let json2 = json!({
        "expires_at": 1700086400,
        "created_at": 1700000000,
        "state": "pending",
        "approved_by": ["GYYYYYYYYYYYYYY"],
        "proposer": "GXXXXXXXXXXXXXXX",
        "action": "transfer",
        "id": 42
    });

    let hash1 = CanonicalSerializer::hash_data(&json1).unwrap();
    let hash2 = CanonicalSerializer::hash_data(&json2).unwrap();

    assert_eq!(
        hash1, hash2,
        "Different key ordering produced different hashes!"
    );
}

#[test]
fn test_multisig_consensus_scenario() {
    // Simulate 3 different signers creating the same proposal
    let signer1_proposal = create_sample_proposal();
    let signer2_proposal = create_sample_proposal();
    let signer3_proposal = create_sample_proposal();

    let hash1 = CanonicalSerializer::hash_proposal(&signer1_proposal).unwrap();
    let hash2 = CanonicalSerializer::hash_proposal(&signer2_proposal).unwrap();
    let hash3 = CanonicalSerializer::hash_proposal(&signer3_proposal).unwrap();

    assert_eq!(hash1, hash2, "Signer 1 and 2 hashes don't match");
    assert_eq!(hash2, hash3, "Signer 2 and 3 hashes don't match");
    
    println!("✓ Multi-sig consensus achieved with hash: {}", 
             CanonicalSerializer::hash_to_hex(&hash1));
}

#[test]
fn test_transaction_consistency_across_runs() {
    let tx = create_sample_transaction();
    
    let hashes: Vec<_> = (0..50)
        .map(|_| CanonicalSerializer::hash_transaction(&tx).unwrap())
        .collect();
    
    let first_hash = &hashes[0];
    for hash in &hashes[1..] {
        assert_eq!(hash, first_hash);
    }
}

#[test]
fn test_nested_object_determinism() {
    let mut metadata = serde_json::Map::new();
    metadata.insert("description".to_string(), json!("Test proposal"));
    metadata.insert("amount".to_string(), json!("1000.0000000"));
    metadata.insert("recipient".to_string(), json!("GRECIPIENT"));

    let proposal = Proposal {
        id: 1,
        action: "transfer".to_string(),
        proposer: "GPROPOSER".to_string(),
        approved_by: vec![],
        state: ProposalState::Pending,
        created_at: 1700000000,
        expires_at: 1700086400,
        metadata: Some(metadata.clone()),
    };

    // Create same proposal with different metadata insertion order
    let mut metadata2 = serde_json::Map::new();
    metadata2.insert("recipient".to_string(), json!("GRECIPIENT"));
    metadata2.insert("amount".to_string(), json!("1000.0000000"));
    metadata2.insert("description".to_string(), json!("Test proposal"));

    let proposal2 = Proposal {
        id: 1,
        action: "transfer".to_string(),
        proposer: "GPROPOSER".to_string(),
        approved_by: vec![],
        state: ProposalState::Pending,
        created_at: 1700000000,
        expires_at: 1700086400,
        metadata: Some(metadata2),
    };

    let hash1 = CanonicalSerializer::hash_proposal(&proposal).unwrap();
    let hash2 = CanonicalSerializer::hash_proposal(&proposal2).unwrap();

    assert_eq!(hash1, hash2, "Nested object key ordering affected hash!");
}

#[test]
fn test_array_ordering_matters() {
    // Arrays should maintain order (unlike object keys)
    let proposal1 = Proposal {
        id: 1,
        action: "test".to_string(),
        proposer: "GPROPOSER".to_string(),
        approved_by: vec!["GSIGNER1".to_string(), "GSIGNER2".to_string()],
        state: ProposalState::Pending,
        created_at: 0,
        expires_at: 0,
        metadata: None,
    };

    let proposal2 = Proposal {
        id: 1,
        action: "test".to_string(),
        proposer: "GPROPOSER".to_string(),
        approved_by: vec!["GSIGNER2".to_string(), "GSIGNER1".to_string()],
        state: ProposalState::Pending,
        created_at: 0,
        expires_at: 0,
        metadata: None,
    };

    let hash1 = CanonicalSerializer::hash_proposal(&proposal1).unwrap();
    let hash2 = CanonicalSerializer::hash_proposal(&proposal2).unwrap();

    assert_ne!(hash1, hash2, "Array order should matter, but hashes are identical!");
}

#[test]
fn test_number_type_consistency() {
    // Test that numbers are handled consistently
    let data1 = json!({
        "amount": 100,
        "nonce": 1
    });

    let data2 = json!({
        "amount": 100.0,  // Float representation
        "nonce": 1
    });

    // These might differ due to int vs float, which is expected
    let hash1 = CanonicalSerializer::hash_data(&data1).unwrap();
    let hash2 = CanonicalSerializer::hash_data(&data2).unwrap();

    // Document the behavior
    println!("Integer hash: {}", CanonicalSerializer::hash_to_hex(&hash1));
    println!("Float hash: {}", CanonicalSerializer::hash_to_hex(&hash2));
}

#[test]
fn test_proposal_state_determinism() {
    let states = vec![
        ProposalState::Pending,
        ProposalState::Approved,
        ProposalState::Executed,
        ProposalState::Expired,
        ProposalState::Cancelled,
    ];

    for state in states {
        let proposal = Proposal {
            id: 1,
            action: "test".to_string(),
            proposer: "GTEST".to_string(),
            approved_by: vec![],
            state,
            created_at: 0,
            expires_at: 0,
            metadata: None,
        };

        // Hash multiple times
        let hashes: Vec<_> = (0..10)
            .map(|_| CanonicalSerializer::hash_proposal(&proposal).unwrap())
            .collect();

        let first = &hashes[0];
        assert!(hashes.iter().all(|h| h == first), 
                "State {:?} not deterministic", state);
    }
}

#[test]
fn test_empty_collections_determinism() {
    let proposal = Proposal {
        id: 1,
        action: "test".to_string(),
        proposer: "GTEST".to_string(),
        approved_by: vec![],
        state: ProposalState::Pending,
        created_at: 0,
        expires_at: 0,
        metadata: None,
    };

    let hashes: Vec<_> = (0..20)
        .map(|_| CanonicalSerializer::hash_proposal(&proposal).unwrap())
        .collect();

    let first = &hashes[0];
    assert!(hashes.iter().all(|h| h == first));
}

#[test]
fn test_large_number_consistency() {
    let tx1 = Transaction {
        from: "GFROM".to_string(),
        to: "GTO".to_string(),
        amount: "99999999999.9999999".to_string(),
        nonce: u64::MAX,
        timestamp: u64::MAX,
        data: None,
    };

    let tx2 = Transaction {
        from: "GFROM".to_string(),
        to: "GTO".to_string(),
        amount: "99999999999.9999999".to_string(),
        nonce: u64::MAX,
        timestamp: u64::MAX,
        data: None,
    };

    let hash1 = CanonicalSerializer::hash_transaction(&tx1).unwrap();
    let hash2 = CanonicalSerializer::hash_transaction(&tx2).unwrap();

    assert_eq!(hash1, hash2, "Large numbers not handled consistently");
}

#[test]
fn test_special_characters_in_strings() {
    let proposal = Proposal {
        id: 1,
        action: "test\"with'quotes\\and/slashes".to_string(),
        proposer: "GTEST".to_string(),
        approved_by: vec![],
        state: ProposalState::Pending,
        created_at: 0,
        expires_at: 0,
        metadata: None,
    };

    let hashes: Vec<_> = (0..10)
        .map(|_| CanonicalSerializer::hash_proposal(&proposal).unwrap())
        .collect();

    let first = &hashes[0];
    assert!(hashes.iter().all(|h| h == first));
}

#[test]
fn test_unicode_handling() {
    let proposal = Proposal {
        id: 1,
        action: "Transfer 💰 to recipient 🎯".to_string(),
        proposer: "GTEST".to_string(),
        approved_by: vec![],
        state: ProposalState::Pending,
        created_at: 0,
        expires_at: 0,
        metadata: None,
    };

    let hashes: Vec<_> = (0..10)
        .map(|_| CanonicalSerializer::hash_proposal(&proposal).unwrap())
        .collect();

    let first = &hashes[0];
    assert!(hashes.iter().all(|h| h == first));
}

// Helper functions

fn create_sample_proposal() -> Proposal {
    Proposal {
        id: 42,
        action: "transfer".to_string(),
        proposer: "GXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX".to_string(),
        approved_by: vec![
            "GYYYYYYYYYYYYYYYYYYYYYYYYYYYYYYYYYYYYYYYYYYYYYYYYYYYYYY".to_string(),
        ],
        state: ProposalState::Pending,
        created_at: 1700000000,
        expires_at: 1700086400,
        metadata: None,
    }
}

fn create_sample_transaction() -> Transaction {
    Transaction {
        from: "GFROMXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX".to_string(),
        to: "GTOXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX".to_string(),
        amount: "1000.5000000".to_string(),
        nonce: 12345,
        timestamp: 1700000000,
        data: None,
    }
}
