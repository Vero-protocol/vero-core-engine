/**
 * rpcHealth.ts — Connectivity probe for RPC endpoints.
 *
 * The dashboard is browser-only so we hit endpoints with a HEAD request and
 * fall back to GET when the server doesn't advertise allow-HEAD. The probe
 * is kept short (2s timeout) so the UI never freezes while the user is
 * waiting on slow nodes.
 *
 * The function is exposed as a pure helper so tests can stub `fetch`
 * without having to spin up a hook.
 */

export interface ProbeResult {
  status: "healthy" | "unreachable";
  latencyMs: number;
  message?: string;
}

const PROBE_TIMEOUT_MS = 2_000;

/**
 * Probe `url` with a HEAD then GET fallback. Returns latency and status.
 * Any thrown error or non-2xx response is treated as `unreachable`.
 */
export async function probeEndpoint(url: string): Promise<ProbeResult> {
  if (!isValidHttpUrl(url)) {
    return {
      status: "unreachable",
      latencyMs: 0,
      message: "Invalid URL",
    };
  }

  const start = performance.now();
  try {
    // Try HEAD first; many RPC servers return 200/204 quickly.
    const res = await fetchWithTimeout(url, { method: "HEAD" }, PROBE_TIMEOUT_MS);
    if (res.ok) {
      return { status: "healthy", latencyMs: Math.round(performance.now() - start) };
    }
    // Some endpoints refuse HEAD; fall through to GET.
    const fallback = await fetchWithTimeout(url, { method: "GET" }, PROBE_TIMEOUT_MS);
    if (fallback.ok) {
      return { status: "healthy", latencyMs: Math.round(performance.now() - start) };
    }
    return {
      status: "unreachable",
      latencyMs: Math.round(performance.now() - start),
      message: `HTTP ${fallback.status}`,
    };
  } catch (err) {
    return {
      status: "unreachable",
      latencyMs: Math.round(performance.now() - start),
      message: (err as Error).message || "Network error",
    };
  }
}

async function fetchWithTimeout(
  url: string,
  init: RequestInit,
  timeoutMs: number
): Promise<Response> {
  const controller = new AbortController();
  // Use the globals directly (no `window.` prefix) so vitest's fake timer
  // swap and node-style environments both work without surprises.
  const timeoutId = setTimeout(() => controller.abort(), timeoutMs);
  try {
    return await fetch(url, { ...init, signal: controller.signal });
  } finally {
    clearTimeout(timeoutId);
  }
}

export function isValidHttpUrl(input: string): boolean {
  if (!input) return false;
  try {
    const parsed = new URL(input);
    return parsed.protocol === "http:" || parsed.protocol === "https:";
  } catch {
    return false;
  }
}
