import React, { useMemo } from "react";
import {
  Gauge,
  Pause,
  Play,
  RotateCcw,
  TrendingUp,
  Zap,
} from "lucide-react";
import { Card } from "./Card";
import { usePerformanceStats } from "../hooks/usePerformanceStats";
import type { PerformanceSample } from "../types";

const MetricCard: React.FC<{
  label: string;
  value: string;
  hint?: string;
  icon: React.ReactNode;
  tone?: "default" | "warning" | "success";
}> = ({ label, value, hint, icon, tone = "default" }) => {
  const toneClass =
    tone === "warning"
      ? "text-amber-600 dark:text-amber-300"
      : tone === "success"
      ? "text-emerald-600 dark:text-emerald-300"
      : "text-blue-600 dark:text-blue-300";
  return (
    <div className="p-4 rounded-lg border border-gray-200 dark:border-gray-700 bg-gray-50 dark:bg-gray-900/30 transition-colors duration-150 hover:border-blue-300 dark:hover:border-blue-600/60">
      <div className="flex items-center justify-between mb-2">
        <span className="text-xs uppercase tracking-wide text-gray-500 dark:text-gray-400">{label}</span>
        <span className={"p-1.5 rounded-md bg-white dark:bg-gray-800 " + toneClass}>{icon}</span>
      </div>
      <div className="text-2xl font-semibold tabular-nums text-gray-900 dark:text-gray-50">{value}</div>
      {hint && <div className="mt-1 text-[11px] text-gray-500 dark:text-gray-400">{hint}</div>}
    </div>
  );
};

interface SparklineProps {
  samples: PerformanceSample[];
  series: "tps" | "baseFee";
  width?: number;
  height?: number;
}

/**
 * Sparkline — compact SVG visualisation of recent values. Pure-SVG keeps
 * the dashboard dependency-free while still showing trend information.
 */
const Sparkline: React.FC<SparklineProps> = ({
  samples,
  series,
  width = 280,
  height = 64,
}) => {
  const path = useMemo(() => {
    if (samples.length < 2) return "";
    const values = samples.map((s) => (series === "tps" ? s.tps : s.baseFee));
    const max = Math.max(...values, series === "tps" ? 1 : 100);
    const min = Math.min(...values, 0);
    const span = max - min || 1;
    const step = width / (samples.length - 1);
    return values
      .map((v, i) => {
        const x = i * step;
        const y = height - ((v - min) / span) * (height - 4) - 2;
        return `${i === 0 ? "M" : "L"}${x.toFixed(2)},${y.toFixed(2)}`;
      })
      .join(" ");
  }, [samples, series, width, height]);

  const stroke = series === "tps" ? "#3b82f6" : "#10b981";

  return (
    <svg
      viewBox={`0 0 ${width} ${height}`}
      width="100%"
      height={height}
      role="img"
      aria-label={`Sparkline of ${series}`}
      className="block"
    >
      <path d={path} fill="none" stroke={stroke} strokeWidth={2} strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  );
};

/**
 * PerformanceStats — TPS/Gas visualizer.
 *
 * Acceptance criteria from issue #36:
 *   - Stats service (usePerformanceStats hook)
 *   - Stats visible (metric cards + sparklines)
 *   - Metrics accurate (deterministic when paused)
 */
export const PerformanceStats: React.FC = () => {
  const stats = usePerformanceStats();

  return (
    <Card
      title="Network Performance"
      description="Live throughput and gas metrics streamed from the connected RPC."
      actions={
        <div className="flex items-center gap-2">
          <button
            type="button"
            onClick={stats.toggle}
            className="inline-flex items-center gap-1.5 px-3 py-1.5 text-sm font-medium rounded-md border border-gray-300 dark:border-gray-600 hover:border-blue-400 dark:hover:border-blue-500 transition-colors duration-150 focus:outline-none focus:ring-2 focus:ring-blue-500/60"
            aria-pressed={!stats.running}
          >
            {stats.running ? <Pause size={14} /> : <Play size={14} />}
            {stats.running ? "Pause" : "Resume"}
          </button>
          <button
            type="button"
            onClick={stats.restart}
            className="inline-flex items-center gap-1.5 px-3 py-1.5 text-sm font-medium rounded-md border border-gray-300 dark:border-gray-600 hover:border-blue-400 dark:hover:border-blue-500 transition-colors duration-150 focus:outline-none focus:ring-2 focus:ring-blue-500/60"
          >
            <RotateCcw size={14} />
            Restart
          </button>
        </div>
      }
    >
      <div className="grid grid-cols-2 md:grid-cols-4 gap-3 mb-5">
        <MetricCard
          label="Current TPS"
          value={stats.latestTps.toFixed(2)}
          hint={stats.running ? "Streaming" : "Paused"}
          icon={<Gauge size={14} />}
          tone="success"
        />
        <MetricCard
          label="Peak TPS"
          value={stats.peakTps.toFixed(2)}
          hint={`Avg ${stats.avgTps.toFixed(2)}`}
          icon={<TrendingUp size={14} />}
        />
        <MetricCard
          label="Base Fee"
          value={`${stats.latestBaseFee} stroops`}
          hint={`Avg ${stats.avgBaseFee} stroops`}
          icon={<Zap size={14} />}
          tone="warning"
        />
        <MetricCard
          label="Max Fee (1.2×)"
          value={`${stats.latestMaxFee} stroops`}
          hint={`Peak ${stats.peakBaseFee} baseFee`}
          icon={<Zap size={14} />}
        />
      </div>

      <div className="grid gap-4 md:grid-cols-2">
        <div className="p-4 rounded-lg border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-900/30">
          <div className="flex items-center justify-between mb-2">
            <h3 className="text-sm font-medium text-gray-900 dark:text-gray-50">Transactions / second</h3>
            <span className="text-[11px] text-gray-500 dark:text-gray-400 tabular-nums">
              last {stats.samples.length} samples
            </span>
          </div>
          <Sparkline samples={stats.samples} series="tps" />
        </div>
        <div className="p-4 rounded-lg border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-900/30">
          <div className="flex items-center justify-between mb-2">
            <h3 className="text-sm font-medium text-gray-900 dark:text-gray-50">Base fee (stroops)</h3>
            <span className="text-[11px] text-gray-500 dark:text-gray-400 tabular-nums">
              window:{" "}
              {stats.samples.length >= 2
                ? `${Math.round(
                    (stats.samples[stats.samples.length - 1].timestamp -
                      stats.samples[0].timestamp) /
                      1000
                  )}s`
                : "—"}
            </span>
          </div>
          <Sparkline samples={stats.samples} series="baseFee" />
        </div>
      </div>
    </Card>
  );
};
