import { ChainStateCache } from "../chain-state-cache";
import { RpcClient } from "../rpc-client";

function makeRpc(): RpcClient {
  const rpc = new RpcClient(["http://test"]);
  rpc.call = async (fn: any) => {
    return fn({});
  };
  return rpc;
}

describe("ChainStateCache", () => {
  describe("bounded cache (maxEntries)", () => {
    it("caps cache size at maxEntries when more distinct keys are inserted", async () => {
      const rpc = makeRpc();
      const cache = new ChainStateCache(rpc, 2000, 3);
      const fetcher = async () => ({ value: Math.random() });

      await cache.getSwr("a", fetcher);
      await cache.getSwr("b", fetcher);
      await cache.getSwr("c", fetcher);
      await cache.getSwr("d", fetcher);

      expect((cache as any).cache.size).toBe(3);
    });

    it("evicts the least recently used entry", async () => {
      const rpc = makeRpc();
      const cache = new ChainStateCache(rpc, 2000, 2);
      const fetcher = async () => ({ value: Math.random() });

      await cache.getSwr("a", fetcher);
      await cache.getSwr("b", fetcher);

      await cache.getSwr("a", fetcher);

      await cache.getSwr("c", fetcher);

      expect((cache as any).cache.has("a")).toBe(true);
      expect((cache as any).cache.has("b")).toBe(false);
      expect((cache as any).cache.has("c")).toBe(true);
    });

    it("does not evict when under maxEntries", async () => {
      const rpc = makeRpc();
      const cache = new ChainStateCache(rpc, 2000, 10);
      const fetcher = async () => ({ value: Math.random() });

      await cache.getSwr("a", fetcher);
      await cache.getSwr("b", fetcher);
      await cache.getSwr("c", fetcher);

      expect((cache as any).cache.size).toBe(3);
    });
  });

  describe("SWR behavior", () => {
    it("returns cached data on subsequent calls with same key", async () => {
      const rpc = makeRpc();
      const cache = new ChainStateCache(rpc, 5000, 10);
      let callCount = 0;
      const fetcher = async () => {
        callCount++;
        return { value: 42 };
      };

      const result1 = await cache.getSwr("x", fetcher);
      expect(result1.value).toBe(42);
      expect(callCount).toBe(1);

      const result2 = await cache.getSwr("x", fetcher);
      expect(result2.value).toBe(42);
      expect(callCount).toBe(1);
    });

    it("serves stale data and revalidates in background", async () => {
      const rpc = makeRpc();
      const cache = new ChainStateCache(rpc, -1, 10);
      let callCount = 0;
      const fetcher = async () => {
        await new Promise(r => setTimeout(r, 10));
        callCount++;
        return { value: callCount };
      };

      await cache.getSwr("y", fetcher);
      expect(callCount).toBe(1);

      await cache.getSwr("y", fetcher);
      expect(callCount).toBe(1);

      await new Promise(r => setTimeout(r, 50));

      expect(callCount).toBe(2);
    });
  });

  describe("invalidate / clear", () => {
    it("invalidate removes a specific key", async () => {
      const rpc = makeRpc();
      const cache = new ChainStateCache(rpc, 2000, 10);
      const fetcher = async () => ({ value: 1 });

      await cache.getSwr("a", fetcher);
      expect((cache as any).cache.has("a")).toBe(true);

      cache.invalidate("a");
      expect((cache as any).cache.has("a")).toBe(false);
    });

    it("clear removes all entries", async () => {
      const rpc = makeRpc();
      const cache = new ChainStateCache(rpc, 2000, 10);
      const fetcher = async () => ({ value: 1 });

      await cache.getSwr("a", fetcher);
      await cache.getSwr("b", fetcher);
      expect((cache as any).cache.size).toBe(2);

      cache.clear();
      expect((cache as any).cache.size).toBe(0);
    });
  });

  describe("in-flight deduplication (AC-1, AC-2, AC-3)", () => {
    it("AC-1 + AC-3: N concurrent getSwr() calls for the same missing key result in exactly one fetcher invocation", async () => {
      const rpc = makeRpc();
      const cache = new ChainStateCache(rpc, 5000, 10);
      let calls = 0;
      const fetcher = async () => {
        calls++;
        await new Promise(r => setTimeout(r, 20));
        return { v: calls };
      };

      await Promise.all([
        cache.getSwr("k", fetcher),
        cache.getSwr("k", fetcher),
        cache.getSwr("k", fetcher),
      ]);

      expect(calls).toBe(1);
    });

    it("AC-2: all concurrent callers receive the correct data from the shared fetch", async () => {
      const rpc = makeRpc();
      const cache = new ChainStateCache(rpc, 5000, 10);
      const fetcher = async () => {
        await new Promise(r => setTimeout(r, 20));
        return { value: 99 };
      };

      const results = await Promise.all([
        cache.getSwr("m", fetcher),
        cache.getSwr("m", fetcher),
        cache.getSwr("m", fetcher),
      ]);

      for (const result of results) {
        expect(result.value).toBe(99);
      }
    });

    it("propagates fetch errors to all concurrent callers", async () => {
      const rpc = makeRpc();
      const cache = new ChainStateCache(rpc, 5000, 10);
      const fetchError = new Error("rpc unavailable");
      const fetcher = async (): Promise<{ v: number }> => {
        await new Promise(r => setTimeout(r, 10));
        throw fetchError;
      };

      const results = await Promise.allSettled([
        cache.getSwr("e", fetcher),
        cache.getSwr("e", fetcher),
      ]);

      for (const result of results) {
        expect(result.status).toBe("rejected");
        if (result.status === "rejected") {
          expect(result.reason).toBe(fetchError);
        }
      }

      // After failure, the inflight entry must be cleared so a fresh attempt succeeds.
      let calls = 0;
      const recoveryFetcher = async () => { calls++; return { v: 1 }; };
      const recovered = await cache.getSwr("e", recoveryFetcher);
      expect(recovered.v).toBe(1);
      expect(calls).toBe(1);
    });
  });
});
