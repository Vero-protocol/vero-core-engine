/**
 * main.ts — Entry point for the engine-bridge service.
 *
 * Orchestrates the RpcClient, EventPropagator, ZkStateSyncer, and
 * HeartbeatMonitor into a running process.
 */

import { RpcClient } from "./rpc-client";
import { EventPropagator } from "./event-propagator";
import { ZkStateSyncer } from "./zk-state-syncer";
import { HeartbeatMonitor } from "./heartbeat-monitor";

async function main() {
  const rpcUrls    = (process.env.RPC_URLS    || "https://soroban-testnet.stellar.org").split(",");
  const contractId =  process.env.CONTRACT_ID || "";
  const port       = parseInt(process.env.PORT || "8080", 10);
  const cursor     = process.env.EVENT_CURSOR;

  console.log("[Bridge] Starting service...");
  console.log(`[Bridge] RPC URLs:   ${rpcUrls.join(", ")}`);
  console.log(`[Bridge] Contract:   ${contractId}`);

  const rpc        = new RpcClient(rpcUrls);
  const propagator = new EventPropagator(rpc, contractId, cursor);
  const syncer     = new ZkStateSyncer(propagator, { port });
  const heartbeat  = new HeartbeatMonitor(rpc, propagator);

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
