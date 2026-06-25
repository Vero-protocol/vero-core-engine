import { GasOracle } from "../gas-oracle";
import { resolveFeePlan } from "../tx-sender";
import { RpcClient } from "../rpc-client";

// Jest globals (ts-jest environment) should provide these at runtime.


function makeRpc(baseFee: number): RpcClient {
  const rpc = new RpcClient(["http://test"]);
  rpc.call = async (fn: any) => {
    return fn({
      getFeeStats: async () => ({ base_fee: baseFee }),
    });
  };
  return rpc;
}

describe("resolveFeePlan", () => {
  it("returns maxFee as string and includes resolution details", async () => {
    const rpc = makeRpc(100);
    const gasOracle = new GasOracle();

    const plan = await resolveFeePlan(rpc, gasOracle, { multiplier: 1.5, safetyStroops: 10 });
    // ceil(100*1.5 + 10) = ceil(160) = 160
    expect(plan.maxFee).toBe("160");
    expect(plan.resolution.baseFee).toBe(100);
    expect(plan.resolution.multiplier).toBe(1.5);
    expect(plan.resolution.safetyStroops).toBe(10);
    expect(plan.resolution.maxFee).toBe(160);
  });
});

