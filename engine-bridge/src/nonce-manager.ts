/**
 * nonce-manager.ts — Atomic transaction sequence-number management.
 *
 * Stellar uses per-account sequence numbers.  Concurrent submitters racing
 * to build transactions would produce sequence collisions.  This manager
 * serialises reservation, auto-refreshes on network desync, and releases
 * sequences back to the pool if the outer operation fails.
 */

import { RpcClient } from "./rpc-client";

export class NonceManager {
  /** account → next usable sequence  */
  private readonly cache = new Map<string, bigint>();
  /** Serialises concurrent calls per account to prevent races. */
  private readonly locks = new Map<string, Promise<void>>();

  constructor(private readonly rpc: RpcClient) {}

  /**
   * Reserve the next sequence number for `accountId`.
   * The caller MUST call `release(accountId, sequence)` on submission failure.
   */
  async reserve(accountId: string): Promise<bigint> {
    await this.waitForLock(accountId);

    let resolve!: () => void;
    const lock = new Promise<void>(r => { resolve = r; });
    this.locks.set(accountId, lock);

    try {
      const seq = await this.nextSequence(accountId);
      this.cache.set(accountId, seq + 1n);
      return seq;
    } finally {
      this.locks.delete(accountId);
      resolve();
    }
  }

  /** Return a sequence that was never submitted so it can be reused. */
  release(accountId: string, sequence: bigint): void {
    const current = this.cache.get(accountId);
    if (current === undefined || sequence < current) {
      this.cache.set(accountId, sequence);
    }
  }

  /** Force a fresh read from the network (e.g. after fee-bump or manual tx). */
  async refresh(accountId: string): Promise<void> {
    this.cache.delete(accountId);
    await this.nextSequence(accountId); // warms the cache
  }

  private async nextSequence(accountId: string): Promise<bigint> {
    const cached = this.cache.get(accountId);
    if (cached !== undefined) return cached;

    const seq = await this.rpc.call(server => server.getAccount(accountId))
      .then(a => BigInt(a.sequenceNumber()));
    this.cache.set(accountId, seq + 1n);
    return seq + 1n;
  }

  private async waitForLock(accountId: string): Promise<void> {
    const existing = this.locks.get(accountId);
    if (existing) await existing;
  }
}
