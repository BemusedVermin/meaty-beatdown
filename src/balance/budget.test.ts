import { describe, it, expect } from "vitest";
import { type Move, type MoveList } from "../moves/move";
import { type Weapon } from "../rpg/equipment";
import {
  moveValue,
  budgetReport,
  checkR1,
  checkR2,
  checkR3,
  checkR4,
  checkR5,
} from "./budget";
import { makeProfile } from "../test-support/fixtures";

const move = (id: string, p: Parameters<typeof makeProfile>[0]): Move => ({
  id,
  name: id,
  moveClass: "LIGHT",
  profile: makeProfile(p),
});

describe("MOVE_VALUE — the budget identity (spec §4.5)", () => {
  it("a fast, safe move scores higher than a slow, unsafe one (before paying for it)", () => {
    const fastSafe = makeProfile({ timing: { startup: 3, recovery: 3 }, hitEffect: { blockstun: 4 } });
    const slowUnsafe = makeProfile({ timing: { startup: 16, recovery: 20 }, hitEffect: { blockstun: 4 } });
    expect(moveValue(fastSafe)).toBeGreaterThan(moveValue(slowUnsafe));
  });

  it("paying resources reduces a move's net value (it buys down its strengths)", () => {
    const free = makeProfile({ cost: {} });
    const costly = makeProfile({ cost: { stamina: 20, focus: 10, ap: 3 } });
    expect(moveValue(costly)).toBeLessThan(moveValue(free));
  });

  it("budgetReport flags an over-budget outlier", () => {
    const moves: MoveList = [
      move("a", { timing: { startup: 6 }, cost: { stamina: 5 } }),
      move("b", { timing: { startup: 6 }, cost: { stamina: 5 } }),
      move("nodownside", { timing: { startup: 1 }, hitEffect: { damage: 99, blockstun: 30 }, cost: {} }),
    ];
    expect(budgetReport(moves, 10).outliers).toContain("nodownside");
  });
});

describe("R-1 — no zero-cost dominant action (spec §3.1)", () => {
  it("passes a costed roster and fails a free one", () => {
    expect(checkR1([move("a", { cost: { stamina: 4 } })]).pass).toBe(true);
    const r = checkR1([move("free", { cost: {} })]);
    expect(r.pass).toBe(false);
    expect(r.detail).toContain("free");
  });
});

describe("R-2 / R-3 — lever assignment and capped floors", () => {
  it("R-2: no attribute drives both an offensive and a defensive major lever", () => {
    expect(checkR2().pass).toBe(true);
  });
  it("R-3: requirement bonuses are capped", () => {
    expect(checkR3().pass).toBe(true);
  });
});

describe("R-4 — weapon tradeoff triangle (spec §4.4)", () => {
  const w = (id: string, maxRange: number, startupDelta: number, damageDelta: number): Weapon => ({
    id,
    weaponClass: id,
    minRange: 0,
    maxRange,
    startupDelta,
    recoveryDelta: 0,
    damageDelta,
    requirements: {},
    grantsMoves: [],
  });
  it("passes a balanced roster, fails a Pareto-dominant one", () => {
    expect(checkR4([w("dagger", 1, -4, -3), w("spear", 5, -1, 0), w("gs", 3, 6, 10)]).pass).toBe(true);
    expect(checkR4([w("spear", 5, -1, 0), w("uber", 5, -2, 1)]).pass).toBe(false);
  });
});

describe("R-5 — no net-positive AP cycle (spec §3.5.3)", () => {
  const looping = (id: string, ap: number, gain: number): Move => ({
    id,
    name: id,
    moveClass: "SPECIAL",
    profile: makeProfile({
      cost: { ap, apGain: { amount: gain, gate: "ON_HIT" } },
      cancelWindows: [{ from: 0, to: 5, gate: "ON_HIT", into: [id], cost: { ap: 0, apGain: null, stamina: 0, focus: 0 } }],
    }),
  });
  it("passes a draining loop and fails a net-positive one", () => {
    expect(checkR5([looping("ok", 2, 1)]).pass).toBe(true);
    const bad = checkR5([looping("loop", 1, 2)]);
    expect(bad.pass).toBe(false);
    expect(bad.detail).toContain("loop");
  });
});
