import { GasOracle } from "../gas-oracle";
import { RpcClient } from "../rpc-client";

// Jest globals (ts-jest environment) should provide these at runtime.
// If your TS config doesn't include Jest types, add "types": ["jest"].

function makeRpc(baseFee: number, onCall?: () => void): RpcClient {
  const rpc = new RpcClient(["http://test"]);
  // Monkey patch RpcClient.call so we can control what the network returns.
  rpc.call = async (fn: any) => {
    if (onCall) onCall();
    return fn({
      getFeeStats: async () => ({ base_fee: baseFee }),
    });
  };
  return rpc;
}

describe("GasOracle", () => {
  it("fetchBaseFee returns parsed base fee", async () => {
    const rpc = makeRpc(100);
    const go = new GasOracle();
    const stats = await go.fetchBaseFee(rpc);
    expect(stats.baseFee).toBe(100);
  });

  it("estimateFee computes deterministic maxFee", () => {
    const go = new GasOracle();
    const res = go.estimateFee({ baseFee: 100 }, { multiplier: 2.0, safetyStroops: 50 });
    // ceil(100*2 + 50) = 250
    expect(res.maxFee).toBe(250);
  });

  it("resolveFee fails closed if base fee fetch throws", async () => {
    const rpc = new RpcClient(["http://test"]);
    rpc.call = async () => {
      throw new Error("network down");
    };

    const go = new GasOracle();
    await expect(go.resolveFee(rpc, { multiplier: 1.2 })).rejects.toThrow("network down");
  });

  it("AC-1: resolveFee with cacheTtlMs: 60000 called twice within 60s hits the network once", async () => {
    let callCount = 0;
    const rpc = makeRpc(100, () => {
      callCount++;
    });

    const go = new GasOracle();
    const res1 = await go.resolveFee(rpc, { multiplier: 1.2, cacheTtlMs: 60_000 });
    const res2 = await go.resolveFee(rpc, { multiplier: 1.2, cacheTtlMs: 60_000 });

    expect(callCount).toBe(1);
    expect(res1.maxFee).toBe(120);
    expect(res2.maxFee).toBe(120);
  });

  it("AC-2: resolveFee with cacheTtlMs: 100 called 200ms apart hits the network twice", async () => {
    let callCount = 0;
    const rpc = makeRpc(100, () => {
      callCount++;
    });

    const go = new GasOracle();
    await go.resolveFee(rpc, { multiplier: 1.2, cacheTtlMs: 100 });
    expect(callCount).toBe(1);

    await new Promise((resolve) => setTimeout(resolve, 200));

    await go.resolveFee(rpc, { multiplier: 1.2, cacheTtlMs: 100 });
    expect(callCount).toBe(2);
  });

  it("AC-3: asserts custom cache duration matches the supplied TTL", async () => {
    let callCount = 0;
    const rpc = makeRpc(100, () => {
      callCount++;
    });

    const go = new GasOracle();
    // Call with 500ms TTL
    await go.resolveFee(rpc, { multiplier: 1.0, cacheTtlMs: 500 });
    expect(callCount).toBe(1);

    // Call after 100ms -> should still be cached
    await new Promise((resolve) => setTimeout(resolve, 100));
    await go.resolveFee(rpc, { multiplier: 1.0, cacheTtlMs: 500 });
    expect(callCount).toBe(1);

    // Call after another 500ms -> should expire and fetch fresh
    await new Promise((resolve) => setTimeout(resolve, 500));
    await go.resolveFee(rpc, { multiplier: 1.0, cacheTtlMs: 500 });
    expect(callCount).toBe(2);
  });
});
