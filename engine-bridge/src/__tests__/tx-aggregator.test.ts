import { Keypair, Networks, Operation } from "@stellar/stellar-sdk";
import { GasOracle } from "../gas-oracle";
import { NonceManager } from "../nonce-manager";
import { RpcClient } from "../rpc-client";
import { TxAggregator } from "../tx-aggregator";

function makeRpc(baseFee: number): RpcClient {
  const rpc = new RpcClient(["http://test"]);
  rpc.call = async (fn: any) => {
    return fn({
      getFeeStats: async () => ({ base_fee: baseFee }),
      getAccount: async (_: string) => ({ sequenceNumber: () => "100" }),
    });
  };
  return rpc;
}

describe("TxAggregator", () => {
  it("amortizes safety stroops across batched operations", async () => {
    const rpc = makeRpc(100);
    const aggregator = new TxAggregator(
      rpc,
      new NonceManager(rpc),
      new GasOracle(),
      Networks.TESTNET,
    );

    const plan = await aggregator.plan(
      [
        Operation.manageData({ name: "op-1", value: "1" }),
        Operation.manageData({ name: "op-2", value: "2" }),
        Operation.manageData({ name: "op-3", value: "3" }),
      ],
      { multiplier: 1.5, safetyStroops: 10 },
    );

    expect(plan.opCount).toBe(3);
    expect(plan.perOperationMaxFee).toBe(150);
    expect(plan.totalMaxFee).toBe("460");
    expect(plan.unbatchedTotalMaxFee).toBe("480");
    expect(plan.savedStroops).toBe("20");
  });

  it("builds and signs a single transaction for multiple operations", async () => {
    const rpc = makeRpc(100);
    const aggregator = new TxAggregator(
      rpc,
      new NonceManager(rpc),
      new GasOracle(),
      Networks.TESTNET,
    );
    const signer = Keypair.random();

    const batch = await aggregator.build({
      sourceAccountId: signer.publicKey(),
      operations: [
        Operation.manageData({ name: "batched-1", value: "a" }),
        Operation.manageData({ name: "batched-2", value: "b" }),
      ],
      signers: [signer],
      feeOptions: { multiplier: 2, safetyStroops: 25 },
    });

    expect(batch.sequence).toBe(101n);
    expect(batch.transaction.operations).toHaveLength(2);
    expect(batch.transaction.fee).toBe("425");
    expect(batch.transaction.signatures).toHaveLength(1);
    expect(batch.feePlan.savedStroops).toBe("25");
    expect(typeof batch.xdr).toBe("string");
    expect(batch.xdr.length).toBeGreaterThan(0);
  });
});
