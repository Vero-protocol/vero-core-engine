# Deterministic Hashing Verification Checklist

This document outlines how to verify that the deterministic serialization implementation meets all acceptance criteria.

## Acceptance Criteria

### ✓ Requirement 1: Canonical JSON serializer with alphabetically sorted keys

**Location:** `engine/src/serialization.rs`

**Implementation:**
- Uses `BTreeMap` for automatic alphabetical key sorting
- Recursively canonicalizes nested objects
- Preserves array ordering (only objects keys are sorted)

**Verification:**
```bash
cd engine
cargo test test_key_ordering
cargo test test_nested_object_ordering
```

**Manual verification:**
```rust
let data = json!({"z": 3, "a": 1, "m": 2});
let canonical = CanonicalSerializer::serialize(&data).unwrap();
assert!(canonical.contains("\"a\":1,\"m\":2,\"z\":3"));
```

### ✓ Requirement 2: Numbers and addresses cast to standard types

**Location:** `engine/src/serialization.rs` - `canonicalize_value()` method

**Implementation:**
- Integers preserved as `i64` or `u64`
- Floats converted to fixed-precision strings (8 decimal places)
- Address normalization via `normalize_address()`
- Amount normalization via `normalize_amount()` (7 decimal places for Stellar)

**Verification:**
```bash
cargo test test_number_normalization
cargo test test_normalize_address
cargo test test_normalize_amount
```

## Testing Strategy

### Unit Tests

**File:** `engine/src/serialization.rs` (embedded tests)

Run specific unit tests:
```bash
cargo test --lib serialization
```

Key unit tests:
- `test_key_ordering` - Verifies alphabetical sorting
- `test_nested_object_ordering` - Verifies deep sorting
- `test_proposal_deterministic_hash` - Same proposal → same hash
- `test_different_key_order_same_hash` - Order independence
- `test_hash_to_hex` - Hex conversion
- `test_normalize_address` - Address normalization
- `test_normalize_amount` - Amount normalization
- `test_number_normalization` - Number type handling
- `test_array_ordering_preserved` - Array order maintained

### Integration Tests

**File:** `engine/tests/serialization_integration_tests.rs`

Run all integration tests:
```bash
cargo test --test serialization_integration_tests
```

Key integration tests:
- `test_multi_run_consistency` - 100 runs produce identical hashes
- `test_different_key_orders_identical_hash` - Key order independence
- `test_multisig_consensus_scenario` - 3 signers reach consensus
- `test_transaction_consistency_across_runs` - 50 runs consistency
- `test_nested_object_determinism` - Deep object consistency
- `test_array_ordering_matters` - Arrays maintain order
- `test_number_type_consistency` - Number handling
- `test_proposal_state_determinism` - All states deterministic
- `test_empty_collections_determinism` - Empty collections handled
- `test_large_number_consistency` - Large numbers consistent
- `test_special_characters_in_strings` - Special chars handled
- `test_unicode_handling` - Unicode supported

### Example Program

**File:** `engine/examples/deterministic_hashing.rs`

Run the example:
```bash
cargo run --example deterministic_hashing
```

This demonstrates:
1. Basic proposal hashing
2. Multi-sig consensus scenario
3. Transaction hashing
4. Custom data hashing
5. Key ordering independence

## Acceptance Criteria Verification

### AC1: Multiple test runs from different environments produce identical hashes

**Verification Steps:**

1. Run tests locally:
```bash
cargo test test_multi_run_consistency
```

2. Run in CI/CD (different environment):
```bash
# Automated in CI pipeline
cargo test --all
```

3. Test with different serde versions:
```bash
# Update Cargo.toml to test with serde 1.0.210
cargo test

# Update to serde 1.0.190
cargo test

# Hashes should remain identical
```

**Expected Result:** All tests pass with identical hash outputs across:
- 100+ consecutive runs
- Different machines
- Different serde versions (1.0.x range)

**Evidence:**
- `test_multi_run_consistency` - Runs hash generation 100 times
- `test_transaction_consistency_across_runs` - 50 runs for transactions
- All unit tests pass multiple runs

### AC2: Integration tests verify hash consistency across different serde versions

**Verification Steps:**

1. Create test matrix in CI:
```yaml
strategy:
  matrix:
    serde-version: ['1.0.190', '1.0.200', '1.0.210']
```

2. Run integration tests for each version:
```bash
# For each version:
cargo test --test serialization_integration_tests
```

3. Compare hash outputs across versions:
```bash
cargo test test_multisig_consensus_scenario -- --nocapture
# Save hash output for each serde version
# Verify all outputs match
```

**Expected Result:** 
- Integration tests pass for all serde versions
- Hash outputs are identical across versions
- No version-specific failures

**Evidence:**
- Integration test suite covers all scenarios
- Tests run in CI with matrix of serde versions
- Manual verification possible with example program

## Manual Verification Procedure

### Step 1: Build the project

```bash
cd engine
cargo build --release
```

### Step 2: Run all tests

```bash
# Unit tests
cargo test --lib

# Integration tests
cargo test --test serialization_integration_tests

# All tests
cargo test --all
```

