import { WebSocket }      from "ws";
import { ZkStateSyncer }  from "../zk-state-syncer";
import type { EngineEvent } from "../event-propagator";
import type { ZkStateSnapshot } from "../zk-state-syncer";

// Minimal EventPropagator stand-in that captures the registered handler.
function makePropagator() {
  let handler: ((e: EngineEvent) => void) | null = null;
  return {
    onEvent(fn: (e: EngineEvent) => void) { handler = fn; },
    emit(e: EngineEvent)                  { handler?.(e); },
  };
}

function makeEvent(overrides: Partial<EngineEvent> = {}): EngineEvent {
  return {
    id:         "evt-1",
    contractId: "CTEST",
    topic:      ["state_commitment"],
    value:      "base64XDR==",
    ledger:     1234,
    timestamp:  "2026-06-19T00:00:00Z",
    ...overrides,
  };
}

async function connectClient(port: number): Promise<WebSocket> {
  const ws = new WebSocket(`ws://127.0.0.1:${port}`);
  await new Promise<void>((resolve, reject) => {
    ws.once("open",  resolve);
    ws.once("error", reject);
  });
  return ws;
}

// ── ZkStateSyncer ─────────────────────────────────────────────────────────────

describe("ZkStateSyncer", () => {
  let syncer: ZkStateSyncer;
  let prop:   ReturnType<typeof makePropagator>;

  beforeEach(async () => {
    prop   = makePropagator();
    syncer = new ZkStateSyncer(prop, { port: 0, pingIntervalMs: 60_000 });
    await syncer.ready;
  });

  afterEach(async () => {
    await syncer.close();
  });

  it("starts with no connected clients", () => {
    expect(syncer.clientCount()).toBe(0);
  });

  it("tracks connected client count", async () => {
    const ws = await connectClient(syncer.getPort());
    expect(syncer.clientCount()).toBe(1);
    ws.close();
    // Allow the close event to propagate by polling up to 400ms
    for (let i = 0; i < 20; i++) {
      if (syncer.clientCount() === 0) break;
      await new Promise(r => setTimeout(r, 20));
    }
    expect(syncer.clientCount()).toBe(0);
  });

  it("pushes a ZkStateSnapshot when a matching event fires", async () => {
    const ws  = await connectClient(syncer.getPort());
    const msg = await new Promise<ZkStateSnapshot>(resolve => {
      ws.once("message", data => resolve(JSON.parse(data.toString())));
      prop.emit(makeEvent());
    });

    expect(msg.type).toBe("zk_state_update");
    expect(msg.eventId).toBe("evt-1");
    expect(msg.contractId).toBe("CTEST");
    expect(msg.ledger).toBe(1234);
    expect(msg.raw).toBe("base64XDR==");

    ws.close();
  });

  it("broadcasts to all connected clients simultaneously", async () => {
    const [ws1, ws2] = await Promise.all([
      connectClient(syncer.getPort()),
      connectClient(syncer.getPort()),
    ]);

    const received = await new Promise<[ZkStateSnapshot, ZkStateSnapshot]>(resolve => {
      const results: ZkStateSnapshot[] = [];
      const onMsg = (data: Buffer) => {
        results.push(JSON.parse(data.toString()));
        if (results.length === 2) resolve(results as [ZkStateSnapshot, ZkStateSnapshot]);
      };
      ws1.once("message", onMsg);
      ws2.once("message", onMsg);
      prop.emit(makeEvent());
    });

    expect(received).toHaveLength(2);
    ws1.close();
    ws2.close();
  });

  it("does NOT push events whose topic lacks the ZK marker", async () => {
    const ws = await connectClient(syncer.getPort());

    let received = false;
    ws.on("message", () => { received = true; });

    prop.emit(makeEvent({ topic: ["governance_vote"] }));
    // Give the event loop a tick to propagate any spurious message
    await new Promise(r => setTimeout(r, 30));

    expect(received).toBe(false);
    ws.close();
  });

  it("respects a custom zkTopic filter", async () => {
    await syncer.close();
    prop    = makePropagator();
    syncer  = new ZkStateSyncer(prop, { port: 0, zkTopic: "breaker_open", pingIntervalMs: 60_000 });
    await syncer.ready;

    const ws  = await connectClient(syncer.getPort());
    const msg = await new Promise<ZkStateSnapshot>(resolve => {
      ws.once("message", data => resolve(JSON.parse(data.toString())));
      prop.emit(makeEvent({ topic: ["breaker_open"], id: "evt-cb" }));
    });

    expect(msg.eventId).toBe("evt-cb");
    ws.close();
  });

  it("snapshot shape includes all required fields", async () => {
    const ws  = await connectClient(syncer.getPort());
    const msg = await new Promise<ZkStateSnapshot>(resolve => {
      ws.once("message", data => resolve(JSON.parse(data.toString())));
      prop.emit(makeEvent({ timestamp: "2026-06-19T12:00:00Z" }));
    });

    expect(msg).toMatchObject<ZkStateSnapshot>({
      type:       "zk_state_update",
      eventId:    expect.any(String),
      contractId: expect.any(String),
      ledger:     expect.any(Number),
      timestamp:  "2026-06-19T12:00:00Z",
      raw:        expect.anything(),
    });

    ws.close();
  });
});
