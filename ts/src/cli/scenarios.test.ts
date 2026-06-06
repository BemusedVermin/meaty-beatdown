import { describe, it, expect } from "vitest";
import { runMatch } from "../core/engine";
import { allScenarios, scenarioById } from "./scenarios";

// Build a FRESH scenario each call — agents are stateful and must not be reused across runs.
const traceOf = (id: string) => {
  const s = scenarioById(id)!;
  return runMatch(s.initial, s.tables, s.agents, s.options).trace;
};

describe("CLI scenarios run and produce coherent traces", () => {
  it("every scenario runs without throwing and emits events", () => {
    for (const s of allScenarios()) {
      const r = runMatch(s.initial, s.tables, s.agents, s.options);
      expect(r.trace.length).toBeGreaterThan(0);
    }
  });

  it("reza-borin: neutral mind-read, then armor absorbs Reza's poke, then a counter-hit", () => {
    const trace = traceOf("reza-borin");
    expect(trace.some((e) => e.kind === "STATE" && e.regime === "NEUTRAL")).toBe(true);
    expect(trace.some((e) => e.kind === "CONTACT" && e.result === "ARMORED")).toBe(true);
    expect(trace.some((e) => e.kind === "CONTACT" && e.result === "HIT" && e.counter)).toBe(true);
  });

  it("sidestep-ap: tempo jab → homing-sweep cancel → finisher denied for lack of AP", () => {
    const trace = traceOf("sidestep-ap");
    expect(trace.some((e) => e.kind === "CANCEL" && e.into === "homing_sweep")).toBe(true);
    expect(trace.some((e) => e.kind === "DENIED" && e.moveId === "heavy_cleave")).toBe(true);
  });

  it("scenarios are deterministic (same trace across fresh runs)", () => {
    for (const built of allScenarios()) {
      const fresh = scenarioById(built.id)!;
      const a = runMatch(built.initial, built.tables, built.agents, built.options).trace;
      const b = runMatch(fresh.initial, fresh.tables, fresh.agents, fresh.options).trace;
      expect(b).toEqual(a);
    }
  });
});
