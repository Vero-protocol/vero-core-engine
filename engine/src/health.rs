use crate::types::{ProviderHealth, RpcError, RpcResult};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

const HEALTH_CHECK_INTERVAL: Duration = Duration::from_secs(10);
const QUARANTINE_DURATION: Duration = Duration::from_secs(30);
const MAX_FAILURE_THRESHOLD: u64 = 3;
const SLIDING_WINDOW_SIZE: usize = 100;

/// Health monitor with sliding window of request outcomes
pub struct HealthMonitor {
    providers: Arc<RwLock<HashMap<String, ProviderState>>>,
    check_interval: Duration,
}

struct ProviderState {
    health: ProviderHealth,
    request_window: Vec<RequestOutcome>,
    latency_samples: Vec<u64>,
}

#[derive(Debug, Clone)]
struct RequestOutcome {
    success: bool,
    latency_ms: u64,
    timestamp: Instant,
}

impl HealthMonitor {
    pub fn new() -> Self {
        Self {
            providers: Arc::new(RwLock::new(HashMap::new())),
            check_interval: HEALTH_CHECK_INTERVAL,
        }
    }

    pub fn with_interval(mut self, interval: Duration) -> Self {
        self.check_interval = interval;
        self
    }

    /// Register a provider for health monitoring
    pub async fn register_provider(&self, url: String, weight: u32) {
        let mut providers = self.providers.write().await;
        providers.insert(
            url.clone(),
            ProviderState {
                health: ProviderHealth::new(url, weight),
                request_window: Vec::with_capacity(SLIDING_WINDOW_SIZE),
                latency_samples: Vec::with_capacity(SLIDING_WINDOW_SIZE),
            },
        );
    }

    /// Record a successful request
    pub async fn record_success(&self, url: &str, latency_ms: u64) {
        let mut providers = self.providers.write().await;
        if let Some(state) = providers.get_mut(url) {
            state.health.success_count += 1;
            state.health.last_check = chrono::Utc::now();
            
            // Add to sliding window
            state.request_window.push(RequestOutcome {
                success: true,
                latency_ms,
                timestamp: Instant::now(),
            });
            
            // Keep window size bounded
            if state.request_window.len() > SLIDING_WINDOW_SIZE {
                state.request_window.remove(0);
            }
            
            // Update latency
            state.latency_samples.push(latency_ms);
            if state.latency_samples.len() > SLIDING_WINDOW_SIZE {
                state.latency_samples.remove(0);
            }
            
            state.health.avg_latency_ms = self.calculate_avg_latency(&state.latency_samples);
            
            // Clear quarantine on success
            if state.health.is_quarantined() {
                info!("Provider {} recovered from quarantine", url);
                state.health.quarantine_until = None;
            }
            
            state.health.is_healthy = true;
            
            debug!(
                "Provider {} success recorded: {}ms latency, success_rate: {:.2}%",
                url,
                latency_ms,
                state.health.success_rate() * 100.0
            );
        }
    }

    /// Record a failed request
    pub async fn record_failure(&self, url: &str) {
        let mut providers = self.providers.write().await;
        if let Some(state) = providers.get_mut(url) {
            state.health.failure_count += 1;
            state.health.last_check = chrono::Utc::now();
            
            // Add to sliding window
            state.request_window.push(RequestOutcome {
                success: false,
                latency_ms: 0,
                timestamp: Instant::now(),
            });
            
            // Keep window size bounded
            if state.request_window.len() > SLIDING_WINDOW_SIZE {
                state.request_window.remove(0);
            }
            
            // Check if we should quarantine
            let recent_failures = self.count_recent_failures(&state.request_window);
            if recent_failures >= MAX_FAILURE_THRESHOLD {
                let until = chrono::Utc::now() + chrono::Duration::from_std(QUARANTINE_DURATION).unwrap();
                state.health.quarantine_until = Some(until);
                state.health.is_healthy = false;
                
                warn!(
                    "Provider {} quarantined until {:?} (recent failures: {})",
                    url, until, recent_failures
                );
            }
            
            debug!(
                "Provider {} failure recorded, success_rate: {:.2}%",
                url,
                state.health.success_rate() * 100.0
            );
        }
    }

    /// Get health status for a specific provider
    pub async fn get_health(&self, url: &str) -> Option<ProviderHealth> {
        let providers = self.providers.read().await;
        providers.get(url).map(|state| state.health.clone())
    }

    /// Get all provider health statuses
    pub async fn get_all_health(&self) -> Vec<ProviderHealth> {
        let providers = self.providers.read().await;
        providers
            .values()
            .map(|state| state.health.clone())
            .collect()
    }

