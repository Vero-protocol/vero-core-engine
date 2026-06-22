/**
 * event-propagator.ts — Real-time Soroban event streaming to guardian dashboard.
 *
 * Subscribes to contract events via Soroban RPC long-poll, normalises them
 * into `EngineEvent` payloads, and queues them for downstream handlers.
 * Persistent event queue prevents data loss during traffic spikes.
 * Automatic cursor persistence enables replay-from-last-known on restart.
 */

import { RpcClient } from "./rpc-client";
import { EventQueue } from "./event-queue";
import { logger } from "./logger";

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
const PROCESS_INTERVAL_MS = 1_000;

export class EventPropagator {
  private readonly handlers: EventHandler[] = [];
  private readonly queue: EventQueue;
  private cursor: string | undefined;
  private running = false;
  private pollTimer: ReturnType<typeof setTimeout> | null = null;
  private processTimer: ReturnType<typeof setTimeout> | null = null;

  constructor(
    private readonly rpc:        RpcClient,
    private readonly contractId: string,
    cursorOverride?: string,
    queuePath?: string,
  ) {
    this.cursor = cursorOverride;
    this.queue = new EventQueue(queuePath);
  }

  /** Register a downstream handler (dashboard websocket, DB writer, etc.). */
  onEvent(handler: EventHandler): void {
    this.handlers.push(handler);
  }

  start(): void {
    if (this.running) return;
    this.running = true;
    this.poll();
    this.processQueue();
  }

  stop(): void {
    this.running = false;
    if (this.pollTimer) clearTimeout(this.pollTimer);
    if (this.processTimer) clearTimeout(this.processTimer);
  }

  /**
   * Poll Soroban RPC for new events and enqueue them.
   */
  private poll(): void {
    this.fetchAndEnqueue()
      .catch(err => logger.error("[EventPropagator] poll error:", err))
      .finally(() => {
        if (this.running) {
          this.pollTimer = setTimeout(() => this.poll(), POLL_INTERVAL_MS);
        }
      });
  }

  /**
   * Process queued events by passing to handlers.
   */
  private processQueue(): void {
    this.handleQueuedEvents()
      .catch(err => logger.error("[EventPropagator] process queue error:", err))
      .finally(() => {
        if (this.running) {
          this.processTimer = setTimeout(() => this.processQueue(), PROCESS_INTERVAL_MS);
        }
      });
  }

  private async fetchAndEnqueue(): Promise<void> {
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

      // Enqueue event for processing by handlers
      const enqueued = this.queue.enqueue(event);
      if (!enqueued) {
        logger.warn("[EventPropagator] Failed to enqueue event:", { eventId: event.id });
      }

      // Update cursor after successful enqueue (not after handler processing)
      this.cursor = raw.id;
    }
  }

  /**
   * Process all queued events, calling registered handlers.
   * Events transition: pending → processing → processed/failed
   */
  private async handleQueuedEvents(): Promise<void> {
    while (true) {
      const queued = this.queue.dequeue();
      if (!queued) break;

      try {
        // Call all handlers in parallel, with error isolation
        const results = await Promise.allSettled(
          this.handlers.map(h => h(queued.eventData))
        );

        // Check if any handler failed
        const failed = results.some(r => r.status === "rejected");
        if (failed) {
          const error = results.find(r => r.status === "rejected") as PromiseRejectedResult;
          throw error.reason;
        }

        this.queue.markProcessed(queued.id);
      } catch (err) {
        const error = err instanceof Error ? err : new Error(String(err));
        logger.error(`[EventPropagator] Handler error for event ${queued.id}:`, error);
        this.queue.markFailed(queued.id, error);
      }
    }
  }

  /** Get queue statistics. */
  getQueueStats() {
    return this.queue.getStats();
  }

  /** Get current cursor — persist this to resume after restart. */
  getCursor(): string | undefined {
    return this.cursor;
  }

  /** Check if the event propagator is currently running. */
  isRunning(): boolean {
    return this.running;
  }

  /** Recovery: process any pending events from previous run. */
  async recoverPendingEvents(): Promise<void> {
    const pending = this.queue.recoverPending();
    logger.info(`[EventPropagator] Recovering ${pending.length} pending events`);

    for (const queued of pending) {
      try {
        const results = await Promise.allSettled(
          this.handlers.map(h => h(queued.eventData))
        );

        const failed = results.some(r => r.status === "rejected");
        if (failed) {
          const error = results.find(r => r.status === "rejected") as PromiseRejectedResult;
          throw error.reason;
        }

        this.queue.markProcessed(queued.id);
      } catch (err) {
        const error = err instanceof Error ? err : new Error(String(err));
        logger.error(`[EventPropagator] Recovery handler error for event ${queued.id}:`, error);
        this.queue.markFailed(queued.id, error);
      }
    }
  }

  /** Close queue database. */
  close(): void {
    this.stop();
    this.queue.close();
  }
}
