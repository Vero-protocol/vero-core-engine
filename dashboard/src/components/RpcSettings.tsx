import React, { useState } from "react";
import {
  Activity,
  CheckCircle2,
  Loader2,
  Plus,
  RefreshCw,
  Trash2,
  WifiOff,
} from "lucide-react";
import { Card } from "./Card";
import { useRpcNodes } from "../hooks/useRpcNodes";

interface RpcStatusDotProps {
  status: "unknown" | "checking" | "healthy" | "unreachable";
}

const RpcStatusDot: React.FC<RpcStatusDotProps> = ({ status }) => {
  if (status === "checking") {
    return <Loader2 size={14} className="animate-spin text-amber-500" aria-label="checking" />;
  }
  if (status === "healthy") {
    return <CheckCircle2 size={14} className="text-emerald-500" aria-label="healthy" />;
  }
  if (status === "unreachable") {
    return <WifiOff size={14} className="text-rose-500" aria-label="unreachable" />;
  }
  return (
    <span
      className="inline-block h-3.5 w-3.5 rounded-full border border-gray-300 dark:border-gray-500"
      aria-label="not yet probed"
    />
  );
};

/**
 * RpcSettings — Custom RPC endpoint manager.
 *
 * Acceptance criteria from issue #38:
 *   - UI for adding RPC nodes
 *   - Connectivity check (probe)
 *   - Active RPC selection persisted across reloads
 */
