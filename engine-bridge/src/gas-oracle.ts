import { RpcClient } from "./rpc-client";

/**
 * gas-oracle.ts — Dynamic fee/base-fee resolution for tx submission.
 *
 * Notes:
 * - This bridge currently focuses on resolving base fee + computing a
 *   deterministic max-fee for callers.
 * - “Fail closed” is used: if base fee cannot be fetched, fee resolution
 *   throws (no silent fallback).
 */

export interface BaseFeeStats {
  /** Stellar base fee (stroops per operation) */
  baseFee: number;
}

export interface FeeResolutionOptions {
  /**
   * Multiplier applied to baseFee.
   * Example: 2.0 means maxFee = ceil(baseFee * multiplier).
   */
  multiplier: number;

  /**
   * Additional safety stroops added after multiplier.
   * Example: +100 stroops.
   */
  safetyStroops?: number;

  /**
   * Cache TTL for baseFee lookups.
   * Short TTL improves “estimates accurate” during submission bursts.
   */
  cacheTtlMs?: number;
}

export interface ResolvedFee {
  baseFee: number;
  multiplier: number;
  safetyStroops: number;
  /** final computed fee in stroops */
  maxFee: number;
}

type Cached<T> = { value: T; expiresAt: number };

const DEFAULTS: Required<Pick<FeeResolutionOptions, "multiplier" | "safetyStroops" | "cacheTtlMs">> = {
  multiplier: 1.2,
  safetyStroops: 0,
  cacheTtlMs: 5_000,
};

export class GasOracle {
  private baseFeeCache?: Cached<BaseFeeStats>;

  /**
   * Fetch current base fee from the network.
   *
   * Fail-closed: throws if the value cannot be fetched/parsed.
   */
  async fetchBaseFee(rpc: RpcClient): Promise<BaseFeeStats> {
    const now = Date.now();
    if (this.baseFeeCache && this.baseFeeCache.expiresAt > now) {
      return this.baseFeeCache.value;
    }

    const ttl = DEFAULTS.cacheTtlMs;

    const stats = await rpc.call(async (server: any) => {
      // Horizon/Soroban endpoint exposed via stellar-sdk.
      // We intentionally use the most common call: getFeeStats.
      // If stellar-sdk changes, this will fail loudly.
      const res = await server.getFeeStats();
      // res.base_fee in stroops per operation
      const baseFee = Number(res.base_fee ?? res.baseFee);
      if (!Number.isFinite(baseFee) || baseFee <= 0) {
        throw new Error(`GasOracle: invalid base fee received: ${JSON.stringify(res)}`);
      }
      return { baseFee };
    });

    this.baseFeeCache = { value: stats, expiresAt: now + ttl };
    return stats;
  }

  /**
   * Deterministically compute max-fee based on resolved base fee.
   */
  estimateFee(stats: BaseFeeStats, opts: FeeResolutionOptions): ResolvedFee {
    const multiplier = opts.multiplier ?? DEFAULTS.multiplier;
    const safetyStroops = opts.safetyStroops ?? DEFAULTS.safetyStroops;

    // Deterministic + integer output (stroops must be integer)
    const maxFee = Math.ceil(stats.baseFee * multiplier + safetyStroops);

    return {
      baseFee: stats.baseFee,
      multiplier,
      safetyStroops,
      maxFee,
    };
  }

  /**
   * Convenience: fetch base fee (with caching) and compute maxFee.
   */
  async resolveFee(rpc: RpcClient, opts: FeeResolutionOptions): Promise<ResolvedFee> {
    const cacheTtlMs = opts.cacheTtlMs ?? DEFAULTS.cacheTtlMs;

    // For now, caching is implemented with a GasOracle-level TTL.
    // If caller provides a different TTL, we bypass the cache to ensure
    // the estimate uses fresh base fee.
    if (cacheTtlMs !== DEFAULTS.cacheTtlMs) {
      this.baseFeeCache = undefined;
    }

    const stats = await this.fetchBaseFee(rpc);
    return this.estimateFee(stats, opts);
  }
}


