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
});
