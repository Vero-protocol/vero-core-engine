//! Canonical serialization for deterministic proposal hashing.
//!
//! This module provides deterministic JSON serialization that ensures:
//! - Keys are sorted alphabetically
//! - Numbers are cast to standard types
//! - Addresses are normalized
//! - Same proposal data always produces the same hash
//!
//! This is critical for multi-sig consensus where different clients must
//! calculate identical hashes for the same proposal.

use crate::types::{RpcError, RpcResult};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::BTreeMap;

/// A proposal that needs deterministic serialization for hashing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Proposal {
    pub id: u64,
    pub action: String,
    pub proposer: String,
    pub approved_by: Vec<String>,
    pub state: ProposalState,
    pub created_at: u64,
    pub expires_at: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Map<String, Value>>,
}

/// Proposal lifecycle states.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProposalState {
    Pending,
    Approved,
    Executed,
    Expired,
    Cancelled,
}

/// Transaction data that requires deterministic serialization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    pub from: String,
    pub to: String,
    pub amount: String,  // Use string to avoid float precision issues
    pub nonce: u64,
    pub timestamp: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Map<String, Value>>,
}

/// Canonical JSON serializer that enforces deterministic output.
pub struct CanonicalSerializer;

impl CanonicalSerializer {
    /// Serialize any value to canonical JSON with sorted keys.
    ///
    /// # Examples
    ///
    /// ```
    /// use vero_engine::serialization::CanonicalSerializer;
    /// use serde_json::json;
    ///
    /// let data = json!({
    ///     "z_field": 3,
    ///     "a_field": 1,
    ///     "m_field": 2
    /// });
    ///
    /// let canonical = CanonicalSerializer::serialize(&data).unwrap();
    /// // Keys will be alphabetically sorted: a_field, m_field, z_field
    /// ```
    pub fn serialize<T: Serialize>(value: &T) -> RpcResult<String> {
        let json_value = serde_json::to_value(value)
            .map_err(|e| RpcError::SerializationError(e.to_string()))?;
        
        let canonical_value = Self::canonicalize_value(json_value)?;
        
        serde_json::to_string(&canonical_value)
            .map_err(|e| RpcError::SerializationError(e.to_string()))
    }

    /// Serialize and compute SHA-256 hash for proposals.
    ///
    /// This ensures that identical proposals always produce the same hash,
    /// regardless of the original key ordering or client library version.
    pub fn hash_proposal(proposal: &Proposal) -> RpcResult<Vec<u8>> {
        let canonical_json = Self::serialize(proposal)?;
        Ok(Self::sha256(canonical_json.as_bytes()))
    }

    /// Serialize and compute SHA-256 hash for transactions.
    pub fn hash_transaction(transaction: &Transaction) -> RpcResult<Vec<u8>> {
        let canonical_json = Self::serialize(transaction)?;
        Ok(Self::sha256(canonical_json.as_bytes()))
    }

    /// Compute SHA-256 hash of arbitrary serializable data.
    pub fn hash_data<T: Serialize>(data: &T) -> RpcResult<Vec<u8>> {
        let canonical_json = Self::serialize(data)?;
        Ok(Self::sha256(canonical_json.as_bytes()))
    }

    /// Canonicalize a JSON value recursively, sorting all object keys.
    fn canonicalize_value(value: Value) -> RpcResult<Value> {
        match value {
            Value::Object(map) => {
                // Use BTreeMap for automatic alphabetical key ordering
                let mut sorted_map = BTreeMap::new();
                
                for (key, val) in map {
                    let canonical_val = Self::canonicalize_value(val)?;
                    sorted_map.insert(key, canonical_val);
                }
                
                // Convert BTreeMap back to serde_json::Map while preserving order
                let mut result_map = Map::new();
                for (key, val) in sorted_map {
                    result_map.insert(key, val);
                }
                
                Ok(Value::Object(result_map))
            }
            Value::Array(arr) => {
                let canonical_arr: Result<Vec<_>, _> = arr
                    .into_iter()
                    .map(|v| Self::canonicalize_value(v))
                    .collect();
                Ok(Value::Array(canonical_arr?))
            }
            Value::Number(n) => {
                // Normalize numbers to avoid precision issues
                if let Some(i) = n.as_i64() {
                    Ok(Value::Number(i.into()))
                } else if let Some(u) = n.as_u64() {
                    Ok(Value::Number(u.into()))
                } else if let Some(f) = n.as_f64() {
                    // For floats, use fixed precision to ensure determinism
                    Ok(Value::String(format!("{:.8}", f)))
                } else {
                    Ok(Value::Number(n))
                }
            }
            // Strings, bools, and null are already canonical
            other => Ok(other),
        }
    }

    /// Compute SHA-256 hash using ring.
    fn sha256(data: &[u8]) -> Vec<u8> {
        use ring::digest::{Context, SHA256};
        
        let mut context = Context::new(&SHA256);
        context.update(data);
        context.finish().as_ref().to_vec()
    }

    /// Convert hash bytes to hex string for display.
    pub fn hash_to_hex(hash: &[u8]) -> String {
        hash.iter()
            .map(|b| format!("{:02x}", b))
            .collect::<String>()
    }

    /// Normalize an address string to a canonical format.
    ///
    /// Removes leading/trailing whitespace and converts to lowercase
    /// for Stellar addresses or maintains case sensitivity for others.
    pub fn normalize_address(address: &str) -> String {
        let trimmed = address.trim();
        
        // Stellar addresses start with 'G' or 'S' and are case-sensitive
        // Ethereum addresses start with '0x' and should be checksummed
        // For now, we'll preserve case but this can be extended
        trimmed.to_string()
    }

