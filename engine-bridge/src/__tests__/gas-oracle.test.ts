import { GasOracle } from "../gas-oracle";
import { RpcClient } from "../rpc-client";

// Jest globals (ts-jest environment) should provide these at runtime.
// If your TS config doesn't include Jest types, add "types": ["jest"].


function makeRpc(baseFee: number): RpcClient {
  const rpc = new RpcClient(["http://test"]);
  // Monkey patch RpcClient.call so we can control what the network returns.
  rpc.call = async (fn: any) => {
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

  it("honors a longer caller-supplied cache TTL", async () => {
    jest.useFakeTimers();
    jest.setSystemTime(0);

    const rpc = makeRpc(100);
    let calls = 0;
    const call = rpc.call.bind(rpc);
    rpc.call = async (fn: any) => {
      calls += 1;
      return call(fn);
    };

    const go = new GasOracle();
    await go.resolveFee(rpc, { multiplier: 1.2, cacheTtlMs: 60_000 });
    jest.advanceTimersByTime(6_000);
    await go.resolveFee(rpc, { multiplier: 1.2, cacheTtlMs: 60_000 });

    expect(calls).toBe(1);
    jest.useRealTimers();
  });

  it("expires the cache after the caller-supplied TTL", async () => {
    jest.useFakeTimers();
    jest.setSystemTime(0);

    const rpc = makeRpc(100);
    let calls = 0;
    const call = rpc.call.bind(rpc);
    rpc.call = async (fn: any) => {
      calls += 1;
      return call(fn);
    };

    const go = new GasOracle();
    await go.resolveFee(rpc, { multiplier: 1.2, cacheTtlMs: 100 });
    jest.advanceTimersByTime(200);
    await go.resolveFee(rpc, { multiplier: 1.2, cacheTtlMs: 100 });

    expect(calls).toBe(2);
    jest.useRealTimers();
  });
});

