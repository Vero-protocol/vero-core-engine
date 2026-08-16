import { RpcClient } from "./rpc-client";

export interface SwrOptions {
  staleTimeMs?: number;
}

interface CacheItem<T> {
  data: T;
  updatedAt: number;
  isRevalidating: boolean;
}

/**
 * chain-state-cache.ts — Stale-While-Revalidate (SWR) caching for chain state.
 *
 * Implements SWR caching to solve slow load times when fetching chain state.
 * Returns stale data immediately while fetching fresh data in the background.
 * Cache is bounded by maxEntries via LRU eviction to prevent unbounded growth.
 */
export class ChainStateCache {
  private cache = new Map<string, CacheItem<any>>();
  /** Tracks one in-flight fetch Promise per key to deduplicate concurrent cache-miss callers. */
  private readonly inflight = new Map<string, Promise<any>>();

  constructor(
    private readonly rpc: RpcClient,
    private readonly defaultStaleTimeMs: number = 2000,
    private readonly maxEntries: number = Infinity
  ) {}

  private touch(key: string): void {
    if (this.cache.has(key)) {
      const item = this.cache.get(key)!;
      this.cache.delete(key);
      this.cache.set(key, item);
    }
  }

  private evictIfNeeded(): void {
    while (this.cache.size > this.maxEntries) {
      const lruKey = this.cache.keys().next().value;
      if (lruKey !== undefined) {
        this.cache.delete(lruKey);
      }
    }
  }

  /**
   * Fetches data using SWR strategy.
   *
   * Cache-miss behaviour: if another caller is already fetching the same key,
   * the new caller awaits the same in-flight Promise instead of issuing its own
   * RPC request. This prevents the "thundering herd" problem on concurrent
   * misses for hot keys.
   *
   * @param key Unique cache key
   * @param fetcher Async function to fetch fresh data
   * @param options SWR options
   */
  async getSwr<T>(
    key: string,
    fetcher: (rpc: RpcClient) => Promise<T>,
    options?: SwrOptions
  ): Promise<T> {
    const staleTimeMs = options?.staleTimeMs ?? this.defaultStaleTimeMs;
    const now = Date.now();
    const item = this.cache.get(key) as CacheItem<T> | undefined;

    if (item) {
      this.touch(key);
      const isStale = now - item.updatedAt > staleTimeMs;

      if (isStale && !item.isRevalidating) {
        item.isRevalidating = true;
        // Background revalidation (fire and forget)
        this.revalidate(key, fetcher).catch(err => {
          console.error(`[ChainStateCache] SWR revalidation failed for ${key}:`, err);
        });
      }
      return item.data;
    }

    // Cache miss — deduplicate concurrent fetches for the same key.
    const existing = this.inflight.get(key) as Promise<T> | undefined;
    if (existing) {
      return existing;
    }

    const fetchPromise: Promise<T> = fetcher(this.rpc).then(
      data => {
        this.cache.set(key, { data, updatedAt: Date.now(), isRevalidating: false });
        this.evictIfNeeded();
        this.inflight.delete(key);
        return data;
      },
      err => {
        this.inflight.delete(key);
        throw err;
      }
    );

    this.inflight.set(key, fetchPromise);
    return fetchPromise;
  }

  private async revalidate<T>(
    key: string,
    fetcher: (rpc: RpcClient) => Promise<T>
  ): Promise<void> {
    try {
      const data = await fetcher(this.rpc);
      this.cache.set(key, { data, updatedAt: Date.now(), isRevalidating: false });
    } catch (error) {
      const item = this.cache.get(key);
      if (item) {
        item.isRevalidating = false; // Reset flag so it can try again
      }
      throw error;
    }
  }

  /**
   * Manually invalidate a cache key
   */
  invalidate(key: string): void {
    this.cache.delete(key);
  }

  /** Evict all entries */
  clear(): void {
    this.cache.clear();
  }
}
