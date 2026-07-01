# Vero Engine - High-Availability RPC Client

An intelligent, high-availability RPC failover mechanism designed to maintain protocol liveness for the Vero protocol.

## Features

### 🚀 Intelligent Failover
- **Automatic provider switching** within <2 seconds of primary failure detection
- **Weighted provider selection** based on real-time performance metrics
- **Zero transaction drops** during provider outages

### 📊 Health Monitoring
- **Proactive health checks** with configurable intervals
- **Sliding window metrics** tracking last 100 requests per provider
- **Real-time latency tracking** with exponentially weighted averages
- **Automatic quarantine** for failing providers (30-second cooldown)

### 🔒 Security
- **Authenticated provider lists** with Ed25519 signature verification
- **Timestamp validation** to prevent replay attacks
- **Secure provider updates** from trusted sources only

### ⚡ Performance
- **Sub-2-second failover** on primary failure
- **Exponential backoff** for retries
- **Configurable timeouts** and retry limits
- **Async/await** throughout for maximum concurrency

### 🔐 Deterministic Serialization
- **Canonical JSON serialization** for consistent proposal hashing
- **Alphabetically sorted keys** at all nesting levels
- **Type normalization** for numbers and addresses
- **Multi-sig consensus** guaranteed across different client versions

## Architecture

### Core Components

1. **RpcClient** - Main client with automatic failover logic
2. **HealthMonitor** - Background health checking and metrics tracking
3. **ProviderAuthenticator** - Signature verification for provider lists
4. **CanonicalSerializer** - Deterministic serialization for proposal hashing
5. **Types** - Core types and error handling

### Health Metrics

Each provider is tracked with:
- Success/failure counts
- Average latency (sliding window)
- Last check timestamp
- Quarantine status
- Effective weight calculation

**Effective Weight Formula:**
```
effective_weight = base_weight × success_rate × latency_factor
```

Where:
- `success_rate` = successful_requests / total_requests
- `latency_factor` = 1 / (1 + avg_latency_seconds)

## Usage

### Deterministic Proposal Hashing

```rust
use vero_engine::serialization::{CanonicalSerializer, Proposal, ProposalState};

// Create a proposal
let proposal = Proposal {
    id: 42,
    action: "transfer".to_string(),
    proposer: "GXXXXXXXXXXXXXXX".to_string(),
    approved_by: vec!["GYYYYYYYYYYYYYY".to_string()],
    state: ProposalState::Pending,
    created_at: 1700000000,
    expires_at: 1700086400,
    metadata: None,
};

// Hash the proposal - guaranteed deterministic
let hash = CanonicalSerializer::hash_proposal(&proposal)?;
let hash_hex = CanonicalSerializer::hash_to_hex(&hash);

println!("Proposal hash: {}", hash_hex);
// All clients will produce the same hash for this proposal
```

**Multi-Sig Consensus:**
```rust
// Signer 1 computes hash
let signer1_hash = CanonicalSerializer::hash_proposal(&proposal)?;

// Signer 2 independently computes hash (even with different JSON key ordering)
let signer2_hash = CanonicalSerializer::hash_proposal(&proposal)?;

// Hashes will always match
assert_eq!(signer1_hash, signer2_hash);
```

See [SERIALIZATION_GUIDE.md](SERIALIZATION_GUIDE.md) for comprehensive documentation.

### Basic RPC Example

```rust
use vero_engine::{RpcClient, RpcConfig, RpcProvider};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Configure providers
    let config = RpcConfig {
        providers: vec![
            RpcProvider {
                url: "https://primary-rpc.example.com".to_string(),
                weight: 100,
            },
            RpcProvider {
                url: "https://backup-rpc.example.com".to_string(),
                weight: 80,
            },
        ],
        timeout: Duration::from_secs(10),
        max_retries: 3,
        failover_threshold_ms: 2000,
        enable_health_monitoring: true,
        health_check_interval: Duration::from_secs(10),
    };

    // Create client
    let client = RpcClient::new(config).await?;

    // Make RPC calls - failover happens automatically
    let result = client
        .call("method_name", serde_json::json!({"param": "value"}))
        .await?;

    println!("Result: {:?}", result);
    Ok(())
}
```

### With Authenticated Provider Updates

