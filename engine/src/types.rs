use serde::{Deserialize, Serialize};
use std::time::Duration;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum RpcError {
    #[error("All RPC providers unavailable")]
    AllProvidersDown,
    
    #[error("Network error: {0}")]
    NetworkError(String),
    
    #[error("Request timeout after {0:?}")]
    Timeout(Duration),
    
    #[error("Provider authentication failed: {0}")]
    AuthenticationFailed(String),
    
    #[error("Invalid provider configuration: {0}")]
    InvalidConfig(String),
    
    #[error("Rate limit exceeded for provider: {0}")]
    RateLimitExceeded(String),
    
    #[error("RPC method error: {0}")]
    MethodError(String),
    
    #[error("Serialization error: {0}")]
    SerializationError(String),
}

pub type RpcResult<T> = Result<T, RpcError>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderHealth {
    pub url: String,
    pub weight: u32,
    pub success_count: u64,
    pub failure_count: u64,
    pub avg_latency_ms: u64,
    pub last_check: chrono::DateTime<chrono::Utc>,
    pub is_healthy: bool,
    pub quarantine_until: Option<chrono::DateTime<chrono::Utc>>,
}

impl ProviderHealth {
    pub fn new(url: String, weight: u32) -> Self {
        Self {
            url,
            weight,
            success_count: 0,
            failure_count: 0,
            avg_latency_ms: 0,
            last_check: chrono::Utc::now(),
            is_healthy: true,
            quarantine_until: None,
        }
    }

    pub fn success_rate(&self) -> f64 {
        let total = self.success_count + self.failure_count;
        if total == 0 {
            return 1.0;
        }
        self.success_count as f64 / total as f64
    }

    pub fn is_quarantined(&self) -> bool {
        if let Some(until) = self.quarantine_until {
            chrono::Utc::now() < until
        } else {
            false
        }
    }

    pub fn effective_weight(&self) -> f64 {
        if self.is_quarantined() || !self.is_healthy {
            return 0.0;
        }
        
        let success_rate = self.success_rate();
        let latency_factor = 1.0 / (1.0 + (self.avg_latency_ms as f64 / 1000.0));
        
        self.weight as f64 * success_rate * latency_factor
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcRequest {
    pub jsonrpc: String,
    pub id: u64,
    pub method: String,
    pub params: serde_json::Value,
}

impl RpcRequest {
    pub fn new(method: impl Into<String>, params: serde_json::Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id: rand::random(),
            method: method.into(),
            params,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcResponse {
    pub jsonrpc: String,
    pub id: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<RpcResponseError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcResponseError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}
