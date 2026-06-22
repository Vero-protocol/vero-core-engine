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
        Operation.manageData({ name: "op-1", value: "1" }) as any,
        Operation.manageData({ name: "op-2", value: "2" }) as any,
        Operation.manageData({ name: "op-3", value: "3" }) as any,
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
        Operation.manageData({ name: "batched-1", value: "a" }) as any,
        Operation.manageData({ name: "batched-2", value: "b" }) as any,
      ],
      signers: [signer],
      feeOptions: { multiplier: 2, safetyStroops: 25 },
    });

    expect(batch.sequence).toBe(101n);
    expect(batch.transaction.operations).toHaveLength(2);
    expect(batch.transaction.fee).toBe("426");
    expect(batch.transaction.signatures).toHaveLength(1);
    expect(batch.feePlan.savedStroops).toBe("25");
    expect(typeof batch.xdr).toBe("string");
    expect(batch.xdr.length).toBeGreaterThan(0);
  });

  it("executes and polls a transaction successfully", async () => {
    const rpc = new RpcClient(["http://test"]);
    let pollCount = 0;
    rpc.call = async (fn: any) => {
      return fn({
        getFeeStats: async () => ({ base_fee: 100 }),
        getAccount: async (_: string) => ({ sequenceNumber: () => "100" }),
        sendTransaction: async () => ({ status: "PENDING", hash: "abc" }),
        getTransaction: async () => {
          pollCount++;
          if (pollCount < 2) {
            return { status: "NOT_FOUND" };
          }
          return { status: "SUCCESS", resultXdr: "result" };
        },
      });
    };

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
        Operation.manageData({ name: "batched-1", value: "a" }) as any,
      ],
      signers: [signer],
      feeOptions: { multiplier: 2, safetyStroops: 25 },
    });

    const res = await aggregator.execute(batch.transaction, { pollIntervalMs: 1, timeoutMs: 100 });
    expect(res.status).toBe("SUCCESS");
    expect(pollCount).toBe(2);
  });

  it("throws error if sendTransaction returns ERROR", async () => {
    const rpc = new RpcClient(["http://test"]);
    rpc.call = async (fn: any) => {
      return fn({
        getFeeStats: async () => ({ base_fee: 100 }),
        getAccount: async (_: string) => ({ sequenceNumber: () => "100" }),
        sendTransaction: async () => ({ status: "ERROR", errorResultXdr: "bad-tx" }),
      });
    };

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
        Operation.manageData({ name: "batched-1", value: "a" }) as any,
      ],
      signers: [signer],
      feeOptions: { multiplier: 2, safetyStroops: 25 },
    });

    await expect(aggregator.execute(batch.transaction, { pollIntervalMs: 1, timeoutMs: 100 }))
      .rejects.toThrow("TxAggregator: sendTransaction failed with status ERROR: bad-tx");
  });

  it("throws error if getTransaction returns FAILED", async () => {
    const rpc = new RpcClient(["http://test"]);
    rpc.call = async (fn: any) => {
      return fn({
        getFeeStats: async () => ({ base_fee: 100 }),
        getAccount: async (_: string) => ({ sequenceNumber: () => "100" }),
        sendTransaction: async () => ({ status: "PENDING" }),
        getTransaction: async () => ({ status: "FAILED", resultXdr: "fail-reason" }),
      });
    };

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
        Operation.manageData({ name: "batched-1", value: "a" }) as any,
      ],
      signers: [signer],
      feeOptions: { multiplier: 2, safetyStroops: 25 },
    });

    await expect(aggregator.execute(batch.transaction, { pollIntervalMs: 1, timeoutMs: 100 }))
      .rejects.toThrow("TxAggregator: transaction failed with result: fail-reason");
  });

  it("throws error on timeout", async () => {
    const rpc = new RpcClient(["http://test"]);
    rpc.call = async (fn: any) => {
      return fn({
        getFeeStats: async () => ({ base_fee: 100 }),
        getAccount: async (_: string) => ({ sequenceNumber: () => "100" }),
        sendTransaction: async () => ({ status: "PENDING" }),
        getTransaction: async () => ({ status: "NOT_FOUND" }),
      });
    };

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
        Operation.manageData({ name: "batched-1", value: "a" }) as any,
      ],
      signers: [signer],
      feeOptions: { multiplier: 2, safetyStroops: 25 },
    });

    await expect(aggregator.execute(batch.transaction, { pollIntervalMs: 5, timeoutMs: 20 }))
      .rejects.toThrow("TxAggregator: transaction execution timed out");
  });
});
