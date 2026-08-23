/**
 * main.ts — Entry point for the engine-bridge service.
 *
 * Orchestrates the RpcClient, EventPropagator, ZkStateSyncer,
 * AlertChannelService, and HeartbeatMonitor into a running process.
 */

import { RpcClient } from "./rpc-client";
import { EventPropagator } from "./event-propagator";
import { ZkStateSyncer } from "./zk-state-syncer";
import { horizonAccountLoader } from "./wallet-connector";
import { Horizon } from "@stellar/stellar-sdk";
import { HeartbeatMonitor } from "./heartbeat-monitor";
import { AlertChannelService, WebhookAlertChannel, ConsoleAlertChannel } from "./alert-channel";

async function main() {
  const rpcUrls    = (process.env.RPC_URLS    || "https://soroban-testnet.stellar.org").split(",");
  const contractId =  process.env.CONTRACT_ID || "";
  const port       = parseInt(process.env.PORT || "8080", 10);
  const cursor     = process.env.EVENT_CURSOR;
  const webhookUrl =  process.env.ALERT_WEBHOOK_URL || "";

  // Relayer authentication. Previously none of this was passed to the syncer,
  // so `verifyClient` was never installed and the broadcast guard short-
  // circuited — the server accepted every connection and sent all ZK state
  // commitments to it, with RelayerAuth unreachable in the only production
  // entrypoint.
  const apiKeys = (process.env.RELAYER_API_KEYS || "")
    .split(",")
    .map((k) => k.trim())
    .filter(Boolean);
  const jwtSecret        = process.env.RELAYER_JWT_SECRET || undefined;
  const serverSigningKey = process.env.SERVER_SIGNING_KEY || undefined;
  const allowUnauthenticated = process.env.ALLOW_UNAUTHENTICATED_SYNCER === "true";
  // SEP-10 verification needs the client account's real signer set.
  const horizonUrl = process.env.HORIZON_URL || "https://horizon-testnet.stellar.org";
  const loadAccount = serverSigningKey
    ? horizonAccountLoader(new Horizon.Server(horizonUrl))
    : undefined;

  console.log("[Bridge] Starting service...");
  console.log(`[Bridge] RPC URLs:   ${rpcUrls.join(", ")}`);
  console.log(`[Bridge] Contract:   ${contractId}`);
  console.log(`[Bridge] Webhook:    ${webhookUrl || "none (console only)"}`);

  const rpc        = new RpcClient(rpcUrls);
  const propagator = new EventPropagator(rpc, contractId, cursor);

  // Alert channel service — console by default, webhook if configured
  const alertChannels: (ConsoleAlertChannel | WebhookAlertChannel)[] = [new ConsoleAlertChannel()];
  if (webhookUrl) {
    alertChannels.push(new WebhookAlertChannel({ url: webhookUrl }));
  }
  const alertService = new AlertChannelService({ channels: alertChannels });

  const hasAuth = apiKeys.length > 0 || Boolean(jwtSecret);
  const syncer   = new ZkStateSyncer(propagator, {
    port,
    ...(hasAuth && { auth: { apiKeys: apiKeys.length ? apiKeys : undefined, jwtSecret } }),
    ...(serverSigningKey && { serverSigningKey }),
    ...(loadAccount && { loadAccount }),
    ...(process.env.NETWORK_PASSPHRASE && { networkPassphrase: process.env.NETWORK_PASSPHRASE }),
    ...(process.env.AUTH_DOMAIN && { domain: process.env.AUTH_DOMAIN }),
    allowUnauthenticated,
  });
  const heartbeat  = new HeartbeatMonitor(rpc, propagator, { alertService });

  heartbeat.start();
  propagator.start();

  await syncer.ready;
  console.log(`[Bridge] ZK State Syncer listening on port ${syncer.getPort()}`);

  // Graceful shutdown
  const shutdown = async () => {
    console.log("[Bridge] Shutting down...");
    heartbeat.stop();
    propagator.stop();
    await syncer.close();
    process.exit(0);
  };

  process.on("SIGINT",  shutdown);
  process.on("SIGTERM", shutdown);
}

if (require.main === module || (process.argv[1] && process.argv[1].endsWith("index.js"))) {
  main().catch(err => {
    console.error("[Bridge] Fatal error:", err);
    process.exit(1);
  });
}

export { main };
