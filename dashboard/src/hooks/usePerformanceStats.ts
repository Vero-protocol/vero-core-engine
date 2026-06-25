/**
 * usePerformanceStats — Drives the TPS/Gas visualizer.
 *
 * The simulation is deterministic on construction but advances on a 1s
 * interval while the hook is mounted. Pausing simply clears the timer
 * without throwing away collected samples, so the sparkline stays visible
 * while the dashboard is in the background.
 */

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  INITIAL_STATE,
  average,
  peak,
  tick,
  type PerformanceState,
} from "../utils/performanceSim";
import type { PerformanceSample } from "../types";

export interface UsePerformanceStatsResult {
  samples: PerformanceSample[];
  latestTps: number;
  latestBaseFee: number;
  latestMaxFee: number;
  peakTps: number;
  avgTps: number;
  peakBaseFee: number;
  avgBaseFee: number;
  running: boolean;
  toggle: () => void;
  restart: () => void;
}

export function usePerformanceStats(
  intervalMs: number = 1000
): UsePerformanceStatsResult {
  const [state, setState] = useState<PerformanceState>(INITIAL_STATE);
  const [running, setRunning] = useState(true);
  const intervalRef = useRef<number | null>(null);

  useEffect(() => {
    if (!running) {
      if (intervalRef.current !== null) {
        window.clearInterval(intervalRef.current);
        intervalRef.current = null;
      }
      return;
    }
    intervalRef.current = window.setInterval(() => {
      setState((prev) => tick(prev, Date.now()));
    }, intervalMs);
    return () => {
      if (intervalRef.current !== null) {
        window.clearInterval(intervalRef.current);
        intervalRef.current = null;
      }
    };
  }, [running, intervalMs]);

  const toggle = useCallback(() => setRunning((r) => !r), []);
  const restart = useCallback(() => {
    setState(INITIAL_STATE);
  }, []);

  const derived = useMemo(() => {
    const tpsValues = state.samples.map((s) => s.tps);
    const baseFeeValues = state.samples.map((s) => s.baseFee);
    return {
      peakTps: peak(tpsValues),
      avgTps: Number(average(tpsValues).toFixed(2)),
      peakBaseFee: peak(baseFeeValues),
      avgBaseFee: Math.round(average(baseFeeValues)),
    };
  }, [state.samples]);

  return {
    samples: state.samples,
    latestTps: state.prevTps,
    latestBaseFee: state.baseFee,
    latestMaxFee: Math.ceil(state.baseFee * 1.2),
    ...derived,
    running,
    toggle,
    restart,
  };
}
