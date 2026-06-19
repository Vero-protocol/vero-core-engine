/**
 * rpc-client.ts — RPC node failover with round-robin and health probing.
 *
 * Maintains an ordered list of endpoints. On any network/RPC error the call
 * is retried on the next healthy endpoint. Dead nodes are quarantined for
 * QUARANTINE_MS before re-admission.
 */

import { SorobanRpc } from "@stellar/stellar-sdk";
import { logger } from "./logger";

const QUARANTINE_MS = 30_000;
const MAX_RETRIES   = 3;

interface Endpoint {
  url:          string;
  deadUntil:    number;  // epoch ms; 0 = healthy
  lastError?:    string | null;
}

export class RpcClient {
  private readonly endpoints: Endpoint[];
  private cursor = 0;

  constructor(urls: string[]) {
    if (urls.length === 0) throw new Error("RpcClient: at least one URL required");
    this.endpoints = urls.map(url => ({ url, deadUntil: 0 }));
  }

  /** Execute `fn` with an active SorobanRpc.Server, failing over on error. */
  async call<T>(fn: (server: SorobanRpc.Server) => Promise<T>): Promise<T> {
    let lastError: unknown;

    for (let attempt = 0; attempt < MAX_RETRIES; attempt++) {
      const ep = this.pickEndpoint();
      if (!ep) throw new Error("RpcClient: all endpoints unavailable");

      try {
        const server = new SorobanRpc.Server(ep.url, { allowHttp: ep.url.startsWith("http://") });
        return await fn(server);
      } catch (err) {
        lastError = err;
        const msg = (err as Error).message;
        ep.deadUntil = Date.now() + QUARANTINE_MS;
        ep.lastError = msg;
        logger.warn(`[RpcClient] ${ep.url} quarantined — ${msg}`);
      }
    }

    throw lastError;
  }

  /**
   * Probe a single endpoint immediately. Returns true if the endpoint responds.
   * Useful for external health checks to attempt re-admission of quarantined nodes.
   */
  async probeEndpoint(url: string): Promise<boolean> {
    const ep = this.endpoints.find(e => e.url === url);
    if (!ep) return false;

    try {
      const server = new SorobanRpc.Server(ep.url, { allowHttp: ep.url.startsWith("http://") });
      // Lightweight probe: fetch latest ledger
      await server.getLatestLedger();
      ep.deadUntil = 0;
      ep.lastError = null;
      logger.info(`[RpcClient] ${ep.url} probe success — marked healthy`);
      return true;
    } catch (err) {
      const msg = (err as Error).message;
      ep.deadUntil = Date.now() + QUARANTINE_MS;
      ep.lastError = msg;
      logger.warn(`[RpcClient] ${ep.url} probe failed — ${msg}`);
      return false;
    }
  }

  /** Return a snapshot of endpoint statuses for monitoring. */
  getStatuses(): Array<{ url: string; healthy: boolean; deadUntil: number; lastError: string | null }>
  {
    const now = Date.now();
    return this.endpoints.map(ep => ({ url: ep.url, healthy: ep.deadUntil <= now, deadUntil: ep.deadUntil, lastError: ep.lastError ?? null }));
  }

  private pickEndpoint(): Endpoint | null {
    const now = Date.now();
    for (let i = 0; i < this.endpoints.length; i++) {
      const ep = this.endpoints[(this.cursor + i) % this.endpoints.length];
      if (ep.deadUntil <= now) {
        this.cursor = (this.cursor + i + 1) % this.endpoints.length;
        return ep;
      }
    }
    return null;
  }

  /** Expose live endpoint count (useful for health checks). */
  liveCount(): number {
    const now = Date.now();
    return this.endpoints.filter(ep => ep.deadUntil <= now).length;
  }
}
