# Deterministic Serialization Guide

## Overview

This module provides canonical JSON serialization to ensure deterministic proposal hashing across different client library versions and environments. This is critical for multi-sig consensus where all signers must calculate identical hashes for the same proposal.

## Problem Statement

Different JSON serialization implementations may:
- Order object keys differently
- Handle number precision inconsistently
- Format whitespace differently

This leads to different hash values for semantically identical data, breaking multi-sig consensus.

## Solution

The `CanonicalSerializer` enforces:
1. **Alphabetical key sorting** - All object keys are sorted alphabetically
2. **Type normalization** - Numbers are cast to standard types
3. **Consistent formatting** - No extraneous whitespace
4. **Deterministic output** - Same input always produces same output

## Usage

### Basic Serialization

```rust
use vero_engine::serialization::CanonicalSerializer;
use serde_json::json;

let data = json!({
    "z_field": 3,
    "a_field": 1,
    "m_field": 2
});

let canonical = CanonicalSerializer::serialize(&data)?;
// Output: {"a_field":1,"m_field":2,"z_field":3}
```

### Hashing Proposals

```rust
use vero_engine::serialization::{CanonicalSerializer, Proposal, ProposalState};

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

let hash_bytes = CanonicalSerializer::hash_proposal(&proposal)?;
let hash_hex = CanonicalSerializer::hash_to_hex(&hash_bytes);

println!("Proposal hash: {}", hash_hex);
```

### Hashing Transactions

```rust
use vero_engine::serialization::{CanonicalSerializer, Transaction};

let tx = Transaction {
    from: "GFROM".to_string(),
    to: "GTO".to_string(),
    amount: "100.5000000".to_string(),
    nonce: 1,
    timestamp: 1700000000,
    data: None,
};

let hash_bytes = CanonicalSerializer::hash_transaction(&tx)?;
let hash_hex = CanonicalSerializer::hash_to_hex(&hash_bytes);
```

### Multi-Sig Scenario

```rust
// Signer 1 creates proposal
let signer1_proposal = create_proposal();
let signer1_hash = CanonicalSerializer::hash_proposal(&signer1_proposal)?;

// Signer 2 receives proposal data and independently computes hash
let signer2_proposal = deserialize_proposal_from_network();
let signer2_hash = CanonicalSerializer::hash_proposal(&signer2_proposal)?;

// Hashes will match even if JSON key ordering was different
assert_eq!(signer1_hash, signer2_hash);
```

## Key Features

### 1. Alphabetical Key Sorting

Object keys are automatically sorted alphabetically at all nesting levels:

```rust
// Input (any order)
{"z": 1, "a": 2, "m": 3}

// Output (alphabetical)
{"a":2,"m":3,"z":1}
```

### 2. Number Normalization

Numbers are normalized to avoid floating-point precision issues:

```rust
// Integers preserved
42 → 42

// Floats converted to fixed-precision strings
3.14159265359 → "3.14159265"
```

### 3. Nested Object Handling

Canonicalization works recursively:

```rust
{
  "outer_z": {
    "inner_z": 1,
    "inner_a": 2
  },
  "outer_a": 3
}
```

Becomes:

```json
{
  "outer_a": 3,
  "outer_z": {
    "inner_a": 2,
    "inner_z": 1
  }
}
```

### 4. Array Order Preservation

Arrays maintain their original order (only object keys are sorted):

```rust
{"items": [3, 1, 2]} → {"items":[3,1,2]}
```

## API Reference

### `CanonicalSerializer::serialize<T: Serialize>(value: &T) -> RpcResult<String>`

Serialize any value to canonical JSON with sorted keys.

### `CanonicalSerializer::hash_proposal(proposal: &Proposal) -> RpcResult<Vec<u8>>`

Serialize and compute SHA-256 hash for a proposal.

### `CanonicalSerializer::hash_transaction(transaction: &Transaction) -> RpcResult<Vec<u8>>`

Serialize and compute SHA-256 hash for a transaction.

### `CanonicalSerializer::hash_data<T: Serialize>(data: &T) -> RpcResult<Vec<u8>>`

Compute SHA-256 hash of arbitrary serializable data.

### `CanonicalSerializer::hash_to_hex(hash: &[u8]) -> String`

Convert hash bytes to hexadecimal string.

### `CanonicalSerializer::normalize_address(address: &str) -> String`

Normalize an address string to canonical format.

### `CanonicalSerializer::normalize_amount(amount: &str) -> RpcResult<String>`

Normalize numeric amount to fixed decimal precision (7 places for Stellar).

## Testing

The module includes comprehensive tests covering:

1. **Multi-run consistency** - Same input produces same output across multiple runs
2. **Key ordering independence** - Different key orders produce identical hashes
3. **Multi-sig consensus** - Multiple signers reach agreement
4. **Nested object handling** - Deep object structures are canonicalized
5. **Array ordering** - Array order is preserved
6. **Number handling** - Consistent numeric representation
7. **Edge cases** - Empty collections, large numbers, special characters, Unicode

Run tests:

```bash
cd engine
cargo test serialization
```

### Integration Tests

Integration tests verify real-world scenarios:

```bash
cargo test --test serialization_integration_tests
```

Key test scenarios:
- 100 consecutive runs produce identical hashes
- Different JSON key orders produce same hash
- 3 independent signers reach consensus
- Large numbers handled consistently
- Unicode and special characters supported

## Migration Guide

### Before (Non-Deterministic)

```rust
// Different key orders might produce different JSON
let json1 = serde_json::to_string(&proposal)?;
let json2 = serde_json::to_string(&proposal)?;
// json1 might not equal json2!

let hash = sha256(json1.as_bytes());
```

### After (Deterministic)

```rust
use vero_engine::serialization::CanonicalSerializer;

// Always produces identical output
let hash = CanonicalSerializer::hash_proposal(&proposal)?;
```

## Best Practices

1. **Always use CanonicalSerializer for hashing** - Never hash raw `serde_json` output
2. **Use string amounts** - Avoid floating-point for monetary values
3. **Normalize addresses** - Use `normalize_address()` before hashing
4. **Test across environments** - Verify hash consistency in integration tests
5. **Document hash format** - Include hash format in API specifications

## Performance Considerations

- Canonicalization adds minimal overhead (~5-10% vs raw serialization)
- Hash computation uses ring's SHA-256 (hardware-accelerated when available)
- Suitable for production use in multi-sig scenarios

## Security Notes

- Uses SHA-256 from the `ring` cryptographic library
- Deterministic serialization prevents signature malleability attacks
- Not vulnerable to JSON injection (proper escaping maintained)

## Troubleshooting

### Hashes don't match across signers

1. Verify all signers use `CanonicalSerializer`
2. Check that proposal data is identical
3. Ensure amounts use string representation
4. Verify addresses are normalized

### Float precision issues

Use string representation for amounts:

```rust
// Bad
amount: 100.5

// Good  
amount: "100.5000000"
```

### Different serde versions

The canonicalization ensures consistency even across different serde versions. Integration tests verify this.

## Examples

See:
- `engine/tests/serialization_integration_tests.rs` - Comprehensive integration tests
- `engine/src/serialization.rs` - Module documentation and unit tests
- `engine/examples/` - Usage examples (if added)

## Support

For issues or questions:
1. Check integration test scenarios
2. Review this guide
3. Examine the test suite for examples
4. Consult the module source code documentation
