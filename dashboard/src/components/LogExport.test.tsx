import { describe, it, expect, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
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
});
