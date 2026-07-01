use crate::health::HealthMonitor;
use crate::provider_auth::ProviderAuthenticator;
use crate::types::{RpcError, RpcRequest, RpcResponse, RpcResult};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_RETRIES: usize = 3;
const FAILOVER_THRESHOLD_MS: u64 = 2000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcProvider {
    pub url: String,
    pub weight: u32,
}

#[derive(Debug, Clone)]
pub struct RpcConfig {
    pub providers: Vec<RpcProvider>,
    pub timeout: Duration,
    pub max_retries: usize,
    pub failover_threshold_ms: u64,
    pub enable_health_monitoring: bool,
    pub health_check_interval: Duration,
}

impl Default for RpcConfig {
    fn default() -> Self {
        Self {
            providers: Vec::new(),
            timeout: DEFAULT_TIMEOUT,
            max_retries: MAX_RETRIES,
            failover_threshold_ms: FAILOVER_THRESHOLD_MS,
            enable_health_monitoring: true,
            health_check_interval: Duration::from_secs(10),
        }
    }
}

/// High-availability RPC client with intelligent failover
pub struct RpcClient {
    config: Arc<RwLock<RpcConfig>>,
    health_monitor: Arc<HealthMonitor>,
    authenticator: Arc<RwLock<Option<ProviderAuthenticator>>>,
    client: reqwest::Client,
}

impl RpcClient {
    /// Create a new RPC client with the given configuration
    pub async fn new(config: RpcConfig) -> RpcResult<Self> {
        if config.providers.is_empty() {
            return Err(RpcError::InvalidConfig(
                "At least one provider is required".to_string(),
            ));
        }

        let health_monitor = Arc::new(
            HealthMonitor::new().with_interval(config.health_check_interval),
        );

        // Register all providers with the health monitor
        for provider in &config.providers {
            health_monitor
                .register_provider(provider.url.clone(), provider.weight)
                .await;
        }

        let client = reqwest::Client::builder()
            .timeout(config.timeout)
            .build()
            .map_err(|e| RpcError::NetworkError(e.to_string()))?;

        let rpc_client = Self {
            config: Arc::new(RwLock::new(config)),
            health_monitor: Arc::clone(&health_monitor),
            authenticator: Arc::new(RwLock::new(None)),
            client,
        };

        // Start background health monitoring if enabled
        let config_read = rpc_client.config.read().await;
        if config_read.enable_health_monitoring {
            info!("Starting background health monitoring");
            Arc::clone(&health_monitor).start_monitoring();
        }

        Ok(rpc_client)
    }

    /// Set the provider authenticator for verifying signed provider lists
    pub async fn set_authenticator(&self, auth: ProviderAuthenticator) {
        let mut authenticator = self.authenticator.write().await;
        *authenticator = Some(auth);
    }

    /// Update providers from an authenticated source
    pub async fn update_providers_from_source(
        &self,
        source_url: &str,
        key_id: &str,
    ) -> RpcResult<()> {
        let auth = self.authenticator.read().await;
        let authenticator = auth
            .as_ref()
            .ok_or_else(|| RpcError::AuthenticationFailed("No authenticator set".to_string()))?;

        let providers = authenticator
            .fetch_verified_providers(source_url, key_id)
            .await?;

        // Update configuration with new providers
        let mut config = self.config.write().await;
        config.providers = providers
            .into_iter()
            .map(|p| RpcProvider {
                url: p.url.clone(),
                weight: p.weight,
            })
            .collect();

        // Re-register providers with health monitor
        for provider in &config.providers {
            self.health_monitor
                .register_provider(provider.url.clone(), provider.weight)
                .await;
        }

        info!("Updated providers from authenticated source");
        Ok(())
    }

    /// Execute an RPC call with automatic failover
    pub async fn call(
        &self,
        method: impl Into<String>,
        params: serde_json::Value,
    ) -> RpcResult<serde_json::Value> {
        let method = method.into();
        let request = RpcRequest::new(method.clone(), params);

        let config = self.config.read().await;
        let max_retries = config.max_retries;
        drop(config);

        let mut last_error = None;

        for attempt in 0..max_retries {
            match self.try_call_with_failover(&request).await {
                Ok(result) => {
                    if attempt > 0 {
                        info!("RPC call succeeded after {} retries", attempt);
                    }
                    return Ok(result);
                }
                Err(e) => {
                    warn!(
                        "RPC call attempt {} failed for method {}: {:?}",
                        attempt + 1,
                        method,
                        e
                    );
                    last_error = Some(e);
                    
                    // Exponential backoff between retries
                    if attempt < max_retries - 1 {
                        let backoff = Duration::from_millis(100 * 2_u64.pow(attempt as u32));
                        tokio::time::sleep(backoff).await;
                    }
                }
            }
        }

        Err(last_error.unwrap_or(RpcError::AllProvidersDown))
    }