export const RpcSettings: React.FC = () => {
  const {
    nodes,
    activeId,
    activeNode,
    addNode,
    removeNode,
    setActive,
    probeAll,
  } = useRpcNodes();

  const [label, setLabel] = useState("");
  const [url, setUrl] = useState("");
  const [formError, setFormError] = useState<string | null>(null);
  const [probing, setProbing] = useState(false);
  const [lastProbedAt, setLastProbedAt] = useState<string | null>(null);

  const handleAdd = (e: React.FormEvent) => {
    e.preventDefault();
    const result = addNode(label, url);
    if (!result.ok) {
      setFormError(result.reason ?? "Unable to add endpoint");
      return;
    }
    setLabel("");
    setUrl("");
    setFormError(null);
  };

  const handleProbeAll = async () => {
    setProbing(true);
    try {
      await probeAll();
      setLastProbedAt(new Date().toISOString());
    } finally {
      setProbing(false);
    }
  };

  return (
    <Card
      title="Custom RPC Endpoints"
      description="Add any number of Soroban/Horizon RPC URLs and pick which one the dashboard should use."
      actions={
        <button
          type="button"
          onClick={handleProbeAll}
          disabled={probing || nodes.length === 0}
          className="inline-flex items-center gap-1.5 px-3 py-1.5 text-sm font-medium rounded-md bg-gray-900 text-white dark:bg-gray-100 dark:text-gray-900 hover:opacity-90 disabled:opacity-40 disabled:cursor-not-allowed focus:outline-none focus:ring-2 focus:ring-blue-500/60 transition-colors duration-150"
        >
          {probing ? <Loader2 size={14} className="animate-spin" /> : <RefreshCw size={14} />}
          Probe all
        </button>
      }
    >
      <form
        onSubmit={handleAdd}
        className="grid gap-3 md:grid-cols-[1fr_2fr_auto] items-start mb-5"
        aria-label="Add RPC endpoint"
      >
        <div>
          <label htmlFor="rpc-label" className="block text-xs font-medium text-gray-500 dark:text-gray-400 mb-1">
            Label
          </label>
          <input
            id="rpc-label"
            type="text"
            value={label}
            onChange={(e) => setLabel(e.target.value)}
            placeholder="SDF Testnet"
            className="w-full px-3 py-2 rounded-md border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-900 text-sm focus:outline-none focus:ring-2 focus:ring-blue-500/60"
          />
        </div>
        <div>
          <label htmlFor="rpc-url" className="block text-xs font-medium text-gray-500 dark:text-gray-400 mb-1">
            RPC URL
          </label>
          <input
            id="rpc-url"
            type="url"
            value={url}
            onChange={(e) => setUrl(e.target.value)}
            placeholder="https://soroban-testnet.stellar.org"
            className="w-full px-3 py-2 rounded-md border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-900 text-sm focus:outline-none focus:ring-2 focus:ring-blue-500/60"
            required
          />
        </div>
        <button
          type="submit"
          className="md:mt-6 inline-flex items-center gap-1.5 px-4 py-2 text-sm font-medium rounded-md bg-blue-600 hover:bg-blue-500 text-white transition-colors duration-150 focus:outline-none focus:ring-2 focus:ring-blue-500/60"
        >
          <Plus size={14} />
          Add
        </button>
      </form>
      {formError && (
        <p className="mb-4 text-sm text-rose-600 dark:text-rose-400" role="alert">
          {formError}
        </p>
      )}

      {nodes.length === 0 ? (
        <div className="flex flex-col items-center justify-center py-12 text-center text-gray-500 dark:text-gray-400 border border-dashed border-gray-200 dark:border-gray-700 rounded-lg">
          <Activity size={28} className="mb-2 opacity-60" />
          <p className="text-sm">No RPC endpoints configured yet.</p>
          <p className="text-xs mt-1">Add one above to start streaming live state from the network.</p>
        </div>
      ) : (
        <ul className="divide-y divide-gray-100 dark:divide-gray-700/60 border border-gray-200 dark:border-gray-700 rounded-lg overflow-hidden">
          {nodes.map((node) => {
            const isActive = node.id === activeId;
            return (
              <li
                key={node.id}
                className={
                  "flex items-center gap-4 px-4 py-3 text-sm transition-colors duration-150 " +
                  (isActive
                    ? "bg-blue-50 dark:bg-blue-900/10"
                    : "hover:bg-gray-50 dark:hover:bg-gray-700/40")
                }
              >
                <RpcStatusDot status={node.status} />
                <div className="flex-1 min-w-0">
                  <div className="flex items-baseline gap-2">
                    <span className="font-medium text-gray-900 dark:text-gray-50 truncate">{node.label}</span>
                    {isActive && (
                      <span className="text-[10px] uppercase tracking-wide font-semibold text-blue-600 dark:text-blue-300">
                        Active
                      </span>
                    )}
                  </div>
                  <div className="text-xs text-gray-500 dark:text-gray-400 truncate" title={node.url}>
                    {node.url}
                  </div>
                  {node.message && (
                    <div className="text-[11px] text-rose-500 dark:text-rose-400 mt-0.5">{node.message}</div>
                  )}
                </div>
                <div className="text-xs tabular-nums text-gray-500 dark:text-gray-400 w-20 text-right">
                  {node.latencyMs !== null ? `${node.latencyMs} ms` : "—"}
                </div>
                <button
                  type="button"
                  onClick={() => setActive(node.id)}
                  className={
                    "text-xs px-2 py-1 rounded-md border transition-colors duration-150 " +
                    (isActive
                      ? "border-blue-500 text-blue-700 dark:text-blue-300"
                      : "border-gray-300 dark:border-gray-600 hover:border-blue-400 hover:text-blue-600 dark:hover:text-blue-300")
                  }
                  aria-pressed={isActive}
                >
                  {isActive ? "Selected" : "Select"}
                </button>
                <button
                  type="button"
                  onClick={() => removeNode(node.id)}
                  className="p-1.5 rounded-md text-gray-400 hover:text-rose-500 hover:bg-rose-50 dark:hover:bg-rose-900/20 transition-colors duration-150 focus:outline-none focus:ring-2 focus:ring-rose-500/60"
                  aria-label={`Remove ${node.label}`}
                >
                  <Trash2 size={14} />
                </button>
              </li>
            );
          })}
        </ul>
      )}

      <div className="mt-4 flex flex-wrap items-center gap-x-4 gap-y-1 text-xs text-gray-500 dark:text-gray-400">
        <span>
          Active endpoint:{" "}
          <code className="px-1.5 py-0.5 rounded bg-gray-100 dark:bg-gray-700 text-gray-800 dark:text-gray-100">
            {activeNode ? activeNode.url : "default"}
          </code>
        </span>
        {lastProbedAt && <span>Last probe: {new Date(lastProbedAt).toLocaleTimeString()}</span>}
      </div>
    </Card>
  );
};
