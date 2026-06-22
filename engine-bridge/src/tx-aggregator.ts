import {
  Account,
  Keypair,
  Memo,
  Operation,
  Transaction,
  TransactionBuilder,
  SorobanRpc,
} from "@stellar/stellar-sdk";
import { GasOracle, FeeResolutionOptions, ResolvedFee } from "./gas-oracle";
import { NonceManager } from "./nonce-manager";
import { RpcClient } from "./rpc-client";

export interface BatchFeePlan {
  opCount: number;
  perOperationMaxFee: number;
  totalMaxFee: string;
  unbatchedTotalMaxFee: string;
  savedStroops: string;
  resolution: ResolvedFee;
}

export interface BuildBatchTransactionOptions {
  sourceAccountId: string;
  operations: Operation[];
  signers: Keypair[];
  feeOptions: FeeResolutionOptions;
  memo?: Memo;
  timeoutSeconds?: number;
}

export interface BuiltBatchTransaction {
  transaction: Transaction;
  xdr: string;
  sequence: bigint;
  feePlan: BatchFeePlan;
}

/**
 * Combines multiple operations into one signed envelope so callers can
 * amortize signature and safety-fee overhead across the batch.
 */
export class TxAggregator {
  constructor(
    private readonly rpc: RpcClient,
    private readonly nonceManager: NonceManager,
    private readonly gasOracle: GasOracle,
    private readonly networkPassphrase: string,
  ) {}

  async plan(operations: Operation[], opts: FeeResolutionOptions): Promise<BatchFeePlan> {
    if (operations.length === 0) {
      throw new Error("TxAggregator: at least one operation is required");
    }

    const resolution = await this.gasOracle.resolveFee(this.rpc, opts);
    const perOperationMaxFee = Math.ceil(resolution.baseFee * resolution.multiplier);
    const totalMaxFee = perOperationMaxFee * operations.length + resolution.safetyStroops;
    const unbatchedTotalMaxFee = (perOperationMaxFee + resolution.safetyStroops) * operations.length;

    return {
      opCount: operations.length,
      perOperationMaxFee,
      totalMaxFee: String(totalMaxFee),
      unbatchedTotalMaxFee: String(unbatchedTotalMaxFee),
      savedStroops: String(unbatchedTotalMaxFee - totalMaxFee),
      resolution,
    };
  }

  async build(options: BuildBatchTransactionOptions): Promise<BuiltBatchTransaction> {
    const {
      sourceAccountId,
      operations,
      signers,
      feeOptions,
      memo,
      timeoutSeconds = 300,
    } = options;

    if (signers.length === 0) {
      throw new Error("TxAggregator: at least one signer is required");
    }

    const sequence = await this.nonceManager.reserve(sourceAccountId);

    try {
      const feePlan = await this.plan(operations, feeOptions);
      const source = new Account(sourceAccountId, String(sequence - 1n));
      const baseFeePerOp = Math.ceil(Number(feePlan.totalMaxFee) / operations.length);
      const builder = new TransactionBuilder(source, {
        fee: String(baseFeePerOp),
        networkPassphrase: this.networkPassphrase,
      });

      for (const operation of operations) {
        builder.addOperation(operation as any);
      }

      if (memo) {
        builder.addMemo(memo);
      }

      const transaction = builder.setTimeout(timeoutSeconds).build();
      transaction.sign(...signers);

      return {
        transaction,
        xdr: transaction.toEnvelope().toXDR("base64").toString(),
        sequence,
        feePlan,
      };
    } catch (error) {
      this.nonceManager.release(sourceAccountId, sequence);
      throw error;
    }
  }

  /**
   * Submits a transaction to the network and polls for its status until it is
   * successful, failed, or a timeout is reached.
   */
  async execute(
    transaction: Transaction,
    opts: { pollIntervalMs?: number; timeoutMs?: number } = {},
  ): Promise<SorobanRpc.Api.GetTransactionResponse> {
    const pollIntervalMs = opts.pollIntervalMs ?? 1000;
    const timeoutMs = opts.timeoutMs ?? 30000;
    const startTime = Date.now();
    const hash = transaction.hash().toString("hex");

    const sendResponse = await this.rpc.call(async (server) => {
      return await server.sendTransaction(transaction);
    });

    if (sendResponse.status === "ERROR") {
      throw new Error(`TxAggregator: sendTransaction failed with status ERROR: ${(sendResponse as any).errorResultXdr || "No error result XDR"}`);
    }

    while (true) {
      if (Date.now() - startTime > timeoutMs) {
        throw new Error(`TxAggregator: transaction execution timed out after ${timeoutMs}ms`);
      }

      const txResponse = await this.rpc.call(async (server) => {
        return await server.getTransaction(hash);
      });

      if (txResponse.status === "SUCCESS") {
        return txResponse;
      } else if (txResponse.status === "FAILED") {
        throw new Error(`TxAggregator: transaction failed with result: ${txResponse.resultXdr?.toString() || "Unknown error"}`);
      }

      await new Promise((resolve) => setTimeout(resolve, pollIntervalMs));
    }
  }
}
