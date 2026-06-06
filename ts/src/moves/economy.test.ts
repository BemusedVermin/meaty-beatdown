import { describe, it, expect } from "vitest";
import { type ResourceCost } from "../core/cost";
import { effectiveHitstun, juggleScaledDamage } from "../core/resolver";
import { CONFIG } from "../core/config";
import { type Move, type MoveList } from "./move";
import { findPositiveApCycles, r5Holds, netAp, governorReport } from "./economy";
import { makeProfile, makeCost } from "../test-support/fixtures";

const move = (id: string, cost: Partial<ResourceCost>, into: readonly string[]): Move => ({
  id,
  name: id,
  moveClass: "SPECIAL",
  profile: makeProfile({
    cost,
    cancelWindows:
      into.length > 0 ? [{ from: 0, to: 5, gate: "ON_HIT", into, cost: makeCost() }] : [],
  }),
});

describe("R-5 — no net-positive AP cycle (spec §3.5.3, audit C-10)", () => {
  it("rejects a self-cancel loop that refunds ≥ its own cost", () => {
    // 'loop' costs 1 AP but refunds 2 on hit, and cancels into itself → net +1 per lap = infinite AP.
    const moves: MoveList = [move("loop", { ap: 1, apGain: { amount: 2, gate: "ON_HIT" } }, ["loop"])];
    expect(netAp(moves[0]!)).toBe(1);
    const cycles = findPositiveApCycles(moves);
    expect(cycles).toHaveLength(1);
    expect(cycles[0]!.nodes).toEqual(["loop"]);
    expect(r5Holds(moves)).toBe(false);
  });

  it("accepts a self-cancel loop that drains AP (gain < cost)", () => {
    const moves: MoveList = [move("loop", { ap: 2, apGain: { amount: 1, gate: "ON_HIT" } }, ["loop"])];
    expect(netAp(moves[0]!)).toBe(-1);
    expect(r5Holds(moves)).toBe(true);
  });

  it("rejects a net-positive two-move cycle (A→B→A)", () => {
    const moves: MoveList = [
      move("a", { ap: 1, apGain: { amount: 2, gate: "ON_HIT" } }, ["b"]),
      move("b", { ap: 1, apGain: { amount: 2, gate: "ON_HIT" } }, ["a"]),
    ];
    const cycles = findPositiveApCycles(moves);
    expect(cycles).toHaveLength(1);
    expect([...cycles[0]!.nodes].sort()).toEqual(["a", "b"]);
    expect(cycles[0]!.netAp).toBe(2);
    expect(r5Holds(moves)).toBe(false);
  });

  it("accepts a net-negative two-move cycle", () => {
    const moves: MoveList = [
      move("a", { ap: 2, apGain: { amount: 1, gate: "ON_HIT" } }, ["b"]),
      move("b", { ap: 2, apGain: { amount: 1, gate: "ON_HIT" } }, ["a"]),
    ];
    expect(r5Holds(moves)).toBe(true);
  });

  it("accepts an acyclic cancel graph regardless of net AP", () => {
    const moves: MoveList = [
      move("opener", { ap: 0, apGain: { amount: 5, gate: "ON_HIT" } }, ["ender"]),
      move("ender", { ap: 0 }, []),
    ];
    expect(r5Holds(moves)).toBe(true);
  });
});

describe("the four combo governors are all present (audit C-4)", () => {
  it("governorReport lists all four as active", () => {
    const report = governorReport();
    expect(report.map((g) => g.governor).sort()).toEqual(
      ["AP_EXHAUSTION", "FOCUS_COST", "HITSTUN_DECAY", "JUGGLE_DECAY"].sort(),
    );
    expect(report.every((g) => g.present)).toBe(true);
  });
});

describe("governor 2 (juggle decay) terminates a juggle", () => {
  it("juggle damage strictly decreases and reaches 0", () => {
    const seq = Array.from({ length: 12 }, (_v, n) => juggleScaledDamage(100, n));
    for (let i = 1; i < seq.length; i++) expect(seq[i]!).toBeLessThanOrEqual(seq[i - 1]!);
    expect(juggleScaledDamage(100, 100)).toBe(0); // a long juggle eventually deals nothing
  });
});

describe("governor 3 (hitstun decay) forces a ground combo to end", () => {
  it("effective hitstun decays until on_hit goes negative (opponent recovers first)", () => {
    const base = 20;
    const recovery = 10;
    const decay = CONFIG.combo.HITSTUN_DECAY_PER_HIT;
    expect(effectiveHitstun(base, 1)).toBe(base); // first hit undecayed
    expect(effectiveHitstun(base, 2)).toBe(base - decay);

    // Find the combo length at which advantage (effectiveHitstun − recovery) first goes negative.
    let combo = 1;
    while (effectiveHitstun(base, combo) - recovery >= 0 && combo < 100) combo++;
    expect(combo).toBeLessThan(100); // it MUST terminate
    expect(effectiveHitstun(base, combo) - recovery).toBeLessThan(0);
    // And it never drops below the floor (the hit still connects, just minus).
    expect(effectiveHitstun(base, 999)).toBe(CONFIG.combo.MIN_HITSTUN);
  });
});