    /// Normalize a numeric amount string to avoid precision issues.
    ///
    /// Converts to a fixed decimal precision string representation.
    pub fn normalize_amount(amount: &str) -> RpcResult<String> {
        let parsed: f64 = amount.parse()
            .map_err(|_| RpcError::SerializationError(
                format!("Invalid amount format: {}", amount)
            ))?;
        
        // Use 7 decimal places (Stellar's standard precision)
        Ok(format!("{:.7}", parsed))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_key_ordering() {
        let data = json!({
            "zebra": 3,
            "apple": 1,
            "middle": 2
        });

        let canonical = CanonicalSerializer::serialize(&data).unwrap();
        
        // Keys should be in alphabetical order
        assert!(canonical.find("apple").unwrap() < canonical.find("middle").unwrap());
        assert!(canonical.find("middle").unwrap() < canonical.find("zebra").unwrap());
    }

    #[test]
    fn test_nested_object_ordering() {
        let data = json!({
            "outer_z": {
                "inner_z": 1,
                "inner_a": 2
            },
            "outer_a": {
                "inner_z": 3,
                "inner_a": 4
            }
        });

        let canonical = CanonicalSerializer::serialize(&data).unwrap();
        
        // Outer keys should be sorted
        assert!(canonical.find("outer_a").unwrap() < canonical.find("outer_z").unwrap());
    }

    #[test]
    fn test_proposal_deterministic_hash() {
        let proposal1 = Proposal {
            id: 42,
            action: "transfer".to_string(),
            proposer: "GXXXXXXXXXXXXXXX".to_string(),
            approved_by: vec!["GYYYYYYYYYYYYYY".to_string()],
            state: ProposalState::Pending,
            created_at: 1700000000,
            expires_at: 1700086400,
            metadata: None,
        };

        let proposal2 = Proposal {
            id: 42,
            action: "transfer".to_string(),
            proposer: "GXXXXXXXXXXXXXXX".to_string(),
            approved_by: vec!["GYYYYYYYYYYYYYY".to_string()],
            state: ProposalState::Pending,
            created_at: 1700000000,
            expires_at: 1700086400,
            metadata: None,
        };

        let hash1 = CanonicalSerializer::hash_proposal(&proposal1).unwrap();
        let hash2 = CanonicalSerializer::hash_proposal(&proposal2).unwrap();

        assert_eq!(hash1, hash2, "Identical proposals must produce identical hashes");
    }

    #[test]
    fn test_different_key_order_same_hash() {
        let data1 = json!({
            "z": 3,
            "a": 1,
            "m": 2
        });

        let data2 = json!({
            "a": 1,
            "m": 2,
            "z": 3
        });

        let hash1 = CanonicalSerializer::hash_data(&data1).unwrap();
        let hash2 = CanonicalSerializer::hash_data(&data2).unwrap();

        assert_eq!(hash1, hash2, "Different key ordering must produce same hash");
    }

    #[test]
    fn test_hash_to_hex() {
        let hash = vec![0xde, 0xad, 0xbe, 0xef];
        let hex = CanonicalSerializer::hash_to_hex(&hash);
        assert_eq!(hex, "deadbeef");
    }

    #[test]
    fn test_normalize_address() {
        let address = "  GXXXXXXXXXXXXXXX  ";
        let normalized = CanonicalSerializer::normalize_address(address);
        assert_eq!(normalized, "GXXXXXXXXXXXXXXX");
    }

    #[test]
    fn test_normalize_amount() {
        let amount = "100.5";
        let normalized = CanonicalSerializer::normalize_amount(amount).unwrap();
        assert_eq!(normalized, "100.5000000");
    }

    #[test]
    fn test_number_normalization() {
        let data = json!({
            "integer": 42,
            "float": 3.14159265359
        });

        let canonical = CanonicalSerializer::serialize(&data).unwrap();
        
        // Float should be converted to string with fixed precision
        assert!(canonical.contains("\"3.14159265\""));
    }

    #[test]
    fn test_array_ordering_preserved() {
        let data = json!({
            "items": [3, 1, 2]
        });

        let canonical = CanonicalSerializer::serialize(&data).unwrap();
        
        // Array order should be preserved (only object keys are sorted)
        assert!(canonical.contains("[3,1,2]"));
    }

    #[test]
    fn test_proposal_state_serialization() {
        let proposal = Proposal {
            id: 1,
            action: "test".to_string(),
            proposer: "GTEST".to_string(),
            approved_by: vec![],
            state: ProposalState::Approved,
            created_at: 0,
            expires_at: 0,
            metadata: None,
        };

        let canonical = CanonicalSerializer::serialize(&proposal).unwrap();
        
        // State should be lowercase
        assert!(canonical.contains("\"approved\""));
    }

    #[test]
    fn test_transaction_deterministic_hash() {
        let tx1 = Transaction {
            from: "GFROM".to_string(),
            to: "GTO".to_string(),
            amount: "100.5".to_string(),
            nonce: 1,
            timestamp: 1700000000,
            data: None,
        };

        let tx2 = Transaction {
            from: "GFROM".to_string(),
            to: "GTO".to_string(),
            amount: "100.5".to_string(),
            nonce: 1,
            timestamp: 1700000000,
            data: None,
        };

        let hash1 = CanonicalSerializer::hash_transaction(&tx1).unwrap();
        let hash2 = CanonicalSerializer::hash_transaction(&tx2).unwrap();

        assert_eq!(hash1, hash2);
    }
}