    /// Get the best available provider based on health metrics
    pub async fn get_best_provider(&self) -> RpcResult<String> {
        let providers = self.providers.read().await;
        
        let mut best: Option<(&String, f64)> = None;
        
        for (url, state) in providers.iter() {
            let weight = state.health.effective_weight();
            if weight > 0.0 {
                if let Some((_, best_weight)) = best {
                    if weight > best_weight {
                        best = Some((url, weight));
                    }
                } else {
                    best = Some((url, weight));
                }
            }
        }
        
        best.map(|(url, _)| url.clone())
            .ok_or(RpcError::AllProvidersDown)
    }

    /// Get providers sorted by effective weight (best first)
    pub async fn get_sorted_providers(&self) -> Vec<(String, f64)> {
        let providers = self.providers.read().await;
        
        let mut weighted: Vec<_> = providers
            .iter()
            .map(|(url, state)| (url.clone(), state.health.effective_weight()))
            .filter(|(_, weight)| *weight > 0.0)
            .collect();
        
        weighted.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        weighted
    }

    /// Start background health check task
    pub fn start_monitoring(self: Arc<Self>) -> tokio::task::JoinHandle<()> {
        let monitor = Arc::clone(&self);
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(monitor.check_interval);
            loop {
                interval.tick().await;
                monitor.perform_health_checks().await;
            }
        })
    }

    async fn perform_health_checks(&self) {
        let urls: Vec<String> = {
            let providers = self.providers.read().await;
            providers.keys().cloned().collect()
        };

        for url in urls {
            if let Err(e) = self.check_provider_health(&url).await {
                debug!("Health check failed for {}: {:?}", url, e);
            }
        }
    }

    async fn check_provider_health(&self, url: &str) -> RpcResult<()> {
        let start = Instant::now();
        
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .map_err(|e| RpcError::NetworkError(e.to_string()))?;

        match client.get(url).send().await {
            Ok(response) if response.status().is_success() => {
                let latency = start.elapsed().as_millis() as u64;
                self.record_success(url, latency).await;
                Ok(())
            }
            Ok(_) | Err(_) => {
                self.record_failure(url).await;
                Err(RpcError::NetworkError("Health check failed".to_string()))
            }
        }
    }

    fn calculate_avg_latency(&self, samples: &[u64]) -> u64 {
        if samples.is_empty() {
            return 0;
        }
        let sum: u64 = samples.iter().sum();
        sum / samples.len() as u64
    }

    fn count_recent_failures(&self, window: &[RequestOutcome]) -> u64 {
        let cutoff = Instant::now() - Duration::from_secs(60); // Last 60 seconds
        window
            .iter()
            .filter(|outcome| outcome.timestamp > cutoff && !outcome.success)
            .count() as u64
    }
}

impl Default for HealthMonitor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_register_provider() {
        let monitor = HealthMonitor::new();
        monitor.register_provider("http://test.com".to_string(), 100).await;
        
        let health = monitor.get_health("http://test.com").await;
        assert!(health.is_some());
        assert_eq!(health.unwrap().weight, 100);
    }

    #[tokio::test]
    async fn test_record_success() {
        let monitor = HealthMonitor::new();
        monitor.register_provider("http://test.com".to_string(), 100).await;
        monitor.record_success("http://test.com", 50).await;
        
        let health = monitor.get_health("http://test.com").await.unwrap();
        assert_eq!(health.success_count, 1);
        assert_eq!(health.avg_latency_ms, 50);
    }

    #[tokio::test]
    async fn test_quarantine_after_failures() {
        let monitor = HealthMonitor::new();
        monitor.register_provider("http://test.com".to_string(), 100).await;
        
        for _ in 0..MAX_FAILURE_THRESHOLD {
            monitor.record_failure("http://test.com").await;
        }
        
        let health = monitor.get_health("http://test.com").await.unwrap();
        assert!(health.is_quarantined());
        assert!(!health.is_healthy);
    }

    #[tokio::test]
    async fn test_get_best_provider() {
        let monitor = HealthMonitor::new();
        monitor.register_provider("http://fast.com".to_string(), 100).await;
        monitor.register_provider("http://slow.com".to_string(), 50).await;
        
        monitor.record_success("http://fast.com", 10).await;
        monitor.record_success("http://slow.com", 100).await;
        
        let best = monitor.get_best_provider().await.unwrap();
        assert_eq!(best, "http://fast.com");
    }
}
