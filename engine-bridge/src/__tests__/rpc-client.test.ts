import { RpcClient } from "../rpc-client";
import { logger } from "../logger";

// Replace SorobanRpc.Server with a lightweight stand-in so no real HTTP/RPC
// machinery is instantiated and the endpoint URL each attempt targets is
// observable through the `url` property.
jest.mock("@stellar/stellar-sdk", () => {
  class FakeServer {
    url: string;
    constructor(url: string) {
      this.url = url;
    }
  }
  return { SorobanRpc: { Server: FakeServer } };
});

// Keep winston from writing noisy JSON to the console during tests.
jest.mock("../logger", () => ({
  logger: { warn: jest.fn() },
}));

const QUARANTINE_MS = 30_000;

function urlOf(server: unknown): string {
  return (server as { url: string }).url;
}

describe("RpcClient", () => {
  afterEach(() => {
    jest.useRealTimers();
    jest.clearAllMocks();
  });

  describe("constructor", () => {
    it("rejects an empty endpoint list", () => {
      expect(() => new RpcClient([])).toThrow(
        "RpcClient: at least one URL required"
      );
    });
  });

  describe("round-robin selection", () => {
    it("cycles through endpoints in order across successive calls", async () => {
      const client = new RpcClient(["http://a", "http://b", "http://c"]);
      const seen: string[] = [];

      for (let i = 0; i < 6; i++) {
        await client.call(async (server) => {
          seen.push(urlOf(server));
          return i;
        });
      }

      expect(seen).toEqual([
        "http://a",
        "http://b",
        "http://c",
        "http://a",
        "http://b",
        "http://c",
      ]);
    });
  });

  describe("failover and quarantine", () => {
    it("fails over to the next endpoint and quarantines the failing one", async () => {
      const client = new RpcClient(["http://a", "http://b"]);
      const seen: string[] = [];

      const result = await client.call(async (server) => {
        const url = urlOf(server);
        seen.push(url);
        if (url === "http://a") throw new Error("node down");
        return "ok";
      });

      expect(result).toBe("ok");
      expect(seen).toEqual(["http://a", "http://b"]);
      expect(client.liveCount()).toBe(1);
      expect(logger.warn).toHaveBeenCalledWith(
        expect.stringContaining("[RpcClient] http://a quarantined")
      );
    });

    it("rethrows the last error after exhausting all retries", async () => {
      const client = new RpcClient(["http://a", "http://b", "http://c"]);
      const seen: string[] = [];

      await expect(
        client.call(async (server) => {
          seen.push(urlOf(server));
          throw new Error("boom");
        })
      ).rejects.toThrow("boom");

      expect(seen).toEqual(["http://a", "http://b", "http://c"]);
      expect(client.liveCount()).toBe(0);
    });
  });

  describe("quarantine expiry / re-admission", () => {
    it("re-admits a quarantined endpoint once the quarantine window elapses", async () => {
      jest.useFakeTimers();

      const client = new RpcClient(["http://a"]);
      const failing = async () => {
        throw new Error("boom");
      };

      await expect(client.call(failing)).rejects.toThrow(
        "RpcClient: all endpoints unavailable"
      );
      expect(client.liveCount()).toBe(0);

      // Still quarantined just before the window elapses.
      jest.advanceTimersByTime(QUARANTINE_MS - 1);
      expect(client.liveCount()).toBe(0);

      // Re-admitted exactly at the expiry boundary.
      jest.advanceTimersByTime(1);
      expect(client.liveCount()).toBe(1);

      // And a subsequent call succeeds again.
      await expect(client.call(async () => "recovered")).resolves.toBe(
        "recovered"
      );
    });
  });

  describe("liveCount and all-dead path", () => {
    it("reports the number of healthy endpoints", () => {
      const client = new RpcClient(["http://a", "http://b", "http://c"]);
      expect(client.liveCount()).toBe(3);
    });

    it("throws when all endpoints are unavailable", async () => {
      const client = new RpcClient(["http://a"]);
      const failing = async () => {
        throw new Error("boom");
      };

      await expect(client.call(failing)).rejects.toThrow(
        "RpcClient: all endpoints unavailable"
      );
      expect(client.liveCount()).toBe(0);
    });
  });
});
