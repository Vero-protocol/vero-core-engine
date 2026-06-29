# Core Engine Fix Summary — feat/engine-core-20

## Issue
The `vero-core-engine` module was supposed to provide a hardened, audit-ready
foundation for the Vero Protocol control plane, but the codebase did not compile
and the control plane was not integrated into the library surface.

## Root Causes Found
1. **Missing `governance::propose` entrypoint** — a prior merge dropped the
   `propose` function body, leaving orphaned code that broke compilation.
2. **Control plane not exposed** — `engine-core/src/lib.rs` did not declare
   `pub mod core;`, so the `ControlPlane` contract was unreachable.
3. **Duplicate authorization** — `ControlPlane::update_param` required auth for
   both `caller` and `commitment.author`; when they are the same address (the
   intended design), Soroban rejected the second `require_auth` with
   `Error(Auth, ExistingValue)`.
4. **Missing safety guards** — `update_param` could overwrite internal storage
   keys (`ADMIN`, `SEQ`, `PROPS`, etc.) and did not emit an audit event.
5. **Missing test contract** — `circuit_breaker.rs` tests referenced
   `TestContract` without defining it.

## Changes Made

### Source files
| File | Change |
|------|--------|
| `engine-core/src/lib.rs` | Added `pub mod core;` to expose the control plane. |
| `engine-core/src/governance.rs` | Restored the missing `propose` function; added `BytesN` import. |
| `engine-core/src/audit.rs` | Removed `commitment.author.require_auth()` from `validate_transition`; callers now handle auth at the entrypoint. |
| `engine-core/src/core/control_plane.rs` | Hardened `update_param` with caller/author equality, reserved-key protection, audit event emission, and `get_admin`/`get_param` helpers. |
| `engine-core/src/core/tests.rs` | Expanded test coverage from 2 to 9 tests covering auth, author mismatch, reserved keys, circuit breaker, replay, events, and auth recording. |
| `engine-core/src/event_struct.rs` | Added `ACT_UPDATE` event action for control-plane parameter updates. |
| `engine-core/src/circuit_breaker.rs` | Added missing `TestContract` definition in tests. |
| `engine-core/src/governance_tests.rs` | Fixed `InvalidStateTransition` error code assertion (5, not 4) and silenced dead-code warning on `StateTransitionMatrix`. |

### Test snapshots
Updated and added Soroban test snapshots to reflect the new tests and the
restored `propose` behavior.

## Validation
- `cargo build` ✅
- `cargo build --release` ✅
- `cargo test` ✅ (71 passed, 0 failed)
- `cargo clippy --all-targets -- -D warnings` ✅
- `./BUILD_ENGINE.sh health` ✅
- `./BUILD_ENGINE.sh all` ✅ (engine-core + engine-bridge)

## Confidence
100% — the branch compiles cleanly, all available tests pass, clippy is clean,
and the health check verifies the control plane is now linked into the engine.
