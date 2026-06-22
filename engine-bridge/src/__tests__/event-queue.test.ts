/**
 * event-queue.test.ts — Tests for persistent event queue.
 *
 * Verifies:
 * - Events are queued and persisted to SQLite
 * - Events survive application restart (recovery)
 * - Failed events are retried up to max attempts
 * - Queue statistics are accurate
 */

import { EventQueue, QueuedEvent } from "../event-queue";
import { EngineEvent } from "../event-propagator";
import * as fs from "fs";
import * as path from "path";

let testQueueCounter = 0;
// Helper to create test queue with unique path
function createTestQueue(): { queue: EventQueue; path: string } {
  testQueueCounter++;
  const testPath = path.join(process.cwd(), `test-queue-${Date.now()}-${testQueueCounter}-${Math.random().toString(36).substring(7)}.db`);
  return {
    queue: new EventQueue(testPath),
    path: testPath,
  };
}

// Helper to clean up test queue
function cleanupTestQueue(queue: EventQueue, dbPath: string): void {
  queue.close();
  [dbPath, `${dbPath}-shm`, `${dbPath}-wal`].forEach(p => {
    if (fs.existsSync(p)) {
      fs.unlinkSync(p);
    }
  });
}

// Helper to create test event
function createTestEvent(id: string, topic: string): EngineEvent {
  return {
    id,
    contractId: "CA123456",
    topic: [topic],
    value: { test: "data" },
    ledger: 1000,
    timestamp: new Date().toISOString(),
  };
}