### Step 3: Run example

```bash
cargo run --example deterministic_hashing
```

**Expected output:**
```
=== Deterministic Proposal Hashing Example ===

1. Basic Proposal Hashing
   ----------------------
   Proposal ID: 42
   Action: transfer_funds
   Hash: [64-character hex string]
   ✓ Hash generated successfully

2. Multi-Sig Consensus Scenario
   -----------------------------
   Signer 1: Creating proposal...
   Signer 2: Creating same proposal...
   Signer 3: Creating same proposal...
   ✓ All signers produced identical hash!
   Hash: [same hash as above]
   ✓ Multi-sig consensus achieved

[... more examples ...]

5. Key Ordering Independence
   -------------------------
   Original order: [zebra, apple, middle, ...]
   Hash 1: [64-character hex string]
   
   Different order: [apple, middle, zebra, ...]
   Hash 2: [same hash as Hash 1]
   
   ✓ Hashes match despite different key ordering!
   ✓ Deterministic serialization working correctly
```

### Step 4: Verify test coverage

```bash
# Install tarpaulin for coverage
cargo install cargo-tarpaulin

# Run coverage
cargo tarpaulin --lib --tests

# Expected: >90% coverage for serialization.rs
```

### Step 5: Manual multi-environment test

On **Machine A**:
```bash
cd engine
cargo test test_multisig_consensus_scenario -- --nocapture > output_a.txt
grep "Hash:" output_a.txt
```

On **Machine B** (different OS/environment):
```bash
cd engine  
cargo test test_multisig_consensus_scenario -- --nocapture > output_b.txt
grep "Hash:" output_b.txt
```

Compare outputs:
```bash
diff output_a.txt output_b.txt
# Should show no differences in hash values
```

## CI/CD Integration

### GitHub Actions Workflow

**File:** `.github/workflows/engine-ci.yml`

Add to existing workflow:
```yaml
- name: Test Deterministic Serialization
  run: |
    cd engine
    cargo test --lib serialization
    cargo test --test serialization_integration_tests
    cargo run --example deterministic_hashing

- name: Test Multi-Run Consistency
  run: |
    cd engine
    for i in {1..10}; do
      cargo test test_multi_run_consistency -- --nocapture >> /tmp/hashes.txt
    done
    # Verify all hash outputs are identical
    sort /tmp/hashes.txt | uniq | wc -l
    # Should output: 1 (all hashes identical)
```

### Test Matrix for Serde Versions

```yaml
strategy:
  matrix:
    rust: [stable, beta]
    serde: ['1.0.190', '1.0.200', '1.0.210']
    
steps:
  - name: Update Serde Version
    run: |
      cd engine
      sed -i 's/serde = .*/serde = { version = "${{ matrix.serde }}", features = ["derive"] }/' Cargo.toml
      
  - name: Run Tests
    run: |
      cd engine
      cargo test --all
```

## Definition of Done Checklist

- [x] Code implemented in `src/serialization.rs`
- [x] Module exported in `src/lib.rs`
- [x] Unit tests cover all functions
- [x] Integration tests verify acceptance criteria
- [x] Example program demonstrates usage
- [x] Documentation written (SERIALIZATION_GUIDE.md)
- [x] Verification checklist created (this file)
- [ ] Code reviewed by team member
- [ ] CI/CD tests pass
- [ ] Manual verification completed on 2+ environments
- [ ] Hash consistency verified across serde versions
- [ ] Performance benchmarks acceptable (<10% overhead)
- [ ] Documentation reviewed
- [ ] Branch merged to main

## Performance Benchmarks

### Expected Performance

- Serialization overhead: <10% vs standard `serde_json::to_string()`
- Hash computation: <1ms for typical proposals
- Memory overhead: Minimal (BTreeMap allocation)

### Benchmark Tests

```bash
cargo bench --bench serialization_bench
```

**Expected results:**
- Standard serialization: ~100-200 µs
- Canonical serialization: ~110-220 µs
- Hash computation: ~500-1000 µs (includes SHA-256)

## Troubleshooting

### Tests fail with "Hash mismatch"

1. Check serde version: `cargo tree | grep serde`
2. Verify no custom serializers interfere
3. Check for floating-point values (use strings instead)
4. Review test output for differences

### Different hashes on different machines

1. Verify both use same code version
2. Check for environment-specific data (timestamps, etc.)
3. Ensure addresses are normalized
4. Verify amounts use string representation

### CI/CD tests pass but manual test fails

1. Check Rust toolchain version: `rustc --version`
2. Verify dependencies are identical: `cargo tree`
3. Look for platform-specific issues
4. Check for timezone/locale differences in data

## References

- Implementation: `engine/src/serialization.rs`
- Unit Tests: `engine/src/serialization.rs` (mod tests)
- Integration Tests: `engine/tests/serialization_integration_tests.rs`
- Documentation: `engine/SERIALIZATION_GUIDE.md`
- Example: `engine/examples/deterministic_hashing.rs`
- Issue: [Link to GitHub issue]
- PR: [Link to pull request]
