import { describe, it, expect, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import { RpcSettings } from "./RpcSettings";

describe("RpcSettings", () => {
  beforeEach(() => {
    localStorage.clear();
  });

  it("renders the panel title and description", () => {
    render(<RpcSettings />);
    expect(screen.getByText(/Custom RPC Endpoints/i)).toBeInTheDocument();
  });

  it("exposes the Probe all and Add controls", () => {
    render(<RpcSettings />);
    expect(screen.getByRole("button", { name: /Probe all/i })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /^Add$/i })).toBeInTheDocument();
  });

  it("shows the empty state when no nodes are configured", () => {
    render(<RpcSettings />);
    expect(screen.getByText(/No RPC endpoints configured yet/i)).toBeInTheDocument();
  });
});
