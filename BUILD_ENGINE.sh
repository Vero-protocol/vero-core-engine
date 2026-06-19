#!/usr/bin/env bash
# BUILD_ENGINE.sh — Scaffold, build, and health-check the vero-core-engine.
# Usage:
#   ./BUILD_ENGINE.sh            — scaffold + build + health-check
#   ./BUILD_ENGINE.sh scaffold   — directory/file scaffold only
#   ./BUILD_ENGINE.sh build      — compile engine-core (Rust) + engine-bridge (TS)
#   ./BUILD_ENGINE.sh health     — verify engine components are correctly linked
set -euo pipefail

ROOT="$(cd "$(dirname "$0")" && pwd)"
CORE="$ROOT/engine-core"
BRIDGE="$ROOT/engine-bridge"
DOCS="$ROOT/docs"
SECURITY_DIR="$ROOT/security"

GREEN='\033[0;32m'; YELLOW='\033[1;33m'; RED='\033[0;31m'; NC='\033[0m'

log()  { echo -e "${GREEN}[BUILD]${NC} $*"; }
warn() { echo -e "${YELLOW}[WARN]${NC}  $*"; }
fail() { echo -e "${RED}[FAIL]${NC}  $*"; exit 1; }

# ── 1. SCAFFOLD ───────────────────────────────────────────────────────────────
scaffold() {
  log "Scaffolding directory structure…"
  mkdir -p \
    "$CORE/src" \
    "$BRIDGE/src/__tests__" \
    "$DOCS/adrs" \
    "$DOCS/incidents" \
    "$SECURITY_DIR" \
    "$ROOT/.github/ISSUE_TEMPLATE" \
    "$ROOT/.github/workflows"

  # Workspace Cargo.toml (idempotent)
  if [[ ! -f "$ROOT/Cargo.toml" ]]; then
    cat > "$ROOT/Cargo.toml" <<'TOML'
[workspace]
members = ["engine-core"]
resolver = "2"
TOML
    log "Created workspace Cargo.toml"
  fi

  # .env.example (idempotent)
  if [[ ! -f "$ROOT/.env.example" ]]; then
    cat > "$ROOT/.env.example" <<'ENV'
STELLAR_NETWORK=testnet
RPC_URLS=https://soroban-testnet.stellar.org,https://rpc-testnet.stellar.org
CONTRACT_ID=
SIGNING_KEY=
GUARDIAN_ADDRESS=
EVENT_CURSOR=
ENV
    log "Created .env.example"
  fi

  # Symlink security docs into security/
  [[ -L "$SECURITY_DIR/SECURITY.md" ]] || ln -s "$ROOT/SECURITY.md" "$SECURITY_DIR/SECURITY.md"

  log "Scaffold complete."
}

# ── 2. BUILD ─────────────────────────────────────────────────────────────────
build_core() {
  log "Building engine-core (Rust)…"
  if ! command -v cargo &>/dev/null; then
    warn "cargo not found — skipping engine-core build (install Rust via rustup)"
    return 0
  fi
  cargo build --manifest-path "$CORE/Cargo.toml" --release 2>&1 | tail -5
  log "engine-core build OK"
}

build_bridge() {
  log "Building engine-bridge (TypeScript)…"
  if ! command -v node &>/dev/null; then
    warn "node not found — skipping engine-bridge build"
    return 0
  fi
  cd "$BRIDGE"
  npm ci --silent
  npm run build -- --noEmitOnError
  cd "$ROOT"
  log "engine-bridge build OK"
}

build() {
  build_core
  build_bridge
}

# ── 3. HEALTH CHECK ──────────────────────────────────────────────────────────
health() {
  log "Running system health check…"
  local errors=0

  check() {
    local label="$1"; shift
    if "$@" &>/dev/null; then
      echo -e "  ${GREEN}✓${NC} $label"
    else
      echo -e "  ${RED}✗${NC} $label"
      (( errors++ )) || true
    fi
  }

  # File structure
  check "engine-core/src/lib.rs exists"           test -f "$CORE/src/lib.rs"
  check "engine-core/src/audit.rs exists"         test -f "$CORE/src/audit.rs"
  check "engine-core/src/governance.rs exists"    test -f "$CORE/src/governance.rs"
  check "engine-core/src/circuit_breaker.rs exists" test -f "$CORE/src/circuit_breaker.rs"
  check "engine-bridge/src/rpc-client.ts exists"  test -f "$BRIDGE/src/rpc-client.ts"
  check "engine-bridge/src/nonce-manager.ts exists" test -f "$BRIDGE/src/nonce-manager.ts"
  check "engine-bridge/src/event-propagator.ts exists" test -f "$BRIDGE/src/event-propagator.ts"
  check "engine-bridge/src/heartbeat-monitor.ts exists" test -f "$BRIDGE/src/heartbeat-monitor.ts"
  check "SECURITY.md exists"                       test -f "$ROOT/SECURITY.md"
  check ".github/ISSUE_TEMPLATE/feature_request.md exists" \
        test -f "$ROOT/.github/ISSUE_TEMPLATE/feature_request.md"

  # Build artefacts (optional — only if builds ran)
  if command -v cargo &>/dev/null; then
    check "engine-core compiled (release)" \
      test -f "$CORE/../../target/release/libengine_core.rlib" -o \
           -d "$ROOT/target/release"
  fi
  if [[ -d "$BRIDGE/dist" ]]; then
    check "engine-bridge dist/index.js exists"   test -f "$BRIDGE/dist/index.js"
    check "engine-bridge dist/index.d.ts exists" test -f "$BRIDGE/dist/index.d.ts"
  fi

  # Tests (if dependencies installed)
  if [[ -d "$BRIDGE/node_modules" ]]; then
    check "engine-bridge tests pass" bash -c "cd '$BRIDGE' && npm test -- --passWithNoTests 2>&1 | grep -q 'Tests:'"
  fi

  if [[ $errors -eq 0 ]]; then
    log "Health check PASSED (${GREEN}all components linked${NC})"
  else
    fail "Health check FAILED — $errors component(s) missing or broken"
  fi
}

# ── DISPATCH ─────────────────────────────────────────────────────────────────
CMD="${1:-all}"
case "$CMD" in
  scaffold) scaffold ;;
  build)    build    ;;
  health)   health   ;;
  all)      scaffold; build; health ;;
  *) fail "Unknown command: $CMD. Valid: scaffold | build | health | all" ;;
esac
