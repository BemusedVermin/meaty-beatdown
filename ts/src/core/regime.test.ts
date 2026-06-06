import { describe, it, expect } from "vitest";
import { computeRegime, nextDecisionTick } from "./regime";
import { makeEntity } from "../test-support/fixtures";

describe("computeRegime — keyed off ready_tick (spec §2.1, audit C-2)", () => {
  it("equal ready_ticks ⇒ NEUTRAL (both commit hidden)", () => {
    expect(computeRegime(makeEntity({ readyTick: 0 }), makeEntity({ readyTick: 0 }))).toEqual({
      kind: "NEUTRAL",
    });
    expect(computeRegime(makeEntity({ readyTick: 42 }), makeEntity({ readyTick: 42 }))).toEqual({
      kind: "NEUTRAL",
    });
  });

  it("the lower ready_tick is the PRESSURE actor (acts with full info)", () => {
    expect(computeRegime(makeEntity({ readyTick: 12 }), makeEntity({ readyTick: 35 }))).toEqual({
      kind: "PRESSURE",
      actor: 0,
    });
    expect(computeRegime(makeEntity({ readyTick: 35 }), makeEntity({ readyTick: 12 }))).toEqual({
      kind: "PRESSURE",
      actor: 1,
    });
  });

  it("nextDecisionTick is the minimum ready_tick", () => {
    expect(nextDecisionTick(makeEntity({ readyTick: 12 }), makeEntity({ readyTick: 35 }))).toBe(12);
    expect(nextDecisionTick(makeEntity({ readyTick: 9 }), makeEntity({ readyTick: 9 }))).toBe(9);
  });
});
