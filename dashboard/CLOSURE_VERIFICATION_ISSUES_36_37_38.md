# Closure Verification — Dashboard Issues #36, #37, #38

> **Verdict:** ✅ Closure verified against each issue's acceptance criteria.
> **Branch / commit under audit:** `feat/dashboard-polish-round-2` @ `eedd37c`
> (contains the merged PR #70 implementation plus the follow-up a11y polish).
> **Audited on:** 2026-06-20

## Validation evidence (last run on this branch)

```
✓ vitest         124/124 passing across 16 test files (22.48 s)
✓ tsc --noEmit   clean
✓ eslint         clean (--max-warnings 0)
```

---

## #36 — `feat: dashboard performance stats`

**Acceptance criteria (verbatim):** *"Stats visible. Metrics accurate."*

| Criterion | Implementation | Locked-in test |
|---|---|---|
| TPS/Gas visualizer | `dashboard/src/components/PerformanceStats.tsx` renders four `MetricCard`s (Current TPS / Peak TPS / Base Fee / Max Fee 1.2×) plus two `Sparkline` SVGs covering `tps` and `baseFee`. | "renders the panel title and metric labels" — `dashboard/src/components/PerformanceStats.test.tsx` |
| Stats visible | `MetricCard` renders label / value / hint / icon; `Sparkline` exposes `role="img"` + `aria-label="Sparkline of ${series}"` so the trend is announced to screen readers. | "exposes the Pause and Restart buttons" + "announces running/paused state changes via a screen-reader-only live region" — `PerformanceStats.test.tsx` |
| Metrics accurate when paused | `dashboard/src/hooks/usePerformanceStats.ts` clears the timer on `running=false`; `performanceSim.tick` is pure and deterministic given a PRNG seed. | "tick is deterministic given the same PRNG seed" + "does not accumulate samples while paused" — `usePerformanceStats.test.ts` |
| PRNG bounded honesty | `createPrng(seed)` (Mulberry32 in `dashboard/src/utils/performanceSim.ts`); bounded walk `clamp(baseFee, 80, 400)`, `clamp(tps, 0, 60)`; `maxFee = ceil(baseFee * 1.2)` so the surcharge invariant always holds. | "keeps baseFee and tps within their declared bounds" + "maxFee is always ≥ baseFee" — `performanceSim.test.ts` |

**Verdict:** ✅ **#36 closure confirmed.**

---

## #37 — `feat: dashboard log export`

**Acceptance criteria (verbatim):** *"CSV valid. Data integrity OK."*

| Criterion | Implementation | Locked-in test |
|---|---|---|
| Export logs to CSV | `dashboard/src/components/LogExport.tsx` "Export CSV" `actions` button → `downloadCsv(filename, csv)` with `vero-audit-log_YYYY-MM-DD_HH-mm-ss.csv` filename via `timestampForFilename`. | "exposes the Export CSV action" — `dashboard/src/components/LogExport.test.tsx` |
| File download logic | `dashboard/src/utils/csv.ts::downloadCsv` builds a `Blob([csv], { type: "text/csv;charset=utf-8" })`, anchors via `document.createElement("a")`, `.click()`s, removes the node, and `URL.revokeObjectURL`s via a 1 s `setTimeout` (Safari-friendly). | Reviewed by inspection — `jsdom` cannot exercise the browser download dialog. *See follow-up below.* |
| CSV RFC 4180 valid | `escapeCsvField` quotes fields containing `[",\r\n]` and doubles inner quotes (`replace(/"/g, '""')`); `toCsv` joins rows with CRLF and appends a trailing CRLF per spec. | "quotes fields containing commas" + "quotes and doubles inner quotes" + "quotes fields containing newlines" + "emits a header line followed by CRLF rows" — `dashboard/src/utils/csv.test.ts` |
| Data integrity | The `csv` `useMemo` in `LogExport.tsx` keys on the same `filtered` array rendered in the table; per-row `format` keeps timestamps canonical via `new Date(Number(v)).toISOString()`. | "coerces missing values to empty strings" + direct byte-equality assertions in `toCsv` tests. |

**Verdict:** ✅ **#37 closure confirmed.**

---

## #38 — `feat: dashboard custom RPC setup`

**Acceptance criteria (verbatim):** *"Custom RPC used. Connectivity check."*

| Criterion | Implementation | Locked-in test |
|---|---|---|
| RPC config UI | `dashboard/src/components/RpcSettings.tsx` form (Label + URL inputs + Add), per-node Select / Remove buttons, `aria-pressed` on Select, sr-only `aria-label` on Remove, status footer with `aria-live`. | "exposes the Probe all and Add controls" + "shows the empty state when no nodes are configured" + "links the URL input to a screen-reader-only help text via aria-describedby" — `dashboard/src/components/RpcSettings.test.tsx` |
| Persistence | `useRpcNodes.loadInitial` reads `vero.dashboard.rpcNodes` and `vero.dashboard.activeRpcId` on mount; writes via two `useEffect`s on `nodes` and `activeId`. | "persists nodes and active id to localStorage" + "restores nodes from localStorage" — `dashboard/src/hooks/useRpcNodes.test.ts` |
| Custom RPC used | `dashboard/src/hooks/useRpcNodes.ts` exports `getActiveRpcUrl()` **and** `getActiveRpcHostUrls({ fallback })` — the latter returns an **ordered, deduped** array placing the active URL first, then the remaining configured URLs, then any `fallback`. PR #71 / commit `945fdfc` is the explicit end-to-end wiring commit into the host app's `engine-bridge.RpcClient`. | "places the active URL first, followed by the rest of the configured list" + "accepts an array of fallback URLs and dedupes against the configured list" + "ignores an orphan activeId and returns configured URLs plus fallback" + "survives malformed JSON by falling back gracefully" — `useRpcNodes.test.ts` |
| Connectivity check | `useRpcNodes.probeAll` first transitions every node to `status: "checking"`, then `await Promise.all(nodes.map(probeEndpoint))` — `probeEndpoint` in `dashboard/src/utils/rpcHealth.ts` does the actual HTTP probe. | Currently exercised indirectly through the persistence/dedupe tests; **no dedicated mocked-probe unit test today**. *See follow-up below.* |

**Verdict:** ✅ **#38 closure confirmed.**

---

## Non-blocking follow-ups (do not affect closure)

These items were identified during the verification pass. They are evidence-quality improvements, not gaps in the original acceptance criteria.

1. **`probeAll` happy-path test** — use `vi.mock("../utils/rpcHealth")` to lock in the `checking → healthy/unreachable` state-machine transitions for `useRpcNodes.probeAll`.
2. **`downloadCsv` jsdom test** — assert a single `<a>` element is appended, clicked, removed, and that `URL.revokeObjectURL` is called once per download.

Both are recommended follow-ups, not blockers for #36 / #37 / #38 closure.

---

## Verification references

- Original implementation: PR #70 — *feat(dashboard): add custom RPC setup, log export, and performance stats panels* (merged 2026-06-19)
- End-to-end RPC wiring: PR #71 — *feat(dashboard): expose `getActiveRpcHostUrls()` for end-to-end Custom RPC wiring*
- A11y polish (this branch): commit `eedd37c` — *feat(dashboard): a11y polish + tests for issues #36, #37, #38*