```rust
use vero_engine::{RpcClient, ProviderAuthenticator};

// Setup authenticator
let mut authenticator = ProviderAuthenticator::new();
authenticator.add_trusted_key("main".to_string(), public_key_bytes);

// Attach to client
client.set_authenticator(authenticator).await;

// Update providers from signed source
client
    .update_providers_from_source(
        "https://config.vero.network/providers",
        "main"
    )
    .await?;
```

### Monitoring Health

```rust
// Get all provider health statuses
let health = client.get_provider_health().await;
for provider in health {
    println!(
        "{}: {} (success rate: {:.1}%, latency: {}ms)",
        provider.url,
        if provider.is_healthy { "healthy" } else { "unhealthy" },
        provider.success_rate() * 100.0,
        provider.avg_latency_ms
    );
}

// Get current best provider
let best = client.get_best_provider().await?;
println!("Best provider: {}", best);
```

## Configuration

### RpcConfig Options

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `providers` | `Vec<RpcProvider>` | `[]` | List of RPC providers with weights |
| `timeout` | `Duration` | `10s` | Request timeout |
| `max_retries` | `usize` | `3` | Maximum retry attempts |
| `failover_threshold_ms` | `u64` | `2000` | Latency threshold for considering failover |
| `enable_health_monitoring` | `bool` | `true` | Enable background health checks |
| `health_check_interval` | `Duration` | `10s` | Interval between health checks |

### Environment Variables

You can also configure via environment:

```bash
VERO_RPC_TIMEOUT=10
VERO_RPC_MAX_RETRIES=3
VERO_RPC_FAILOVER_THRESHOLD=2000
VERO_RPC_HEALTH_CHECK_INTERVAL=10
```

## Testing

Run the test suite:

```bash
cd engine
cargo test
```

Run serialization tests specifically:

```bash
# Unit tests
cargo test --lib serialization

# Integration tests (verifies hash consistency)
cargo test --test serialization_integration_tests

# Run deterministic hashing example
cargo run --example deterministic_hashing
```

Run with logging:

```bash
RUST_LOG=debug cargo test -- --nocapture
```

Run all examples:

```bash
cargo run --example basic_usage
cargo run --example deterministic_hashing
```

## Performance Characteristics

### Failover Speed
- Detection: <100ms (based on request timeout)
- Switch time: <2 seconds (including retry logic)
- Total recovery: <2.1 seconds from failure to restored service

### Resource Usage
- Background health checker: ~1% CPU
- Memory per provider: ~1KB (including sliding window)
- Network overhead: 1 health check per provider per interval

### Throughput
- No throughput impact during normal operation
- Minimal impact during failover (single retry overhead)

## Security Considerations

### Provider Authentication
- Provider lists must be signed with Ed25519
- Signatures verified before applying updates
- Timestamps checked to prevent replay attacks (1-hour validity window)

### Network Security
- HTTPS enforced for all RPC endpoints (configurable)
- No credential storage in code
- Environment-based configuration recommended

### Audit Trail
- All provider switches logged
- Health status changes logged
- Authentication failures logged

## Production Deployment

### Recommended Setup

1. **Multiple providers across regions**
   ```rust
   providers: vec![
       RpcProvider { url: "https://us-east.rpc", weight: 100 },
       RpcProvider { url: "https://eu-west.rpc", weight: 90 },
       RpcProvider { url: "https://ap-south.rpc", weight: 80 },
   ]
   ```

2. **Authenticated provider updates**
   - Deploy signed provider list endpoint
   - Rotate provider list every 24 hours
   - Monitor for authentication failures

3. **Monitoring & Alerting**
   - Export health metrics to your monitoring system
   - Alert on: all providers unhealthy, high failure rates, extended quarantines

4. **Logging**
   ```rust
   tracing_subscriber::fmt()
       .with_max_level(tracing::Level::INFO)
       .init();
   ```

## Roadmap

- [ ] Metrics export (Prometheus format)
- [ ] Circuit breaker pattern
- [ ] Geographic provider routing
- [ ] Dynamic weight adjustment based on latency percentiles
- [ ] Provider SLA tracking

## Contributing

See [CONTRIBUTING.md](../CONTRIBUTING.md) for development guidelines.

## License

See [LICENSE](../LICENSE) for details.
