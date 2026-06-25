import { RpcClient } from "./rpc-client";
import { GasOracle, FeeResolutionOptions, ResolvedFee } from "./gas-oracle";

/**
 * tx-sender.ts — Fee resolution entrypoint for transaction submission.
 *
 * The repo currently doesn’t include a full transaction builder/submitter.
 * This module provides the *protocol-grade* fee oracle integration so any
 * future tx construction/submission code can:
 *   1) fetch current base fee (fail closed)
 *   2) compute deterministic maxFee
 *   3) apply/return fee parameters consistently.
 */

export interface FeePlan {
  /** Final fee in stroops that callers should set on their tx. */
  maxFee: string;
  /** Raw resolution details for audit/logging. */
  resolution: ResolvedFee;
}

export async function resolveFeePlan(
  rpc: RpcClient,
  gasOracle: GasOracle,
  opts: FeeResolutionOptions,
): Promise<FeePlan> {
  const resolution = await gasOracle.resolveFee(rpc, opts);
  return {
    // stellar-sdk typically expects fee as string/number depending on tx type.
    // We return string to avoid bigint/float issues.
    maxFee: String(resolution.maxFee),
    resolution,
  };
}

