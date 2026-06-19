/**
 * Shared dashboard types.
 *
 * These mirror the shape surfaced by the engine-core (`audit.rs`) and
 * engine-bridge (`rpc-client.ts`, `gas-oracle.ts`) modules, so the
 * dashboard renders a faithful picture of the on-chain state.
 */

export type RpcHealthStatus = "unknown" | "checking" | "healthy" | "unreachable";

export interface RpcNode {
  /** Stable id for React keys and persistence. */
  id: string;
  /** Human-readable label (e.g. "SDF Testnet"). */
  label: string;
  /** HTTPS RPC endpoint. */
  url: string;
  /** Last measured latency in milliseconds. */
  latencyMs: number | null;
  /** Most recent probe status. */
  status: RpcHealthStatus;
  /** Last status message from the last probe. */
  message?: string;
  /** ISO timestamp of the most recent probe. */
  lastChecked?: string;
}

export type AuditEventType = "commit" | "replay" | "mismatch" | "snapshot";

export type AuditStatus = "success" | "rejected" | "pending";

export interface AuditEntry {
  id: string;
  /** Epoch milliseconds when the event was emitted. */
  timestamp: number;
  /** Monotonic sequence derived from the audit layer. */
  sequence: number;
  /** Ledger the event was anchored to. */
  ledger: number;
  /** Stellar address of the author/prover. */
  author: string;
  /** Hex-encoded 32-byte state commitment hash. */
  stateHash: string;
  eventType: AuditEventType;
  status: AuditStatus;
  /** Payload size in bytes. */
  payloadBytes: number;
  /** Fee charged for the underlying transaction (stroops). */
  feeStroops: number;
}

export interface PerformanceSample {
  /** Epoch ms for this sample. */
  timestamp: number;
  /** Base fee in stroops per operation. */
  baseFee: number;
  /** Computed max fee (base fee * multiplier, rounded up). */
  maxFee: number;
  /** Transactions completed in the current sliding window. */
  txCount: number;
  /** TPS derived from txCount/windowSec. */
  tps: number;
}
