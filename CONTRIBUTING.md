# Contributing to vero-core-engine

Thank you for contributing to protocol-grade infrastructure. This guide defines the standards required for all contributions. Non-conforming PRs will be closed without review.

---

## Table of Contents

1. [Code of Conduct](#code-of-conduct)
2. [Conventional Commits](#conventional-commits)
3. [Branch Strategy](#branch-strategy)
4. [Pull Request Workflow](#pull-request-workflow)
5. [Code Review Expectations](#code-review-expectations)
6. [Development Setup](#development-setup)
7. [Testing Requirements](#testing-requirements)
8. [Security-Sensitive Changes](#security-sensitive-changes)

---

## Code of Conduct

All contributors must adhere to the [Contributor Covenant Code of Conduct](CODE_OF_CONDUCT.md). Violations result in immediate removal.

Report unacceptable behavior by [opening an issue](https://github.com/Vero-protocol/vero-core-engine/issues) on this repository.

---

## Conventional Commits

All commit messages **must** conform to [Conventional Commits v1.0](https://www.conventionalcommits.org/en/v1.0.0/).

### Format

```
<type>(<scope>): <short summary>

[optional body — wrap at 72 chars]

[optional footer: BREAKING CHANGE, Closes #N, Refs #N]
```

### Allowed Types

| Type | When to use |
|---|---|
| `feat` | New feature or capability |
| `fix` | Bug fix |
| `refactor` | Code restructuring without behavior change |
| `perf` | Performance improvement |
| `test` | Adding or correcting tests |
| `docs` | Documentation only |
| `chore` | Tooling, deps, CI changes |
| `security` | Security hardening (non-breaking) |
| `audit` | Audit trail or ZK-proof updates |
| `revert` | Reverts a previous commit |

### Allowed Scopes

`engine-core` · `engine-bridge` · `dashboard` · `governance` · `zk` · `ci` · `deps` · `docs`

`engine-core` and `engine-bridge` map to the directories of the same name; `governance` and `zk` are logical scopes for changes inside `engine-core`.

### Examples

```
feat(engine-core): add ZK-audit hook interface to core state machine

Implements the hook interface defined in #3. All state transitions
now emit a structured audit event consumable by the ZK proof layer.

Closes #3
```

```
security(engine-bridge): enforce Ed25519 signature verification on all receipts

BREAKING CHANGE: receipt schema v1 is no longer accepted; callers
must upgrade to schema v2 before this release.
```

### Enforcement

Commit messages must follow the Conventional Commits format above; they are enforced during code review. Since feature branches are squash-merged onto `main` using the PR title, keep PR titles in the same format.

---

## Branch Strategy

```
main          ← protected; requires 2 approvals + passing CI
  └─ milestone/M1-engine-core
       └─ feat/engine-core-zk-audit-hook   ← your branch
  └─ milestone/M2-engine-bridge
  └─ hotfix/critical-patch-description    ← hotfixes only
```

- Branch from the relevant `milestone/*` branch, never directly from `main`.
- Name branches: `<type>/<short-slug>` (e.g., `feat/zk-audit-hook`, `fix/receipt-nonce-collision`).
- Delete branches after merge.

---

## Pull Request Workflow

### Before Opening a PR

- [ ] All tests pass locally: `cargo test` from the repo root, plus `npm test` in `engine-bridge/` and `dashboard/`
- [ ] Linters pass: `npm run lint` in `engine-bridge/` and `dashboard/`
- [ ] New code has tests (unit + integration where applicable)
- [ ] Commit history is clean — squash WIP commits
- [ ] PR references the issue it closes: `Closes #N`

### PR Title

Follow the same Conventional Commits format:

```
feat(engine-core): implement ZK-audit hook interface
```

### PR Description Template

```markdown
## Summary
<!-- One paragraph: what does this PR do and why? -->

## Changes
<!-- Bullet list of significant changes -->

## Testing
<!-- How was this tested? Include commands. -->

## Security Considerations
<!-- Any auth, crypto, or data-handling implications? -->

## Checklist
- [ ] Tests added / updated
- [ ] Docs updated if behavior changed
- [ ] No secrets committed
- [ ] Breaking changes noted in footer
```

### PR Size Guidelines

| Size | Lines Changed | Policy |
|---|---|---|
| XS | < 50 | Merge same day |
| S | 50–200 | 1 reviewer |
| M | 200–500 | 2 reviewers |
| L | 500–1000 | 2 reviewers + architecture review |
| XL | > 1000 | Must be pre-approved; break it up |

---

## Code Review Expectations

### For Authors

- Respond to review comments within **48 hours**.
- Don't resolve threads you didn't open.
- Mark the PR `Draft` if it's not ready; don't open for review prematurely.

### For Reviewers

Review within **72 hours** of assignment. Check:

1. **Correctness** — Does it do what the issue requires?
2. **Security** — New attack surface? Input validation? Auth bypass?
3. **Protocol integrity** — Does this maintain auditability and ZK-readiness?
4. **Test quality** — Are tests asserting behavior or just achieving coverage?
5. **Commit hygiene** — Are commits atomic and correctly scoped?

### Review Verdicts

| Verdict | Meaning |
|---|---|
| ✅ Approve | Ready to merge as-is |
| 💬 Comment | Non-blocking feedback |
| 🔄 Request Changes | Must be addressed before merge |
| 🚫 NACK | Architectural objection — escalate to maintainers |

### Merging

- **Squash merge** for feature branches (single clean commit on `main`).
- **Merge commit** for `milestone/*` into `main` (preserves history).
- Only maintainers with write access may merge into `main`.

---

## Development Setup

The repo has no root `package.json`; the Cargo workspace at the repo root covers `engine-core/` and `src/audit-guard/`, while `engine-bridge/` and `dashboard/` are separate npm projects. Node 20 is used in CI.

```bash
# 1. Install Rust toolchain (for engine-core contracts)
rustup target add wasm32-unknown-unknown
cargo install stellar-cli --locked

# 2. Run contract tests and the WASM build from the repo root
cargo test
cargo build --target wasm32-unknown-unknown --release

# 3. engine-bridge (TypeScript relayer service)
cd engine-bridge && npm ci && npm test

# 4. dashboard (TypeScript/React frontend)
cd dashboard && npm ci && npm test
```

Build and lint each npm project exactly as CI does:

```bash
cd engine-bridge && npm run build && npm run lint
cd dashboard && npm run build && npm run lint
```

---

## Testing Requirements

| Layer | Minimum Requirement |
|---|---|
| Engine-core (smart contracts) | Unit tests for every public function; fuzz tests for state transitions |
| Engine-bridge (relayer) | Unit + integration tests; mock Horizon responses |
| Dashboard API | Unit tests + OpenAPI contract tests |
| E2E | Smoke test must pass on testnet before PR merge |

Coverage threshold: **80% line coverage** enforced in CI. Security-critical paths require **100%**.

---

## Security-Sensitive Changes

Any change touching:
- Cryptographic key handling
- Signature verification
- Access control / authorization
- ZK proof generation or verification
- Treasury or governance logic

**Must** include:
1. A threat model section in the PR description.
2. A second reviewer with security background.
3. Reference to the relevant SECURITY.md disclosure policy.

For vulnerabilities discovered during development, follow [SECURITY.md](SECURITY.md) — do **not** open a public issue.
