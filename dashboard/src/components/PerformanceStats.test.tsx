import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
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

  it("flips aria-pressed on the Pause/Resume toggle to reflect running state", () => {
    render(<PerformanceStats />);
    const toggle = screen.getByRole("button", { name: /Pause/i });
    expect(toggle.getAttribute("aria-pressed")).toBe("false");

    fireEvent.click(toggle);
    const resume = screen.getByRole("button", { name: /Resume/i });
    expect(resume.getAttribute("aria-pressed")).toBe("true");
  });

  it("announces running/paused state changes via a screen-reader-only live region", () => {
    render(<PerformanceStats />);
    const live = document.querySelector('[aria-live="polite"]');
    expect(live).not.toBeNull();
    expect(live?.textContent).toMatch(/running/i);
  });
});
