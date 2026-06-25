import { describe, it, expect, beforeEach } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
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

  it("flags form inputs as aria-invalid and announces errors via role=alert when submission fails", () => {
    render(<RpcSettings />);
    const urlInput = screen.getByLabelText(/RPC URL/i) as HTMLInputElement;
    const labelInput = screen.getByLabelText(/^Label$/i) as HTMLInputElement;

    expect(urlInput.getAttribute("aria-invalid")).toBeNull();
    expect(labelInput.getAttribute("aria-invalid")).toBeNull();

    fireEvent.change(urlInput, { target: { value: "ftp://nope.example.com" } });
    fireEvent.change(labelInput, { target: { value: "Bad" } });
    fireEvent.click(screen.getByRole("button", { name: /^Add$/i }));

    const alert = screen.getByRole("alert");
    expect(alert.textContent).toMatch(/http/i);
    expect(urlInput.getAttribute("aria-invalid")).toBe("true");
    expect(labelInput.getAttribute("aria-invalid")).toBe("true");
    expect(alert.getAttribute("aria-live")).toBe("polite");
  });

  it("links the URL input to a screen-reader-only help text via aria-describedby", () => {
    render(<RpcSettings />);
    const urlInput = screen.getByLabelText(/RPC URL/i);
    const describedById = urlInput.getAttribute("aria-describedby");
    expect(describedById).toBeTruthy();
    const helpEl = describedById ? document.getElementById(describedById) : null;
    expect(helpEl).not.toBeNull();
    expect(helpEl?.className).toContain("sr-only");
    expect(helpEl?.textContent).toMatch(/http or https/i);
  });
});
