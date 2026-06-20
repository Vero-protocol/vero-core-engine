/**
 * performanceSim.ts — Deterministic generator of TPS/gas samples.
 *
 * `usePerformanceStats` advances the simulation on a timer; to keep the model
 * testable and pure this file hosts the deterministic part. The walk is
 * bounded so values stay believable (baseFee stays in the range 80–400
 * stroops; TPS stays in the range 0–60).
 */

import type { PerformanceSample } from "../types";

export interface PerformanceState {
  baseFee: number;
  prevTps: number;
  prevTxCount: number;
  samples: PerformanceSample[];
}

const WINDOW_MS = 60_000 * 3;

export const INITIAL_STATE: PerformanceState = {
  baseFee: 120,
  prevTps: 6,
  prevTxCount: 6,
  samples: [],
};

/** A randomness source returning a value in [0, 1). */
export type RandomSource = () => number;

const defaultRandom: RandomSource = () => Math.random();

/** Mulberry32 — tiny seedable PRNG (32-bit state, good enough for sims/tests). */
export function createPrng(seed: number): RandomSource {
  let state = seed >>> 0;
  return () => {
    state = (state + 0x6d2b79f5) >>> 0;
    let t = state;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}

/**
 * Advance the simulation by one tick. `now` is injected so tests are
 * deterministic. `random` is also injectable so callers (and tests) can
 * produce reproducible sequences; it defaults to `Math.random`. The
 * returned state is fully immutable; callers can swap it into React state
 * directly.
 */
export function tick(
  state: PerformanceState,
  now: number,
  random: RandomSource = defaultRandom
): PerformanceState {
  // Bounded random walk for base fee (symmetric step in [-1, 1]).
  const nextBaseFee = clamp(
    state.baseFee + Math.round((random() * 2 - 1) * 18),
    80,
    400
  );

  // TPS drifts around its previous value with an upward bias during normal
  // operation and occasional spikes.
  const drift = (random() - 0.45) * 4;
  const nextTps = clamp(state.prevTps + drift + random() * 2, 0, 60);

  const elapsedSec = 1; // one tick == one second of simulated activity
  const txCount = state.prevTxCount + Math.round(nextTps * elapsedSec);

  const sample: PerformanceSample = {
    timestamp: now,
    baseFee: nextBaseFee,
    maxFee: Math.ceil(nextBaseFee * 1.2),
    txCount,
    tps: Number(nextTps.toFixed(2)),
  };

  const cutoff = now - WINDOW_MS;
  const samples = [...state.samples, sample].filter(
    (s) => s.timestamp >= cutoff
  );

  return {
    baseFee: nextBaseFee,
    prevTps: nextTps,
    prevTxCount: txCount,
    samples,
  };
}

/** Aggregate helpers used by the UI. Pure functions, fully unit-testable. */
export function average(values: number[]): number {
  if (values.length === 0) return 0;
  return values.reduce((sum, v) => sum + v, 0) / values.length;
}

export function peak(values: number[]): number {
  return values.length === 0 ? 0 : Math.max(...values);
}

export function latestTps(state: PerformanceState): number {
  return state.prevTps;
}

export function latestBaseFee(state: PerformanceState): number {
  return state.baseFee;
}
