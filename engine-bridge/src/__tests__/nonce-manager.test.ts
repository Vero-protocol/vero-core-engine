import { NonceManager } from "../nonce-manager";
import { RpcClient } from "../rpc-client";

// Minimal deferred so a getAccount round-trip can be suspended mid-flight and
// resolved from the test body at an exact point in the interleaving.
function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>(r => { resolve = r; });
  return { promise, resolve };
}

/**
 * Stub RpcClient whose `getAccount` hands out the queued sequence numbers in
 * call order. `calls` records every round-trip so the test can assert that a
 * second one never starts while the first is still outstanding.
 */
function stubRpc(sequences: Array<Promise<string> | string>) {
  const calls: string[] = [];

  const rpc = {
    call: <T>(fn: (server: never) => Promise<T>): Promise<T> => {
      const sequence = sequences[calls.length];
      calls.push("getAccount");
      const server = {
        getAccount: async (accountId: string) => {
          void accountId;
          const sequenceNumber = await sequence;
          return { sequenceNumber: () => sequenceNumber };
        },
      };
      return fn(server as never);
    },
  } as unknown as RpcClient;

  return { rpc, calls };
}

const ACCOUNT = "GABCDEF";

/** Let all currently queued microtasks run. */
const flush = () => new Promise<void>(r => setImmediate(r));

describe("NonceManager", () => {
  it("hands out consecutive sequences from one network read", async () => {
    const { rpc, calls } = stubRpc(["100"]);
    const manager = new NonceManager(rpc);

    expect(await manager.reserve(ACCOUNT)).toBe(101n);
    expect(await manager.reserve(ACCOUNT)).toBe(102n);
    expect(calls).toHaveLength(1);
  });

  it("re-reads the network on refresh", async () => {
    const { rpc } = stubRpc(["100", "200"]);
    const manager = new NonceManager(rpc);

    expect(await manager.reserve(ACCOUNT)).toBe(101n);
    await manager.refresh(ACCOUNT);
    expect(await manager.reserve(ACCOUNT)).toBe(201n);
  });

  it("does not let refresh() interleave with an in-flight reserve()", async () => {
    const slowRead = deferred<string>();
    const { rpc, calls } = stubRpc([slowRead.promise, "200"]);
    const manager = new NonceManager(rpc);

    // reserve() suspends inside getAccount with no cache entry written yet.
    const reserved = manager.reserve(ACCOUNT);
    await flush();
    expect(calls).toHaveLength(1);

    // refresh() arrives mid-flight; it must block on the per-account lock
    // instead of issuing its own round-trip and writing the cache.
    const refreshed = manager.refresh(ACCOUNT);
    await flush();
    expect(calls).toHaveLength(1);

    slowRead.resolve("100");
    expect(await reserved).toBe(101n);
    await refreshed;
    expect(calls).toHaveLength(2);

    // refresh()'s fresher read is the one that survives — without the lock the
    // suspended reserve() would have overwritten it, handing back 102n next.
    expect(await manager.reserve(ACCOUNT)).toBe(201n);
  });

  it("does not let reserve() interleave with an in-flight refresh()", async () => {
    const slowRead = deferred<string>();
    const { rpc, calls } = stubRpc([slowRead.promise, "300"]);
    const manager = new NonceManager(rpc);

    const refreshed = manager.refresh(ACCOUNT);
    await flush();
    expect(calls).toHaveLength(1);

    const reserved = manager.reserve(ACCOUNT);
    await flush();
    expect(calls).toHaveLength(1);

    slowRead.resolve("200");
    await refreshed;
    // Reads the sequence refresh() warmed the cache with, not a second read.
    expect(await reserved).toBe(201n);
    expect(calls).toHaveLength(1);
  });
});
