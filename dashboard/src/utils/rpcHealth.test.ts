import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { isValidHttpUrl, probeEndpoint } from "./rpcHealth";

describe("isValidHttpUrl", () => {
  it("accepts http and https URLs", () => {
    expect(isValidHttpUrl("https://example.com")).toBe(true);
    expect(isValidHttpUrl("http://example.com")).toBe(true);
    expect(isValidHttpUrl("https://example.com:9000/path")).toBe(true);
  });
  it("rejects non-http schemes and garbage", () => {
    expect(isValidHttpUrl("ftp://example.com")).toBe(false);
    expect(isValidHttpUrl("ws://example.com")).toBe(false);
    expect(isValidHttpUrl("")).toBe(false);
    expect(isValidHttpUrl("not a url")).toBe(false);
  });
});

describe("probeEndpoint", () => {
  const originalFetch = globalThis.fetch;

  beforeEach(() => {
    vi.useFakeTimers();
  });
  afterEach(() => {
    globalThis.fetch = originalFetch;
    vi.useRealTimers();
  });

  it("returns unreachable for invalid URLs without calling fetch", () => {
    const fetchSpy = vi.fn();
    globalThis.fetch = fetchSpy;
    return probeEndpoint("not a url").then((res) => {
      expect(res.status).toBe("unreachable");
      expect(fetchSpy).not.toHaveBeenCalled();
    });
  });

  it("returns healthy on 2xx HEAD response", async () => {
    globalThis.fetch = vi.fn().mockResolvedValue({ ok: true, status: 200 });
    const res = await probeEndpoint("https://example.com");
    expect(res.status).toBe("healthy");
    expect(res.latencyMs).toBeGreaterThanOrEqual(0);
  });

  it("falls back to GET and returns healthy on 2xx", async () => {
    globalThis.fetch = vi
      .fn()
      .mockResolvedValueOnce({ ok: false, status: 405 })
      .mockResolvedValueOnce({ ok: true, status: 200 });
    const res = await probeEndpoint("https://example.com");
    expect(res.status).toBe("healthy");
  });

  it("returns unreachable when both probes fail", async () => {
    globalThis.fetch = vi
      .fn()
      .mockResolvedValue({ ok: false, status: 500 });
    const res = await probeEndpoint("https://example.com");
    expect(res.status).toBe("unreachable");
    expect(res.message).toMatch(/HTTP 500/);
  });
});
