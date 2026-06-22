import { EventQueue } from "../event-queue";
import { EventPropagator, EngineEvent } from "../event-propagator";
import * as fs from "fs";
import * as path from "path";

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

function cleanupQueueFiles(dbPath: string) {
  [dbPath, `${dbPath}-shm`, `${dbPath}-wal`].forEach(p => {
    if (fs.existsSync(p)) fs.unlinkSync(p);
  });
}

describe("EventPropagator batching", () => {
  it("respects maxEventsPerCycle during recovery", async () => {
    const dbPath = path.join(process.cwd(), `test-prop-queue-${Date.now()}.db`);

    // Create a queue and enqueue 5 events
    const q = new EventQueue(dbPath);
    for (let i = 1; i <= 5; i++) {
      q.enqueue(createTestEvent(`evt-${i}`, `topic.${i}`));
    }
    q.close();

    // Create propagator that uses same queue path and limits to 2 events per cycle
    const propagator = new EventPropagator({} as any, "CA123456", undefined, dbPath, 2);

    const processed: string[] = [];
    propagator.onEvent(async (ev) => {
      // Simulate async handler work
      await new Promise(r => setTimeout(r, 1));
      processed.push(ev.id);
    });

    // Run recovery: should process only 2 events
    await propagator.recoverPendingEvents();

    expect(processed.length).toBe(2);

    // Check remaining pending events in the queue (should be 3)
    const q2 = new EventQueue(dbPath);
    const pending = q2.recoverPending();
    expect(pending.length).toBe(3);

    // Cleanup
    q2.close();
    cleanupQueueFiles(dbPath);
  });
});