    /// Try to execute the call with automatic failover to other providers
    async fn try_call_with_failover(&self, request: &RpcRequest) -> RpcResult<serde_json::Value> {
        // Get sorted list of providers (best first)
        let providers = self.health_monitor.get_sorted_providers().await;

        if providers.is_empty() {
            error!("No healthy providers available");
            return Err(RpcError::AllProvidersDown);
        }

        let mut last_error = None;

        for (url, weight) in providers {
            debug!(
                "Attempting RPC call to {} (weight: {:.2})",
                url, weight
            );

            match self.execute_request(&url, request).await {
                Ok(result) => {
                    info!("RPC call successful to {}", url);
                    return Ok(result);
                }
                Err(e) => {
                    warn!("Provider {} failed: {:?}", url, e);
                    last_error = Some(e);
                    // Continue to next provider
                }
            }
        }

        Err(last_error.unwrap_or(RpcError::AllProvidersDown))
    }

    /// Execute a single request to a specific provider
    async fn execute_request(
        &self,
        url: &str,
        request: &RpcRequest,
    ) -> RpcResult<serde_json::Value> {
        let start = Instant::now();

        let response = self
            .client
            .post(url)
            .json(request)
            .send()
            .await
            .map_err(|e| {
                self.handle_request_error(url, e);
                RpcError::NetworkError("Request failed".to_string())
            })?;

        let latency = start.elapsed().as_millis() as u64;

        // Check if response is successful
        if !response.status().is_success() {
            let status = response.status();
            self.health_monitor.record_failure(url).await;
            return Err(RpcError::NetworkError(format!("HTTP {}", status)));
        }

        let rpc_response: RpcResponse = response.json().await.map_err(|e| {
            self.health_monitor
                .record_failure(url)
                .then(|| RpcError::SerializationError(e.to_string()));
            RpcError::SerializationError(e.to_string())
        })?;

        // Check for RPC-level errors
        if let Some(error) = rpc_response.error {
            self.health_monitor.record_failure(url).await;
            return Err(RpcError::MethodError(format!(
                "RPC error {}: {}",
                error.code, error.message
            )));
        }

        // Record success
        self.health_monitor.record_success(url, latency).await;

        // Check if we should trigger failover due to high latency
        let config = self.config.read().await;
        if latency > config.failover_threshold_ms {
            warn!(
                "Provider {} exceeded latency threshold: {}ms > {}ms",
                url, latency, config.failover_threshold_ms
            );
        }

        rpc_response
            .result
            .ok_or_else(|| RpcError::MethodError("No result in response".to_string()))
    }

    fn handle_request_error(&self, url: &str, error: reqwest::Error) {
        let url = url.to_string();
        let health_monitor = Arc::clone(&self.health_monitor);
        
        tokio::spawn(async move {
            health_monitor.record_failure(&url).await;
        });

        if error.is_timeout() {
            warn!("Request timeout for provider: {}", url);
        } else if error.is_connect() {
            warn!("Connection failed for provider: {}", url);
        } else {
            warn!("Request error for provider {}: {:?}", url, error);
        }
    }

    /// Get health status of all providers
    pub async fn get_provider_health(&self) -> Vec<crate::types::ProviderHealth> {
        self.health_monitor.get_all_health().await
    }

    /// Get the current best provider
    pub async fn get_best_provider(&self) -> RpcResult<String> {
        self.health_monitor.get_best_provider().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_config() -> RpcConfig {
        RpcConfig {
            providers: vec![
                RpcProvider {
                    url: "http://primary.test".to_string(),
                    weight: 100,
                },
                RpcProvider {
                    url: "http://secondary.test".to_string(),
                    weight: 50,
                },
            ],
            timeout: Duration::from_secs(5),
            max_retries: 3,
            failover_threshold_ms: 1000,
            enable_health_monitoring: false, // Disable for tests
            health_check_interval: Duration::from_secs(10),
        }
    }

    #[tokio::test]
    async fn test_client_creation() {
        let config = create_test_config();
        let client = RpcClient::new(config).await;
        assert!(client.is_ok());
    }

    #[tokio::test]
    async fn test_empty_providers_rejected() {
        let config = RpcConfig {
            providers: vec![],
            ..Default::default()
        };
        let result = RpcClient::new(config).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_get_provider_health() {
        let config = create_test_config();
        let client = RpcClient::new(config).await.unwrap();
        let health = client.get_provider_health().await;
        assert_eq!(health.len(), 2);
    }
}
