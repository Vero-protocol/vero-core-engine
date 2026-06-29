/**
 * heartbeat-monitor.ts — Periodic health reporting for the engine-bridge.
 *
 * Performs a lightweight "pulse check" against the RPC cluster, monitors
 * the EventPropagator state, and logs system resource usage.
 * Can route DEGRADED alerts to an AlertChannelService for external
 * notification (email, webhook, Slack, PagerDuty, etc.).
 */

import { RpcClient } from "./rpc-client";
import { EventPropagator } from "./event-propagator";
import type { AlertChannelService, Alert } from "./alert-channel";

export interface HeartbeatOptions {
  /** Interval between heartbeat logs in ms. Defaults to 60,000 (1 min). */
  intervalMs?: number;
  /** Optional alert channel service for DEGRADED notifications. */
  alertService?: AlertChannelService;
}

const DEFAULT_INTERVAL = 60_000;

export class HeartbeatMonitor {
  private timer: ReturnType<typeof setInterval> | null = null;

  constructor(
    private readonly rpc:        RpcClient,
    private readonly propagator: EventPropagator,
    private readonly options:    HeartbeatOptions = {},
  ) {}

  /** Start the periodic heartbeat timer. */
  start(): void {
    if (this.timer) return;
    const interval = this.options.intervalMs ?? DEFAULT_INTERVAL;
    this.timer = setInterval(() => {
      this.pulse().catch(err => console.error("[Heartbeat] Pulse failed:", err));
    }, interval);
    // Execute first pulse immediately
    this.pulse().catch(err => console.error("[Heartbeat] Initial pulse failed:", err));
  }

  /** Stop the periodic heartbeat timer. */
  stop(): void {
    if (this.timer) {
      clearInterval(this.timer);
      this.timer = null;
    }
  }

  private async pulse(): Promise<void> {
    let status: "HEALTHY" | "DEGRADED" = "HEALTHY";
    let rpcError: string | undefined;

    try {
      // Lightweight RPC check: fetch latest ledger
      await this.rpc.call(server => server.getLatestLedger());
    } catch (err) {
      status = "DEGRADED";
      rpcError = (err as Error).message;
    }

    const report = {
      timestamp:  new Date().toISOString(),
      status,
      rpc: {
        liveNodes: this.rpc.liveCount(),
        error:     rpcError,
      },
      eventPropagator: {
        running: this.propagator.isRunning(),
        cursor:  this.propagator.getCursor() ?? "none",
      },
      system: {
        memoryRssMb: Math.round(process.memoryUsage().rss / 1024 / 1024),
        uptimeSec:   Math.round(process.uptime()),
      },
    };

    if (status === "HEALTHY") {
      console.log("[Heartbeat] Pulse check:", JSON.stringify(report));
    } else {
      console.warn("[Heartbeat] Pulse check DEGRADED:", JSON.stringify(report));

      this.options.alertService?.send({
        id: `heartbeat-${Date.now()}`,
        severity: "CRITICAL",
        title: "Engine bridge DEGRADED",
        message: rpcError ?? "RPC cluster unreachable",
        timestamp: report.timestamp,
        metadata: report as unknown as Record<string, unknown>,
      });
    }
  }
}
