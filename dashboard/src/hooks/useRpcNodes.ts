/**
 * useRpcNodes — State hook for the user's RPC endpoint list plus active
 * selection. Persisted to localStorage so a configured node survives
 * reloads. Health probes are kicked off explicitly via `probeAll` rather
 * than on mount, so the UI is responsive and requests don't fire from
 * every renderer.
 */

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { RpcNode } from "../types";
import { probeEndpoint } from "../utils/rpcHealth";

const STORAGE_KEY = "vero.dashboard.rpcNodes";
const ACTIVE_KEY = "vero.dashboard.activeRpcId";

function loadInitial(): { nodes: RpcNode[]; activeId: string | null } {
  if (typeof localStorage === "undefined") {
    return { nodes: [], activeId: null };
  }
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    const nodes: RpcNode[] = raw ? (JSON.parse(raw) as RpcNode[]) : [];
    const activeId = localStorage.getItem(ACTIVE_KEY);
    return {
      nodes,
      activeId: activeId && nodes.some((n) => n.id === activeId) ? activeId : null,
    };
  } catch {
    return { nodes: [], activeId: null };
  }
}

/**
 * Stable id derived from the URL. We deliberately don't pull in a UUID lib
 * because the URL already encodes enough entropy and the result is opaque
 * enough for local use.
 */
function deriveId(url: string): string {
  let hash = 0;
  for (let i = 0; i < url.length; i++) {
    hash = (hash * 31 + url.charCodeAt(i)) | 0;
  }
  return `rpc-${Math.abs(hash).toString(36)}`;
}

export function useRpcNodes() {
  const [{ nodes, activeId }, setState] = useState(loadInitial);
  // keep a ref so async probes can compare against up-to-date list without
  // re-firing effects.
  const nodesRef = useRef(nodes);
  nodesRef.current = nodes;

  // Persist whenever the list or active id changes.
  useEffect(() => {
    if (typeof localStorage === "undefined") return;
    localStorage.setItem(STORAGE_KEY, JSON.stringify(nodes));
  }, [nodes]);

  useEffect(() => {
    if (typeof localStorage === "undefined") return;
    if (activeId) {
      localStorage.setItem(ACTIVE_KEY, activeId);
    } else {
      localStorage.removeItem(ACTIVE_KEY);
    }
  }, [activeId]);

  const updateNode = useCallback((id: string, patch: Partial<RpcNode>) => {
    setState((prev) => ({
      ...prev,
      nodes: prev.nodes.map((n) => (n.id === id ? { ...n, ...patch } : n)),
    }));
  }, []);

  const addNode = useCallback((label: string, url: string): { ok: boolean; reason?: string; id?: string } => {
    const trimmed = normalizeRpcUrl(url.trim());
    if (!trimmed) return { ok: false, reason: "URL is required" };
    try {
      const parsed = new URL(trimmed);
      if (parsed.protocol !== "https:" && parsed.protocol !== "http:") {
        return { ok: false, reason: "Only http(s) URLs are supported" };
      }
    } catch {
      return { ok: false, reason: "Invalid URL format" };
    }
    const id = deriveId(trimmed);
    const exists = nodesRef.current.some((n) => n.id === id);
    if (exists) {
      return { ok: false, reason: "This endpoint is already configured" };
    }
    const next: RpcNode = {
      id,
      label: label.trim() || trimmed,
      url: trimmed,
      latencyMs: null,
      status: "unknown",
    };
    setState((prev) => ({
      nodes: [...prev.nodes, next],
      // Activate the freshly-added node if there was no active selection.
      activeId: prev.activeId ?? id,
    }));
    return { ok: true, id };
  }, []);

  const removeNode = useCallback((id: string) => {
    setState((prev) => {
      const nodes = prev.nodes.filter((n) => n.id !== id);
      const activeId =
        prev.activeId === id ? (nodes[0]?.id ?? null) : prev.activeId;
      return { nodes, activeId };
    });
  }, []);

  const setActive = useCallback((id: string) => {
    setState((prev) =>
      prev.nodes.some((n) => n.id === id) ? { ...prev, activeId: id } : prev
    );
  }, []);

  const probeAll = useCallback(async () => {
    for (const node of nodesRef.current) {
      updateNode(node.id, { status: "checking", message: undefined });
    }
    await Promise.all(
      nodesRef.current.map(async (node) => {
        const res = await probeEndpoint(node.url);
        updateNode(node.id, {
          status: res.status,
          latencyMs: res.latencyMs,
          message: res.message,
          lastChecked: new Date().toISOString(),
        });
      })
    );
  }, [updateNode]);

  const activeNode = useMemo(
    () => nodes.find((n) => n.id === activeId) ?? null,
    [nodes, activeId]
  );

  return {
    nodes,
    activeId,
    activeNode,
    addNode,
    removeNode,
    setActive,
    probeAll,
  };
}

export type { RpcNode };

/**
 * Read the active RPC URL directly from localStorage. This is the
 * bridge-friendly counterpart to {@link useRpcNodes} — a host application
 * (e.g., one that instantiates `engine-bridge`'s `RpcClient`) can call
 * this without mounting the React hook. Returns `null` if no active
 * endpoint is configured.
 */
export function getActiveRpcUrl(): string | null {
  if (typeof localStorage === "undefined") return null;
  const activeId = localStorage.getItem(ACTIVE_KEY);
  if (!activeId) return null;
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    const nodes: RpcNode[] = raw ? (JSON.parse(raw) as RpcNode[]) : [];
    const match = nodes.find((n) => n.id === activeId);
    return match ? match.url : null;
  } catch {
    return null;
  }
}

/** Normalise an RPC URL by stripping a trailing slash, if present. */
export function normalizeRpcUrl(url: string): string {
  return url.replace(/\/+$/, "");
}
