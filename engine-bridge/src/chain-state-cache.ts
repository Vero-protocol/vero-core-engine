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
 */
export class ChainStateCache {
  private cache = new Map<string, CacheItem<any>>();

  constructor(
    private readonly rpc: RpcClient,
    private readonly defaultStaleTimeMs: number = 2000
  ) {}

  /**
   * Fetches data using SWR strategy.
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

    // Cache miss, fetch synchronously
    const data = await fetcher(this.rpc);
    this.cache.set(key, { data, updatedAt: Date.now(), isRevalidating: false });
    return data;
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
}
