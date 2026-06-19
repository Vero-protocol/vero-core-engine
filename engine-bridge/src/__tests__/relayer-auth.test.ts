import { WebSocket } from "ws";
import { RelayerAuth } from "../relayer-auth";
import { ZkStateSyncer } from "../zk-state-syncer";
import type { EngineEvent } from "../event-propagator";
import type { IncomingMessage } from "http";

function makePropagator() {
  let handler: ((e: EngineEvent) => void) | null = null;
  return {
    onEvent(fn: (e: EngineEvent) => void) { handler = fn; },
    emit(e: EngineEvent) { handler?.(e); },
  };
}

function makeReq(headers: Record<string, string>): IncomingMessage {
  const req = { headers } as unknown as IncomingMessage;
  return req;
}

function makeInfo(headers: Record<string, string>) {
  return {
    origin: "",
    secure: false,
    req: makeReq(headers),
  };
}

function verify(
  auth: RelayerAuth,
  headers: Record<string, string>,
): Promise<{ allowed: boolean; code?: number; message?: string }> {
  return new Promise(resolve => {
    auth.verifyClient(makeInfo(headers), (allowed, code, message) => {
      resolve({ allowed, code, message });
    });
  });
}

describe("RelayerAuth", () => {
  describe("API key auth", () => {
    it("allows connection with valid x-api-key header", async () => {
      const auth = new RelayerAuth({ apiKeys: ["sk-valid-key"] });
      const result = await verify(auth, { "x-api-key": "sk-valid-key" });
      expect(result.allowed).toBe(true);
    });

    it("rejects connection with missing API key", async () => {
      const auth = new RelayerAuth({ apiKeys: ["sk-valid-key"] });
      const result = await verify(auth, {});
      expect(result.allowed).toBe(false);
      expect(result.code).toBe(401);
    });

    it("rejects connection with invalid API key", async () => {
      const auth = new RelayerAuth({ apiKeys: ["sk-valid-key"] });
      const result = await verify(auth, { "x-api-key": "sk-wrong-key" });
      expect(result.allowed).toBe(false);
      expect(result.code).toBe(401);
    });

    it("accepts API key via Authorization Bearer header", async () => {
      const auth = new RelayerAuth({ apiKeys: ["sk-bearer-token"] });
      const result = await verify(auth, { authorization: "Bearer sk-bearer-token" });
      expect(result.allowed).toBe(true);
    });

    it("rejects Authorization header with wrong scheme", async () => {
      const auth = new RelayerAuth({ apiKeys: ["sk-valid"] });
      const result = await verify(auth, { authorization: "Basic sk-valid" });
      expect(result.allowed).toBe(false);
      expect(result.code).toBe(401);
    });
  });

  describe("with no keys configured", () => {
    it("rejects all connections when no API keys are set", async () => {
      const auth = new RelayerAuth({});
      const result = await verify(auth, { "x-api-key": "some-key" });
      expect(result.allowed).toBe(false);
      expect(result.code).toBe(401);
    });
  });
});

describe("ZkStateSyncer with auth", () => {
  let syncer: ZkStateSyncer;
  let prop: ReturnType<typeof makePropagator>;

  afterEach(async () => {
    await syncer?.close();
  });

  async function connectClient(port: number, headers?: Record<string, string>): Promise<WebSocket> {
    const ws = new WebSocket(`ws://127.0.0.1:${port}`, { headers });
    await new Promise<void>((resolve, reject) => {
      ws.once("open", resolve);
      ws.once("error", reject);
    });
    return ws;
  }

  it("rejects WebSocket connection without API key when auth is configured", async () => {
    prop = makePropagator();
    syncer = new ZkStateSyncer(prop, {
      port: 0,
      pingIntervalMs: 60_000,
      auth: { apiKeys: ["sk-relayer-1"] },
    });
    await syncer.ready;

    const ws = new WebSocket(`ws://127.0.0.1:${syncer.getPort()}`);
    await expect(
      new Promise<void>((resolve, reject) => {
        ws.once("open", resolve);
        ws.once("error", reject);
      }),
    ).rejects.toThrow();
    expect(syncer.clientCount()).toBe(0);
  });

  it("accepts WebSocket connection with valid API key", async () => {
    prop = makePropagator();
    syncer = new ZkStateSyncer(prop, {
      port: 0,
      pingIntervalMs: 60_000,
      auth: { apiKeys: ["sk-relayer-1"] },
    });
    await syncer.ready;

    const ws = await connectClient(syncer.getPort(), { "x-api-key": "sk-relayer-1" });
    expect(syncer.clientCount()).toBe(1);
    ws.close();
  });

  it("broadcasts events to authenticated clients", async () => {
    prop = makePropagator();
    syncer = new ZkStateSyncer(prop, {
      port: 0,
      pingIntervalMs: 60_000,
      auth: { apiKeys: ["sk-relayer-1"] },
    });
    await syncer.ready;

    const ws = await connectClient(syncer.getPort(), { "x-api-key": "sk-relayer-1" });
    const msg = await new Promise<Record<string, unknown>>(resolve => {
      ws.once("message", data => resolve(JSON.parse(data.toString())));
      prop.emit({
        id: "evt-auth",
        contractId: "CTEST",
        topic: ["state_commitment"],
        value: "xdr==",
        ledger: 1,
        timestamp: "2026-06-19T00:00:00Z",
      });
    });

    expect(msg.type).toBe("zk_state_update");
    expect(msg.eventId).toBe("evt-auth");
    ws.close();
  });

  it("works without auth when no auth config is provided", async () => {
    prop = makePropagator();
    syncer = new ZkStateSyncer(prop, { port: 0, pingIntervalMs: 60_000 });
    await syncer.ready;

    const ws = await new Promise<WebSocket>((resolve, reject) => {
      const s = new WebSocket(`ws://127.0.0.1:${syncer.getPort()}`);
      s.once("open", () => resolve(s));
      s.once("error", reject);
    });
    expect(syncer.clientCount()).toBe(1);
    ws.close();
  });
});
