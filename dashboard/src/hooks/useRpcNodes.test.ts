import { describe, it, expect, beforeEach } from "vitest";
import { renderHook, act } from "@testing-library/react";
import {
  useRpcNodes,
  getActiveRpcUrl,
  getActiveRpcHostUrls,
  normalizeRpcUrl,
} from "./useRpcNodes";

describe("useRpcNodes", () => {
  beforeEach(() => {
    localStorage.clear();
  });

  it("returns an empty list and null active id when nothing is stored", () => {
    const { result } = renderHook(() => useRpcNodes());
    expect(result.current.nodes).toEqual([]);
    expect(result.current.activeId).toBeNull();
    expect(result.current.activeNode).toBeNull();
  });

  it("rejects empty URLs", () => {
    const { result } = renderHook(() => useRpcNodes());
    act(() => {
      const r = result.current.addNode("label", "");
      expect(r.ok).toBe(false);
    });
    expect(result.current.nodes).toHaveLength(0);
  });

  it("rejects non-http schemes", () => {
    const { result } = renderHook(() => useRpcNodes());
    act(() => {
      const r = result.current.addNode("test", "ftp://example.com");
      expect(r.ok).toBe(false);
    });
    expect(result.current.nodes).toHaveLength(0);
  });

  it("rejects duplicate URLs", () => {
    const { result } = renderHook(() => useRpcNodes());
    act(() => {
      expect(result.current.addNode("a", "https://rpc.example.com").ok).toBe(true);
    });
    act(() => {
      const r = result.current.addNode("b", "https://rpc.example.com");
      expect(r.ok).toBe(false);
      expect(r.reason).toMatch(/already/i);
    });
    expect(result.current.nodes).toHaveLength(1);
  });

  it("treats trailing-slash variants as the same endpoint", () => {
    const { result } = renderHook(() => useRpcNodes());
    act(() => {
      expect(result.current.addNode("a", "https://rpc.example.com/").ok).toBe(true);
    });
    act(() => {
      const r = result.current.addNode("b", "https://rpc.example.com");
      expect(r.ok).toBe(false);
      expect(r.reason).toMatch(/already/i);
    });
    expect(result.current.nodes).toHaveLength(1);
  });

  it("adds a node and auto-activates when no active id exists", () => {
    const { result } = renderHook(() => useRpcNodes());
    act(() => {
      const r = result.current.addNode("SDF", "https://soroban.stellar.org");
      expect(r.ok).toBe(true);
      expect(r.id).toBeTruthy();
    });
    expect(result.current.nodes).toHaveLength(1);
    expect(result.current.activeId).toBe(result.current.nodes[0].id);
  });

  it("persists nodes and active id to localStorage", () => {
    const { result } = renderHook(() => useRpcNodes());
    act(() => {
      result.current.addNode("a", "https://a.example.com");
    });
    const stored = JSON.parse(localStorage.getItem("vero.dashboard.rpcNodes") ?? "[]");
    expect(stored).toHaveLength(1);
    expect(stored[0].url).toBe("https://a.example.com");
    expect(localStorage.getItem("vero.dashboard.activeRpcId")).toBe(result.current.activeId);
  });

  it("restores nodes from localStorage", () => {
    localStorage.setItem(
      "vero.dashboard.rpcNodes",
      JSON.stringify([{ id: "rpc-1", label: "L", url: "https://x", latencyMs: null, status: "unknown" }])
    );
    localStorage.setItem("vero.dashboard.activeRpcId", "rpc-1");
    const { result } = renderHook(() => useRpcNodes());
    expect(result.current.nodes).toHaveLength(1);
    expect(result.current.activeId).toBe("rpc-1");
  });

  it("removeNode falls back to the next node when removing the active one", () => {
    const { result } = renderHook(() => useRpcNodes());
    act(() => {
      result.current.addNode("a", "https://a.example.com");
    });
    const firstId = result.current.nodes[0].id;
    act(() => {
      result.current.addNode("b", "https://b.example.com");
    });
    expect(result.current.activeId).toBe(firstId);
    act(() => result.current.removeNode(firstId));
    expect(result.current.activeId).not.toBe(firstId);
    expect(result.current.nodes.find((n) => n.id === firstId)).toBeUndefined();
  });

  it("setActive ignores unknown ids", () => {
    const { result } = renderHook(() => useRpcNodes());
    act(() => {
      result.current.addNode("a", "https://a.example.com");
    });
    const initialActive = result.current.activeId;
    act(() => result.current.setActive("does-not-exist"));
    expect(result.current.activeId).toBe(initialActive);
  });
});

