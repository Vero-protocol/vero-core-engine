# Quick Start Guide - RPC Failover

Get up and running with the Vero Engine RPC failover system in 5 minutes.

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
vero-engine = { path = "../engine" }
tokio = { version = "1", features = ["full"] }
serde_json = "1.0"
```

## Basic Usage (3 steps)

### 1. Create Configuration

```rust
use vero_engine::{RpcConfig, RpcProvider};
use std::time::Duration;

let config = RpcConfig {
    providers: vec![
        RpcProvider {
            url: "https://soroban-testnet.stellar.org".to_string(),
            weight: 100,
        },
        RpcProvider {
            url: "https://backup-rpc.example.com".to_string(),
            weight: 80,
        },
    ],
    timeout: Duration::from_secs(10),
    max_retries: 3,
    ..Default::default()
};
```

### 2. Create Client

```rust
use vero_engine::RpcClient;

let client = RpcClient::new(config).await?;
```

### 3. Make RPC Calls

```rust
// Automatic failover happens transparently
let result = client
    .call("getHealth", serde_json::json!({}))
    .await?;

println!("Result: {:?}", result);
```

## Complete Example

```rust
use vero_engine::{RpcClient, RpcConfig, RpcProvider};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Configure
    let config = RpcConfig {
        providers: vec![
            RpcProvider {
                url: "https://soroban-testnet.stellar.org".to_string(),
                weight: 100,
            },
        ],
        ..Default::default()
    };

    // 2. Create client
    let client = RpcClient::new(config).await?;

    // 3. Use it
    let health = client
        .call("getHealth", serde_json::json!({}))
        .await?;
    
    println!("Health: {:?}", health);
    
    Ok(())
}
```

## Common Patterns

### Check Provider Health

```rust
let health = client.get_provider_health().await;
for provider in health {
    println!("{}: success_rate={:.1}%, latency={}ms",
        provider.url,
        provider.success_rate() * 100.0,
        provider.avg_latency_ms
    );
}
```

### Get Best Provider

```rust
match client.get_best_provider().await {
    Ok(url) => println!("Using: {}", url),
    Err(_) => println!("No healthy providers!"),
}
```

### Authenticated Provider Updates

```rust
use vero_engine::ProviderAuthenticator;

// Setup authenticator
let mut auth = ProviderAuthenticator::new();
auth.add_trusted_key("main".to_string(), public_key);
client.set_authenticator(auth).await;

// Update from secure source
client.update_providers_from_source(
    "https://config.example.com/providers",
    "main"
).await?;
```

## Configuration Options

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `providers` | `Vec<RpcProvider>` | `[]` | Provider URLs and weights |
| `timeout` | `Duration` | `10s` | Request timeout |
| `max_retries` | `usize` | `3` | Maximum retry attempts |
| `failover_threshold_ms` | `u64` | `2000` | Latency warning threshold |
| `enable_health_monitoring` | `bool` | `true` | Enable background checks |
| `health_check_interval` | `Duration` | `10s` | Check interval |

## Testing Your Integration

### 1. Run Example

```bash
cd engine
cargo run --example basic_usage
```

### 2. Run Tests

```bash
cargo test
```

### 3. Enable Logging

```rust
// In your main.rs or lib.rs
use tracing_subscriber;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();
    
    // Your code here
}
```

## Common Issues

### Issue: "At least one provider is required"
**Solution**: Add at least one provider to the configuration.

```rust
let config = RpcConfig {
    providers: vec![
        RpcProvider {
            url: "https://your-rpc.com".to_string(),
            weight: 100,
        },
    ],
    ..Default::default()
};
```

### Issue: All providers timing out
**Solution**: Check network connectivity or increase timeout.

```rust
let config = RpcConfig {
    timeout: Duration::from_secs(30), // Increase timeout
    ..Default::default()
};
```

### Issue: Provider keeps getting quarantined
**Solution**: The provider is failing health checks. Check:
1. Is the URL correct?
2. Is the provider online?
3. Are there network issues?

View health status:
```rust
let health = client.get_provider_health().await;
for p in health {
    if p.is_quarantined() {
        println!("{} quarantined until {:?}", p.url, p.quarantine_until);
    }
}
```

## Environment Configuration

You can use environment variables:

```bash
# Set in your shell or .env file
export VERO_RPC_PRIMARY="https://rpc1.example.com"
export VERO_RPC_BACKUP="https://rpc2.example.com"
export VERO_RPC_TIMEOUT=10
export VERO_RPC_MAX_RETRIES=3
```

Then in your code:

```rust
use std::env;

let config = RpcConfig {
    providers: vec![
        RpcProvider {
            url: env::var("VERO_RPC_PRIMARY")?,
            weight: 100,
        },
        RpcProvider {
            url: env::var("VERO_RPC_BACKUP")?,
            weight: 80,
        },
    ],
    timeout: Duration::from_secs(
        env::var("VERO_RPC_TIMEOUT")?.parse()?
    ),
    ..Default::default()
};
```

## Production Checklist

Before deploying to production:

- [ ] Configure multiple providers (minimum 2)
- [ ] Use HTTPS URLs only
- [ ] Set appropriate timeouts
- [ ] Enable health monitoring
- [ ] Set up logging (INFO or WARN level)
- [ ] Configure authenticated provider updates
- [ ] Set up monitoring/alerting
- [ ] Test failover behavior
- [ ] Document provider list management

## Next Steps

- **Read the full documentation**: See [README.md](README.md)
- **Understand the implementation**: See [IMPLEMENTATION.md](IMPLEMENTATION.md)
- **Run the example**: `cargo run --example basic_usage`
- **Add monitoring**: Integrate with your metrics system
- **Set up alerts**: Monitor provider health

## Need Help?

- **Documentation**: See README.md and IMPLEMENTATION.md
- **Examples**: Check `examples/basic_usage.rs`
- **Issues**: Open an issue on GitHub
- **Code Review**: See CODE_REVIEW_CHECKLIST.md

## Performance Tips

1. **Use multiple providers** - Distribute load and improve reliability
2. **Weight providers by performance** - Higher weight = more traffic
3. **Enable health monitoring** - Proactive issue detection
4. **Set reasonable timeouts** - Balance speed vs. reliability
5. **Monitor metrics** - Track success rates and latencies

## Security Tips

1. **Use authenticated provider lists** - Prevent unauthorized changes
2. **Verify signatures** - Only trust known public keys
3. **Use HTTPS** - Encrypt all communication
4. **Rotate keys** - Update public keys periodically
5. **Monitor auth failures** - Alert on verification errors

---

That's it! You now have a high-availability RPC client with automatic failover. 🚀
