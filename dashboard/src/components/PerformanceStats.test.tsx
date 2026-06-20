import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { PerformanceStats } from "./PerformanceStats";

describe("PerformanceStats", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    localStorage.clear();
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it("renders the panel title and metric labels", () => {
    render(<PerformanceStats />);
    expect(screen.getByText(/Network Performance/i)).toBeInTheDocument();
    expect(screen.getByText(/Current TPS/i)).toBeInTheDocument();
    // The sparkline panel below also mentions "Base fee", so anchor the
    // metric-card label to avoid a `getMultipleElementsFound` collision.
    expect(screen.getByText(/^Base Fee$/i)).toBeInTheDocument();
  });

  it("exposes the Pause and Restart buttons", () => {
    render(<PerformanceStats />);
    expect(screen.getByRole("button", { name: /Pause/i })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Restart/i })).toBeInTheDocument();
  });
});
