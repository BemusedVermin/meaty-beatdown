import { describe, it, expect } from "vitest";
import { fromInt } from "./fixed";
import { type MoveLevel, type InvulnType } from "./frameprofile";
import {
  type AttackContext,
  type DefenderContext,
  classifyContact,
  counterHitDamage,
  counterHitHitstun,
  juggleScaledDamage,
} from "./resolver";

const strike = (level: MoveLevel = "MID"): AttackContext => ({ type: "STRIKE", level });
const throwAtk: AttackContext = { type: "THROW", level: "THROW" };

const def = (o: Partial<DefenderContext> = {}): DefenderContext => ({
  invulnTo: new Set<InvulnType>(),
  guardPointActive: false,
  blockCovers: null,
  armorRemaining: 0,
  armorDamageMult: fromInt(1),
  throwTeching: false,
  counterHitState: false,
  ...o,
});

describe("classifyContact — interaction priority invuln > parry > block > armor > hit (spec §2.4)", () => {
  it("i-frames win over everything (even with parry/block/armor present)", () => {
    const d = def({
      invulnTo: new Set<InvulnType>(["STRIKE"]),
      guardPointActive: true,
      blockCovers: ["MID"],
      armorRemaining: 5,
    });
    expect(classifyContact(strike(), d)).toEqual({ kind: "WHIFF" });
  });

  it("parry beats block and armor", () => {
    const d = def({ guardPointActive: true, blockCovers: ["MID"], armorRemaining: 5 });
    expect(classifyContact(strike(), d)).toEqual({ kind: "PARRIED" });
  });

  it("block beats armor; a covered level is BLOCKED", () => {
    const d = def({ blockCovers: ["HIGH", "MID"], armorRemaining: 5 });
    expect(classifyContact(strike("MID"), d)).toEqual({ kind: "BLOCKED" });
  });

  it("an UNCOVERED level is the mixup landing — a clean hit", () => {
    const d = def({ blockCovers: ["HIGH", "MID"] }); // standing block does not cover LOW
    expect(classifyContact(strike("LOW"), d)).toEqual({ kind: "HIT", counter: false });
  });

  it("armor absorbs a strike when nothing higher applies", () => {
    expect(classifyContact(strike(), def({ armorRemaining: 1 }))).toEqual({ kind: "ARMORED" });
    expect(classifyContact(strike(), def({ armorRemaining: 0 }))).toEqual({
      kind: "HIT",
      counter: false,
    });
  });

  it("a strike vs a counter-hit-state defender is a COUNTER hit", () => {
    expect(classifyContact(strike(), def({ counterHitState: true }))).toEqual({
      kind: "HIT",
      counter: true,
    });
  });
});

describe("throws resolve separately (spec §2.6; decision 1: throws beat armor)", () => {
  it("a throw beats armor (THROWN, not ARMORED)", () => {
    expect(classifyContact(throwAtk, def({ armorRemaining: 99 }))).toEqual({ kind: "THROWN" });
  });

  it("a throw beats block and parry", () => {
    expect(classifyContact(throwAtk, def({ blockCovers: ["MID", "HIGH"] }))).toEqual({
      kind: "THROWN",
    });
    expect(classifyContact(throwAtk, def({ guardPointActive: true }))).toEqual({ kind: "THROWN" });
  });

  it("two throws on the same tick clash (THROW_TECH)", () => {
    expect(classifyContact(throwAtk, def({ throwTeching: true }))).toEqual({ kind: "THROW_TECH" });
  });

  it("throw invuln stops a throw", () => {
    expect(classifyContact(throwAtk, def({ invulnTo: new Set<InvulnType>(["THROW"]) }))).toEqual({
      kind: "WHIFF",
    });
  });
});

describe("damage scaling — fixed-point, floored (spec §2.7, §2.8)", () => {
  it("counter-hit ×1.25 (rounded half-up) and +6 hitstun", () => {
    expect(counterHitDamage(10)).toBe(13); // 12.5 → 13
    expect(counterHitDamage(100)).toBe(125);
    expect(counterHitHitstun(9)).toBe(15);
  });

  it("juggle decay 0.9^n (rounded half-up) so combos terminate", () => {
    expect(juggleScaledDamage(100, 0)).toBe(100);
    expect(juggleScaledDamage(100, 1)).toBe(90);
    expect(juggleScaledDamage(100, 2)).toBe(81);
    expect(juggleScaledDamage(100, 3)).toBe(73); // 72.9 → 73
    // strictly decreasing → combos terminate
    const seq = [0, 1, 2, 3, 4, 5].map((n) => juggleScaledDamage(100, n));
    for (let i = 1; i < seq.length; i++) expect(seq[i]!).toBeLessThan(seq[i - 1]!);
  });
});
