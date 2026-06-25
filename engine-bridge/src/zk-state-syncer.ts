/**
 * zk-state-syncer.ts — WebSocket push server for ZK state-commitment events.
 *
 * Hooks into EventPropagator, filters Soroban events whose topic contains the
 * ZK state-commitment marker, and immediately broadcasts a ZkStateSnapshot to
 * every connected dashboard client.  Push latency from event receipt to client
 * delivery is O(1) — no additional polling round-trip.
 *
 * Includes Stellar wallet authentication (SEP-10) to verify dashboard clients.
 */

import { WebSocketServer, WebSocket } from "ws";
import { Keypair } from "@stellar/stellar-sdk";
import type { EventPropagator, EngineEvent } from "./event-propagator";
import { RelayerAuth } from "./relayer-auth";
import type { RelayerAuthOptions } from "./relayer-auth";
import { WalletConnector } from "./wallet-connector";

export interface ZkStateSnapshot {
  type:       "zk_state_update";
  eventId:    string;
  contractId: string;
  ledger:     number;
  timestamp:  string;
  /** Base64-encoded XDR of the raw StateCommitment value. */
  raw:        unknown;
}

export interface ZkStateSyncerOptions {
  /** TCP port for the WebSocket server (use 0 to let the OS assign one). */
  port:            number;
  /**
   * Substring matched against each base64-encoded topic to identify ZK
   * state-commitment events.  Defaults to "state_commitment".
   */
  zkTopic?:        string;
  /** Interval between keep-alive pings in ms.  Defaults to 30 000. */
  pingIntervalMs?: number;
  /** Optional relayer authentication config. */
  auth?:           RelayerAuthOptions;
  serverSigningKey?: string;
  networkPassphrase?: string;
  domain?: string;
}

const DEFAULT_ZK_TOPIC     = "state_commitment";
const DEFAULT_PING_INTERVAL = 30_000;

export class ZkStateSyncer {
  /** Resolves once the WebSocket server is bound and ready to accept clients. */
  readonly ready: Promise<void>;

  private readonly wss:        WebSocketServer;
  private readonly clients   = new Map<WebSocket, { authenticatedAs?: string }>();
  private          pingTimer: ReturnType<typeof setInterval> | null = null;
  private readonly zkTopic:   string;
  private readonly options:   ZkStateSyncerOptions;

  constructor(
    propagator: Pick<EventPropagator, "onEvent">,
    options:    ZkStateSyncerOptions,
  ) {
    this.options = options;
    this.zkTopic = options.zkTopic ?? DEFAULT_ZK_TOPIC;

    this.wss = new WebSocketServer({
      port: options.port,
      ...(options.auth && { verifyClient: (info, cb) => new RelayerAuth(options.auth!).verifyClient(info, cb) }),
    });
    this.ready = new Promise(resolve => this.wss.once("listening", resolve));

    this.wss.on("connection", (ws: WebSocket) => {
      this.clients.set(ws, {});

      ws.on("message", (data) => this.handleClientMessage(ws, data.toString()));
      ws.on("close",  () => this.clients.delete(ws));
      ws.on("error",  () => this.clients.delete(ws));
    });

    const pingMs = options.pingIntervalMs ?? DEFAULT_PING_INTERVAL;
    this.pingTimer = setInterval(() => this.heartbeat(), pingMs);

    propagator.onEvent(event => this.handleEvent(event));
  }

  /** Number of currently connected dashboard clients. */
  clientCount(): number {
    return this.clients.size;
  }

  /** Bound port — useful when constructed with port 0. */
  getPort(): number {
    const addr = this.wss.address();
    if (typeof addr === "object" && addr !== null) return addr.port;
    throw new Error("ZkStateSyncer: server not bound to a TCP port");
  }

  /** Gracefully close the server and all active connections. */
  close(): Promise<void> {
    if (this.pingTimer) {
      clearInterval(this.pingTimer);
      this.pingTimer = null;
    }
    return new Promise((resolve, reject) =>
      this.wss.close(err => (err ? reject(err) : resolve()))
    );
  }

  private handleClientMessage(ws: WebSocket, message: string): void {
    try {
      const msg = JSON.parse(message);

      switch (msg.type) {
        case "auth_request":
          this.handleAuthRequest(ws, msg.address);
          break;
        case "auth_submit":
          this.handleAuthSubmit(ws, msg.xdr);
          break;
      }
    } catch (err) {
      console.error("[ZkStateSyncer] Failed to parse client message:", err);
    }
  }

  private handleAuthRequest(ws: WebSocket, address: string): void {
    const { serverSigningKey, networkPassphrase, domain } = this.options;
    if (!serverSigningKey || !networkPassphrase || !domain) {
      ws.send(JSON.stringify({ type: "auth_error", message: "Auth not configured on server" }));
      return;
    }

    try {
      const serverKeypair = Keypair.fromSecret(serverSigningKey);
      const xdr = WalletConnector.createChallenge({
        serverKeypair,
        clientAddress: address,
        networkPassphrase,
        domain
      });
      ws.send(JSON.stringify({ type: "auth_challenge", xdr }));
    } catch (err) {
      ws.send(JSON.stringify({ type: "auth_error", message: (err as Error).message }));
    }
  }

  private handleAuthSubmit(ws: WebSocket, xdr: string): void {
    const { serverSigningKey, networkPassphrase, domain } = this.options;
    if (!serverSigningKey || !networkPassphrase || !domain) return;

    try {
      const serverKeypair = Keypair.fromSecret(serverSigningKey);
      const address = WalletConnector.verifyResponse(
        xdr,
        serverKeypair.publicKey(),
        networkPassphrase,
        domain
      );

      const client = this.clients.get(ws);
      if (client) {
        client.authenticatedAs = address;
        ws.send(JSON.stringify({ type: "auth_success", address }));
      }
    } catch (err) {
      ws.send(JSON.stringify({ type: "auth_error", message: (err as Error).message }));
    }
  }

  handleEvent(event: EngineEvent): void {
    if (!event.topic.some(t => t.includes(this.zkTopic))) return;

    const snapshot: ZkStateSnapshot = {
      type:       "zk_state_update",
      eventId:    event.id,
      contractId: event.contractId,
      ledger:     event.ledger,
      timestamp:  event.timestamp,
      raw:        event.value,
    };

    this.broadcast(snapshot);
  }

  private broadcast(snapshot: ZkStateSnapshot): void {
    const msg = JSON.stringify(snapshot);
    for (const [ws, state] of this.clients) {
      // Only broadcast to authenticated clients if auth is configured
      if (this.options.serverSigningKey && !state.authenticatedAs) continue;

      if (ws.readyState === WebSocket.OPEN) {
        ws.send(msg);
      } else {
        this.clients.delete(ws);
      }
    }
  }

  private heartbeat(): void {
    for (const [ws] of this.clients) {
      if (ws.readyState === WebSocket.OPEN) {
        ws.ping();
      } else {
        this.clients.delete(ws);
      }
    }
  }
}
