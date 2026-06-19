# Vero Core Engine — Development Roadmap

> 50 fully-specified issues across 5 milestones. Each issue follows the Gold Standard format.

---

## Milestone 1 — Core Contracts (Issues 1–10)

---

## Issue #1: Contract Initialization & Access-Control Harness
**Milestone:** M1 — Core Contracts
**Labels:** `contracts`, `security`, `access-control`
**Priority:** Critical

### Description
Establish the foundational initialization routine for the `vero-core` Soroban contract, including an owner/admin role registry and a permission-check harness used by every subsequent contract function. This harness must be immutable after deployment unless explicitly upgraded via the upgrade path defined in Issue #8. All privileged operations must gate on role membership verified through this module.

### Problem Statement
Without a well-defined initialization and access-control layer, any account can invoke administrative functions such as pausing the contract, upgrading logic, or modifying state-machine parameters. A missing or bypassed access-control harness is the single most common root cause of smart-contract exploits and would make the entire protocol insecure by default.

### Technical Requirements
- [ ] Implement an `initialize(admin: Address, operators: Vec<Address>)` entry point that can only be called once (idempotency guard via persistent storage flag)
- [ ] Define role constants: `ROLE_ADMIN`, `ROLE_OPERATOR`, `ROLE_AUDITOR` stored in contract persistent storage
- [ ] Provide `require_role(env: &Env, caller: Address, role: Symbol)` helper that panics with a typed error on failure
- [ ] Emit an `Initialized { admin, timestamp }` contract event upon successful initialization

### Implementation Guide
1. Create `contracts/vero-core/src/access.rs` with role storage keys and `require_role` implementation.
2. Add `initialize` function to `contracts/vero-core/src/lib.rs`; write to `INIT_FLAG` key and `ADMIN_KEY` in persistent storage.
3. Gate every existing stub function with `require_role` calls appropriate to their privilege level.
4. Write unit tests in `contracts/vero-core/src/tests/access_tests.rs` covering: double-init rejection, role assignment, unauthorized call rejection.

### Acceptance Criteria
- [ ] Calling `initialize` a second time returns `AlreadyInitialized` error
- [ ] A non-admin caller invoking an admin-only function receives `Unauthorized` error
- [ ] `Initialized` event appears in the transaction meta with correct fields
- [ ] All unit tests pass under `cargo test`

### Security & Audit Considerations
The initialization function must be protected against front-running on deployment: use a constructor pattern or include a deployer-address check so only the deployer can call `initialize`. Role storage keys must be namespaced to avoid collision with application state keys. Ensure `require_role` uses `Address::require_auth()` to validate the caller's signature rather than trusting a passed parameter.

### Definition of Done
- [ ] Unit tests written and passing
- [ ] Integration test added
- [ ] Documentation updated
- [ ] Security review completed (if applicable)
- [ ] CI pipeline green
- [ ] PR linked to this issue

---

## Issue #2: State-Machine Enforcement with Overflow Guards
**Milestone:** M1 — Core Contracts
**Labels:** `contracts`, `state-machine`, `safety`
**Priority:** Critical

### Description
Implement a typed state-machine for the core protocol lifecycle (e.g., `Pending → Active → Settled → Closed`) with explicit transition guards that prevent invalid state jumps. All arithmetic operations on balances and counters must use checked or saturating math to eliminate integer overflow/underflow vulnerabilities.

### Problem Statement
Without enforced state transitions, an attacker or buggy client can invoke functions out of sequence — for example, settling a transaction that was never activated — leading to incorrect fund distribution or locked assets. Unchecked arithmetic in Soroban contracts compiled to WASM can silently wrap on overflow, enabling balance manipulation attacks.

### Technical Requirements
- [ ] Define `ContractState` enum (`Pending`, `Active`, `Settled`, `Closed`) stored in persistent storage
- [ ] Implement `transition_state(env, expected: ContractState, next: ContractState)` that atomically validates and updates state
- [ ] Replace all raw arithmetic (`+`, `-`, `*`) on `i128`/`u128` values with `checked_add`, `checked_sub`, `checked_mul` — panic with `ArithmeticOverflow` on `None`
- [ ] Emit `StateTransition { from, to, timestamp }` event on every valid transition

### Implementation Guide
1. Add `contracts/vero-core/src/state.rs` defining the `ContractState` enum and storage key.
2. Implement `transition_state` with a match guard; return typed `ContractError::InvalidTransition` if current state != expected.
3. Audit all arithmetic in existing stubs; replace with checked variants and add a `math.rs` helper module with safe wrappers.
4. Add unit tests for each valid transition path and each invalid transition path.

### Acceptance Criteria
- [ ] All 12 valid state transitions succeed; all invalid transitions return `InvalidTransition`
- [ ] Arithmetic overflow on any balance field panics with `ArithmeticOverflow` (verified via unit test with `i128::MAX`)
- [ ] `StateTransition` events are emitted and queryable
- [ ] No `clippy` warnings related to unchecked arithmetic

