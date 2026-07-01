use vero_engine::{RpcClient, RpcConfig, RpcProvider, ProviderAuthenticator};
use std::time::Duration;
use tracing_subscriber;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    // Create RPC configuration with multiple providers
    let config = RpcConfig {
        providers: vec![
            RpcProvider {
                url: "https://soroban-testnet.stellar.org".to_string(),
                weight: 100,
            },
            RpcProvider {
                url: "https://rpc-backup1.example.com".to_string(),
                weight: 80,
            },
            RpcProvider {
                url: "https://rpc-backup2.example.com".to_string(),
                weight: 60,
            },
        ],
        timeout: Duration::from_secs(10),
        max_retries: 3,
        failover_threshold_ms: 2000,
        enable_health_monitoring: true,
        health_check_interval: Duration::from_secs(10),
    };

    // Create the RPC client
    let client = RpcClient::new(config).await?;

    println!("RPC Client initialized with {} providers", 3);

    // Example 1: Make a simple RPC call
    println!("\n=== Example 1: Simple RPC Call ===");
    match client
        .call(
            "getHealth",
            serde_json::json!({})
        )
        .await
    {
        Ok(result) => println!("Health check result: {:?}", result),
        Err(e) => eprintln!("Health check failed: {:?}", e),
    }

    // Example 2: Check provider health
    println!("\n=== Example 2: Provider Health Status ===");
    let health_status = client.get_provider_health().await;
    for provider in health_status {
        println!(
            "Provider: {}\n  Healthy: {}\n  Weight: {}\n  Success Rate: {:.2}%\n  Avg Latency: {}ms\n  Quarantined: {}",
            provider.url,
            provider.is_healthy,
            provider.weight,
            provider.success_rate() * 100.0,
            provider.avg_latency_ms,
            provider.is_quarantined()
        );
    }

    // Example 3: Get best provider
    println!("\n=== Example 3: Best Provider ===");
    match client.get_best_provider().await {
        Ok(url) => println!("Best provider: {}", url),
        Err(e) => eprintln!("No healthy provider available: {:?}", e),
    }

    // Example 4: Using authenticated provider list
    println!("\n=== Example 4: Authenticated Provider Updates ===");
    let mut authenticator = ProviderAuthenticator::new();
    
    // Add a trusted public key (example - in production, load from secure config)
    let public_key = vec![0u8; 32]; // Placeholder
    authenticator.add_trusted_key("main".to_string(), public_key);
    
    client.set_authenticator(authenticator).await;
    
    // Note: This would fail without a proper signed provider list
    // match client.update_providers_from_source("https://config.example.com/providers", "main").await {
    //     Ok(_) => println!("Providers updated from authenticated source"),
    //     Err(e) => eprintln!("Failed to update providers: {:?}", e),
    // }

    // Example 5: Simulating failover
    println!("\n=== Example 5: Automatic Failover ===");
    println!("Making multiple calls to demonstrate failover...");
    
    for i in 0..5 {
        match client
            .call(
                "getLedgerEntries",
                serde_json::json!({
                    "keys": []
                })
            )
            .await
        {
            Ok(_) => println!("Call {} succeeded", i + 1),
            Err(e) => println!("Call {} failed: {:?}", i + 1, e),
        }
        
        tokio::time::sleep(Duration::from_secs(1)).await;
    }

    println!("\n=== Example completed ===");
    println!("The client will continue monitoring providers in the background.");
    
    // Keep the program running to see background health checks
    tokio::time::sleep(Duration::from_secs(30)).await;

    Ok(())
}
