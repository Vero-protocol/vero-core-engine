/**
 * event-propagator.ts — Real-time Soroban event streaming to guardian dashboard.
 *
 * Subscribes to contract events via Soroban RPC long-poll, normalises them
 * into `EngineEvent` payloads, and fans them out to registered handlers.
 * Automatic cursor persistence enables replay-from-last-known on restart.
 */

import { RpcClient } from "./rpc-client";

export interface EngineEvent {
  id:          string;
  contractId:  string;
  topic:       string[];
  value:       unknown;
  ledger:      number;
  timestamp:   string;
}

type EventHandler = (event: EngineEvent) => void | Promise<void>;

const POLL_INTERVAL_MS = 5_000;

export class EventPropagator {
  private readonly handlers: EventHandler[] = [];
  private cursor: string | undefined;
  private running = false;
  private timer: ReturnType<typeof setTimeout> | null = null;

  constructor(
    private readonly rpc:        RpcClient,
    private readonly contractId: string,
    cursorOverride?: string,
  ) {
    this.cursor = cursorOverride;
  }

  /** Register a downstream handler (dashboard websocket, DB writer, etc.). */
  onEvent(handler: EventHandler): void {
    this.handlers.push(handler);
  }

  start(): void {
    if (this.running) return;
    this.running = true;
    this.poll();
  }

  stop(): void {
    this.running = false;
    if (this.timer) clearTimeout(this.timer);
  }

  private poll(): void {
    this.fetchAndEmit()
      .catch(err => console.error("[EventPropagator] poll error:", err))
      .finally(() => {
        if (this.running) {
          this.timer = setTimeout(() => this.poll(), POLL_INTERVAL_MS);
        }
      });
  }

  private async fetchAndEmit(): Promise<void> {
    const result = await this.rpc.call(server =>
      server.getEvents({
        startLedger: this.cursor ? undefined : 0,
        cursor:      this.cursor,
        filters: [{
          type:        "contract",
          contractIds: [this.contractId],
        }],
        limit: 100,
      })
    );

    for (const raw of result.events) {
      const event: EngineEvent = {
        id:         raw.id,
        contractId: raw.contractId?.contractId() ?? this.contractId,
        topic:      raw.topic.map(t => t.toXDR("base64")),
        value:      raw.value?.toXDR("base64") ?? null,
        ledger:     raw.ledger,
        timestamp:  raw.ledgerClosedAt,
      };
      await Promise.allSettled(this.handlers.map(h => h(event)));
      this.cursor = raw.id; // advance cursor after successful emit
    }
  }

  /** Current cursor — persist this to resume after restart. */
  getCursor(): string | undefined {
    return this.cursor;
  }
}