### Security & Audit Considerations
State transitions must be atomic — read and write in the same contract invocation with no intermediate observable state. Confirm that `transition_state` is not externally callable without role gating (Issue #1). Test wrap-around behavior explicitly with boundary values. Document which transitions are irreversible.

### Definition of Done
- [ ] Unit tests written and passing
- [ ] Integration test added
- [ ] Documentation updated
- [ ] Security review completed (if applicable)
- [ ] CI pipeline green
- [ ] PR linked to this issue

---

## Issue #3: ZK-Audit Hook Interface
**Milestone:** M1 — Core Contracts
**Labels:** `contracts`, `zk`, `audit`
**Priority:** High

### Description
Define and implement a stable hook interface that allows an external ZK proof verifier to attest to contract state transitions. The interface exposes a `register_proof(proof_hash: BytesN<32>, metadata: Map<Symbol, Val>)` entry point and emits structured events consumed by the off-chain ZK audit layer. This is the on-chain anchor for the ZK pipeline described in the architecture.

### Problem Statement
Without a defined hook interface, the ZK audit layer has no stable on-chain surface to write proof attestations against. Ad-hoc event emission would break the off-chain verifier on every contract upgrade and provide no integrity guarantee that a proof corresponds to a specific state transition.

### Technical Requirements
- [ ] Define `ZkProofRegistered { proof_hash: BytesN<32>, state_root: BytesN<32>, block_seq: u32, metadata: Map<Symbol, Val> }` event schema
- [ ] Implement `register_proof` entry point callable only by `ROLE_AUDITOR`
- [ ] Store the latest `proof_hash` in contract instance storage keyed by `state_root`
- [ ] Provide `get_proof(state_root: BytesN<32>) -> Option<BytesN<32>>` read-only query

### Implementation Guide
1. Create `contracts/vero-core/src/zk_hooks.rs` with event struct definitions and storage logic.
2. Implement `register_proof` with role check, storage write, and event emission.
3. Implement `get_proof` as a read-only `#[contractimpl]` method.
4. Write tests: valid registration, duplicate registration (should overwrite with new hash), unauthorized registration.

### Acceptance Criteria
- [ ] `register_proof` called by a non-auditor returns `Unauthorized`
- [ ] Proof hash is retrievable via `get_proof` after registration
- [ ] `ZkProofRegistered` event is emitted with all fields populated
- [ ] Interface ABI is stable (no breaking changes after initial merge)

### Security & Audit Considerations
Proof hashes must be 32-byte cryptographic digests — validate length before storage. Metadata map must be size-bounded to prevent storage griefing. Consider whether `register_proof` should require the auditor to also provide a Merkle inclusion path to the state root to prevent fabricated registrations. Document the off-chain ZK circuit assumptions clearly.

### Definition of Done
- [ ] Unit tests written and passing
- [ ] Integration test added
- [ ] Documentation updated
- [ ] Security review completed (if applicable)
- [ ] CI pipeline green
- [ ] PR linked to this issue

---

## Issue #4: Reentrancy and Front-Running Protections
**Milestone:** M1 — Core Contracts
**Labels:** `contracts`, `security`, `reentrancy`
**Priority:** Critical

### Description
Implement defensive patterns against reentrancy attacks and front-running exploitation in all state-mutating contract functions. While Soroban's execution model mitigates classic EVM reentrancy, cross-contract call patterns require explicit guards. Front-running mitigations include commit-reveal schemes and minimum-delay enforcement on sensitive operations.

### Problem Statement
Cross-contract calls in Soroban can still produce reentrancy-like vulnerabilities if state is read before an external call and written after. Without protections, an adversary controlling another contract in the call chain can observe pending state and submit competing transactions, draining funds or stealing execution priority.

### Technical Requirements
- [ ] Implement a `reentrancy_guard` using a temporary storage lock flag set at function entry and cleared at exit
- [ ] Add a `nonce` field to sensitive operations (settlement, withdrawal) to prevent replay and front-running
- [ ] Enforce a configurable `MIN_DELAY_LEDGERS` between operation submission and execution for high-value transfers
- [ ] Write a cross-contract call test scenario that verifies the reentrancy guard triggers correctly

### Implementation Guide
1. Add `contracts/vero-core/src/guards.rs` with `enter_guard(env)` / `exit_guard(env)` using temporary storage.
2. Wrap all state-mutating public functions with guard entry/exit using Rust's drop guard pattern.
3. Add `nonce` tracking to `OperationRequest` struct and validate nonce increments atomically.
4. Implement `MIN_DELAY_LEDGERS` check in the execution path, reading the submission ledger from storage.

### Acceptance Criteria
- [ ] Simulated reentrant call via a mock contract returns `ReentrancyDetected` error
- [ ] Replaying an operation with a used nonce returns `InvalidNonce`
- [ ] An operation submitted and immediately executed (0-ledger delay) is rejected when delay is configured > 0
- [ ] All existing tests continue to pass with guards in place

### Security & Audit Considerations
Verify that temporary storage is cleared in all exit paths, including panic/error exits. The commit-reveal pattern must hash the operation parameters with a user-provided salt; ensure the salt is not predictable. Document the ledger-delay parameter as a governance-controlled value (Issue #46). Review Soroban's cross-contract invocation semantics to confirm no storage isolation gaps.

### Definition of Done
- [ ] Unit tests written and passing
- [ ] Integration test added
- [ ] Documentation updated
- [ ] Security review completed (if applicable)
- [ ] CI pipeline green
- [ ] PR linked to this issue

---

## Issue #5: Fuzz-Test Harness (cargo-fuzz)
**Milestone:** M1 — Core Contracts
**Labels:** `contracts`, `testing`, `fuzzing`
**Priority:** High

### Description
Set up a `cargo-fuzz` harness targeting all public contract entry points, with structured input generation using `arbitrary` to produce valid-looking but boundary-testing Soroban invocation arguments. The harness should run in CI on every PR and maintain a corpus of interesting inputs in the repository.

### Problem Statement
Unit tests cover known scenarios but cannot explore the full input space of contract functions. Without fuzz testing, subtle panics, arithmetic edge cases, and unexpected state transitions caused by malformed inputs will only be discovered in production, potentially after funds are at risk.

### Technical Requirements
- [ ] Add `contracts/vero-core/fuzz/` directory with `Cargo.toml` configuring `libfuzzer-sys`
- [ ] Implement `fuzz_target!` targets for: `initialize`, `register_proof`, `transition_state`, and all balance-mutating functions
- [ ] Derive `Arbitrary` for all public input structs to enable structured fuzzing
- [ ] Integrate fuzz runs into CI with a 60-second timeout per target using `cargo fuzz run --jobs 4 -- -max_total_time=60`

### Implementation Guide
1. Install `cargo-fuzz` and scaffold `fuzz/Cargo.toml` with appropriate workspace dependencies.
2. Create one fuzz target file per entry point under `fuzz/fuzz_targets/`.
3. Implement `Arbitrary` derivations for `OperationRequest`, `ZkProofMetadata`, and other input types.
4. Add a `fuzz` job to `.github/workflows/contracts.yml` that runs all targets with the corpus seed.

### Acceptance Criteria
- [ ] `cargo fuzz list` shows all entry-point targets
- [ ] Each target runs for 60 seconds in CI without panics not caught by `catch_unwind`
- [ ] Corpus directory contains at least 10 seed inputs per target
- [ ] Any new panic discovered by the fuzzer automatically opens a GitHub issue via CI script

### Security & Audit Considerations
Fuzz targets must not execute against a live network — use the in-process Soroban test environment. Corpus files should be reviewed before committing to avoid accidentally including sensitive data. Crashes found by the fuzzer must be triaged within 48 hours. Consider adding `AddressSanitizer` and `UBSan` builds to the fuzz CI job.

### Definition of Done
- [ ] Unit tests written and passing
- [ ] Integration test added
- [ ] Documentation updated
- [ ] Security review completed (if applicable)
- [ ] CI pipeline green
- [ ] PR linked to this issue

---

## Issue #6: Formal Invariant Specification (TLA+)
**Milestone:** M1 — Core Contracts
**Labels:** `contracts`, `formal-verification`, `documentation`
**Priority:** Medium

### Description
Write a TLA+ specification covering the core state-machine invariants of `vero-core`: valid state transitions, balance conservation, role membership monotonicity, and the liveness property that every `Active` state eventually reaches `Settled` or `Closed`. The spec serves as the authoritative reference for auditors and reviewers.

### Problem Statement
Without a formal specification, developers rely on informal reasoning about invariants, leading to subtle logic errors that pass code review. Auditors lack a machine-checkable reference, and there is no automated way to verify that a proposed state-machine change preserves all invariants.

### Technical Requirements
- [ ] Create `docs/formal/vero_core.tla` covering: state variables, initial predicate, next-state relation, and invariants
- [ ] Define `BalanceConservation` invariant: total issued balances equal total deposited assets at all times
- [ ] Define `RoleMonotonicity` invariant: role revocation requires admin approval (no self-demotion)
- [ ] Run TLC model checker with at least 3 initial states and verify no invariant violations

### Implementation Guide
1. Install TLA+ Toolbox or use the `tla-bin` npm package for CI integration.
2. Draft the `VARIABLES` section covering `contractState`, `balances`, `roles`, `nonces`.
3. Define `Init`, `Next`, and all action predicates for each state transition.
4. Add `INVARIANTS` clause and run TLC; resolve any counterexamples before merging.

### Acceptance Criteria
- [ ] TLC model checker reports zero invariant violations across all reachable states
- [ ] Spec covers all 4 defined contract states and all valid transitions
- [ ] `BalanceConservation` and `RoleMonotonicity` invariants are explicitly named and checked
- [ ] CI job runs TLC and fails the build on any violation

### Security & Audit Considerations
The TLA+ spec should be treated as a living document — any change to contract state-machine logic requires a corresponding spec update and TLC re-run before the PR can merge. The spec should be shared with external auditors as part of the audit package. Consider publishing the spec alongside the contract ABI in the docs site.

### Definition of Done
- [ ] Unit tests written and passing
- [ ] Integration test added
- [ ] Documentation updated
- [ ] Security review completed (if applicable)
- [ ] CI pipeline green
- [ ] PR linked to this issue

---

## Issue #7: Emergency Pause / Circuit-Breaker
**Milestone:** M1 — Core Contracts
**Labels:** `contracts`, `security`, `emergency`
**Priority:** Critical

### Description
Implement an emergency pause mechanism that allows the admin (or a designated pause guardian) to halt all state-mutating contract functions within a single transaction. The pause must be reversible by the admin and must emit auditable events. All paused function calls must return a descriptive error rather than silently failing.

### Problem Statement
Without a circuit-breaker, a discovered exploit or critical bug cannot be contained without deploying a contract upgrade, which takes time. A pause mechanism provides a first-response tool to freeze the contract while a fix is prepared, minimizing the window of exposure and potential fund loss.

### Technical Requirements
- [ ] Add `PAUSED: bool` to persistent storage, defaulting to `false` on initialization
- [ ] Implement `pause(env, reason: String)` callable by `ROLE_ADMIN` or `ROLE_PAUSE_GUARDIAN`
- [ ] Implement `unpause(env)` callable only by `ROLE_ADMIN`
- [ ] Add `require_not_paused(env)` guard macro applied to all state-mutating functions; returns `ContractPaused` error when active

### Implementation Guide
1. Add `PAUSE_GUARDIAN` role to the access-control harness (Issue #1).
2. Implement `pause` / `unpause` in `contracts/vero-core/src/circuit_breaker.rs`.
3. Create `require_not_paused!` macro and apply it at the top of every `#[contractimpl]` state-mutating method.
4. Emit `ContractPaused { reason, guardian, timestamp }` and `ContractUnpaused { admin, timestamp }` events.

### Acceptance Criteria
- [ ] Calling any state-mutating function while paused returns `ContractPaused`
- [ ] Read-only query functions remain accessible while paused
- [ ] Only admin can unpause; pause guardian cannot unpause
- [ ] `ContractPaused` event is emitted with reason string and caller identity

### Security & Audit Considerations
The pause guardian role should be a multisig address in production, not a single EOA, to prevent a single compromised key from maliciously pausing the contract. The `reason` string must be length-capped to prevent storage griefing. Consider adding a maximum pause duration after which the contract auto-unpauses to prevent admin key loss from permanently locking user funds.

### Definition of Done
- [ ] Unit tests written and passing
- [ ] Integration test added
- [ ] Documentation updated
- [ ] Security review completed (if applicable)
- [ ] CI pipeline green
- [ ] PR linked to this issue

---

## Issue #8: Contract Upgrade Path with Storage Migration
**Milestone:** M1 — Core Contracts
**Labels:** `contracts`, `upgrade`, `migration`
**Priority:** High

### Description
Design and implement a safe contract upgrade mechanism using Soroban's `update_current_contract_wasm` instruction, paired with a storage migration framework that transforms persistent storage schemas between versions. Upgrades must be gated by the multi-sig governance module and must include a dry-run migration validation step before the WASM is swapped.

### Problem Statement
Without a defined upgrade path, any bug fix or feature addition requires deploying a new contract address, breaking all integrations and losing historical state. An unsafe upgrade (WASM swap without migration) risks corrupting persistent storage, making the contract unusable or exploitable.

### Technical Requirements
- [ ] Implement `upgrade(env, new_wasm_hash: BytesN<32>)` callable only by governance multi-sig (Issue #41)
- [ ] Define a `STORAGE_VERSION: u32` persistent key incremented with each migration
- [ ] Implement `migrate(env, from_version: u32)` that applies incremental migration steps up to the current version
- [ ] Ensure `migrate` is idempotent — calling it twice on the same version is a no-op

### Implementation Guide
1. Add `contracts/vero-core/src/upgrade.rs` with `upgrade` and `migrate` implementations.
2. Define a `MigrationStep` trait with `applies_to_version() -> u32` and `run(env: &Env)`.
3. Register migration steps in a `MIGRATIONS` static slice; `migrate` iterates and applies only steps where version >= from_version.
4. Write integration tests simulating a full upgrade cycle: deploy v1, write state, upgrade to v2, call migrate, verify state integrity.

### Acceptance Criteria
- [ ] Calling `upgrade` with an invalid WASM hash returns `InvalidWasmHash`
- [ ] Post-upgrade `migrate` correctly transforms v1 storage to v2 schema
- [ ] Double-calling `migrate` on current version is a no-op with no state change
- [ ] `ContractUpgraded { old_hash, new_hash, version, timestamp }` event is emitted

### Security & Audit Considerations
The upgrade function must require multi-sig authorization (Issue #41) and a time-lock delay (Issue #43) to prevent flash upgrades. The WASM hash must be verified against a pre-approved hash stored in governance storage. Migration steps must be tested on a fork of mainnet state before production deployment. Never allow arbitrary storage key deletion in migration steps.

### Definition of Done
- [ ] Unit tests written and passing
- [ ] Integration test added
- [ ] Documentation updated
- [ ] Security review completed (if applicable)
- [ ] CI pipeline green
- [ ] PR linked to this issue

---

## Issue #9: Multi-Asset Support
**Milestone:** M1 — Core Contracts
**Labels:** `contracts`, `multi-asset`, `stellar`
**Priority:** High

### Description
Extend the `vero-core` contract to handle multiple Stellar asset types (native XLM, classic assets via SEP-0011 identifiers, and Soroban token interface assets) within a single contract instance. Balance accounting must be per-asset, and all operations must specify the asset they act on.

### Problem Statement
The initial single-asset implementation limits the protocol to XLM-only use cases. Without multi-asset support, the engine cannot serve DEX settlement, stablecoin remittance, or any cross-asset application — blocking adoption of the entire platform in its primary target markets.

### Technical Requirements
- [ ] Define `AssetId` enum: `Native`, `Classic(String)`, `Soroban(Address)` and implement serialization
- [ ] Replace single `balance: i128` storage with `Map<AssetId, i128>` keyed per asset
- [ ] All entry points that touch balances must accept an `asset: AssetId` parameter
- [ ] Implement `get_balance(env, asset: AssetId, account: Address) -> i128` read-only query

### Implementation Guide
1. Define `AssetId` in `contracts/vero-core/src/assets.rs` with `Eq`, `Hash`, and `TryFromVal` implementations.
2. Refactor balance storage from a flat key to a composite `(BALANCE_PREFIX, asset_id, account_address)` key.
3. Update all entry points to accept and thread `asset: AssetId` through internal calls.
4. Add tests for each asset type: native deposit/withdrawal, classic asset round-trip, Soroban token interface interaction.

### Acceptance Criteria
- [ ] Simultaneous balances for XLM, USDC (classic), and a Soroban token are independently tracked
- [ ] Operating on one asset does not affect another asset's balance
- [ ] `get_balance` returns correct values for all three asset types
- [ ] Invalid `AssetId` serializations return `InvalidAsset` error

### Security & Audit Considerations
Asset identifiers must be canonicalized before use as storage keys to prevent aliasing attacks (e.g., two representations of the same classic asset mapping to different balances). Soroban token interface calls should use the standard `TokenClient` and verify the returned values before crediting internal balances. Validate that classic asset strings conform to SEP-0011 format.

### Definition of Done
- [ ] Unit tests written and passing
- [ ] Integration test added
- [ ] Documentation updated
- [ ] Security review completed (if applicable)
- [ ] CI pipeline green
- [ ] PR linked to this issue

---

## Issue #10: Gas / Fee Benchmarking Suite
**Milestone:** M1 — Core Contracts
**Labels:** `contracts`, `performance`, `benchmarking`
**Priority:** Medium

### Description
Build a benchmarking suite that measures the Soroban instruction count, memory footprint, and ledger storage consumption for every public contract entry point. Results must be tracked over time to detect regressions and inform fee estimation for users and integrators.

### Problem Statement
Without instrumented benchmarks, contract developers have no visibility into resource consumption trends. A single unnoticed regression can make a contract function unaffordable to call on mainnet, breaking user-facing features silently and only discovered by users paying inflated fees.

### Technical Requirements
- [ ] Use `soroban-sdk`'s budget tracking (`env.budget()`) to record CPU instructions and memory bytes per entry point call
- [ ] Implement a benchmark runner script at `scripts/benchmark_contracts.ts` that invokes each entry point and records results to `benchmarks/results.json`
- [ ] Add a CI regression check that fails if any entry point exceeds 110% of its baseline instruction count
- [ ] Generate a markdown report `benchmarks/report.md` with a table of current vs. baseline metrics

### Implementation Guide
1. Add budget inspection calls at the end of each test in a dedicated `benches/` module within the contract crate.
2. Write `scripts/benchmark_contracts.ts` using the Stellar RPC simulate endpoint to collect real resource estimates.
3. Store baseline values in `benchmarks/baseline.json`; CI compares current results against baseline.
4. Add `npm run benchmark` script and integrate it into the weekly scheduled CI job.

### Acceptance Criteria
- [ ] All 10+ entry points have recorded baselines in `benchmarks/baseline.json`
- [ ] CI regression check passes on current code and fails on an intentionally bloated test branch
- [ ] `benchmarks/report.md` is auto-updated on each CI run
- [ ] A 20% instruction increase on any function triggers a `benchmark-regression` CI job failure

### Security & Audit Considerations
Benchmark results should be recorded with the contract WASM hash so that baseline comparisons are pinned to a specific build. Ensure that benchmark scripts cannot be used to probe production contract state — they must run against the local test environment only. Regression thresholds should be reviewed quarterly as Soroban fee schedules evolve.

### Definition of Done
- [ ] Unit tests written and passing
- [ ] Integration test added
- [ ] Documentation updated
- [ ] Security review completed (if applicable)
- [ ] CI pipeline green
- [ ] PR linked to this issue

---

## Milestone 2 — Relayer Service (Issues 11–20)

---

## Issue #11: Core Event Consumer with Horizon Streaming
**Milestone:** M2 — Relayer Service
**Labels:** `relayer`, `horizon`, `events`
**Priority:** Critical

### Description
Build the foundational event consumer service that subscribes to the Horizon SSE event stream for `vero-core` contract events, parses them into typed domain objects, and dispatches them to internal processing pipelines. This is the entry point for all off-chain processing and must be reliable, resumable, and observable.

### Problem Statement
Without a functioning event consumer, no off-chain processing occurs — signed receipts are not generated, ZK proofs are not triggered, and the analytics dashboard has no data. The entire relayer service depends on this component being operational and correctly parsing every contract event.

### Technical Requirements
- [ ] Implement `HorizonStreamClient` in `relayer/src/horizon/client.ts` using `EventSource` with automatic reconnect and exponential backoff
- [ ] Parse raw Horizon event payloads into typed `ContractEvent` union types matching the contract's ABI
- [ ] Track the last processed ledger cursor in persistent storage and resume from cursor on restart
- [ ] Expose a health-check endpoint `GET /health/stream` returning stream lag (ledgers behind tip)

### Implementation Guide
1. Add `horizon-event-source` or native `EventSource` dependency; implement `HorizonStreamClient` with configurable `contractId` and `startLedger`.
2. Define TypeScript types in `relayer/src/types/events.ts` for each contract event (use `zod` for runtime validation).
3. Implement cursor persistence in `relayer/src/store/cursor.ts` using a SQLite or file-based store.
4. Wire the stream client into the main service entrypoint with graceful shutdown on SIGTERM.

### Acceptance Criteria
- [ ] Service connects to Horizon SSE endpoint and receives events within 5 seconds of contract invocation
- [ ] On restart, service resumes from the persisted cursor without re-processing old events
- [ ] Invalid event payloads are logged with full raw payload and routed to the dead-letter queue (Issue #15)
- [ ] `GET /health/stream` returns `{ status: "ok", lagLedgers: N }` under normal operation

### Security & Audit Considerations
The Horizon endpoint URL must be validated against a whitelist to prevent SSRF attacks if the URL is configurable at runtime. Event payloads must be validated against the expected schema before processing — never trust raw Horizon data without parsing. TLS must be enforced for all Horizon connections; reject HTTP connections in production mode.

### Definition of Done
- [ ] Unit tests written and passing
- [ ] Integration test added
- [ ] Documentation updated
- [ ] Security review completed (if applicable)
- [ ] CI pipeline green
- [ ] PR linked to this issue

---

## Issue #12: Signed Receipt Generation (Ed25519)
**Milestone:** M2 — Relayer Service
**Labels:** `relayer`, `cryptography`, `receipts`
**Priority:** Critical

### Description
Implement the receipt generation pipeline that transforms parsed contract events into signed JSON receipts using Ed25519 signatures. Each receipt binds a contract event to a relayer identity, a timestamp, and a content hash, providing non-repudiable proof that the relayer observed a specific event at a specific time.

### Problem Statement
Without signed receipts, consumers of the relayer's output have no cryptographic guarantee that the data was not tampered with after observation. Unsigned receipts also provide no attribution — in a multi-relayer scenario, there is no way to identify which relayer produced a given receipt or hold them accountable.

### Technical Requirements
- [ ] Generate `ReceiptV1` JSON objects with fields: `version`, `event_hash`, `contract_id`, `ledger_seq`, `timestamp`, `relayer_pubkey`, `signature`
- [ ] Sign receipts using `@noble/ed25519` with the relayer's private key loaded from an HSM or environment variable
- [ ] Write receipts to `./data/receipts/{ledger_seq}/{event_hash}.json` with atomic write (temp file + rename)
- [ ] Expose `GET /receipts/:eventHash` endpoint returning the signed receipt JSON

### Implementation Guide
1. Define `ReceiptV1` TypeScript interface and JSON schema in `relayer/src/types/receipt.ts`.
2. Implement `ReceiptSigner` in `relayer/src/signing/signer.ts` loading key from `RELAYER_SIGNING_KEY` env var.
3. Implement atomic file write using `fs.rename` after writing to a `.tmp` file in the same directory.
4. Add receipt retrieval endpoint to the Express/Fastify API server.

### Acceptance Criteria
- [ ] Every processed event produces a corresponding `.json` receipt file on disk
- [ ] Receipt signature verifies correctly using the relayer's public key with `@noble/ed25519`
- [ ] Atomic write ensures no partial receipt files exist on disk after a crash
- [ ] `GET /receipts/:eventHash` returns `404` for unknown hashes and the full receipt for known ones

### Security & Audit Considerations
The relayer's signing private key must never be logged, stored in plaintext in config files, or included in error messages. Use `RELAYER_SIGNING_KEY` as an environment variable injected at runtime. In production, integrate with AWS KMS or HashiCorp Vault for key storage. Receipt files should have `0644` permissions; the receipts directory should be `0700`.

### Definition of Done
- [ ] Unit tests written and passing
- [ ] Integration test added
- [ ] Documentation updated
- [ ] Security review completed (if applicable)
- [ ] CI pipeline green
- [ ] PR linked to this issue

---

## Issue #13: Multi-Relayer Consensus Protocol
**Milestone:** M2 — Relayer Service
**Labels:** `relayer`, `consensus`, `distributed-systems`
**Priority:** High

### Description
Design and implement a lightweight consensus protocol allowing multiple independent relayer instances to agree on the canonical receipt for each contract event. The protocol must tolerate up to `f` Byzantine or crashed relayers (where `2f+1` relayers are needed for quorum) and produce an aggregated multi-signature receipt.

### Problem Statement
A single relayer is a central point of failure and trust. If it goes offline, no receipts are generated; if it is compromised, it can produce fraudulent receipts. A multi-relayer consensus protocol eliminates both failure modes by requiring agreement from a quorum of independent operators before a receipt is considered final.

### Technical Requirements
- [ ] Implement a gossip-based protocol where each relayer broadcasts its signed receipt to all peers within 2 ledger periods
- [ ] Define `AggregatedReceipt` containing a set of `N` individual signatures where `N >= QUORUM_SIZE`
- [ ] Implement quorum detection: when `QUORUM_SIZE` matching receipts are collected, promote to `AggregatedReceipt`
- [ ] Configure peer list and quorum size via `RELAYER_PEERS` and `RELAYER_QUORUM_SIZE` environment variables

### Implementation Guide
1. Implement `PeerClient` in `relayer/src/consensus/peer.ts` with HTTP/2 push for receipt broadcasting.
2. Implement `QuorumCollector` that accumulates signatures per event hash and triggers aggregation at threshold.
3. Serialize `AggregatedReceipt` to disk alongside individual receipts in `./data/receipts/{ledger_seq}/{event_hash}.agg.json`.
4. Add a `GET /consensus/:eventHash` endpoint returning the aggregation status and signature count.

### Acceptance Criteria
- [ ] With 3 relayers and `QUORUM_SIZE=2`, an `AggregatedReceipt` is produced when any 2 relayers agree
- [ ] If a relayer submits a mismatched event hash, it is logged and excluded from quorum counting
- [ ] Consensus state is persisted and survives a relayer restart without re-collecting signatures
- [ ] `GET /consensus/:eventHash` returns `{ status: "pending" | "finalized", sigCount: N }`

### Security & Audit Considerations
Peer communication must use mutual TLS to prevent a compromised relayer from impersonating a legitimate peer. Receipt signatures must be verified before being counted toward quorum — never trust a peer's aggregated signature without re-verification. Implement rate limiting on the peer gossip endpoint to prevent DoS via signature flooding.

### Definition of Done
- [ ] Unit tests written and passing
- [ ] Integration test added
- [ ] Documentation updated
- [ ] Security review completed (if applicable)
- [ ] CI pipeline green
- [ ] PR linked to this issue

---

## Issue #14: Persistent Event Queue with Replay
**Milestone:** M2 — Relayer Service
**Labels:** `relayer`, `queue`, `durability`
**Priority:** High

### Description
Implement a durable, ordered event queue that buffers incoming Horizon events before processing and supports full replay from any ledger cursor. The queue must survive process restarts, support at-least-once delivery semantics, and allow operators to replay specific ledger ranges for recovery scenarios.

### Problem Statement
Without persistence, events received during a relayer crash or processing error are lost permanently. There is no way to recover missed events or re-process incorrect results. At-least-once delivery is required to guarantee that every contract event produces a signed receipt, even under adverse conditions.

### Technical Requirements
- [ ] Implement an append-only event log using SQLite (`better-sqlite3`) with schema: `(id, ledger_seq, event_hash, payload, status, created_at, processed_at)`
- [ ] Implement `EventQueue.enqueue(event)`, `EventQueue.dequeue(batchSize)`, and `EventQueue.ack(id)` methods
- [ ] Implement `EventQueue.replayFrom(ledger: number)` that resets `status` to `pending` for all events at or after the given ledger
- [ ] Add `POST /admin/replay` endpoint accepting `{ fromLedger: number }` for operator-triggered replays

### Implementation Guide
1. Create `relayer/src/queue/EventQueue.ts` with SQLite-backed implementation using WAL mode for concurrency.
2. Wrap all event processing in a transaction: dequeue → process → ack; failed processing leaves the event in `pending` state.
3. Implement a background retry loop that re-processes `pending` events older than `RETRY_AFTER_SECONDS`.
4. Write integration tests simulating crash mid-processing and verifying replay delivers the event exactly once.

### Acceptance Criteria
- [ ] Events are persisted to SQLite before processing begins
- [ ] After a simulated crash (process kill -9), restarted relayer processes all unacknowledged events
- [ ] `replayFrom(ledger)` correctly resets and reprocesses all events from the specified ledger
- [ ] Queue depth and retry count are exposed as Prometheus metrics

### Security & Audit Considerations
The SQLite database file must not be accessible via the HTTP API. Replay operations must require admin authentication to prevent unauthorized event re-injection. Validate that replayed events have not been tampered with by comparing stored payload hashes against re-fetched Horizon data. Set a maximum queue depth limit to prevent disk exhaustion.

### Definition of Done
- [ ] Unit tests written and passing
- [ ] Integration test added
- [ ] Documentation updated
- [ ] Security review completed (if applicable)
- [ ] CI pipeline green
- [ ] PR linked to this issue

---

## Issue #15: Dead-Letter Queue and Alerting
**Milestone:** M2 — Relayer Service
**Labels:** `relayer`, `reliability`, `alerting`
**Priority:** High

### Description
Implement a dead-letter queue (DLQ) for events that fail processing after a configurable number of retries, along with an alerting system that notifies operators via PagerDuty/Slack/email when events enter the DLQ. The DLQ must support manual inspection and requeue operations.

### Problem Statement
Without a DLQ, permanently failing events are either silently dropped (losing data) or block the main processing pipeline indefinitely (causing backpressure). Operators have no visibility into failed events and no operational tooling to investigate and recover them.

### Technical Requirements
- [ ] Move events to a `dead_letter_events` table after `MAX_RETRY_COUNT` (configurable, default 5) failed attempts
- [ ] Implement `DLQManager.list(limit, offset)`, `DLQManager.inspect(id)`, and `DLQManager.requeue(id)` methods
- [ ] Send an alert webhook (configurable URL + secret) within 60 seconds of any event entering the DLQ
- [ ] Expose `GET /admin/dlq` and `POST /admin/dlq/:id/requeue` endpoints

### Implementation Guide
1. Extend the SQLite schema with `dead_letter_events` table including `failure_reason` and `retry_count` columns.
2. Implement `DLQManager` in `relayer/src/queue/DLQManager.ts` with full CRUD operations.
3. Implement `AlertDispatcher` in `relayer/src/alerts/dispatcher.ts` supporting webhook and structured log output.
4. Wire DLQ promotion into the retry loop after `MAX_RETRY_COUNT` exhaustion.

### Acceptance Criteria
- [ ] An event that fails processing 5 times is moved to the DLQ and does not block subsequent events
- [ ] An alert webhook fires within 60 seconds of DLQ entry (verified in integration test with mock webhook server)
- [ ] `GET /admin/dlq` returns paginated list of dead-lettered events with failure reasons
- [ ] `POST /admin/dlq/:id/requeue` moves the event back to `pending` and it is subsequently processed

### Security & Audit Considerations
The DLQ admin endpoints must require authentication (Issue #29 RBAC) — unauthenticated access to requeue could allow an attacker to replay events. Alert webhook payloads must not include raw event data that could leak sensitive information; include only event ID, ledger, and error category. Webhook secrets must be HMAC-validated to ensure alerts are not spoofed.

### Definition of Done
- [ ] Unit tests written and passing
- [ ] Integration test added
- [ ] Documentation updated
- [ ] Security review completed (if applicable)
- [ ] CI pipeline green
- [ ] PR linked to this issue

---

## Issue #16: Horizontal Scaling with Leader Election
**Milestone:** M2 — Relayer Service
**Labels:** `relayer`, `scaling`, `distributed-systems`
**Priority:** High

### Description
Enable the relayer service to run as multiple concurrent instances with a leader-election mechanism that designates one instance as the active Horizon stream consumer while others remain in hot-standby. On leader failure, a standby takes over within one ledger period without event loss.

### Problem Statement
A single relayer instance cannot provide high availability. Without leader election, running multiple instances causes duplicate event processing and conflicting receipt writes. Without automatic failover, a crashed relayer causes a gap in receipt coverage until an operator manually restarts it.

### Technical Requirements
- [ ] Implement leader election using Redis `SET NX PX` (or etcd lease) with a `LEADER_TTL_MS` heartbeat
- [ ] Only the leader instance subscribes to the Horizon SSE stream; standbys poll the leader heartbeat key
- [ ] On leader failure (TTL expiry), the first standby to acquire the lock becomes leader and resumes from the persisted cursor
- [ ] Expose `GET /status` returning `{ role: "leader" | "standby", leaderId: string, uptimeMs: number }`

### Implementation Guide
1. Implement `LeaderElector` in `relayer/src/cluster/leader.ts` using `ioredis` with `SET leader:{instanceId} NX PX {ttl}`.
2. Run a heartbeat loop every `LEADER_TTL_MS / 3` that renews the lock if held or attempts acquisition if not.
3. Add a state machine in the main service that starts/stops the Horizon stream client based on leadership state.
4. Write a multi-process integration test that kills the leader process and verifies standby promotion within 2 seconds.

### Acceptance Criteria
- [ ] Two relayer instances running against the same Redis: only one subscribes to Horizon at any time
- [ ] Killing the leader process causes a standby to promote within one `LEADER_TTL_MS` window
- [ ] No events are double-processed during leadership transition (verified by receipt file count)
- [ ] `GET /status` accurately reflects current leadership role on each instance

### Security & Audit Considerations
The Redis connection must use TLS and authentication. The leader lock key must be instance-specific (include instance UUID) so only the lock holder can renew it — preventing a slow instance from incorrectly extending a lock it no longer holds. Implement a fencing token to prevent split-brain writes during network partitions.

### Definition of Done
- [ ] Unit tests written and passing
- [ ] Integration test added
- [ ] Documentation updated
- [ ] Security review completed (if applicable)
- [ ] CI pipeline green
- [ ] PR linked to this issue

---

## Issue #17: gRPC Streaming API
**Milestone:** M2 — Relayer Service
**Labels:** `relayer`, `api`, `grpc`
**Priority:** Medium

### Description
Expose a gRPC streaming API that allows downstream consumers (dashboard, bridges, external auditors) to subscribe to the real-time receipt stream and consensus state. The API must be defined with Protocol Buffers, versioned, and accompanied by a generated TypeScript client SDK.

### Problem Statement
The HTTP REST API for receipts is pull-based and requires polling, introducing latency and unnecessary load. Without a push-based streaming API, the dashboard cannot display real-time data, and external consumers must implement their own polling loops with no guaranteed ordering or delivery semantics.

### Technical Requirements
- [ ] Define `relayer/proto/receipts/v1/receipts.proto` with `ReceiptStream`, `ConsensusStream`, and `QueryReceipt` RPC definitions
- [ ] Implement gRPC server using `@grpc/grpc-js` with streaming handlers for `WatchReceipts` and `WatchConsensus`
- [ ] Generate TypeScript client stubs using `protoc` with `ts-proto`; publish to `relayer/src/generated/`
- [ ] Implement server-side filtering: clients can subscribe to events by `contractId`, `assetId`, or `ledgerRange`

### Implementation Guide
1. Write `receipts.proto` with `message Receipt`, `message ConsensusUpdate`, and `service RelayerService`.
2. Implement `RelayerServiceImpl` in `relayer/src/grpc/service.ts` connecting to the internal event bus.
3. Add `protoc` codegen to the build pipeline via `package.json` script.
4. Write an integration test using the generated client to verify streaming delivery of 10 consecutive receipts.

### Acceptance Criteria
- [ ] `WatchReceipts` stream delivers new receipts to connected clients within 500ms of generation
- [ ] Clients can filter by `contractId` and receive only matching receipts
- [ ] gRPC server handles 100 concurrent streaming connections without memory leak
- [ ] Proto definitions are backward-compatible (no field removal or type changes between minor versions)

### Security & Audit Considerations
gRPC server must require mutual TLS in production mode. Implement per-client rate limiting to prevent a single consumer from starving others. The `QueryReceipt` RPC must not expose the relayer's private signing key or internal state beyond receipt data. Validate all incoming filter parameters against an allowlist to prevent injection.

### Definition of Done
- [ ] Unit tests written and passing
- [ ] Integration test added
- [ ] Documentation updated
- [ ] Security review completed (if applicable)
- [ ] CI pipeline green
- [ ] PR linked to this issue

---

## Issue #18: Rate-Limiting and Back-Pressure
**Milestone:** M2 — Relayer Service
**Labels:** `relayer`, `reliability`, `performance`
**Priority:** Medium

### Description
Implement rate-limiting on all inbound API endpoints and back-pressure mechanisms on the event processing pipeline to prevent overload during high-throughput periods. The system must gracefully degrade under load by queuing excess work rather than dropping it, while protecting downstream dependencies from being overwhelmed.

### Problem Statement
Without rate limiting, a single misbehaving consumer can exhaust relayer resources, causing latency spikes for all other consumers. Without back-pressure on the processing pipeline, a burst of contract events can overwhelm the signing and consensus modules, leading to receipt generation failures and data gaps.

### Technical Requirements
- [ ] Implement per-IP and per-API-key rate limiting on HTTP/gRPC endpoints using a sliding window algorithm
- [ ] Add a semaphore-based concurrency limiter on the event processing pipeline (configurable `MAX_CONCURRENT_PROCESSING`)
- [ ] Implement a bounded in-memory buffer between the Horizon stream reader and the processing workers; apply back-pressure (pause stream) when buffer is full
- [ ] Return `429 Too Many Requests` with `Retry-After` header on rate limit breach

### Implementation Guide
1. Integrate `@fastify/rate-limit` (or custom Redis-backed sliding window) for HTTP endpoints.
2. Implement `ProcessingSemaphore` in `relayer/src/pipeline/semaphore.ts` using `async-sema` or manual promise queue.
3. Add buffer depth monitoring: when `bufferDepth >= MAX_BUFFER_SIZE * 0.9`, pause the SSE stream reader.
4. Expose `RATE_LIMIT_WINDOW_MS`, `RATE_LIMIT_MAX_REQUESTS`, and `MAX_CONCURRENT_PROCESSING` as env config.

### Acceptance Criteria
- [ ] Sending 1000 requests/second from a single IP triggers `429` responses after the configured threshold
- [ ] With `MAX_CONCURRENT_PROCESSING=5`, at most 5 events are processed simultaneously (verified by timing test)
- [ ] SSE stream is paused when buffer reaches 90% capacity and resumed when it drops below 50%
- [ ] Rate limit metrics (accepted, throttled, queued) are exposed as Prometheus counters

### Security & Audit Considerations
Rate limit keys must be based on authenticated identity (API key or mTLS client cert CN) in addition to IP address, as IP-based limiting alone is easily bypassed with proxies. Ensure that rate limit counters are stored in Redis (not in-process) so limits apply consistently across all relayer instances in the cluster.

### Definition of Done
- [ ] Unit tests written and passing
- [ ] Integration test added
- [ ] Documentation updated
- [ ] Security review completed (if applicable)
- [ ] CI pipeline green
- [ ] PR linked to this issue

---

## Issue #19: Relayer Operator SDK
**Milestone:** M2 — Relayer Service
**Labels:** `relayer`, `sdk`, `developer-experience`
**Priority:** Medium

### Description
Publish a TypeScript/JavaScript SDK that wraps the relayer's gRPC and REST APIs, providing operators and integrators with a high-level client for subscribing to receipts, querying consensus state, and managing the relayer lifecycle. The SDK must be well-documented, tree-shakeable, and published to npm.

### Problem Statement
Without an SDK, every consumer must hand-roll HTTP clients and gRPC stubs from raw API documentation. This leads to inconsistent integration patterns, duplicated error handling, and a high barrier to adoption for third-party developers building on top of the relayer.

### Technical Requirements
- [ ] Create `packages/relayer-sdk/` as a workspace package with separate ESM and CJS builds
- [ ] Expose `RelayerClient` class with methods: `watchReceipts(filter)`, `getReceipt(hash)`, `getConsensusStatus(hash)`, `getHealth()`
- [ ] Bundle generated proto stubs and provide automatic reconnect logic for streaming subscriptions
- [ ] Publish to npm as `@vero/relayer-sdk` with full TypeScript type definitions

### Implementation Guide
1. Scaffold `packages/relayer-sdk/` with `tsconfig.json`, `package.json`, and `rollup.config.ts` for dual ESM/CJS output.
2. Copy generated proto stubs into the SDK package; wrap with ergonomic async iterators for streaming methods.
3. Implement `RelayerClient` with constructor accepting `{ endpoint, apiKey, tlsCert }` config.
4. Write JSDoc comments for all public APIs; configure `typedoc` to generate API reference docs.

### Acceptance Criteria
- [ ] `npm install @vero/relayer-sdk` and the quickstart example runs without errors
- [ ] `watchReceipts()` returns an `AsyncIterable<Receipt>` that automatically reconnects on stream drop
- [ ] All public methods have TypeScript types with no `any` usage
- [ ] SDK bundle size (ESM) is under 50 KB gzipped

### Security & Audit Considerations
The SDK must not log API keys or TLS private keys at any log level. API key handling must use constant-time comparison if the SDK performs any local validation. Document that API keys should be stored in environment variables, not hardcoded. Include a security policy in the npm package README.

### Definition of Done
- [ ] Unit tests written and passing
- [ ] Integration test added
- [ ] Documentation updated
- [ ] Security review completed (if applicable)
- [ ] CI pipeline green
- [ ] PR linked to this issue

---

## Issue #20: End-to-End Latency Benchmarks
**Milestone:** M2 — Relayer Service
**Labels:** `relayer`, `performance`, `benchmarking`
**Priority:** Medium

### Description
Implement an end-to-end latency benchmark suite measuring the time from a contract event being included in a Stellar ledger to a signed receipt being available via the relayer API. Benchmarks must run against a local Stellar validator and produce P50/P95/P99 latency distributions.

### Problem Statement
Without latency benchmarks, there is no objective measure of relayer performance and no way to detect regressions introduced by new features. SLA commitments to integrators cannot be made or monitored without empirical latency data across varying load conditions.

### Technical Requirements
- [ ] Implement `scripts/benchmark_relayer.ts` that submits N contract invocations and records time-to-receipt for each
- [ ] Measure and report: ledger-to-event (Horizon propagation), event-to-receipt (signing latency), and total end-to-end latency
- [ ] Run benchmarks at three load levels: 1 TPS, 10 TPS, and 50 TPS; report P50/P95/P99 per level
- [ ] Store results in `benchmarks/relayer_results.json` and compare against baselines in CI

### Implementation Guide
1. Write `benchmark_relayer.ts` using the Stellar SDK to submit transactions and the relayer SDK (Issue #19) to poll for receipts.
2. Record high-resolution timestamps at: transaction submission, Horizon event delivery, receipt file write, and receipt API availability.
3. Compute percentiles using the `percentile` npm package; output a formatted table to stdout.
4. Add a `benchmark:relayer` npm script and a weekly CI schedule trigger.

### Acceptance Criteria
- [ ] P95 end-to-end latency at 1 TPS is under 5 seconds on the local testnet
- [ ] Benchmark results at all three load levels are recorded without errors
- [ ] A 50% regression in P95 latency triggers a CI benchmark-regression alert
- [ ] Benchmark report includes environment metadata (Stellar CLI version, relayer version, machine specs)

### Security & Audit Considerations
Benchmark scripts must only run against local or testnet environments — add a guard that checks `STELLAR_NETWORK != "mainnet"` before executing. Benchmark results should not be committed with real account keys or sensitive config embedded. Ensure benchmark load does not interfere with other concurrent CI jobs by isolating the local validator instance.

### Definition of Done
- [ ] Unit tests written and passing
- [ ] Integration test added
- [ ] Documentation updated
- [ ] Security review completed (if applicable)
- [ ] CI pipeline green
- [ ] PR linked to this issue

---

## Milestone 3 — Dashboard Analytics (Issues 21–30)

---

## Issue #21: Real-Time Event Feed (WebSocket)
**Milestone:** M3 — Dashboard Analytics
**Labels:** `dashboard`, `websocket`, `real-time`
**Priority:** High

### Description
Implement a WebSocket server on the dashboard API that pushes live contract events and relayer receipts to connected browser clients as they arrive. The feed must support event-type filtering, handle client reconnections gracefully, and broadcast to all subscribed clients with sub-second latency.

### Problem Statement
Without a real-time feed, the dashboard must poll the API for updates, introducing latency that makes the "live" view stale by several seconds and generating unnecessary server load. Operators and auditors need immediate visibility into contract activity, especially during incidents.

### Technical Requirements
- [ ] Implement WebSocket server using `ws` library in `dashboard/src/api/websocket.ts`
- [ ] Support subscription filtering via initial handshake message: `{ filter: { eventTypes: string[], contractId?: string } }`
- [ ] Broadcast `ContractEvent`, `ReceiptGenerated`, and `ConsensusFinalized` message types to matching subscribers
- [ ] Implement heartbeat ping/pong every 30 seconds; disconnect clients that miss 2 consecutive pongs

### Implementation Guide
1. Create `dashboard/src/api/websocket.ts` with `WebSocketServer` wrapping the existing HTTP server.
2. Implement `SubscriptionManager` maintaining a `Map<WebSocket, SubscriptionFilter>` for targeted broadcasting.
3. Wire the relayer SDK's `watchReceipts` stream into the broadcast pipeline.
4. Add connection count and message throughput to Prometheus metrics.

### Acceptance Criteria
- [ ] Browser client receives a new event within 500ms of it being processed by the relayer
- [ ] A client subscribing to `eventTypes: ["ReceiptGenerated"]` does not receive `ConsensusFinalized` messages
- [ ] Disconnected clients that reconnect within 60 seconds receive missed events via a catch-up replay
- [ ] Server handles 500 concurrent WebSocket connections without memory leak (verified by load test)

### Security & Audit Considerations
WebSocket connections must be authenticated using a JWT or API key passed in the initial handshake. Unauthenticated connections must be rejected after a 5-second grace period. Validate all incoming subscription filter messages against a strict schema to prevent prototype pollution or injection. Implement a per-connection message rate limit to prevent flood attacks.

### Definition of Done
- [ ] Unit tests written and passing
- [ ] Integration test added
- [ ] Documentation updated
- [ ] Security review completed (if applicable)
- [ ] CI pipeline green
- [ ] PR linked to this issue

---

## Issue #22: Contract State Explorer
**Milestone:** M3 — Dashboard Analytics
**Labels:** `dashboard`, `explorer`, `contracts`
**Priority:** High

### Description
Build a contract state explorer UI component and supporting API that allows users to inspect the current and historical state of the `vero-core` contract — including current lifecycle state, asset balances, role assignments, and the latest ZK proof hash. State is fetched via the Stellar RPC `getLedgerEntries` API.

### Problem Statement
Without a state explorer, operators must use raw CLI commands to inspect contract state, which is error-prone and inaccessible to non-technical stakeholders. Auditors require a human-readable view of contract state at any historical ledger to validate that the contract behaved correctly.

### Technical Requirements
- [ ] Implement `GET /api/contract/:contractId/state` returning the full deserialized contract state as JSON
- [ ] Implement `GET /api/contract/:contractId/state?ledger=N` for historical state at ledger N using archival RPC
- [ ] Build a React component `ContractStatePanel` displaying state fields with human-readable labels and change indicators
- [ ] Poll for state updates every 5 seconds when the panel is visible; use WebSocket (Issue #21) when available

### Implementation Guide
1. Create `dashboard/src/api/routes/contract.ts` with state fetching logic using `@stellar/stellar-sdk`.
2. Implement XDR deserialization for all known contract storage key types; return structured JSON.
3. Build `ContractStatePanel.tsx` React component with field-level change highlighting using `usePrevious` hook.
4. Add loading skeleton and error boundary for RPC failures.

### Acceptance Criteria
- [ ] State panel displays current contract lifecycle state, total balances per asset, and active role count
- [ ] Historical state query for any ledger in the last 1000 returns correct data
- [ ] State changes since the last poll are highlighted in the UI for 3 seconds
- [ ] RPC errors display a user-friendly message with retry option, not a raw stack trace

### Security & Audit Considerations
The state explorer API must validate `contractId` against a whitelist of known contracts to prevent arbitrary RPC proxying. Historical ledger queries should be rate-limited to prevent archival RPC abuse. Ensure that role assignment display does not leak sensitive account addresses to unauthorized dashboard users.

### Definition of Done
- [ ] Unit tests written and passing
- [ ] Integration test added
- [ ] Documentation updated
- [ ] Security review completed (if applicable)
- [ ] CI pipeline green
- [ ] PR linked to this issue

---

## Issue #23: Transaction Volume Analytics
**Milestone:** M3 — Dashboard Analytics
**Labels:** `dashboard`, `analytics`, `metrics`
**Priority:** Medium

### Description
Implement a time-series analytics pipeline that aggregates contract event data into transaction volume metrics (events per hour/day/week, value transferred per asset, unique accounts active) and exposes them via a charting API. Charts are rendered in the dashboard using a lightweight visualization library.

### Problem Statement
Without aggregated analytics, the dashboard only shows raw event feeds with no ability to spot trends, anomalies, or growth patterns. Business stakeholders need summary views to understand protocol health and usage, and these cannot be derived in real-time from raw event streams at scale.

### Technical Requirements
- [ ] Create a SQLite analytics table `event_aggregates(bucket_start, bucket_duration, event_type, asset_id, count, total_value)`
- [ ] Implement a background aggregation worker that rolls up events into 1-hour, 1-day, and 7-day buckets
- [ ] Expose `GET /api/analytics/volume?asset=&period=&resolution=` returning time-series JSON
- [ ] Render volume charts in the dashboard using `recharts` with hover tooltips and zoom controls

### Implementation Guide
1. Create `dashboard/src/workers/aggregator.ts` that runs every 5 minutes, computing bucket aggregates from raw event logs.
2. Define the `event_aggregates` SQLite schema with indexes on `(bucket_start, asset_id, event_type)`.
3. Implement the analytics API route with query parameter validation using `zod`.
4. Build `VolumeChart.tsx` using `recharts` `AreaChart` with responsive container.

### Acceptance Criteria
- [ ] Volume chart displays correct hourly buckets for the last 24 hours, verified against raw event count
- [ ] API supports `resolution=1h|1d|7d` and `asset=XLM|USDC|all` query parameters
- [ ] Aggregation worker completes a 24-hour rollup in under 10 seconds
- [ ] Chart updates automatically when new data arrives (WebSocket or polling)

### Security & Audit Considerations
Analytics queries must be parameterized — never interpolate user-provided `asset` or `period` values directly into SQL. Implement query result caching with a 60-second TTL to prevent database overload from dashboard polling. Ensure aggregated metrics do not inadvertently reveal account-level data that should be private.

### Definition of Done
- [ ] Unit tests written and passing
- [ ] Integration test added
- [ ] Documentation updated
- [ ] Security review completed (if applicable)
- [ ] CI pipeline green
- [ ] PR linked to this issue

---

## Issue #24: Anomaly Detection Engine
**Milestone:** M3 — Dashboard Analytics
**Labels:** `dashboard`, `security`, `anomaly-detection`
**Priority:** High

### Description
Implement a rule-based and statistical anomaly detection engine that monitors the event stream for suspicious patterns: unusual transaction volumes, large single transfers, rapid state transitions, and deviation from historical baselines. Detected anomalies are surfaced in the dashboard with severity levels and trigger alerts.

### Problem Statement
Without automated anomaly detection, operators must manually monitor raw event feeds to identify attacks or bugs, which is impractical at scale. By the time a human notices an anomaly, significant damage may already have occurred. Automated detection reduces the mean time to detect (MTTD) for incidents from hours to seconds.

### Technical Requirements
- [ ] Implement configurable rule engine in `dashboard/src/anomaly/rules.ts` with rules: `LargeTransfer(threshold)`, `VolumeSpike(multiplier, window)`, `RapidStateChange(maxPerMinute)`, and `UnknownAsset`
- [ ] Implement a statistical baseline model using rolling Z-score on 7-day event volume history
- [ ] Store detected anomalies in `anomaly_alerts(id, rule, severity, event_hash, detected_at, acknowledged_at)`
- [ ] Emit anomaly alerts to the WebSocket feed (Issue #21) and the webhook alerting system (Issue #15)

### Implementation Guide
1. Create `dashboard/src/anomaly/engine.ts` as a pipeline stage that evaluates each incoming event against all active rules.
2. Implement `ZScoreBaseline` using the rolling 7-day aggregate from Issue #23's analytics tables.
3. Build `AnomalyAlertPanel.tsx` in the dashboard showing unacknowledged alerts with severity badges.
4. Implement `POST /api/anomaly/:id/acknowledge` endpoint for operators to dismiss alerts.

### Acceptance Criteria
- [ ] A transfer exceeding the `LargeTransfer` threshold generates a `HIGH` severity alert within 2 seconds
- [ ] A volume 3x above the 7-day rolling average triggers a `VolumeSpike` alert
- [ ] Acknowledged alerts are hidden from the default view and retained in history for 30 days
- [ ] False positive rate in integration tests with synthetic normal traffic is under 1%

### Security & Audit Considerations
Anomaly detection rules must be configurable by admins only (RBAC Issue #29) to prevent an attacker from disabling detection. Anomaly alert data must be retained even after acknowledgment for forensic purposes. Ensure the detection engine cannot be used as an oracle by external parties to probe the contract's behavioral limits.

### Definition of Done
- [ ] Unit tests written and passing
- [ ] Integration test added
- [ ] Documentation updated
- [ ] Security review completed (if applicable)
- [ ] CI pipeline green
- [ ] PR linked to this issue

---

## Issue #25: Historical State Diff Viewer
**Milestone:** M3 — Dashboard Analytics
**Labels:** `dashboard`, `audit`, `history`
**Priority:** Medium

### Description
Build a UI tool that displays a visual diff of contract storage between any two ledger numbers, showing which storage keys were added, modified, or deleted. This tool is essential for post-incident analysis and for auditors verifying that an upgrade migration (Issue #8) produced the expected storage changes.

### Problem Statement
Without a state diff viewer, analyzing the impact of a contract upgrade or investigating a suspicious transaction requires manually querying and comparing raw XDR data — a time-consuming and error-prone process. Auditors need a clear, reproducible view of what changed and when.

### Technical Requirements
- [ ] Implement `GET /api/contract/:contractId/diff?from=LEDGER_A&to=LEDGER_B` returning added/modified/removed key-value pairs
- [ ] Deserialize XDR storage values into human-readable JSON for known key types
- [ ] Build `StateDiffViewer.tsx` component with a two-column diff layout, color-coded changes
- [ ] Support permalink URLs encoding the selected ledger range for sharing with auditors

### Implementation Guide
1. Implement state fetching for two ledgers using archival RPC in `dashboard/src/api/routes/diff.ts`.
2. Implement `diffStorageMaps(before, after)` utility returning `{ added, modified, removed }` categorized changes.
3. Build the diff viewer using a `react-diff-viewer` or custom implementation with syntax highlighting.
4. Add permalink routing using URL query params (`?contractId=&from=&to=`).

### Acceptance Criteria
- [ ] Diff between two consecutive ledgers shows only the storage keys changed by the transaction in that ledger
- [ ] Added keys are shown in green, removed in red, modified with before/after values
- [ ] Permalink URL correctly restores the diff view when opened in a new browser tab
- [ ] XDR deserialization correctly renders all known storage types; unknown types show raw hex

### Security & Audit Considerations
Archival RPC calls for historical diffs must be rate-limited and cached to prevent abuse. Ensure that raw XDR display does not expose internal key naming patterns that could assist an attacker in crafting storage collision exploits. Access to the diff viewer should be restricted to authenticated users (Issue #29).

### Definition of Done
- [ ] Unit tests written and passing
- [ ] Integration test added
- [ ] Documentation updated
- [ ] Security review completed (if applicable)
- [ ] CI pipeline green
- [ ] PR linked to this issue

---

## Issue #26: Relayer Health Monitor
**Milestone:** M3 — Dashboard Analytics
**Labels:** `dashboard`, `monitoring`, `relayer`
**Priority:** High

### Description
Build a relayer health monitoring dashboard that displays the status of all known relayer instances: stream lag, queue depth, DLQ size, consensus participation rate, and signing key validity. The monitor polls the relayer fleet's `/health` and `/status` endpoints and aggregates results into a single fleet-wide view.

### Problem Statement
Without a health monitor, operators cannot distinguish between a relayer that is healthy, degraded, or completely offline without manually checking each instance. Degraded relayers generate incomplete receipt coverage, but this is invisible without centralized health visibility.

### Technical Requirements
- [ ] Implement `dashboard/src/services/RelayerFleetMonitor.ts` polling all configured relayer endpoints every 30 seconds
- [ ] Aggregate per-instance metrics into fleet-wide stats: min/max/avg stream lag, total DLQ size, consensus quorum met (yes/no)
- [ ] Build `RelayerHealthPanel.tsx` showing per-instance status cards with color-coded health indicators
- [ ] Alert via WebSocket (Issue #21) when any instance's stream lag exceeds `MAX_ACCEPTABLE_LAG_LEDGERS`

### Implementation Guide
1. Configure relayer endpoint list via `RELAYER_ENDPOINTS` env variable (comma-separated URLs).
2. Implement parallel polling using `Promise.allSettled` with a 5-second timeout per endpoint.
3. Map health response fields to a `RelayerInstanceStatus` TypeScript interface.
4. Build status cards using a traffic-light color scheme (green/amber/red) based on lag and DLQ thresholds.

### Acceptance Criteria
- [ ] Dashboard shows correct status for all configured relayer instances within 35 seconds of a status change
- [ ] An offline relayer is shown as `UNREACHABLE` with the last known timestamp
- [ ] Quorum indicator turns amber when fewer than `QUORUM_SIZE` relayers are healthy
- [ ] All health poll failures are logged with instance URL and error type

### Security & Audit Considerations
Fleet monitor polling must authenticate to relayer health endpoints using a shared API key or mTLS. Health endpoint responses must not include sensitive internal state (private key identifiers, raw database paths). Ensure the monitor cannot be used to enumerate relayer infrastructure details by unauthorized users.

### Definition of Done
- [ ] Unit tests written and passing
- [ ] Integration test added
- [ ] Documentation updated
- [ ] Security review completed (if applicable)
- [ ] CI pipeline green
- [ ] PR linked to this issue

---

## Issue #27: ZK-Proof Status Dashboard
**Milestone:** M3 — Dashboard Analytics
**Labels:** `dashboard`, `zk`, `audit`
**Priority:** Medium

### Description
Build a ZK-proof status panel in the dashboard that displays the latest registered proof hash for each state root, the proof generation latency, and whether the on-chain proof registry (Issue #3) is current with the latest contract state. Integrates with the off-chain ZK audit layer to show proof pipeline health.

### Problem Statement
Without visibility into ZK-proof status, operators and auditors cannot verify that the ZK audit layer is functioning correctly. A silently stalled proof pipeline means the system is operating without its primary auditability guarantee, but no one is alerted until an external audit request fails.

### Technical Requirements
- [ ] Implement `GET /api/zk/proofs?limit=20` returning the latest registered proofs with state roots and timestamps
- [ ] Implement `GET /api/zk/lag` returning the ledgers-since-last-proof metric
- [ ] Build `ZkProofPanel.tsx` showing proof history, current lag, and a warning when lag exceeds `MAX_PROOF_LAG_LEDGERS`
- [ ] Subscribe to `ZkProofRegistered` events via WebSocket (Issue #21) to update the panel in real-time

### Implementation Guide
1. Index `ZkProofRegistered` events from the relayer receipt store into a `zk_proofs` SQLite table.
2. Implement the API routes in `dashboard/src/api/routes/zk.ts`.
3. Build the panel component with a timeline visualization of recent proof registrations.
4. Add a configurable `MAX_PROOF_LAG_LEDGERS` threshold with a visible alert banner when exceeded.

### Acceptance Criteria
- [ ] Panel displays the last 20 proof registrations with state root hashes truncated for readability
- [ ] Lag indicator updates within 5 seconds of a new proof being registered on-chain
- [ ] A warning banner appears when no proof has been registered in the last `MAX_PROOF_LAG_LEDGERS` ledgers
- [ ] Proof hash links open the Stellar explorer transaction view in a new tab

### Security & Audit Considerations
Proof hash display must show the full 32-byte hash (64 hex characters) without truncation in the detail view to prevent hash collision confusion. The lag threshold should be configurable only by admins. Ensure the ZK proof data source is the on-chain contract registry, not an off-chain database that could be manipulated.

### Definition of Done
- [ ] Unit tests written and passing
- [ ] Integration test added
- [ ] Documentation updated
- [ ] Security review completed (if applicable)
- [ ] CI pipeline green
- [ ] PR linked to this issue

---

## Issue #28: CSV/JSON Audit Export
**Milestone:** M3 — Dashboard Analytics
**Labels:** `dashboard`, `audit`, `export`
**Priority:** Medium

### Description
Implement an audit export system that allows authorized users to download complete, verifiable datasets of contract events, signed receipts, and state transitions for any time range in CSV or JSON format. Exports must include a manifest with checksums to allow offline verification of data integrity.

### Problem Statement
External auditors and compliance teams require bulk data exports for periodic audits. Without an export feature, they must either use the API page-by-page (slow and error-prone) or get direct database access (a security risk). An unverifiable export has no value in a compliance context.

### Technical Requirements
- [ ] Implement `POST /api/export` accepting `{ format: "csv"|"json", from: ISO8601, to: ISO8601, types: string[] }` and returning a job ID
- [ ] Process exports asynchronously; poll via `GET /api/export/:jobId/status` and download via `GET /api/export/:jobId/download`
- [ ] Include a `manifest.json` in each export with: file hash (SHA-256), record count, export timestamp, and exporter identity
- [ ] Sign the manifest with the dashboard API's Ed25519 key so its authenticity can be verified offline

### Implementation Guide
1. Implement export job queue in `dashboard/src/services/ExportService.ts` with async processing.
2. Stream records from SQLite in batches to avoid loading entire datasets into memory.
3. For CSV: use `csv-stringify`; for JSON: write as newline-delimited JSON (NDJSON) for streaming compatibility.
4. Compute SHA-256 of the output file using a streaming hash; include in `manifest.json`.

### Acceptance Criteria
- [ ] A JSON export of 10,000 events completes in under 30 seconds
- [ ] The manifest SHA-256 matches the downloaded file (verified by test using `crypto.createHash`)
- [ ] Exports older than 7 days are automatically cleaned up from the server
- [ ] CSV export opens correctly in Excel and LibreOffice with correct column headers

### Security & Audit Considerations
Export downloads must require authentication and the requesting user's identity must be recorded in the manifest. Implement rate limiting on `POST /api/export` to prevent resource exhaustion via large export requests. Exports must never include raw signing keys or internal infrastructure identifiers. Set a maximum export window of 90 days to limit query scope.

### Definition of Done
- [ ] Unit tests written and passing
- [ ] Integration test added
- [ ] Documentation updated
- [ ] Security review completed (if applicable)
- [ ] CI pipeline green
- [ ] PR linked to this issue

---

## Issue #29: Role-Based Access Control for Dashboard
**Milestone:** M3 — Dashboard Analytics
**Labels:** `dashboard`, `security`, `rbac`
**Priority:** Critical

### Description
Implement a role-based access control system for the dashboard API and UI, defining roles (`Viewer`, `Operator`, `Admin`, `Auditor`) with distinct permission sets. All API endpoints must be gated by role checks, and the UI must adapt to show only features accessible to the authenticated user's role.

### Problem Statement
Without RBAC, any authenticated user can access every dashboard feature including admin functions (replay, DLQ requeue, anomaly rule management), creating a severe privilege escalation risk. Regulatory compliance frameworks (SOC 2, ISO 27001) require demonstrable access control over audit data.

### Technical Requirements
- [ ] Define role hierarchy: `Viewer < Auditor < Operator < Admin` with explicit permission sets per role
- [ ] Implement JWT-based authentication with role claim; verify JWT on every API request using middleware
- [ ] Add `requireRole(role)` middleware to all API routes; return `403 Forbidden` on insufficient privilege
- [ ] Persist users and role assignments in `dashboard_users(id, email, role, created_at)` SQLite table

### Implementation Guide
1. Implement `AuthMiddleware` in `dashboard/src/middleware/auth.ts` using `jsonwebtoken` library.
2. Define `PERMISSIONS` map in `dashboard/src/auth/permissions.ts` mapping each role to allowed route patterns.
3. Apply `requireRole` middleware to all existing and future API routes.
4. Build `UserManagementPage.tsx` (Admin only) for creating users and assigning roles.

### Acceptance Criteria
- [ ] A `Viewer` role user receives `403` when accessing `/api/admin/replay`
- [ ] JWT expiry is enforced; expired tokens return `401 Unauthorized`
- [ ] Admin can create, update role, and deactivate users via the user management UI
- [ ] All `403` and `401` responses are logged with user identity and requested resource

### Security & Audit Considerations
JWTs must use RS256 (asymmetric) signing in production — never HS256 with a shared secret in a multi-service architecture. Token expiry should be 1 hour with refresh token rotation. Implement token revocation via a Redis denylist for immediate logout capability. All role assignment changes must be logged to an immutable audit trail.

### Definition of Done
- [ ] Unit tests written and passing
- [ ] Integration test added
- [ ] Documentation updated
- [ ] Security review completed (if applicable)
- [ ] CI pipeline green
- [ ] PR linked to this issue

---

## Issue #30: Mobile-Responsive UI
**Milestone:** M3 — Dashboard Analytics
**Labels:** `dashboard`, `ui`, `accessibility`
**Priority:** Medium

### Description
Ensure the entire dashboard UI renders correctly and is fully functional on mobile viewports (320px–768px) and tablet viewports (768px–1024px). All charts, tables, and panels must reflow gracefully, and touch interactions must be equivalent to mouse interactions for all primary user workflows.

### Problem Statement
Operators often need to check relayer health or review anomaly alerts while away from their desks. A desktop-only UI excludes mobile use cases entirely, reducing the speed of incident response. Accessibility regulations in many jurisdictions also require responsive design for enterprise tools.

### Technical Requirements
- [ ] Implement a responsive CSS grid/flexbox layout system using TailwindCSS breakpoints (`sm`, `md`, `lg`)
- [ ] Convert all fixed-width tables to horizontally scrollable containers on small viewports with sticky first column
- [ ] Ensure all interactive elements (buttons, inputs, chart tooltips) meet WCAG 2.1 AA touch target size (44×44px minimum)
- [ ] Test all primary user workflows on mobile using Playwright's mobile viewport emulation

### Implementation Guide
1. Audit all existing components for fixed-width elements; replace with responsive Tailwind classes.
2. Wrap tables in `overflow-x-auto` containers; use `sticky left-0` for the first column on key tables.
3. Increase button and icon button padding to meet the 44px touch target requirement.
4. Add Playwright test scenarios using `iPhone 12` and `iPad` device presets for the 5 primary user workflows.

### Acceptance Criteria
- [ ] Dashboard is fully functional on 375px viewport width (iPhone SE) with no horizontal overflow
- [ ] All WCAG 2.1 AA color contrast requirements pass in Lighthouse audit (score ≥ 90)
- [ ] Primary workflows (view events, check relayer health, acknowledge anomaly) complete on mobile Playwright tests
- [ ] No JavaScript errors in the browser console on any supported viewport size

### Security & Audit Considerations
Mobile responsiveness changes must not introduce CSP violations or inline style injections. Ensure that touch-accessible buttons do not inadvertently expose admin functions on shared/kiosk devices — session timeout should be reduced to 15 minutes on mobile user-agent strings. Review that responsive layout changes do not obscure security-critical information (e.g., alert banners).

### Definition of Done
- [ ] Unit tests written and passing
- [ ] Integration test added
- [ ] Documentation updated
- [ ] Security review completed (if applicable)
- [ ] CI pipeline green
- [ ] PR linked to this issue

---

## Milestone 4 — Security & Audit Tools (Issues 31–40)

---

## Issue #31: Automated Static Analysis Pipeline
**Milestone:** M4 — Security & Audit Tools
**Labels:** `security`, `static-analysis`, `ci`
**Priority:** Critical

### Description
Integrate automated static analysis tools into the CI pipeline for all code layers: `clippy` + `cargo-audit` for Rust contracts, `eslint` + `semgrep` for TypeScript services, and `hadolint` for Dockerfiles. Analysis results must block merges on critical findings and produce a unified SARIF report uploaded to GitHub Code Scanning.

### Problem Statement
Without automated static analysis, security vulnerabilities and code quality issues are only caught during manual code review, which is inconsistent and easy to skip under deadline pressure. Critical vulnerabilities in smart contracts or the relayer signing pipeline can result in fund loss or receipt forgery.

### Technical Requirements
- [ ] Configure `clippy` with `#![deny(clippy::all, clippy::pedantic)]` in the contracts crate and fix all existing warnings
- [ ] Add `cargo-audit` to the CI pipeline checking all contract dependencies against the RustSec advisory database
- [ ] Configure `semgrep` with the `p/typescript` and `p/nodejs` rule sets for the relayer and dashboard
- [ ] Upload SARIF results from all tools to GitHub Code Scanning via `github/codeql-action/upload-sarif@v3`

### Implementation Guide
1. Create `.github/workflows/static-analysis.yml` running on every PR with jobs for each tool.
2. Add `clippy.toml` to the contracts crate with deny-list for known dangerous patterns.
3. Configure `semgrep.yml` with custom rules for Soroban-specific anti-patterns (e.g., unchecked environment access).
4. Aggregate SARIF outputs using `sarif-tools` before upload to provide a single unified report.

### Acceptance Criteria
- [ ] PR with an intentional `clippy::unwrap_used` violation is blocked by CI
- [ ] `cargo-audit` fails the build when a critical advisory exists for any contract dependency
- [ ] SARIF reports appear in the GitHub Security tab for every PR
- [ ] Zero suppressed findings without an accompanying justification comment

### Security & Audit Considerations
Static analysis suppression annotations (`#[allow(clippy::...)]` or `// nosemgrep`) must require a comment explaining the rationale and be reviewed in PR. Keep tool versions pinned in CI to prevent supply-chain attacks via updated rule sets changing behavior unexpectedly. Rotate the `GITHUB_TOKEN` permissions used for SARIF upload to write-only on the security-events scope.

### Definition of Done
- [ ] Unit tests written and passing
- [ ] Integration test added
- [ ] Documentation updated
- [ ] Security review completed (if applicable)
- [ ] CI pipeline green
- [ ] PR linked to this issue

---

## Issue #32: Dependency Vulnerability Scanner
**Milestone:** M4 — Security & Audit Tools
**Labels:** `security`, `dependencies`, `supply-chain`
**Priority:** High

### Description
Implement continuous dependency vulnerability scanning for all package managers in the monorepo: `cargo audit` for Rust, `npm audit` for Node.js packages, and `trivy` for Docker base images. Scans run on every PR, daily on `main`, and produce actionable reports with remediation guidance.

### Problem Statement
Third-party dependencies are a significant attack surface. A single vulnerable dependency in the relayer's signing pipeline or the contract build toolchain can compromise the entire system. Without continuous scanning, known CVEs remain unpatched for extended periods as teams focus on feature development.

### Technical Requirements
- [ ] Run `cargo audit --deny warnings` in the contracts CI job; break on any severity
- [ ] Run `npm audit --audit-level=high` for relayer and dashboard packages; break on high/critical
- [ ] Integrate `trivy image` scanning for all Docker images built in CI; break on CRITICAL CVEs
- [ ] Generate a weekly dependency health report and post it to a tracked GitHub issue

### Implementation Guide
1. Add vulnerability scanning steps to existing CI workflows; do not create separate workflows for each.
2. Configure `.cargo/audit.toml` with any accepted exceptions and their expiry dates.
3. Add `.nsprc` or `npm audit` configuration for any accepted false positives with justification.
4. Implement `scripts/dep_report.sh` generating a markdown summary and posting it via GitHub CLI.

### Acceptance Criteria
- [ ] A PR introducing a dependency with a known CRITICAL CVE is blocked by CI
- [ ] Weekly report lists all current vulnerabilities by severity with linked advisories
- [ ] Exception entries in config files have mandatory `expires` dates no more than 90 days in the future
- [ ] Docker image scans cover all layers including the base OS packages

### Security & Audit Considerations
Dependency exceptions must be tracked in the security backlog and reviewed weekly. Do not pin specific vulnerable versions as exceptions — instead, temporarily suppress the finding with a deadline for patching. Ensure scanning tools themselves are pinned to specific versions and their checksums verified to prevent supply-chain attacks on the scanner.

### Definition of Done
- [ ] Unit tests written and passing
- [ ] Integration test added
- [ ] Documentation updated
- [ ] Security review completed (if applicable)
- [ ] CI pipeline green
- [ ] PR linked to this issue

---

## Issue #33: Secrets Detection Pre-Commit Hooks
**Milestone:** M4 — Security & Audit Tools
**Labels:** `security`, `secrets`, `pre-commit`
**Priority:** Critical

### Description
Install and configure `gitleaks` and `detect-secrets` as pre-commit hooks that scan staged files for hardcoded secrets, API keys, private keys, and seed phrases before they are committed. The same scan runs in CI to catch any secrets that bypass the local hook.

### Problem Statement
A single accidentally committed private key (e.g., a Stellar signing key or AWS secret) in the repository can result in immediate fund theft or infrastructure compromise. Pre-commit hooks provide the first line of defense; CI scanning provides the safety net for developers who bypass local hooks.

### Technical Requirements
- [ ] Install `gitleaks` as a pre-commit hook via `.pre-commit-config.yaml` and verify on `git commit`
- [ ] Configure `gitleaks.toml` with custom regex rules for Stellar seed phrases (56-char base32 starting with `S`)
- [ ] Add `gitleaks` scan to CI as a required check on every PR scanning the full commit diff
- [ ] Implement a baseline file for known false positives (e.g., test vectors) using `gitleaks generate-config`

### Implementation Guide
1. Create `.pre-commit-config.yaml` with `gitleaks` hook pinned to a specific version.
2. Write `gitleaks.toml` adding rules for: Stellar seed phrase, Soroban private key hex, JWT secrets, and generic high-entropy strings in config files.
3. Add `.github/workflows/secrets-scan.yml` running `gitleaks detect --source . --log-opts="origin/main..HEAD"` on every PR.
4. Document the false-positive allowlist process in `docs/security/secrets-policy.md`.

### Acceptance Criteria
- [ ] Attempting to commit a file containing `SCZANGBA...` (Stellar seed phrase pattern) is blocked by the pre-commit hook
- [ ] CI scan catches a secret added in a commit that bypassed the local hook (verified with test branch)
- [ ] False positives for test vectors in `tests/fixtures/` are suppressed without disabling the rule globally
- [ ] Hook installation instructions are in `CONTRIBUTING.md` with `pre-commit install` as the single setup step

### Security & Audit Considerations
The secrets baseline file (`.gitleaks-baseline.json`) must be reviewed and approved by a security team member before merging changes to it. Never add an entire file to the allowlist — suppress individual findings by line hash only. The CI secrets scan job must run with read-only repository access and no ability to export scan results to external services.

### Definition of Done
- [ ] Unit tests written and passing
- [ ] Integration test added
- [ ] Documentation updated
- [ ] Security review completed (if applicable)
- [ ] CI pipeline green
- [ ] PR linked to this issue

---

## Issue #34: Threat-Model Documentation (STRIDE)
**Milestone:** M4 — Security & Audit Tools
**Labels:** `security`, `documentation`, `threat-model`
**Priority:** High

### Description
Produce a comprehensive STRIDE threat model for the entire `vero-core-engine` system covering all components: the Soroban contract, relayer service, dashboard API, and governance module. Each identified threat must map to a specific mitigation already implemented or tracked as a backlog issue.

### Problem Statement
Without a formal threat model, security work is reactive and ad-hoc. Development teams make security decisions based on intuition rather than systematic analysis, leading to blind spots. External auditors require a threat model as part of their audit package, and its absence delays or invalidates audit engagements.

### Technical Requirements
- [ ] Complete a data-flow diagram (DFD) in `docs/security/threat-model/dfd.md` covering all trust boundaries
- [ ] Document all identified threats in a structured table: `ID | Component | Threat Category (STRIDE) | Description | Likelihood | Impact | Mitigation | Status`
- [ ] Achieve minimum coverage: 5 threats per STRIDE category across all components
- [ ] Map every `Critical` and `High` threat to either an implemented control (with issue reference) or an open tracking issue

### Implementation Guide
1. Conduct a threat modeling workshop with the team using the STRIDE mnemonic for each DFD element.
2. Create `docs/security/threat-model/threats.md` with the structured threat table.
3. For each threat, link to the GitHub issue implementing the mitigation.
4. Schedule a quarterly review cadence and document it in `docs/security/threat-model/review-schedule.md`.

### Acceptance Criteria
- [ ] DFD covers all 5 major components and all trust boundaries between them
- [ ] Minimum 30 documented threats with 5+ per STRIDE category
- [ ] Every `Critical` threat has a linked mitigation issue or a `ACCEPTED` status with documented rationale
- [ ] Threat model is reviewed and signed off by at least one external security reviewer

### Security & Audit Considerations
The threat model document must be treated as sensitive — it lists known attack vectors. Restrict repository access appropriately or maintain a sanitized public version and a detailed internal version. Ensure the model is updated within 30 days of any significant architecture change. Include supply-chain threats targeting the build pipeline and deployment process.

### Definition of Done
- [ ] Unit tests written and passing
- [ ] Integration test added
- [ ] Documentation updated
- [ ] Security review completed (if applicable)
- [ ] CI pipeline green
- [ ] PR linked to this issue

---

## Issue #35: Penetration Test Harness
**Milestone:** M4 — Security & Audit Tools
**Labels:** `security`, `penetration-testing`, `testing`
**Priority:** High

### Description
Build an automated penetration testing harness for the API surface of the relayer and dashboard services, covering the OWASP Top 10 categories most relevant to this system. The harness runs in a dedicated CI environment against a fully deployed local stack and produces structured vulnerability reports.

### Problem Statement
Manual penetration testing is expensive and infrequent. Without an automated harness, security regressions introduced by new features are not caught until the next scheduled manual pen test, which may be months away. Automated testing enables continuous security validation.

### Technical Requirements
- [ ] Implement test scenarios for: authentication bypass, JWT manipulation, IDOR on receipt endpoints, SQL injection in query parameters, and privilege escalation via role manipulation
- [ ] Use `OWASP ZAP` in headless mode (`zap-api-scan.py`) for automated API scanning against the OpenAPI spec
- [ ] Implement custom attack scripts in `scripts/pentest/` using `axios` for targeted API abuse scenarios
- [ ] Generate a SARIF report from ZAP output and upload to GitHub Code Scanning

### Implementation Guide
1. Create `docker-compose.pentest.yml` spinning up the full stack (relayer + dashboard + Stellar validator) for isolated testing.
2. Write `scripts/pentest/auth_bypass.ts`, `scripts/pentest/idor_receipts.ts`, and `scripts/pentest/sqli_probe.ts`.
3. Configure ZAP with the dashboard's OpenAPI spec and run `zap-api-scan.py -t openapi.json`.
4. Add a `pentest` CI workflow running weekly and on-demand via `workflow_dispatch`.

### Acceptance Criteria
- [ ] JWT with a tampered role claim is rejected with `401` (verified by pentest script)
- [ ] SQL injection probes on all query parameters return no database errors or data leakage
- [ ] ZAP scan completes with zero HIGH or CRITICAL findings
- [ ] SARIF report is uploaded and visible in GitHub Security tab after each run

### Security & Audit Considerations
The pentest environment must be completely isolated from any real Stellar network and use ephemeral test accounts only. Pentest scripts must not be usable against production systems — add a `PENTEST_TARGET` environment variable with an allowlist of valid test hostnames. Pentest reports must be reviewed by the security team and findings triaged within 5 business days.

### Definition of Done
- [ ] Unit tests written and passing
- [ ] Integration test added
- [ ] Documentation updated
- [ ] Security review completed (if applicable)
- [ ] CI pipeline green
- [ ] PR linked to this issue

---

## Issue #36: Invariant Monitor (On-Chain)
**Milestone:** M4 — Security & Audit Tools
**Labels:** `security`, `monitoring`, `contracts`
**Priority:** High

### Description
Implement an off-chain invariant monitoring service that periodically reads contract state via Stellar RPC and verifies that all defined invariants hold (balance conservation, valid state machine position, role count bounds). On violation, the monitor emits an alert and can optionally trigger the emergency pause (Issue #7).

### Problem Statement
On-chain invariants can be violated by bugs in contract logic that pass all tests but fail under specific real-world conditions. Without continuous monitoring, a violated invariant (e.g., total balances exceeding deposits) may go undetected until funds are irretrievably lost.

### Technical Requirements
- [ ] Implement `relayer/src/monitors/InvariantMonitor.ts` that runs invariant checks every `INVARIANT_CHECK_INTERVAL_LEDGERS`
- [ ] Implement checks: `BalanceConservation` (sum of internal balances ≤ sum of deposited assets), `ValidContractState` (state is a known enum value), `RoleCountBounds` (at least 1 admin, no more than `MAX_OPERATORS`)
- [ ] On violation, emit an alert via the alerting system (Issue #15) with severity `CRITICAL`
- [ ] Optionally auto-pause the contract if `AUTO_PAUSE_ON_VIOLATION=true` and the violation is `BalanceConservation`

### Implementation Guide
1. Implement `readContractState(rpcUrl, contractId)` utility fetching all relevant storage keys via `getLedgerEntries`.
2. Implement each invariant as a pure function `checkInvariant(state: ContractState): InvariantResult`.
3. Wire the monitor into the relayer's scheduled task runner using `node-cron`.
4. Write integration tests using a mock contract state with pre-seeded violations.

### Acceptance Criteria
- [ ] `BalanceConservation` violation is detected within one check interval and alert is fired
- [ ] With `AUTO_PAUSE_ON_VIOLATION=true`, the contract is paused automatically within 5 seconds of violation detection
- [ ] False positives during normal operations are zero (verified by 24-hour soak test)
- [ ] Monitor check latency is under 2 seconds per cycle

### Security & Audit Considerations
The invariant monitor must use a read-only RPC account with no signing capability to prevent a compromised monitor from modifying contract state. Auto-pause functionality must use the designated pause guardian key, which must be stored in a separate HSM from the relayer signing key. Alert suppression mechanisms must not be available to non-admin users.

### Definition of Done
- [ ] Unit tests written and passing
- [ ] Integration test added
- [ ] Documentation updated
- [ ] Security review completed (if applicable)
- [ ] CI pipeline green
- [ ] PR linked to this issue

---

## Issue #37: Cryptographic Library Audit
**Milestone:** M4 — Security & Audit Tools
**Labels:** `security`, `cryptography`, `audit`
**Priority:** High

### Description
Conduct a comprehensive audit of all cryptographic primitives used across the system: Ed25519 signing in the relayer (Issue #12), hash functions used in receipt and ZK proof generation, and any PRNG usage. Replace any non-standard or misused cryptographic implementations with audited libraries.

### Problem Statement
Misuse of cryptography is one of the most common and severe vulnerability categories. Using a PRNG for key material generation, applying signatures to malleable data, or using MD5/SHA-1 for security-sensitive hashes are all critical vulnerabilities that are easy to introduce and hard to spot in code review.

### Technical Requirements
- [ ] Audit all usages of `crypto` module, `@noble/*`, and any hash function calls across the relayer and dashboard codebases
- [ ] Verify that all signatures are over a domain-separated, length-prefixed message (not raw arbitrary data)
- [ ] Replace any `Math.random()` used in security contexts with `crypto.getRandomValues()`
- [ ] Produce `docs/security/cryptography-audit.md` documenting each cryptographic operation, the library used, and its audit status

### Implementation Guide
1. Run `grep -r "Math.random\|crypto.createHash\|md5\|sha1" relayer/ dashboard/` to enumerate all crypto usage.
2. Review each `@noble/ed25519` usage to confirm domain separation in the signed message construction.
3. Review receipt hash construction to confirm SHA-256 with length-prefix binding of all fields.
4. Document findings and remediation steps in the audit report.

### Acceptance Criteria
- [ ] Zero uses of `Math.random()` in security-relevant code paths (verified by `semgrep` rule)
- [ ] All Ed25519 signed messages include a domain separator prefix (e.g., `"vero-receipt-v1:"`)
- [ ] `docs/security/cryptography-audit.md` covers all 8+ identified cryptographic operations
- [ ] All findings are resolved or tracked with a linked GitHub issue

### Security & Audit Considerations
The cryptography audit must be reviewed by someone with explicit cryptographic engineering experience — not just general security experience. Use only audited, widely-deployed libraries (`@noble/ed25519` ≥ 1.7.3, Node.js built-in `crypto`). Never implement custom cryptographic primitives. Document key sizes and algorithm choices with justification.

### Definition of Done
- [ ] Unit tests written and passing
- [ ] Integration test added
- [ ] Documentation updated
- [ ] Security review completed (if applicable)
- [ ] CI pipeline green
- [ ] PR linked to this issue

---

## Issue #38: Supply-Chain Security (SLSA Level 3)
**Milestone:** M4 — Security & Audit Tools
**Labels:** `security`, `supply-chain`, `slsa`
**Priority:** High

### Description
Achieve SLSA Level 3 compliance for all build artifacts (contract WASM, Docker images, npm packages) by implementing hermetic builds, provenance generation, and artifact signing. This requires using GitHub Actions reusable workflows with OIDC-based signing and publishing signed provenance attestations.

### Problem Statement
Without supply-chain security controls, a compromised CI runner or build dependency could silently modify production artifacts. An attacker inserting malicious code into a Docker image or contract WASM would be undetectable without provenance attestations linking artifacts to specific source commits and build environments.

### Technical Requirements
- [ ] Use `slsa-framework/slsa-github-generator` to generate SLSA provenance for all release artifacts
- [ ] Sign all Docker images using `cosign` with OIDC keyless signing; store signatures in the OCI registry
- [ ] Sign all npm package tarballs using `npm pack` + `cosign` before publishing
- [ ] Generate `contracts/vero-core/WASM_HASH.txt` with the SHA-256 of the built WASM on every release

### Implementation Guide
1. Create `.github/workflows/release.yml` using the SLSA generator reusable workflow for each artifact type.
2. Add `cosign sign` steps for Docker images after push, using GitHub OIDC token for keyless signing.
3. Publish provenance attestations to a public Rekor transparency log for independent verification.
4. Document the artifact verification process in `docs/security/artifact-verification.md`.

### Acceptance Criteria
- [ ] Contract WASM build is reproducible: same source commit produces identical SHA-256 on two independent builds
- [ ] Docker image signatures verify correctly with `cosign verify --certificate-oidc-issuer https://token.actions.githubusercontent.com`
- [ ] SLSA provenance JSON is attached to each GitHub release as a downloadable asset
- [ ] `docs/security/artifact-verification.md` provides step-by-step verification instructions for external users

### Security & Audit Considerations
Hermetic builds require pinning all build dependencies including the Rust toolchain version and the `stellar-cli` version used in CI. Build cache must be content-addressed (e.g., Rust's `sccache` with S3 backend keyed by source hash) to prevent cache poisoning. GitHub Actions workflows must use pinned SHA references for all external actions, never mutable branch or tag references.

### Definition of Done
- [ ] Unit tests written and passing
- [ ] Integration test added
- [ ] Documentation updated
- [ ] Security review completed (if applicable)
- [ ] CI pipeline green
- [ ] PR linked to this issue

---

## Issue #39: Bug-Bounty Program Scaffolding
**Milestone:** M4 — Security & Audit Tools
**Labels:** `security`, `bug-bounty`, `community`
**Priority:** Medium

### Description
Set up the infrastructure and documentation for a public bug-bounty program: a `SECURITY.md` policy, a responsible disclosure process, reward tiers mapped to CVSS severity, and a private disclosure channel (HackerOne or equivalent). Define the in-scope and out-of-scope boundaries clearly.

### Problem Statement
Without a formal bug-bounty program, security researchers who discover vulnerabilities have no official channel to report them responsibly. This leads to either silent exploitation, public disclosure without a fix, or researchers abandoning the disclosure entirely. A well-structured program incentivizes responsible disclosure.

### Technical Requirements
- [ ] Create `SECURITY.md` at the repository root with: disclosure process, response SLAs, reward tiers, and contact email
- [ ] Define reward tiers: `Critical (fund loss, >$10K)`, `High (protocol manipulation, >$2K)`, `Medium (data exposure, >$500)`, `Low (informational, >$100)`
- [ ] Configure HackerOne or Immunefi program with in-scope assets: contract address, relayer API, dashboard API
- [ ] Implement `scripts/triage_report.sh` for consistent internal report triage and response tracking

### Implementation Guide
1. Draft `SECURITY.md` following the GitHub Security Advisory template and industry best practices.
2. Set up the bounty platform program with scope, rules, and reward ranges.
3. Create a private Slack channel and PagerDuty policy for bounty report notifications.
4. Document the internal triage process in `docs/security/bounty-triage-process.md`.

### Acceptance Criteria
- [ ] `SECURITY.md` is present at repository root and references the bounty platform URL
- [ ] Bounty platform program is publicly visible with at least 3 in-scope asset definitions
- [ ] A test submission through the platform reaches the security team within 1 hour (verified by drill)
- [ ] Response SLA targets are: 24h acknowledgment, 7d triage, 30d remediation for Critical

### Security & Audit Considerations
The security contact email must be monitored 24/7 with on-call rotation. Ensure that the reward payment process is documented, including KYC requirements for large rewards, to prevent legal complications. Reserve a portion of the treasury (Issue #45) for bug-bounty payments. Do not disclose reporter identities without explicit consent.

### Definition of Done
- [ ] Unit tests written and passing
- [ ] Integration test added
- [ ] Documentation updated
- [ ] Security review completed (if applicable)
- [ ] CI pipeline green
- [ ] PR linked to this issue

---

## Issue #40: SOC 2 Evidence Collection Automation
**Milestone:** M4 — Security & Audit Tools
**Labels:** `security`, `compliance`, `soc2`
**Priority:** Medium

### Description
Implement automated evidence collection pipelines that gather, timestamp, and store audit artifacts required for SOC 2 Type II compliance: access logs, change management records, deployment audit trails, vulnerability scan results, and security training records. Evidence is stored in an immutable S3 bucket with signed manifests.

### Problem Statement
Manual SOC 2 evidence collection is labor-intensive, error-prone, and typically deferred until shortly before the audit period — resulting in gaps and last-minute scrambles. Automated collection ensures evidence is continuously gathered and auditors have immediate access to a complete, tamper-evident record.

### Technical Requirements
- [ ] Implement `scripts/soc2/collect_evidence.sh` that runs daily and collects: CI pipeline results, deployment records, access logs summary, and vulnerability scan outputs
- [ ] Upload all evidence to an S3 bucket `vero-soc2-evidence/{year}/{month}/{day}/` with `AES-256` server-side encryption and versioning enabled
- [ ] Generate a signed manifest (SHA-256 of each file + Ed25519 signature) for each daily collection
- [ ] Map each evidence type to specific SOC 2 Trust Services Criteria in `docs/compliance/soc2-evidence-map.md`

### Implementation Guide
1. Create `scripts/soc2/collect_evidence.sh` orchestrating collection from CI, GitHub, and application logs.
2. Configure the S3 bucket with Object Lock (Governance mode, 7-year retention) to prevent deletion.
3. Implement the manifest signing using the same Ed25519 key as the dashboard API.
4. Document the criteria mapping for CC6, CC7, CC8, and CC9 in the evidence map.

### Acceptance Criteria
- [ ] Daily evidence collection runs without errors and produces all required artifact types
- [ ] S3 bucket has Object Lock enabled; attempts to delete evidence files within retention period fail
- [ ] Evidence manifest signature verifies correctly using the published public key
- [ ] `docs/compliance/soc2-evidence-map.md` maps all 8 required evidence types to SOC 2 criteria

### Security & Audit Considerations
The evidence collection script must run with minimal IAM permissions (S3 PutObject to the specific bucket only). The signing key used for manifests must be rotated annually and the rotation process documented. Evidence must include access logs showing who accessed the evidence bucket itself. Ensure PII in access logs is minimized before storage.

### Definition of Done
- [ ] Unit tests written and passing
- [ ] Integration test added
- [ ] Documentation updated
- [ ] Security review completed (if applicable)
- [ ] CI pipeline green
- [ ] PR linked to this issue

---

## Milestone 5 — Governance (Issues 41–50)

---

## Issue #41: Multi-Sig Contract Module
**Milestone:** M5 — Governance
**Labels:** `governance`, `contracts`, `multi-sig`
**Priority:** Critical

### Description
Implement a multi-signature authorization module as a standalone Soroban contract that wraps any administrative action requiring M-of-N approval. The multi-sig contract holds a registry of signers and a configurable threshold, and it is the authorization entry point for contract upgrades, parameter changes, and treasury operations.

### Problem Statement
Single-key admin control is the most common critical vulnerability in smart contract systems. A single compromised admin key gives an attacker full control over the protocol. Without multi-sig, there is no defense against key compromise, insider threat, or targeted phishing attacks against admin keyholders.

### Technical Requirements
- [ ] Deploy `contracts/vero-multisig/` as a separate Soroban contract with `register_signers(signers: Vec<Address>, threshold: u32)` initializer
- [ ] Implement `propose(action: Action, calldata: Bytes) -> ProposalId` callable by any registered signer
- [ ] Implement `approve(proposal_id: ProposalId)` collecting signer approvals; execute automatically when threshold is met
- [ ] Emit `ProposalCreated`, `ApprovalAdded`, and `ProposalExecuted` events for all lifecycle transitions

### Implementation Guide
1. Scaffold `contracts/vero-multisig/` Rust crate with its own `Cargo.toml` and `lib.rs`.
2. Define `Action` enum covering all governable operations: `UpgradeContract`, `UpdateParameter`, `TransferTreasury`, `PauseContract`.
3. Implement proposal storage using a `Map<ProposalId, ProposalState>` in persistent storage.
4. Write integration tests: 2-of-3 approval scenario, expired proposal cleanup, duplicate approval rejection.

### Acceptance Criteria
- [ ] A proposal with 2 approvals out of a 3-signer 2-of-3 config executes the encoded action correctly
- [ ] A signer cannot approve the same proposal twice
- [ ] A non-signer cannot create or approve proposals (returns `Unauthorized`)
- [ ] `ProposalExecuted` event is emitted with the action type and execution timestamp

### Security & Audit Considerations
The multi-sig contract itself must not have a privileged admin account — the signer set is the governance mechanism. Implement a time-lock on proposal execution (Issue #43) to allow time for emergency veto. Signer set changes (adding/removing signers) must themselves require a multi-sig proposal to prevent a signer from unilaterally expanding the signer set. Protect against proposal hash collision in the `ProposalId` generation.

### Definition of Done
- [ ] Unit tests written and passing
- [ ] Integration test added
- [ ] Documentation updated
- [ ] Security review completed (if applicable)
- [ ] CI pipeline green
- [ ] PR linked to this issue

---

## Issue #42: Proposal Creation and Voting
**Milestone:** M5 — Governance
**Labels:** `governance`, `voting`, `contracts`
**Priority:** High

### Description
Extend the governance system with a full proposal lifecycle supporting on-chain voting by token holders or designated governance participants, beyond the multi-sig approval model. Proposals have defined voting periods, quorum requirements, and pass/fail thresholds. Results are binding and automatically executed on-chain.

### Problem Statement
Multi-sig governance is appropriate for operational decisions but insufficient for protocol-level changes that affect all stakeholders. Token holders or protocol participants require a mechanism to express preferences and override core team decisions, ensuring the protocol is truly community-governed rather than operator-controlled.

### Technical Requirements
- [ ] Implement `GovernorContract` with `create_proposal(description: String, action: Action, voting_period_ledgers: u32) -> ProposalId`
- [ ] Implement `cast_vote(proposal_id: ProposalId, support: bool, weight: i128)` with weight derived from token balance snapshot at proposal creation ledger
- [ ] Define quorum as configurable percentage of total voting weight; proposal passes if `yes_votes / total_votes >= PASS_THRESHOLD`
- [ ] Implement proposal state machine: `Draft → Active → Succeeded/Defeated → Queued → Executed`

### Implementation Guide
1. Create `contracts/vero-governor/` crate with the `GovernorContract` implementation.
2. Implement voting weight snapshot using Stellar asset balance at a specific ledger (requires archival RPC for verification).
3. Implement `queue(proposal_id)` that moves a succeeded proposal into the time-lock queue (Issue #43).
4. Write tests covering: quorum not met (proposal defeated), quorum met with majority yes (passed), quorum met with majority no (defeated).

### Acceptance Criteria
- [ ] Proposal passes when quorum is met and `yes_votes / total_votes > 0.5` (configurable)
- [ ] Voting after the voting period ends returns `VotingClosed` error
- [ ] A voter cannot vote twice on the same proposal
- [ ] Proposal state transitions are all reflected in emitted events

### Security & Audit Considerations
Vote weight snapshots must be taken at proposal creation (not at voting time) to prevent flash-loan attacks that temporarily inflate voting power. The `GovernorContract` must not be upgradeable by a simple multi-sig — upgrades require a governance vote. Implement a minimum voting period of 48 hours to give all stakeholders time to participate. Protect the vote tally against integer overflow with checked arithmetic.

### Definition of Done
- [ ] Unit tests written and passing
- [ ] Integration test added
- [ ] Documentation updated
- [ ] Security review completed (if applicable)
- [ ] CI pipeline green
- [ ] PR linked to this issue

---

## Issue #43: Time-Lock Enforcement
**Milestone:** M5 — Governance
**Labels:** `governance`, `time-lock`, `security`
**Priority:** Critical

### Description
Implement a `TimelockController` contract that enforces a mandatory delay between a governance decision (multi-sig approval or vote passage) and the actual execution of the encoded action. The delay gives stakeholders time to detect malicious or erroneous proposals and trigger emergency responses before irreversible changes take effect.

### Problem Statement
Without a time-lock, a compromised multi-sig or a flash governance attack can immediately execute destructive actions (e.g., transferring all treasury funds) with no window for response. A mandatory delay is the last line of defense between a governance compromise and irreversible harm.

### Technical Requirements
- [ ] Implement `TimelockController` with `schedule(action: Action, eta: u64)`, `execute(action_hash: BytesN<32>)`, and `cancel(action_hash: BytesN<32>)`
- [ ] Enforce minimum delay: `eta - current_ledger_timestamp >= MIN_TIMELOCK_DELAY_SECONDS` (configurable, default 48 hours)
- [ ] `cancel` is callable only by the multi-sig (Issue #41) or the emergency veto mechanism (Issue #44)
- [ ] Emit `ActionScheduled { action_hash, eta }`, `ActionExecuted { action_hash }`, and `ActionCancelled { action_hash, reason }` events

### Implementation Guide
1. Create `contracts/vero-timelock/` crate implementing the `TimelockController`.
2. Store scheduled actions as `Map<BytesN<32>, ScheduledAction>` where key is `keccak256(action || eta || salt)`.
3. In `execute`, verify `current_timestamp >= eta` before dispatching the encoded action via cross-contract call.
4. Wire the timelock as the executor for both the multi-sig (Issue #41) and governor (Issue #42) contracts.

### Acceptance Criteria
- [ ] Attempting to execute a scheduled action before `eta` returns `TimelockNotExpired`
- [ ] Action executes successfully exactly at or after `eta`
- [ ] Only the multi-sig can cancel a scheduled action; unauthorized cancellation returns `Unauthorized`
- [ ] `ActionScheduled` event is emitted immediately upon scheduling with the correct `eta`

### Security & Audit Considerations
The `MIN_TIMELOCK_DELAY_SECONDS` must itself require a governance vote to change, with the change subject to the existing time-lock delay — preventing a fast reduction of the delay as a prelude to an attack. Implement a maximum time-lock window (e.g., 30 days) after which unexecuted actions automatically expire to prevent griefing by scheduling actions and blocking the queue. Protect the action hash against pre-image attacks.

### Definition of Done
- [ ] Unit tests written and passing
- [ ] Integration test added
- [ ] Documentation updated
- [ ] Security review completed (if applicable)
- [ ] CI pipeline green
- [ ] PR linked to this issue

---

## Issue #44: Veto-Window Mechanism
**Milestone:** M5 — Governance
**Labels:** `governance`, `security`, `veto`
**Priority:** High

### Description
Implement a veto mechanism that allows a designated guardian address (or multi-sig) to cancel any time-locked action during the veto window, which runs concurrent with the time-lock delay. The guardian can only cancel, never modify or execute, and the guardian role itself is subject to governance control.

### Problem Statement
The time-lock delay (Issue #43) creates an observation window but has no effect if no party is empowered to act on what they observe. Without a veto mechanism, stakeholders can watch a malicious action approach its execution time but are powerless to stop it, making the time-lock a warning system rather than a defense.

### Technical Requirements
- [ ] Add a `VETO_GUARDIAN` role to the governance system, assignable only by governance vote
- [ ] Implement `veto(action_hash: BytesN<32>, reason: String)` in the `TimelockController` callable only by `VETO_GUARDIAN`
- [ ] Vetoed actions are moved to a `VetoedActions` archive with reason, vetoer, and timestamp
- [ ] Emit `ActionVetoed { action_hash, guardian, reason, timestamp }` event

### Implementation Guide
1. Extend `contracts/vero-timelock/src/lib.rs` with `veto` function and `VETO_GUARDIAN` storage key.
2. Add `VetoedActions` archive storage as an append-only log (never delete vetoed action records).
3. Implement the `VETO_GUARDIAN` assignment proposal type in the `GovernorContract` (Issue #42).
4. Write tests: guardian vetoes a scheduled action successfully; non-guardian veto attempt fails; re-scheduling a vetoed action is allowed.

### Acceptance Criteria
- [ ] `VETO_GUARDIAN` successfully cancels a scheduled action before its `eta`
- [ ] A non-guardian address calling `veto` returns `Unauthorized`
- [ ] Vetoed actions appear in the `VetoedActions` archive with full metadata
- [ ] A vetoed action cannot be executed (attempting returns `ActionVetoed`)

### Security & Audit Considerations
The `VETO_GUARDIAN` role must be a multi-sig account to prevent a single compromised key from vetoing legitimate governance actions (griefing). Implement a `VETO_COOLDOWN` period after each veto to prevent a malicious guardian from continuously vetoing all proposals. The veto mechanism must not be able to veto the action that would remove the guardian role — this could lock the protocol into an unremovable malicious guardian scenario. Coordinate the design with the emergency governance override (Issue #49).

### Definition of Done
- [ ] Unit tests written and passing
- [ ] Integration test added
- [ ] Documentation updated
- [ ] Security review completed (if applicable)
- [ ] CI pipeline green
- [ ] PR linked to this issue

---

## Issue #45: Treasury Management Contract
**Milestone:** M5 — Governance
**Labels:** `governance`, `treasury`, `contracts`
**Priority:** High

### Description
Implement a `TreasuryManager` Soroban contract that holds protocol-owned assets, enforces spending limits per time period, and releases funds only via governance-approved proposals. The treasury supports multiple asset types (Issue #9) and provides transparent on-chain accounting of all inflows and outflows.

### Problem Statement
Without a governed treasury contract, protocol funds held in a multi-sig wallet are opaque to the community, susceptible to unilateral spending by key holders, and not subject to the same auditability standards as the core protocol. A transparent on-chain treasury is essential for community trust and long-term sustainability.

### Technical Requirements
- [ ] Implement `TreasuryManager` with `deposit(asset: AssetId, amount: i128)`, `propose_withdrawal(asset, amount, recipient, reason)`, and `execute_withdrawal(proposal_id)` functions
- [ ] Enforce a `DAILY_WITHDRAWAL_LIMIT` per asset that resets every 24 hours; amounts exceeding the limit require a higher-threshold governance approval
- [ ] Maintain an on-chain ledger of all deposits and withdrawals queryable via `get_transaction_history(limit, offset)`
- [ ] Emit `TreasuryDeposit` and `TreasuryWithdrawal` events for all fund movements

### Implementation Guide
1. Create `contracts/vero-treasury/` crate with full asset multi-support using `AssetId` from Issue #9.
2. Implement the daily limit tracker using a `(asset_id, day_bucket)` composite key reset by comparing `current_ledger_timestamp / 86400`.
3. Wire withdrawal proposals through the `GovernorContract` (Issue #42) and `TimelockController` (Issue #43).
4. Write tests: normal withdrawal within limit, over-limit withdrawal requires higher threshold, unauthorized withdrawal rejected.

### Acceptance Criteria
- [ ] Treasury balance is accurately reflected after deposit and withdrawal operations
- [ ] A withdrawal exceeding `DAILY_WITHDRAWAL_LIMIT` is rejected without governance approval
- [ ] `get_transaction_history` returns paginated results sorted by timestamp descending
- [ ] Treasury state is queryable from the dashboard (Issue #22 contract state explorer)

### Security & Audit Considerations
The `TreasuryManager` must not have a privileged admin that can bypass withdrawal limits — all fund movements must go through governance. Implement a minimum withdrawal proposal review period (minimum 48 hours via time-lock) for amounts above a threshold. Validate that `deposit` correctly handles Soroban token transfers and does not double-credit. The treasury contract address must be published in `docs/contracts.md` for independent monitoring.

### Definition of Done
- [ ] Unit tests written and passing
- [ ] Integration test added
- [ ] Documentation updated
- [ ] Security review completed (if applicable)
- [ ] CI pipeline green
- [ ] PR linked to this issue

---

## Issue #46: On-Chain Parameter Registry
**Milestone:** M5 — Governance
**Labels:** `governance`, `contracts`, `configuration`
**Priority:** Medium

### Description
Implement an on-chain parameter registry contract that stores all configurable protocol parameters (e.g., `MIN_DELAY_LEDGERS`, `MAX_OPERATORS`, `QUORUM_SIZE`, `DAILY_WITHDRAWAL_LIMIT`) as key-value pairs. Parameter updates require governance approval and emit change events, providing a complete audit trail of configuration history.

### Problem Statement
Without a governed parameter registry, protocol parameters are either hardcoded (requiring contract upgrades to change) or stored in off-chain config files (allowing silent, unauthorized changes). A centralized on-chain registry ensures all parameter changes are transparent, auditable, and governance-gated.

### Technical Requirements
- [ ] Implement `ParameterRegistry` contract with `set_parameter(key: Symbol, value: Val)` gated by governance multi-sig
- [ ] Store parameters as `Map<Symbol, Val>` in persistent storage with type metadata for validation
- [ ] Implement `get_parameter(key: Symbol) -> Val` and `get_parameter_history(key: Symbol) -> Vec<ParameterChange>`
- [ ] Emit `ParameterUpdated { key, old_value, new_value, updated_by, timestamp }` on every change

### Implementation Guide
1. Create `contracts/vero-registry/` crate defining the `ParameterRegistry` contract.
2. Define a `ParameterType` enum (`U32`, `I128`, `Bool`, `Address`, `BytesN32`) with validation logic per type.
3. Store parameter change history as an append-only log keyed by `(parameter_key, change_index)`.
4. Integrate with `vero-core` contract to read parameters via cross-contract call rather than hardcoded values.

### Acceptance Criteria
- [ ] Parameter update by a non-governance address returns `Unauthorized`
- [ ] Updated parameters are reflected in `vero-core` contract behavior within the same ledger
- [ ] `get_parameter_history` returns the full change log for any parameter including old values
- [ ] Invalid parameter values (e.g., `QUORUM_SIZE = 0`) are rejected with `InvalidParameterValue`

### Security & Audit Considerations
Parameter validation must be strict — define min/max bounds for all numeric parameters to prevent extreme values that could lock the protocol (e.g., `QUORUM_SIZE = MAX_INT`). Cross-contract calls to read parameters must handle the case where the registry contract is unreachable (use cached fallback values). Ensure that the parameter registry itself cannot be upgraded without the same governance process as the core contract.

### Definition of Done
- [ ] Unit tests written and passing
- [ ] Integration test added
- [ ] Documentation updated
- [ ] Security review completed (if applicable)
- [ ] CI pipeline green
- [ ] PR linked to this issue

---

## Issue #47: Governance Analytics Dashboard
**Milestone:** M5 — Governance
**Labels:** `governance`, `dashboard`, `analytics`
**Priority:** Medium

### Description
Build a governance analytics section in the dashboard displaying active proposals, voting history, time-lock queue status, treasury balances, and parameter change history. The view must be accessible to any stakeholder without authentication for transparency, while governance actions remain gated by RBAC (Issue #29).

### Problem Statement
Without a governance dashboard, community members have no accessible way to monitor governance activity, track upcoming time-locked actions, or review historical voting outcomes. Opaque governance processes erode community trust and reduce participation in governance votes.

### Technical Requirements
- [ ] Implement `GET /api/governance/proposals?status=active|queued|executed&limit=20` returning proposal summaries
- [ ] Implement `GET /api/governance/timelock/queue` returning pending time-locked actions with ETAs
- [ ] Build `GovernancePage.tsx` with tabs: Active Proposals, Voting History, Time-Lock Queue, Parameter Registry, Treasury
- [ ] Make the governance page publicly accessible (no auth required for read-only view)

### Implementation Guide
1. Index governance events (proposal created/voted/executed, parameter changed) from the relayer receipt store.
2. Implement API routes in `dashboard/src/api/routes/governance.ts`.
3. Build the governance page with proposal cards showing vote tallies using progress bars.
4. Add a countdown timer for time-locked actions showing time remaining until execution.

### Acceptance Criteria
- [ ] Active proposals display current vote tally, quorum status, and time remaining in voting period
- [ ] Time-lock queue shows all pending actions sorted by ETA ascending with a countdown
- [ ] Governance page loads in under 2 seconds with 50+ historical proposals in the database
- [ ] Public (unauthenticated) access to the governance page returns read-only data with no admin controls visible

### Security & Audit Considerations
Ensure that the public governance API endpoints are rate-limited to prevent scraping or DoS. Vote tally data must be derived from on-chain events — never allow the dashboard database to be the authoritative source for governance outcomes. Proposal descriptions must be sanitized before display to prevent XSS injection.

### Definition of Done
- [ ] Unit tests written and passing
- [ ] Integration test added
- [ ] Documentation updated
- [ ] Security review completed (if applicable)
- [ ] CI pipeline green
- [ ] PR linked to this issue

---

## Issue #48: Delegate / Proxy Voting
**Milestone:** M5 — Governance
**Labels:** `governance`, `voting`, `delegation`
**Priority:** Medium

### Description
Implement a vote delegation mechanism allowing token holders to delegate their voting weight to a trusted representative without transferring custody of their tokens. Delegations are on-chain, revocable at any time, and support transitive delegation up to a configurable depth limit.

### Problem Statement
Low governance participation is a common failure mode in token-based governance systems. Many token holders do not have the time or expertise to evaluate every proposal. Without delegation, voting weight from inactive holders is never represented, making it difficult to reach quorum and giving outsized influence to the most active (not necessarily most informed) participants.

### Technical Requirements
- [ ] Implement `delegate(delegatee: Address)` and `undelegate()` functions on the governance token or a separate `DelegationRegistry` contract
- [ ] Resolve effective voting weight at vote-cast time by following delegation chains up to `MAX_DELEGATION_DEPTH` (default 3)
- [ ] Store delegation state in persistent storage; emit `DelegationSet { delegator, delegatee }` and `DelegationRevoked { delegator }` events
- [ ] `get_effective_weight(account: Address, ledger: u32) -> i128` query function accounting for full delegation chain

### Implementation Guide
1. Implement `DelegationRegistry` contract tracking `Map<Address, Address>` (delegator → delegatee).
2. Implement `resolve_delegation_chain(account, max_depth)` using iterative traversal to avoid deep recursion.
3. Update `GovernorContract.cast_vote` to use `get_effective_weight` instead of raw token balance.
4. Write tests: simple delegation, chain delegation (A→B→C), cycle detection (A→B→A should revert), revocation mid-vote.

### Acceptance Criteria
- [ ] A delegator's weight is correctly reflected in the delegatee's vote when the delegatee casts a vote
- [ ] Delegation cycle (A→B→A) is detected and rejected with `DelegationCycle` error
- [ ] Revoking delegation before voting end correctly removes the delegated weight from the delegatee
- [ ] `get_effective_weight` correctly resolves a 3-level delegation chain

### Security & Audit Considerations
Delegation cycles must be detected and rejected to prevent infinite loops in weight resolution. Delegation depth limit must be enforced to bound the computation cost of `resolve_delegation_chain`. Ensure that a delegatee cannot delegate their received weight onward without the original delegator's intent — clarify in documentation whether transitive delegation is intentional and document the depth limit prominently.

### Definition of Done
- [ ] Unit tests written and passing
- [ ] Integration test added
- [ ] Documentation updated
- [ ] Security review completed (if applicable)
- [ ] CI pipeline green
- [ ] PR linked to this issue

---

## Issue #49: Emergency Governance Override
**Milestone:** M5 — Governance
**Labels:** `governance`, `emergency`, `security`
**Priority:** Critical

### Description
Implement an emergency governance override mechanism that allows a supermajority of the multi-sig (e.g., 4-of-5 signers) to bypass the normal time-lock delay in cases of critical security incidents. The override is logged with full justification, subject to post-hoc review, and has a strictly limited scope of actions it can authorize.

### Problem Statement
Normal governance time-locks (Issue #43) are designed for routine operations but create a dangerous delay in genuine emergencies — for example, a discovered exploit actively draining funds. Without an emergency bypass, the protocol may be unable to pause or patch itself before catastrophic loss occurs.

### Technical Requirements
- [ ] Implement `EmergencyAction` enum with a limited set of permitted emergency actions: `PauseContract`, `CancelTimelockAction`, `SetParameter(key, value)` with a strict allowlist of `key`
- [ ] Require `EMERGENCY_THRESHOLD` of `N` multi-sig approvals (configurable, default 4-of-5) to execute an `EmergencyAction`
- [ ] Enforce a 7-day cooldown between emergency overrides to prevent repeated bypass of the time-lock
- [ ] Emit `EmergencyActionExecuted { action, approvers, justification, timestamp }` event and write to an immutable emergency log

### Implementation Guide
1. Add `emergency_propose(action: EmergencyAction, justification: String)` and `emergency_approve(proposal_id)` to the `MultiSigContract`.
2. Implement the `EMERGENCY_THRESHOLD` check as a separate approval counter from normal proposal approvals.
3. Implement the 7-day cooldown using a `LAST_EMERGENCY_TIMESTAMP` persistent storage key.
4. Write tests: successful emergency pause with 4/5 approvals, failed attempt with 3/5, cooldown enforcement.

### Acceptance Criteria
- [ ] Emergency pause executes within 1 ledger of the 4th approval being submitted
- [ ] A 3-of-5 approval attempt does not execute and does not consume the cooldown timer
- [ ] A second emergency action within 7 days is rejected with `EmergencyCoolddownActive`
- [ ] Emergency log records all approver addresses and the justification string immutably

### Security & Audit Considerations
The scope of emergency actions must be strictly limited — never allow emergency override to execute arbitrary calldata or transfer funds. The `justification` field is mandatory and must be a non-empty string; reject empty justifications. Emergency actions must be publicly visible on-chain to ensure community accountability. Post-incident governance should review all emergency overrides within 30 days and ratify or penalize the action via a community vote.

### Definition of Done
- [ ] Unit tests written and passing
- [ ] Integration test added
- [ ] Documentation updated
- [ ] Security review completed (if applicable)
- [ ] CI pipeline green
- [ ] PR linked to this issue

---

## Issue #50: Governance Documentation and Runbooks
**Milestone:** M5 — Governance
**Labels:** `governance`, `documentation`, `runbooks`
**Priority:** Medium

### Description
Produce comprehensive governance documentation including: a governance overview and decision-making framework, operator runbooks for all governance procedures, and a governance FAQ for community stakeholders. Documentation must be versioned alongside the contracts and kept up to date with each governance system change.

### Problem Statement
Even a technically sound governance system fails if participants do not understand how to use it. Without documentation, signer key holders do not know how to submit proposals, community members cannot verify governance is working correctly, and incident responders cannot quickly execute emergency procedures under pressure.

### Technical Requirements
- [ ] Write `docs/governance/overview.md` covering: governance structure, roles, decision-making process, and escalation paths
- [ ] Write operator runbooks for: creating a proposal, approving a multi-sig action, executing a time-locked action, triggering emergency pause, and rotating signer keys
- [ ] Write `docs/governance/faq.md` covering the 15 most common community questions about the governance process
- [ ] Automate documentation link-checking in CI using `markdown-link-check` to prevent dead links

### Implementation Guide
1. Draft all documentation in `docs/governance/` using the existing documentation structure and style.
2. Create runbook templates with step-by-step CLI commands tested against the testnet environment.
3. Add `markdown-link-check` to the CI pipeline as a separate job running on all `.md` files.
4. Schedule a quarterly documentation review as a recurring GitHub issue using Actions scheduled workflow.

### Acceptance Criteria
- [ ] All 5 operator runbooks contain step-by-step instructions verifiable against the testnet
- [ ] Governance FAQ answers at least 15 distinct questions with accurate, current information
- [ ] `markdown-link-check` CI job passes with zero broken links
- [ ] Documentation is reviewed and approved by at least one non-developer stakeholder

### Security & Audit Considerations
Runbooks must include security warnings for sensitive operations (key rotation, emergency override) emphasizing verification steps and confirmation prompts. Never include example private keys or seed phrases in documentation — use clearly fictional placeholders (e.g., `SXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX`). Ensure runbooks are tested on testnet before publication; untested runbooks executed under incident pressure are a liability.

### Definition of Done
- [ ] Unit tests written and passing
- [ ] Integration test added
- [ ] Documentation updated
- [ ] Security review completed (if applicable)
- [ ] CI pipeline green
- [ ] PR linked to this issue

---
