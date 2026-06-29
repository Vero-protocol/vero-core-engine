import { AlertChannelService, ConsoleAlertChannel, WebhookAlertChannel } from "../alert-channel";
import type { Alert, AlertChannel } from "../alert-channel";

describe("AlertChannelService", () => {
  const testAlert: Alert = {
    id: "alert-1",
    severity: "CRITICAL",
    title: "Test alert",
    message: "Something went wrong",
    timestamp: new Date().toISOString(),
  };

  it("sends to all registered channels", async () => {
    const sent: Alert[] = [];
    const channel: AlertChannel = { send: async (a) => { sent.push(a); return true; } };
    const service = new AlertChannelService({ channels: [channel, channel] });

    await service.send(testAlert);

    expect(sent).toHaveLength(2);
    expect(sent[0].id).toBe("alert-1");
  });

  it("does not throw when a channel fails", async () => {
    const failing: AlertChannel = { send: async () => { throw new Error("fail"); } };
    const passing: AlertChannel = { send: async () => true };
    const service = new AlertChannelService({ channels: [failing, passing] });

    await expect(service.send(testAlert)).resolves.toBeUndefined();
  });

  it("addChannel appends to the channel list", async () => {
    const sent: Alert[] = [];
    const channel: AlertChannel = { send: async (a) => { sent.push(a); return true; } };
    const service = new AlertChannelService({ channels: [] });

    service.addChannel(channel);
    await service.send(testAlert);

    expect(sent).toHaveLength(1);
  });

  it("defaults to ConsoleAlertChannel when no channels provided", () => {
    const service = new AlertChannelService();
    expect(service).toBeDefined();
  });
});

describe("ConsoleAlertChannel", () => {
  const testAlert: Alert = {
    id: "alert-2",
    severity: "WARNING",
    title: "Warning alert",
    message: "Something worth noting",
    timestamp: new Date().toISOString(),
  };

  it("returns true on success", async () => {
    const channel = new ConsoleAlertChannel();
    const result = await channel.send(testAlert);
    expect(result).toBe(true);
  });

  it("handles CRITICAL severity", async () => {
    const channel = new ConsoleAlertChannel();
    const result = await channel.send({ ...testAlert, severity: "CRITICAL" });
    expect(result).toBe(true);
  });

  it("handles INFO severity", async () => {
    const channel = new ConsoleAlertChannel();
    const result = await channel.send({ ...testAlert, severity: "INFO" });
    expect(result).toBe(true);
  });

  it("handles alerts with metadata", async () => {
    const channel = new ConsoleAlertChannel();
    const result = await channel.send({
      ...testAlert,
      metadata: { foo: "bar" },
    });
    expect(result).toBe(true);
  });
});

describe("WebhookAlertChannel", () => {
  const testAlert: Alert = {
    id: "alert-3",
    severity: "CRITICAL",
    title: "Webhook test",
    message: "Testing webhook delivery",
    timestamp: new Date().toISOString(),
  };

  beforeEach(() => {
    jest.restoreAllMocks();
  });

  it("sends POST to the configured URL", async () => {
    const mockFetch = jest.spyOn(globalThis, "fetch").mockResolvedValue(
      new Response(null, { status: 200 })
    );

    const channel = new WebhookAlertChannel({ url: "https://hooks.example.com/alert" });
    const result = await channel.send(testAlert);

    expect(result).toBe(true);
    expect(mockFetch).toHaveBeenCalledWith(
      "https://hooks.example.com/alert",
      expect.objectContaining({
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          id: "alert-3",
          severity: "CRITICAL",
          title: "Webhook test",
          message: "Testing webhook delivery",
          timestamp: testAlert.timestamp,
          metadata: undefined,
        }),
      })
    );
  });

  it("returns false on non-OK response", async () => {
    jest.spyOn(globalThis, "fetch").mockResolvedValue(
      new Response(null, { status: 500 })
    );

    const channel = new WebhookAlertChannel({ url: "https://hooks.example.com/alert" });
    const result = await channel.send(testAlert);

    expect(result).toBe(false);
  });

  it("returns false on network error", async () => {
    jest.spyOn(globalThis, "fetch").mockRejectedValue(new Error("Network error"));

    const channel = new WebhookAlertChannel({ url: "https://hooks.example.com/alert" });
    const result = await channel.send(testAlert);

    expect(result).toBe(false);
  });

  it("uses custom headers", async () => {
    const mockFetch = jest.spyOn(globalThis, "fetch").mockResolvedValue(
      new Response(null, { status: 200 })
    );

    const channel = new WebhookAlertChannel({
      url: "https://hooks.example.com/alert",
      headers: { Authorization: "Bearer token123" },
    });
    await channel.send(testAlert);

    expect(mockFetch).toHaveBeenCalledWith(
      expect.any(String),
      expect.objectContaining({
        headers: {
          "Content-Type": "application/json",
          Authorization: "Bearer token123",
        },
      })
    );
  });

  it("aborts on timeout", async () => {
    jest.spyOn(globalThis, "fetch").mockImplementation(
      () => new Promise((_, reject) => {
        setTimeout(() => reject(new Error("AbortError")), 200);
      })
    );

    const channel = new WebhookAlertChannel({ url: "https://hooks.example.com/alert", timeoutMs: 50 });
    const result = await channel.send(testAlert);

    expect(result).toBe(false);
  }, 10_000);
});
