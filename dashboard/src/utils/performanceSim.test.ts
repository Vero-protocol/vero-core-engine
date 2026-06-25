import { describe, it, expect } from "vitest";
import {
  INITIAL_STATE,
  average,
  createPrng,
  peak,
  tick,
} from "./performanceSim";

describe("performanceSim", () => {
  it("average returns 0 for empty input", () => {
    expect(average([])).toBe(0);
  });

  it("peak returns 0 for empty input", () => {
    expect(peak([])).toBe(0);
  });

  it("tick is deterministic given the same PRNG seed", () => {
    const now = 1_700_000_000_000;
    const a = [INITIAL_STATE, INITIAL_STATE, INITIAL_STATE].reduce(
      (state) => tick(state, now, createPrng(42)),
      INITIAL_STATE
    );
    const b = [INITIAL_STATE, INITIAL_STATE, INITIAL_STATE].reduce(
      (state) => tick(state, now, createPrng(42)),
      INITIAL_STATE
    );
    expect(a.samples.map((s) => s.tps)).toEqual(
      b.samples.map((s) => s.tps)
    );
    expect(a.samples.map((s) => s.baseFee)).toEqual(
      b.samples.map((s) => s.baseFee)
    );
  });

  it("tick produces different sequences for different seeds", () => {
    const now = 1_700_000_000_000;
    const a = tick(INITIAL_STATE, now, createPrng(1));
    const b = tick(INITIAL_STATE, now, createPrng(2));
    const firstTps = a.samples[0]?.tps ?? -1;
    const firstBaseFee = a.samples[0]?.baseFee ?? -1;
    expect(
      firstTps !== (b.samples[0]?.tps ?? -1) ||
        firstBaseFee !== (b.samples[0]?.baseFee ?? -1)
    ).toBe(true);
  });

  it("keeps baseFee and tps within their declared bounds", () => {
    const now = 1_700_000_000_000;
    let state = INITIAL_STATE;
    const rng = createPrng(7);
    for (let i = 0; i < 100; i++) {
      state = tick(state, now + i * 1000, rng);
      const last = state.samples[state.samples.length - 1];
      expect(last.baseFee).toBeGreaterThanOrEqual(80);
      expect(last.baseFee).toBeLessThanOrEqual(400);
      expect(last.tps).toBeGreaterThanOrEqual(0);
      expect(last.tps).toBeLessThanOrEqual(60);
    }
  });

  it("maxFee is always >= baseFee", () => {
    const now = 1_700_000_000_000;
    let state = INITIAL_STATE;
    const rng = createPrng(99);
    for (let i = 0; i < 20; i++) {
      state = tick(state, now + i * 1000, rng);
      const last = state.samples[state.samples.length - 1];
      expect(last.maxFee).toBeGreaterThanOrEqual(last.baseFee);
    }
  });
});
