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
        ep.deadUntil = Date.now() + QUARANTINE_MS;
        logger.warn(`[RpcClient] ${ep.url} quarantined — ${(err as Error).message}`);
      }
    }

    throw lastError;
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