describe("EventQueue", () => {
  describe("Enqueue and Dequeue", () => {
    it("enqueues an event successfully", () => {
      const { queue, path } = createTestQueue();
      const event = createTestEvent("evt-1", "test.event");

      const result = queue.enqueue(event);
      expect(result).toBe(true);

      cleanupTestQueue(queue, path);
    });

    it("dequeues pending event and transitions to processing", () => {
      const { queue, path } = createTestQueue();
      const event = createTestEvent("evt-1", "test.event");

      queue.enqueue(event);
      const dequeued = queue.dequeue();

      expect(dequeued).not.toBeNull();
      expect(dequeued!.id).toBe("evt-1");
      expect(dequeued!.status).toBe("processing");
      expect(dequeued!.attempts).toBe(1);

      cleanupTestQueue(queue, path);
    });

    it("dequeues events in FIFO order", () => {
      const { queue, path } = createTestQueue();

      queue.enqueue(createTestEvent("evt-1", "event.first"));
      queue.enqueue(createTestEvent("evt-2", "event.second"));
      queue.enqueue(createTestEvent("evt-3", "event.third"));

      const first = queue.dequeue();
      const second = queue.dequeue();
      const third = queue.dequeue();
      const empty = queue.dequeue();

      expect(first!.id).toBe("evt-1");
      expect(second!.id).toBe("evt-2");
      expect(third!.id).toBe("evt-3");
      expect(empty).toBeNull();

      cleanupTestQueue(queue, path);
    });

    it("returns null when no pending events", () => {
      const { queue, path } = createTestQueue();

      const dequeued = queue.dequeue();
      expect(dequeued).toBeNull();

      cleanupTestQueue(queue, path);
    });
  });

  describe("Event Processing States", () => {
    it("marks event as processed", () => {
      const { queue, path } = createTestQueue();
      const event = createTestEvent("evt-1", "test.event");

      queue.enqueue(event);
      queue.dequeue();
      const marked = queue.markProcessed("evt-1");

      expect(marked).toBe(true);

      // Verify event is no longer dequeued
      const next = queue.dequeue();
      expect(next).toBeNull();

      cleanupTestQueue(queue, path);
    });

    it("marks event as failed with retry", () => {
      const { queue, path } = createTestQueue();
      const event = createTestEvent("evt-1", "test.event");

      queue.enqueue(event);
      const dequeued = queue.dequeue();
      expect(dequeued!.attempts).toBe(1);

      // Mark failed (should transition back to pending for retry)
      const marked = queue.markFailed("evt-1", new Error("handler failed"));
      expect(marked).toBe(true);

      // Should be dequeued again for retry
      const retried = queue.dequeue();
      expect(retried).not.toBeNull();
      expect(retried!.attempts).toBe(2);
      expect(retried!.error).toBe("handler failed");

      cleanupTestQueue(queue, path);
    });

    it("exhausts retries after max attempts", () => {
      const { queue, path } = createTestQueue();
      const event = createTestEvent("evt-1", "test.event");

      queue.enqueue(event);

      // Fail 3 times (max retries = 3)
      for (let i = 0; i < 3; i++) {
        const dequeued = queue.dequeue();
        expect(dequeued).not.toBeNull();
        queue.markFailed("evt-1", new Error(`attempt ${i + 1} failed`));
      }

      // Next dequeue should be null (max retries exhausted)
      const nextTry = queue.dequeue();
      expect(nextTry).toBeNull();

      cleanupTestQueue(queue, path);
    });
  });

  describe("Recovery (Persistence)", () => {
    it("recovers pending events after queue recreation", () => {
      let { queue, path } = createTestQueue();

      // Enqueue events
      queue.enqueue(createTestEvent("evt-1", "event.first"));
      queue.enqueue(createTestEvent("evt-2", "event.second"));
      queue.enqueue(createTestEvent("evt-3", "event.third"));

      // Dequeue first event to change its status to processing
      queue.dequeue();

      // Close queue
      queue.close();

      // Recreate queue with same path
      queue = new EventQueue(path);

      // Recover pending events
      const pending = queue.recoverPending();

      expect(pending.length).toBe(3); // All 3 events should be recoverable
      expect(pending.map(e => e.id)).toEqual(["evt-1", "evt-2", "evt-3"]);

      cleanupTestQueue(queue, path);
    });

    it("recovers failed events with retries available", () => {
      let { queue, path } = createTestQueue();

      const event = createTestEvent("evt-1", "test.event");
      queue.enqueue(event);

      // Fail once
      queue.dequeue();
      queue.markFailed("evt-1", new Error("first failure"));

      // Close and recreate
      queue.close();
      queue = new EventQueue(path);

      // Should recover the failed event
      const pending = queue.recoverPending();
      expect(pending.length).toBe(1);
      expect(pending[0].status).toBe("pending");
      expect(pending[0].attempts).toBe(1);
      expect(pending[0].error).toBe("first failure");

      cleanupTestQueue(queue, path);
    });

    it("does not recover processed events", () => {
      let { queue, path } = createTestQueue();

      queue.enqueue(createTestEvent("evt-1", "processed.event"));
      queue.enqueue(createTestEvent("evt-2", "pending.event"));

      queue.dequeue();
      queue.markProcessed("evt-1");

      // Close and recreate
      queue.close();
      queue = new EventQueue(path);

      const pending = queue.recoverPending();

      expect(pending.length).toBe(1);
      expect(pending[0].id).toBe("evt-2");

      cleanupTestQueue(queue, path);
    });
  });

  describe("Queue Statistics", () => {
    it("reports accurate queue statistics", () => {
      const { queue, path } = createTestQueue();

      // Enqueue 5 events
      for (let i = 1; i <= 5; i++) {
        queue.enqueue(createTestEvent(`evt-${i}`, "test"));
      }

      // Process different states
      queue.dequeue();
      queue.markProcessed("evt-1");

      queue.dequeue();
      queue.markFailed("evt-2", new Error("test error"));

      const stats = queue.getStats();

      expect(stats.total).toBe(5);
      expect(stats.processed).toBe(1);
      expect(stats.pending).toBe(3);
      expect(stats.processing).toBe(1); // evt-2 is still in processing state
      expect(stats.failed).toBe(0); // evt-2 was retried as pending, not failed

      cleanupTestQueue(queue, path);
    });

    it("calculates oldest event age", () => {
      const { queue, path } = createTestQueue();

      queue.enqueue(createTestEvent("evt-1", "test"));

      // Wait a bit
      const start = Date.now();
      while (Date.now() - start < 100) {
        /* spin */
      }

      const stats = queue.getStats();

      expect(stats.oldestEventAge).not.toBeNull();
      expect(stats.oldestEventAge!).toBeGreaterThanOrEqual(100);

      cleanupTestQueue(queue, path);
    });
  });

  describe("Cleanup", () => {
    it("removes old processed events", () => {
      const { queue, path } = createTestQueue();

      // Enqueue and process events
      for (let i = 1; i <= 3; i++) {
        queue.enqueue(createTestEvent(`evt-${i}`, "test"));
        queue.dequeue();
        queue.markProcessed(`evt-${i}`);
      }

      // Process one more event and don't process it yet
      queue.enqueue(createTestEvent("evt-4", "pending"));

      let stats = queue.getStats();
      expect(stats.processed).toBe(3);
      expect(stats.pending).toBe(1);

      // Cleanup with 0ms threshold (removes all old processed)
      const deleted = queue.cleanup(0);

      expect(deleted).toBe(3);

      stats = queue.getStats();
      expect(stats.processed).toBe(0);
      expect(stats.pending).toBe(1);

      cleanupTestQueue(queue, path);
    });
  });
});