describe("getActiveRpcUrl", () => {
  beforeEach(() => {
    localStorage.clear();
  });

  it("returns null when nothing is configured", () => {
    expect(getActiveRpcUrl()).toBeNull();
  });

  it("returns the active node URL from localStorage", () => {
    const id = "rpc-test";
    localStorage.setItem(
      "vero.dashboard.rpcNodes",
      JSON.stringify([
        { id, label: "L", url: "https://a.example.com", latencyMs: null, status: "unknown" },
      ])
    );
    localStorage.setItem("vero.dashboard.activeRpcId", id);
    expect(getActiveRpcUrl()).toBe("https://a.example.com");
  });

  it("returns null when the stored active id does not match any node", () => {
    localStorage.setItem("vero.dashboard.rpcNodes", JSON.stringify([]));
    localStorage.setItem("vero.dashboard.activeRpcId", "rpc-ghost");
    expect(getActiveRpcUrl()).toBeNull();
  });

  it("survives malformed JSON without throwing", () => {
    localStorage.setItem("vero.dashboard.rpcNodes", "{not json");
    localStorage.setItem("vero.dashboard.activeRpcId", "rpc-x");
    expect(getActiveRpcUrl()).toBeNull();
  });
});

describe("normalizeRpcUrl", () => {
  it("strips a single trailing slash", () => {
    expect(normalizeRpcUrl("https://a.com/")).toBe("https://a.com");
  });

  it("strips multiple trailing slashes", () => {
    expect(normalizeRpcUrl("https://a.com///")).toBe("https://a.com");
  });

  it("leaves URLs without trailing slashes unchanged", () => {
    expect(normalizeRpcUrl("https://a.com/path")).toBe("https://a.com/path");
  });
});

describe("getActiveRpcHostUrls", () => {
  beforeEach(() => {
    localStorage.clear();
  });

  it("returns just the fallback when nothing is configured", () => {
    expect(
      getActiveRpcHostUrls({ fallback: "https://default.example.com" })
    ).toEqual(["https://default.example.com"]);
  });

  it("returns an empty array when nothing is configured and no fallback is supplied", () => {
    expect(getActiveRpcHostUrls()).toEqual([]);
  });

  it("places the active URL first, followed by the rest of the configured list", () => {
    const idActive = "rpc-active";
    localStorage.setItem(
      "vero.dashboard.rpcNodes",
      JSON.stringify([
        { id: "rpc-a", label: "A", url: "https://a.example.com", latencyMs: null, status: "unknown" },
        { id: idActive, label: "Active", url: "https://active.example.com", latencyMs: null, status: "unknown" },
        { id: "rpc-b", label: "B", url: "https://b.example.com", latencyMs: null, status: "unknown" },
      ])
    );
    localStorage.setItem("vero.dashboard.activeRpcId", idActive);
    expect(getActiveRpcHostUrls()).toEqual([
      "https://active.example.com",
      "https://a.example.com",
      "https://b.example.com",
    ]);
  });

  it("appends a fallback URL after the configured list", () => {
    const idActive = "rpc-active";
    localStorage.setItem(
      "vero.dashboard.rpcNodes",
      JSON.stringify([
        { id: idActive, label: "Active", url: "https://active.example.com", latencyMs: null, status: "unknown" },
      ])
    );
    localStorage.setItem("vero.dashboard.activeRpcId", idActive);
    expect(
      getActiveRpcHostUrls({ fallback: "https://default.example.com" })
    ).toEqual([
      "https://active.example.com",
      "https://default.example.com",
    ]);
  });

  it("accepts an array of fallback URLs and dedupes against the configured list", () => {
    const idActive = "rpc-active";
    localStorage.setItem(
      "vero.dashboard.rpcNodes",
      JSON.stringify([
        { id: idActive, label: "Active", url: "https://active.example.com", latencyMs: null, status: "unknown" },
      ])
    );
    localStorage.setItem("vero.dashboard.activeRpcId", idActive);
    expect(
      getActiveRpcHostUrls({
        fallback: ["https://active.example.com", "https://default.example.com"],
      })
    ).toEqual([
      "https://active.example.com",
      "https://default.example.com",
    ]);
  });

  it("survives malformed JSON by falling back gracefully", () => {
    localStorage.setItem("vero.dashboard.rpcNodes", "{not json");
    localStorage.setItem("vero.dashboard.activeRpcId", "rpc-x");
    expect(
      getActiveRpcHostUrls({ fallback: "https://default.example.com" })
    ).toEqual(["https://default.example.com"]);
  });

  it("ignores an orphan activeId and returns configured URLs plus fallback", () => {
    localStorage.setItem(
      "vero.dashboard.rpcNodes",
      JSON.stringify([
        { id: "rpc-a", label: "A", url: "https://a.example.com", latencyMs: null, status: "unknown" },
        { id: "rpc-b", label: "B", url: "https://b.example.com", latencyMs: null, status: "unknown" },
      ])
    );
    localStorage.setItem("vero.dashboard.activeRpcId", "rpc-ghost");
    expect(
      getActiveRpcHostUrls({ fallback: "https://default.example.com" })
    ).toEqual([
      "https://a.example.com",
      "https://b.example.com",
      "https://default.example.com",
    ]);
  });
});
