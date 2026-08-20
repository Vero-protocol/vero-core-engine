import { EventPropagator } from "../event-propagator";
import { RpcClient } from "../rpc-client";
import { EventQueue } from "../event-queue";

// Mock dependencies
jest.mock("../rpc-client");
jest.mock("../event-queue");
jest.mock("../logger", () => ({
  logger: {
    info: jest.fn(),
    warn: jest.fn(),
    error: jest.fn(),
  }
}));

describe("EventPropagator", () => {
  let propagator: EventPropagator;
  let mockRpcClient: jest.Mocked<RpcClient>;
  let mockEventQueue: jest.Mocked<EventQueue>;

  beforeEach(() => {
    jest.clearAllMocks();
    
    mockRpcClient = {
      call: jest.fn(),
    } as unknown as jest.Mocked<RpcClient>;

    mockEventQueue = {
      enqueue: jest.fn(),
      dequeue: jest.fn(),
      markProcessed: jest.fn(),
      markFailed: jest.fn(),
      getStats: jest.fn(),
      recoverPending: jest.fn(),
      close: jest.fn(),
    } as unknown as jest.Mocked<EventQueue>;

    // We need to mock the constructor of EventQueue since it's instantiated inside EventPropagator
    (EventQueue as jest.Mock).mockImplementation(() => mockEventQueue);

    propagator = new EventPropagator(mockRpcClient, "test-contract");
  });

  afterEach(() => {
    propagator.stop();
  });

  it("AC-1: does not advance cursor past an event that failed to enqueue", async () => {
    const mockEvents = [
      {
        id: "evt-1",
        contractId: { contractId: () => "test-contract" },
        topic: [{ toXDR: () => "topic1" }],
        value: { toXDR: () => "val1" },
        ledger: 100,
        ledgerClosedAt: "2023-01-01T00:00:00Z"
      },
      {
        id: "evt-2",
        contractId: { contractId: () => "test-contract" },
        topic: [{ toXDR: () => "topic2" }],
        value: { toXDR: () => "val2" },
        ledger: 101,
        ledgerClosedAt: "2023-01-01T00:00:10Z"
      },
      {
        id: "evt-3",
        contractId: { contractId: () => "test-contract" },
        topic: [{ toXDR: () => "topic3" }],
        value: { toXDR: () => "val3" },
        ledger: 102,
        ledgerClosedAt: "2023-01-01T00:00:20Z"
      }
    ];

    mockRpcClient.call.mockResolvedValueOnce({ events: mockEvents });

    // Mock enqueue to fail on the second event
    mockEventQueue.enqueue
      .mockReturnValueOnce(true)  // evt-1 succeeds
      .mockReturnValueOnce(false) // evt-2 fails
      .mockReturnValueOnce(true); // evt-3 succeeds (should not be reached)

    // Call private method fetchAndEnqueue
    await (propagator as unknown as { fetchAndEnqueue: () => Promise<void> }).fetchAndEnqueue();

    // Verify cursor only advanced to evt-1
    expect(propagator.getCursor()).toBe("evt-1");
    
    // Verify enqueue was only called for evt-1 and evt-2
    expect(mockEventQueue.enqueue).toHaveBeenCalledTimes(2);
  });
});
