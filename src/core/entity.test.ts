import { describe, it, expect } from "vitest";
import { fromInt } from "./fixed";
import { type FrameProfile } from "./frameprofile";
import {
  type EntityState,
  type MoveInstance,
  type Resources,
  type Entity,
  phaseAt,
  moveReadyTick,
  entityStateTag,
  stateMove,
  isActionable,
} from "./entity";

const heavyCleaveProfile: FrameProfile = {
  timing: { startup: 14, active: 3, recovery: 18 }, // total 35; active window [14,16]
  hitEffect: {
    damage: 40,
    hitstun: 23,
    blockstun: 12,
    chipDamage: 4,
    knockback: fromInt(3),
    launches: false,
  },
  properties: [],
  level: "MID",
  reach: {
    minRange: fromInt(0),
    maxRange: fromInt(4),
    heightLow: fromInt(0),
    heightHigh: fromInt(2),
    advance: fromInt(1),
    lateralBand: fromInt(1),
    stepIn: fromInt(0),
    trackSide: 0,
  },
};

const move: MoveInstance = {
  moveId: "heavy_cleave",
  profile: heavyCleaveProfile,
  startTick: 100, // start at an arbitrary non-zero T to exercise the elapsed math
};

const resources: Resources = {
  hp: 100,
  hpMax: 100,
  stamina: 50,
  staminaMax: 50,
  poise: 30,
  poiseMax: 30,
  focus: 10,
  focusMax: 10,
  ap: 5,
  apMax: 5,
};

describe("phaseAt — STARTUP→ACTIVE→RECOVERY→DONE from elapsed = T − startTick", () => {
  it("boundaries are correct for Heavy Cleave (st14 a3 r18, start at T=100)", () => {
    expect(phaseAt(move, 100)).toBe("STARTUP"); // elapsed 0
    expect(phaseAt(move, 113)).toBe("STARTUP"); // elapsed 13 (last startup frame)
    expect(phaseAt(move, 114)).toBe("ACTIVE"); // elapsed 14 (first active frame)
    expect(phaseAt(move, 116)).toBe("ACTIVE"); // elapsed 16 (last active frame)
    expect(phaseAt(move, 117)).toBe("RECOVERY"); // elapsed 17
    expect(phaseAt(move, 134)).toBe("RECOVERY"); // elapsed 34 (last frame)
    expect(phaseAt(move, 135)).toBe("DONE"); // elapsed 35 → actionable
    expect(phaseAt(move, 200)).toBe("DONE");
  });

  it("moveReadyTick = startTick + total", () => {
    expect(moveReadyTick(move)).toBe(135);
  });
});

describe("EntityState — tagged union, exhaustive accessors (decision 11)", () => {
  const states: readonly EntityState[] = [
    { kind: "NEUTRAL" },
    { kind: "STARTUP", move },
    { kind: "ACTIVE", move },
    { kind: "RECOVERY", move },
    { kind: "HITSTUN", until: 150 },
    { kind: "BLOCKSTUN", until: 140 },
    { kind: "AIRBORNE", until: 160 },
    { kind: "DOWN", wakeupTick: 180 },
    { kind: "GUARDBROKEN", until: 170 },
  ];

  it("entityStateTag returns the state's name for every variant", () => {
    expect(states.map(entityStateTag)).toEqual([
      "NEUTRAL",
      "STARTUP",
      "ACTIVE",
      "RECOVERY",
      "HITSTUN",
      "BLOCKSTUN",
      "AIRBORNE",
      "DOWN",
      "GUARDBROKEN",
    ]);
  });

  it("stateMove returns the in-flight move only for the move-execution states", () => {
    expect(stateMove({ kind: "STARTUP", move })).toBe(move);
    expect(stateMove({ kind: "ACTIVE", move })).toBe(move);
    expect(stateMove({ kind: "RECOVERY", move })).toBe(move);
    expect(stateMove({ kind: "NEUTRAL" })).toBeNull();
    expect(stateMove({ kind: "HITSTUN", until: 150 })).toBeNull();
    expect(stateMove({ kind: "DOWN", wakeupTick: 180 })).toBeNull();
  });
});

describe("isActionable — keyed off ready_tick (spec §0.4)", () => {
  it("becomes actionable at ready_tick", () => {
    const e: Entity = {
      id: "reza",
      state: { kind: "NEUTRAL" },
      readyTick: 135,
      resources,
      spatial: { pos: fromInt(0), offset: fromInt(0), height: fromInt(1), facing: 1 },
    };
    expect(isActionable(e, 134)).toBe(false);
    expect(isActionable(e, 135)).toBe(true);
    expect(isActionable(e, 200)).toBe(true);
  });
});
