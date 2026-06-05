import { describe, it, expect } from "vitest";
import {
  type Weapon,
  meetsRequirements,
  paretoDominations,
  r4Holds,
  tradeoffScore,
} from "./equipment";

const weapon = (o: Partial<Weapon> = {}): Weapon => ({
  id: "w",
  weaponClass: "sword",
  minRange: 0,
  maxRange: 2,
  startupDelta: 0,
  recoveryDelta: 0,
  damageDelta: 0,
  requirements: {},
  grantsMoves: [],
  ...o,
});

// A balanced roster: each weapon is strictly best on exactly one axis (range / speed / damage).
const dagger = weapon({ id: "dagger", maxRange: 1, startupDelta: -4, damageDelta: -3 }); // fastest
const spear = weapon({ id: "spear", maxRange: 5, startupDelta: -1, damageDelta: 0 }); // longest
const greatsword = weapon({ id: "greatsword", maxRange: 3, startupDelta: 6, damageDelta: 10 }); // hardest

describe("R-4 — the range/speed/damage tradeoff triangle (spec §4.4)", () => {
  it("tradeoffScore maps a weapon onto the three axes (higher = better)", () => {
    expect(tradeoffScore(greatsword)).toEqual({ reach: 3, speed: -6, damage: 10 });
  });

  it("a balanced roster has no Pareto-dominant weapon", () => {
    expect(paretoDominations([dagger, spear, greatsword])).toEqual([]);
    expect(r4Holds([dagger, spear, greatsword])).toBe(true);
  });

  it("flags a weapon that is better-or-equal on all three axes (and strictly better on one)", () => {
    // 'ubersword': spear's reach AND faster AND more damage than the spear → dominates it.
    const ubersword = weapon({ id: "ubersword", maxRange: 5, startupDelta: -3, damageDelta: 1 });
    const doms = paretoDominations([dagger, spear, greatsword, ubersword]);
    expect(doms.some((d) => d.dominator === "ubersword" && d.dominated === "spear")).toBe(true);
    expect(r4Holds([dagger, spear, greatsword, ubersword])).toBe(false);
  });
});

describe("meetsRequirements — gates are floors (R-3)", () => {
  it("checks STR/DEX thresholds", () => {
    const greatswordReq = { str: 3 };
    expect(meetsRequirements(greatswordReq, 3, 0)).toBe(true);
    expect(meetsRequirements(greatswordReq, 2, 0)).toBe(false);
    expect(meetsRequirements({}, 0, 0)).toBe(true);
  });
});
