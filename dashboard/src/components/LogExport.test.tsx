import { describe, it, expect, beforeEach } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import { LogExport } from "./LogExport";

describe("LogExport", () => {
  beforeEach(() => {
    localStorage.clear();
  });

  it("renders the panel title", () => {
    render(<LogExport />);
    expect(screen.getByText(/Audit Log Export/i)).toBeInTheDocument();
  });

  it("exposes the Export CSV action", () => {
    render(<LogExport />);
    expect(screen.getByRole("button", { name: /Export CSV/i })).toBeInTheDocument();
  });

  it("renders an entry count summary", () => {
    render(<LogExport />);
    // Anchor on the "{filtered} / {logs} entries" counter to avoid colliding
    // with the description text ("…audit entries and download…").
    expect(screen.getByText(/\d+\s*\/\s*\d+\s+entries/)).toBeInTheDocument();
  });

  it("announces the filtered count via aria-live=polite and updates the visible number on filter change", () => {
    render(<LogExport />);
    const counter = screen.getByText(/\d+\s*\/\s*\d+\s+entries/).parentElement;
    expect(counter?.getAttribute("aria-live")).toBe("polite");

    const before = counter?.textContent ?? "";
    // 'rejected' is always a strict subset of 'all' rows in the synthetic
    // dataset, so the displayed "X / Y entries" must change.
    fireEvent.change(screen.getByLabelText(/Status/i), {
      target: { value: "rejected" },
    });
    const after = screen.getByText(/\d+\s*\/\s*\d+\s+entries/).textContent ?? "";
    expect(after).not.toBe(before);
  });

  it("shows the empty-result message when no rows match the active filter", () => {
    render(<LogExport />);
    // 'mismatch'/'pending' status combinations don't occur in the synthetic
    // data, so picking both produces zero matches and the empty state.
    fireEvent.change(screen.getByLabelText(/Status/i), {
      target: { value: "pending" },
    });
    fireEvent.change(screen.getByLabelText(/Type/i), {
      target: { value: "replay" },
    });
    expect(screen.getByText(/No entries match the current filters/i)).toBeInTheDocument();
  });
});
