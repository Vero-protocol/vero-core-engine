import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { usePerformanceStats } from "./usePerformanceStats";

describe("usePerformanceStats", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it("starts running by default", () => {
    const { result } = renderHook(() => usePerformanceStats(1000));
    expect(result.current.running).toBe(true);
  });

  it("toggle pauses and then resumes", () => {
    const { result } = renderHook(() => usePerformanceStats(1000));
    act(() => result.current.toggle());
    expect(result.current.running).toBe(false);
    act(() => result.current.toggle());
    expect(result.current.running).toBe(true);
  });

  it("accumulates samples as the timer advances", () => {
    const { result } = renderHook(() => usePerformanceStats(1000));
    act(() => vi.advanceTimersByTime(3000));
    expect(result.current.samples.length).toBeGreaterThanOrEqual(2);
  });

  it("does not accumulate samples while paused", () => {
    const { result } = renderHook(() => usePerformanceStats(1000));
    act(() => result.current.toggle());
    const before = result.current.samples.length;
    act(() => vi.advanceTimersByTime(3000));
    expect(result.current.samples.length).toBe(before);
  });

  it("restart clears accumulated samples and counters", () => {
    const { result } = renderHook(() => usePerformanceStats(1000));
    act(() => vi.advanceTimersByTime(3000));
    expect(result.current.samples.length).toBeGreaterThan(0);
    act(() => result.current.restart());
    expect(result.current.samples).toHaveLength(0);
  });

  it("computed aggregates are non-negative after ticks", () => {
    const { result } = renderHook(() => usePerformanceStats(1000));
    act(() => vi.advanceTimersByTime(2000));
    expect(result.current.peakTps).toBeGreaterThanOrEqual(0);
    expect(result.current.avgTps).toBeGreaterThanOrEqual(0);
    expect(result.current.latestMaxFee).toBeGreaterThanOrEqual(result.current.latestBaseFee);
  });
});
