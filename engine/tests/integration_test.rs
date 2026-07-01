use vero_engine::{RpcClient, RpcConfig, RpcProvider};
use std::time::Duration;

/// Test that verifies failover happens within 2 seconds
#[tokio::test]
async fn test_failover_speed() {
    let config = RpcConfig {
        providers: vec![
            RpcProvider {
                url: "http://localhost:9999".to_string(), // Non-existent - will fail
                weight: 100,
            },
            RpcProvider {
                url: "http://localhost:9998".to_string(), // Also non-existent
                weight: 80,
            },
        ],
        timeout: Duration::from_millis(500),
        max_retries: 2,
        failover_threshold_ms: 2000,
        enable_health_monitoring: false, // Disable for faster test
        health_check_interval: Duration::from_secs(10),
    };

    let client = RpcClient::new(config).await.unwrap();
    
    let start = std::time::Instant::now();
    let result = client.call("test_method", serde_json::json!({})).await;
    let elapsed = start.elapsed();

    // Should fail since both providers are down
    assert!(result.is_err());
    
    // But should fail within 2 seconds (with retries and failover)
    assert!(
        elapsed < Duration::from_secs(2),
        "Failover took too long: {:?}",
        elapsed
    );
}

/// Test that client rejects empty provider list
#[tokio::test]
async fn test_empty_providers_rejected() {
    let config = RpcConfig {
        providers: vec![],
        ..Default::default()
    };

    let result = RpcClient::new(config).await;
    assert!(result.is_err());
}

/// Test health monitoring tracks provider status
#[tokio::test]
async fn test_health_monitoring() {
    let config = RpcConfig {
        providers: vec![
            RpcProvider {
                url: "http://test1.example.com".to_string(),
                weight: 100,
            },
            RpcProvider {
                url: "http://test2.example.com".to_string(),
                weight: 80,
            },
        ],
        enable_health_monitoring: false,
        ..Default::default()
    };

    let client = RpcClient::new(config).await.unwrap();
    
    // Get initial health
    let health = client.get_provider_health().await;
    assert_eq!(health.len(), 2);
    
    // All providers should start healthy
    for provider in health {
        assert!(provider.is_healthy);
        assert_eq!(provider.success_count, 0);
        assert_eq!(provider.failure_count, 0);
    }
}

/// Test weighted provider selection
#[tokio::test]
async fn test_weighted_providers() {
    let config = RpcConfig {
        providers: vec![
            RpcProvider {
                url: "http://high-weight.example.com".to_string(),
                weight: 100,
            },
            RpcProvider {
                url: "http://low-weight.example.com".to_string(),
                weight: 10,
            },
        ],
        enable_health_monitoring: false,
        ..Default::default()
    };

    let client = RpcClient::new(config).await.unwrap();
    
    // With no request history, the higher weight provider should be selected
    let best = client.get_best_provider().await.unwrap();
    assert_eq!(best, "http://high-weight.example.com");
}

/// Test configuration validation
#[tokio::test]
async fn test_configuration_defaults() {
    let config = RpcConfig::default();
    
    assert_eq!(config.timeout, Duration::from_secs(10));
    assert_eq!(config.max_retries, 3);
    assert_eq!(config.failover_threshold_ms, 2000);
    assert!(config.enable_health_monitoring);
}

/// Test that retries use exponential backoff
#[tokio::test]
async fn test_retry_timing() {
    let config = RpcConfig {
        providers: vec![
            RpcProvider {
                url: "http://localhost:9997".to_string(),
                weight: 100,
            },
        ],
        timeout: Duration::from_millis(200),
        max_retries: 3,
        enable_health_monitoring: false,
        ..Default::default()
    };

    let client = RpcClient::new(config).await.unwrap();
    
    let start = std::time::Instant::now();
    let _ = client.call("test", serde_json::json!({})).await;
    let elapsed = start.elapsed();

    // With 3 retries and exponential backoff (100ms, 200ms, 400ms)
    // Plus 3x 200ms timeouts = 600ms
    // Total should be around 1300ms but less than 2000ms
    assert!(
        elapsed >= Duration::from_millis(600),
        "Should include retry delays and timeouts"
    );
    assert!(
        elapsed < Duration::from_secs(2),
        "Should complete within failover threshold"
    );
}
