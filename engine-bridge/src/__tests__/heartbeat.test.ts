import { RpcClient } from "../rpc-client";
import { EventPropagator } from "../event-propagator";
import { HeartbeatMonitor } from "../heartbeat-monitor";

describe("HeartbeatMonitor", () => {
  let rpc: RpcClient;
  let propagator: EventPropagator;
  let monitor: HeartbeatMonitor;
  let logSpy: jest.SpyInstance;
  let warnSpy: jest.SpyInstance;

  beforeEach(() => {
    rpc = new RpcClient(["http://test"]);
    propagator = new EventPropagator(rpc, "CCONTRACT");
    monitor = new HeartbeatMonitor(rpc, propagator, { intervalMs: 100 });
    logSpy = jest.spyOn(console, "log").mockImplementation(() => {});
    warnSpy = jest.spyOn(console, "warn").mockImplementation(() => {});
  });

  afterEach(() => {
    monitor.stop();
    logSpy.mockRestore();
    warnSpy.mockRestore();
  });

  it("logs healthy pulse on start", async () => {
    rpc.call = jest.fn().mockResolvedValue({ protocolVersion: 20 });

    monitor.start();
    // Wait for the immediate pulse
    await new Promise(resolve => setTimeout(resolve, 10));

    expect(logSpy).toHaveBeenCalledWith(
      "[Heartbeat] Pulse check:",
      expect.stringContaining('"status":"HEALTHY"')
    );
    expect(logSpy).toHaveBeenCalledWith(
      "[Heartbeat] Pulse check:",
      expect.stringContaining('"liveNodes":1')
    );
  });

  it("logs degraded pulse on RPC failure", async () => {
    rpc.call = jest.fn().mockRejectedValue(new Error("RPC Down"));

    monitor.start();
    await new Promise(resolve => setTimeout(resolve, 10));

    expect(warnSpy).toHaveBeenCalledWith(
      "[Heartbeat] Pulse check DEGRADED:",
      expect.stringContaining('"status":"DEGRADED"')
    );
    expect(warnSpy).toHaveBeenCalledWith(
      "[Heartbeat] Pulse check DEGRADED:",
      expect.stringContaining('"error":"RPC Down"')
    );
  });

  it("reports event propagator state", async () => {
    rpc.call = jest.fn().mockResolvedValue({});
    propagator.start();
    // Manually set a cursor via private access for testing if needed,
    // or just let it be "none".

    monitor.start();
    await new Promise(resolve => setTimeout(resolve, 10));

    expect(logSpy).toHaveBeenCalledWith(
      "[Heartbeat] Pulse check:",
      expect.stringContaining('"eventPropagator":{"running":true,"cursor":"none"}')
    );
    propagator.stop();
  });

  it("logs at intervals", async () => {
    rpc.call = jest.fn().mockResolvedValue({});

    monitor.start();
    await new Promise(resolve => setTimeout(resolve, 150)); // interval is 100ms

    // 1 immediate + 1 interval
    expect(logSpy).toHaveBeenCalledTimes(2);
  });
});
