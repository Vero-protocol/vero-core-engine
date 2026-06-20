import { describe, it, expect } from "vitest";
import { generateAuditLog } from "./auditLog";

describe("generateAuditLog", () => {
  it("produces the requested number of entries", () => {
    const log = generateAuditLog(20, 1_700_000_000_000, 0xdeadbeef);
    expect(log).toHaveLength(20);
  });

  it("uses the supplied timestamp as the upper bound", () => {
    const now = 1_700_000_000_000;
    const log = generateAuditLog(5, now, 1);
    for (const entry of log) {
      expect(entry.timestamp).toBeLessThanOrEqual(now);
      expect(entry.timestamp).toBeGreaterThan(now - 5 * 1000 - 10_000);
    }
  });

  it("is deterministic given the same seed", () => {
    const a = generateAuditLog(15, 1_700_000_000_000, 42);
    const b = generateAuditLog(15, 1_700_000_000_000, 42);
    expect(a).toEqual(b);
  });

  it("uses plausible field shapes", () => {
    const log = generateAuditLog(10, 1_700_000_000_000, 7);
    for (const entry of log) {
      expect(entry.sequence).toBeGreaterThan(0);
      expect(entry.ledger).toBeGreaterThan(0);
      expect(entry.stateHash).toMatch(/^[0-9a-f]{64}$/);
      expect(entry.author.length).toBeGreaterThanOrEqual(40);
      expect(entry.payloadBytes).toBeGreaterThan(0);
      expect(entry.feeStroops).toBeGreaterThan(0);
    }
  });
});
