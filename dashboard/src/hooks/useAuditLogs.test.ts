import { describe, it, expect } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { useAuditLogs } from "./useAuditLogs";

describe("useAuditLogs", () => {
  it("returns a non-empty set of logs and full set with no filters", () => {
    const { result } = renderHook(() => useAuditLogs());
    expect(result.current.logs.length).toBeGreaterThan(0);
    expect(result.current.filtered.length).toBe(result.current.logs.length);
  });

  it("filters by status", () => {
    const { result } = renderHook(() => useAuditLogs());
    act(() => result.current.setStatusFilter("success"));
    expect(result.current.filtered.length).toBeGreaterThan(0);
    expect(result.current.filtered.every((e) => e.status === "success")).toBe(true);
  });

  it("filters by type", () => {
    const { result } = renderHook(() => useAuditLogs());
    act(() => result.current.setTypeFilter("commit"));
    expect(result.current.filtered.length).toBeGreaterThan(0);
    expect(result.current.filtered.every((e) => e.eventType === "commit")).toBe(true);
  });

  it("combines status and type filters with AND semantics", () => {
    const { result } = renderHook(() => useAuditLogs());
    act(() => result.current.setStatusFilter("rejected"));
    act(() => result.current.setTypeFilter("replay"));
    // Deterministic generator must include at least one replay/rejected entry.
    expect(result.current.filtered.length).toBeGreaterThan(0);
    expect(
      result.current.filtered.every(
        (e) => e.status === "rejected" && e.eventType === "replay"
      )
    ).toBe(true);
  });
});
