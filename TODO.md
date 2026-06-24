# Task #69 — Circuit breaker checks + enhanced event processing

- [ ] Gather repo context: circuit breaker usage in engine-core; event processing flow in engine-bridge; dashboard wiring
- [ ] Resolve discrepancies found in current code (e.g., EventPropagator/HeartbeatMonitor API mismatches)
- [ ] Implement/verify circuit breaker checks integration (contract-side already present; ensure any missing entry-points call assert_closed)
- [ ] Enhance event processing reliability:
  - [ ] Ensure max-events-per-cycle enforced in both normal processing and recovery
  - [ ] Fix cursor persistence / heartbeat visibility (EventPropagator.isRunning usage)
  - [ ] Ensure RPC endpoint probing and status reporting are exposed/used correctly
  - [ ] Confirm queue processing error isolation + retries behavior
- [ ] Run tests/build:
  - [ ] cargo test (engine-core)
  - [ ] npm test/build (engine-bridge, dashboard)
- [ ] Finalize by ensuring TypeScript compile passes and tests are green

