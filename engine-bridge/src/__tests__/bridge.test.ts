import { RpcClient }   from "../rpc-client";
import { NonceManager } from "../nonce-manager";

// ── RpcClient ────────────────────────────────────────────────────────────────

describe("RpcClient", () => {
  it("fails over to second endpoint when first is dead", async () => {
    const order: number[] = [];
    const client = new RpcClient(["http://dead", "http://live"]);

    // Patch pickEndpoint to simulate first being quarantined
    (client as any).endpoints[0].deadUntil = Date.now() + 60_000;

    const result = await client.call(async (_server: unknown) => {
      order.push(1);
      return "ok";
    });

    expect(result).toBe("ok");
  });

  it("throws when all endpoints are dead", async () => {
    const client = new RpcClient(["http://dead"]);
    (client as any).endpoints[0].deadUntil = Date.now() + 60_000;

    await expect(client.call(async () => "x")).rejects.toThrow("all endpoints unavailable");
  });

  it("reports liveCount correctly", () => {
    const client = new RpcClient(["http://a", "http://b"]);
    (client as any).endpoints[0].deadUntil = Date.now() + 60_000;
    expect(client.liveCount()).toBe(1);
  });
});

// ── NonceManager ─────────────────────────────────────────────────────────────

describe("NonceManager", () => {
  function makeRpc(seq: bigint): RpcClient {
    const rpc = new RpcClient(["http://test"]);
    rpc.call = async (fn: any) =>
      fn({ getAccount: async (_: string) => ({ sequenceNumber: () => seq.toString() }) });
    return rpc;
  }

  it("returns incrementing sequences", async () => {
    const nm = new NonceManager(makeRpc(100n));
    const a  = await nm.reserve("GTEST");
    const b  = await nm.reserve("GTEST");
    expect(b).toBe(a + 1n);
  });

  it("release rewinds the counter", async () => {
    const nm  = new NonceManager(makeRpc(100n));
    const seq = await nm.reserve("GTEST");
    nm.release("GTEST", seq);
    const next = await nm.reserve("GTEST");
    expect(next).toBe(seq);
  });
});
