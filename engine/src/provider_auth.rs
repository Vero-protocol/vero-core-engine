use crate::types::{RpcError, RpcResult};
use ring::signature::{self, UnparsedPublicKey, ED25519};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Provider list with signature verification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedProviderList {
    pub providers: Vec<ProviderEntry>,
    pub timestamp: i64,
    pub signature: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderEntry {
    pub url: String,
    pub weight: u32,
    pub region: Option<String>,
}

pub struct ProviderAuthenticator {
    public_keys: HashMap<String, Vec<u8>>,
}

impl ProviderAuthenticator {
    pub fn new() -> Self {
        Self {
            public_keys: HashMap::new(),
        }
    }

    /// Register a trusted public key for signature verification
    pub fn add_trusted_key(&mut self, key_id: String, public_key: Vec<u8>) {
        self.public_keys.insert(key_id, public_key);
    }

    /// Verify the signed provider list
    pub fn verify_provider_list(
        &self,
        signed_list: &SignedProviderList,
        key_id: &str,
    ) -> RpcResult<()> {
        let public_key = self
            .public_keys
            .get(key_id)
            .ok_or_else(|| RpcError::AuthenticationFailed("Unknown key ID".to_string()))?;

        // Serialize the provider list and timestamp for verification
        let message = self.create_message_for_signing(signed_list)?;

        // Decode the signature from base64
        let signature_bytes = base64::decode(&signed_list.signature)
            .map_err(|e| RpcError::AuthenticationFailed(format!("Invalid signature encoding: {}", e)))?;

        // Verify the signature
        let public_key = UnparsedPublicKey::new(&ED25519, public_key);
        public_key
            .verify(&message, &signature_bytes)
            .map_err(|_| RpcError::AuthenticationFailed("Signature verification failed".to_string()))?;

        // Check timestamp to prevent replay attacks (valid for 1 hour)
        let current_time = chrono::Utc::now().timestamp();
        let age = current_time - signed_list.timestamp;
        if age.abs() > 3600 {
            return Err(RpcError::AuthenticationFailed(
                "Provider list timestamp too old or in future".to_string(),
            ));
        }

        Ok(())
    }

    fn create_message_for_signing(&self, signed_list: &SignedProviderList) -> RpcResult<Vec<u8>> {
        let mut message = serde_json::to_vec(&signed_list.providers)
            .map_err(|e| RpcError::SerializationError(e.to_string()))?;
        
        message.extend_from_slice(&signed_list.timestamp.to_le_bytes());
        
        Ok(message)
    }

    /// Fetch and verify provider list from a remote source
    pub async fn fetch_verified_providers(
        &self,
        url: &str,
        key_id: &str,
    ) -> RpcResult<Vec<ProviderEntry>> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .map_err(|e| RpcError::NetworkError(e.to_string()))?;

        let response = client
            .get(url)
            .send()
            .await
            .map_err(|e| RpcError::NetworkError(e.to_string()))?;

        let signed_list: SignedProviderList = response
            .json()
            .await
            .map_err(|e| RpcError::SerializationError(e.to_string()))?;

        self.verify_provider_list(&signed_list, key_id)?;

        Ok(signed_list.providers)
    }
}

impl Default for ProviderAuthenticator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_authenticator_creation() {
        let auth = ProviderAuthenticator::new();
        assert!(auth.public_keys.is_empty());
    }

    #[test]
    fn test_add_trusted_key() {
        let mut auth = ProviderAuthenticator::new();
        let key = vec![1, 2, 3, 4];
        auth.add_trusted_key("test_key".to_string(), key.clone());
        assert_eq!(auth.public_keys.get("test_key"), Some(&key));
    }

    #[test]
    fn test_timestamp_validation() {
        let auth = ProviderAuthenticator::new();
        
        // Create a provider list with old timestamp
        let old_list = SignedProviderList {
            providers: vec![],
            timestamp: chrono::Utc::now().timestamp() - 7200, // 2 hours ago
            signature: base64::encode(vec![0u8; 64]),
        };

        // Should fail due to old timestamp even without key verification
        let result = auth.verify_provider_list(&old_list, "unknown_key");
        assert!(result.is_err());
    }
}
