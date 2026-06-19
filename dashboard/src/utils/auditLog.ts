/**
 * auditLog.ts — Synthetic audit log generator for the dashboard.
 *
 * The dashboard renders the audit layer surfaced by `engine-core/src/audit.rs`
 * but has no backend in this repo. We generate shaped-but-deterministic
 * entries that mirror the on-chain data: sequence, ledger, author, state
 * hash, event type and status. Determinism is anchored on a fixed seed so
 * snapshot tests stay stable.
 */

import type { AuditEntry, AuditEventType, AuditStatus } from "../types";
import { createPrng } from "./performanceSim";

function hex(bytes: number, prng: () => number): string {
  let out = "";
  for (let i = 0; i < bytes; i++) {
    const byte = Math.floor(prng() * 256);
    out += byte.toString(16).padStart(2, "0");
  }
  return out;
}

const AUTHORS = [
  "GABCXH5FQZ7YPRZ5JB7K3K7VZKRPAWZQ4MZJY3Y7CZJLPQNVTDB",
  "GBG2NFV7OX3ZQ5GX5VQ5XJX7XZTCQYHZF4QYQJZQJZQZQZQ",
  "GCDTF34JVR2S5YJZRNZJXYZQZQZQZQZQZQZQZQZQZQZQZQ",
  "GDAFG4VNWZJZ4YQZ2QYVQ5XJX7XZTCQYHZF4QYQJZQJZQZQ",
  "GEFCE4YQZQZQZQZQZQZQZQZQZQZQZQZQZQZQZQZQZQZQZQ",
];

const EVENT_TYPES: AuditEventType[] = [
  "commit",
  "commit",
  "commit",
  "commit",
  "commit",
  "snapshot",
  "replay",
  "mismatch",
];

const STATUS_BY_EVENT: Record<AuditEventType, AuditStatus[]> = {
  commit: ["success", "success", "success", "success", "rejected"],
  snapshot: ["success", "success"],
  replay: ["rejected"],
  mismatch: ["rejected"],
};

function authorAt(prng: () => number): string {
  return AUTHORS[Math.floor(prng() * AUTHORS.length)];
}

/**
 * Generate a list of audit entries ending at `now`.
 *
 * The default count and spacing produces ~1 minute of synthetic activity so
 * the visualisation feels alive without being overwhelming.
 */
export function generateAuditLog(
  count: number = 60,
  now: number = Date.now(),
  seed: number = 0xc0ffee
): AuditEntry[] {
  const prng = createPrng(seed);
  const out: AuditEntry[] = [];
  for (let i = 0; i < count; i++) {
    const eventType =
      EVENT_TYPES[Math.floor(prng() * EVENT_TYPES.length)];
    const statusOptions = STATUS_BY_EVENT[eventType];
    const status = statusOptions[Math.floor(prng() * statusOptions.length)];
    const ts = now - (count - i) * 1000 + Math.floor(prng() * 600);
    out.push({
      id: `audit-${i.toString(36)}-${ts.toString(36)}`,
      timestamp: ts,
      sequence: 18_000_000 + i,
      ledger: 56_000_000 + i * 64,
      author: authorAt(prng),
      stateHash: hex(32, prng),
      eventType,
      status,
      payloadBytes: 256 + Math.floor(prng() * 1536),
      feeStroops: 80 + Math.floor(prng() * 320),
    });
  }
  return out;
}
