# Security Policy

## Supported Versions

Only the versions listed below receive active security patches. If you are running an unsupported version, please upgrade before reporting a vulnerability.

| Version | Supported          | Notes                          |
|---------|--------------------|--------------------------------|
| `main`  | ✅ Yes             | Active development branch      |
| `0.x`   | ✅ Yes             | Current pre-release series     |
| < `0.1` | ❌ No              | Deprecated — no backports      |

---

## Reporting a Vulnerability

**Do not open a public GitHub issue for security vulnerabilities.** Doing so exposes users before a fix is available.

### Reporting Channels

| Channel | Details |
|---------|---------|
| **GitHub Private Advisory** | [Submit a private advisory](https://github.com/your-org/vero-core-engine/security/advisories/new) — preferred method |
| **Email** | `security@vero-protocol.io` — PGP key available on request |

We aim to acknowledge all reports within **48 hours** and will keep you informed throughout the remediation process.

### Report Template

Please include as much of the following as possible in your report:

```
**Vulnerability Summary**
A concise one- or two-sentence description of the issue.

**Affected Component**
e.g., contracts/vero-core, relayer/event-consumer, dashboard/api

**Severity (your assessment)**
Critical / High / Medium / Low

**Steps to Reproduce**
1.
2.
3.

**Proof of Concept**
Code snippet, transaction hash, or screenshot demonstrating the issue.

**Impact**
What can an attacker achieve? What data or funds are at risk?

**Suggested Fix (optional)**
Any recommendations for remediation.

**Your contact details**
How should we reach you for follow-up?
```

---

## Disclosure Timeline

| Day | Activity |
|-----|----------|
| **Day 0** | Vulnerability report received |
| **Day 1–2** | Acknowledgement sent to reporter |
| **Day 3–7** | Triage and severity assessment completed; reporter notified of outcome |
| **Day 8–14** | Fix developed and reviewed internally |
| **Day 15–21** | Fix tested against all supported versions; regression tests added |
| **Day 22–25** | Patch released and advisory draft prepared |
| **Day 26–29** | CVE requested (if applicable); affected users notified via advisory |
| **Day 30** | Public disclosure of advisory |
| **Day 30+** | Extended timeline negotiated with reporter if complexity warrants it |

We follow a **coordinated disclosure** model. We ask reporters to observe a 30-day embargo to allow users time to upgrade before full public disclosure.

---

## Severity Definitions

| Severity | CVSS Score | Description | Examples |
|----------|------------|-------------|----------|
| **Critical** | 9.0–10.0 | Direct loss of funds, complete contract takeover, or full authentication bypass with no prerequisites | Reentrancy enabling fund drainage; admin key extraction; unauthorized contract upgrade |
| **High** | 7.0–8.9 | Significant impact on data integrity, availability, or partial fund loss; exploitation requires low privileges or user interaction | State machine bypass allowing double-spend; relayer signature forgery; privilege escalation within contract roles |
| **Medium** | 4.0–6.9 | Limited impact or requires significant attacker prerequisites; degrades reliability or leaks non-critical data | Event replay enabling analytics manipulation; relayer DoS via malformed input; information disclosure from dashboard API |
| **Low** | 0.1–3.9 | Minimal impact; theoretical attack paths with little real-world exploitability | Minor information leakage; dependency with known low-severity CVE; documentation revealing internal endpoints |

---

## Safe Harbor

We consider security research conducted in good faith to be authorized and will not pursue legal action against researchers who:

- Report vulnerabilities through the channels above before public disclosure.
- Avoid accessing, modifying, or destroying data belonging to other users.
- Do not perform denial-of-service attacks against any production or testnet infrastructure.
- Do not violate any applicable law in conducting their research.
- Make a good-faith effort to avoid privacy violations and disruption to others.

We will work with researchers to understand and resolve issues quickly, and we commit to not taking legal action against researchers who abide by this policy.

---

## Scope

### In Scope

- Soroban smart contracts in `contracts/`
- Relayer service in `relayer/`
- Dashboard API and frontend in `dashboard/`
- Deployment scripts in `scripts/`
- CI/CD pipeline configurations in `.github/`
- Cryptographic routines (signing, verification, ZK interfaces)
- Access control and multi-sig logic
- Dependency vulnerabilities with a direct exploitation path

### Out of Scope

- Vulnerabilities in third-party dependencies without a direct exploitation path in this project
- Issues in the Stellar network protocol or Soroban runtime itself (report those to [Stellar's security team](https://www.stellar.org/bug-bounty-program))
- Social engineering attacks targeting project maintainers
- Physical security attacks
- Denial-of-service attacks that require exceptional resources (e.g., volumetric DDoS)
- Issues already known and tracked in our public issue tracker
- Findings from automated scanners submitted without a proof of concept

---

## Bug Bounty

A formal bug bounty program is planned as part of **[Issue #39 — Bug-bounty program scaffolding](https://github.com/your-org/vero-core-engine/issues/39)**. Until that program is live, we offer public acknowledgement in release notes and our Hall of Fame for validated, responsibly disclosed vulnerabilities. Monetary rewards are not currently available but may be offered at maintainer discretion for Critical and High severity findings.

---

## Hall of Fame

We gratefully acknowledge researchers who have helped improve the security of `vero-core-engine`. Contributors will be listed here upon public disclosure with their consent.

| Researcher | Severity | Summary | Disclosed |
|------------|----------|---------|-----------|
| *(none yet)* | — | — | — |

---

---

## Incident Response — Engine Components

### engine-core (Rust / Soroban)

| Trigger | First Response | Escalation |
|---------|---------------|------------|
| Audit hash mismatch detected on-chain | Trip circuit-breaker via `circuit_breaker::trip(guardian)` | Notify security@ within 1 hour |
| Governance proposal executed anomalously | Freeze treasury via emergency override proposal | Convene multi-sig holders within 4 hours |
| Replay attack on state commitment | Pause contract; capture ledger range and `sequence` gap | File private advisory + CVE request |

**Immediate containment playbook (engine-core):**
```bash
# 1. Trip the circuit-breaker (halts all state transitions)
stellar contract invoke --id $CONTRACT_ID -- trip --guardian $GUARDIAN_ADDR

# 2. Capture current state commitment for forensics
stellar contract invoke --id $CONTRACT_ID -- get_state_hash > incident-$(date +%s).json

# 3. Page on-call via PagerDuty / Slack #security-incidents
```

### engine-bridge (TypeScript)

| Trigger | First Response | Escalation |
|---------|---------------|------------|
| All RPC endpoints quarantined | Check node health; manually promote backup endpoint | Notify infra team; SLA breach if >5 min |
| Nonce desync causing tx failures | Call `NonceManager.refresh(accountId)` | Inspect mempool for stuck transactions |
| Event cursor corruption / gap | Roll back cursor to last verified ledger; trigger replay | Alert dashboard team; audit missed events |

**Immediate containment playbook (engine-bridge):**
```bash
# 1. Restart bridge with explicit cursor rollback
ENGINE_CURSOR=<last_good_cursor> npm run bridge:start

# 2. Verify RPC failover status
curl http://localhost:3000/health | jq '.rpc.liveCount'

# 3. Check event backlog
npm run bridge:audit-events -- --from <ledger> --to <ledger>
```

### Post-Incident Requirements

- Root-cause analysis document committed to `docs/incidents/YYYY-MM-DD-<slug>.md` within 72 hours.
- Regression test added to prevent recurrence.
- SECURITY.md updated if the incident reveals a gap in scope or process.

---

*This policy was last updated: 2026-06-19*
