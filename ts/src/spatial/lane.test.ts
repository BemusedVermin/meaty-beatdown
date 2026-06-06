import { describe, it, expect } from "vitest";
import { fromInt, fromRatio } from "../core/fixed";
import { type SpatialState, type ReachProfile } from "../core/spatial-types";
import { type InvulnType } from "../core/frameprofile";
import {
  doesHit,
  laneDistance,
  lateralGap,
  inLaneRange,
  inHeightBand,
  inLateralBand,
  attackTypeOf,
} from "./lane";

const spatial = (over: Partial<SpatialState> = {}): SpatialState => ({
  pos: fromInt(0),
  offset: fromInt(0),
  height: fromInt(1),
  facing: 1,
  ...over,
});

const reach = (over: Partial<ReachProfile> = {}): ReachProfile => ({
  minRange: fromInt(0),
  maxRange: fromInt(3),
  heightLow: fromInt(0),
  heightHigh: fromInt(2),
  advance: fromInt(0),
  lateralBand: fromRatio(1, 2), // 0.5
  stepIn: fromInt(0), // LINEAR by default
  trackSide: 0,
  ...over,
});

const invuln = (...types: InvulnType[]): ReadonlySet<InvulnType> => new Set(types);

describe("fixed-point spacing — exact raw values", () => {
  it("laneDistance applies advance in the facing direction (facing +1)", () => {
    // attacker pos 0, advance 0.5 → effective pos 32768; defender pos 3 (196608); dist = 2.5 (163840).
    const a = spatial({ pos: fromInt(0), facing: 1 });
    const d = spatial({ pos: fromInt(3) });
    const r = reach({ advance: fromRatio(1, 2) });
    expect(laneDistance(a, d, r) as number).toBe(163840);
  });

  it("laneDistance is symmetric under facing −1", () => {
    // attacker pos 5 (327680) facing −1, advance 0.5 → effective 294912; defender pos 2 (131072); dist 2.5.
    const a = spatial({ pos: fromInt(5), facing: -1 });
    const d = spatial({ pos: fromInt(2) });
    const r = reach({ advance: fromRatio(1, 2) });
    expect(laneDistance(a, d, r) as number).toBe(163840);
  });

  it("lateralGap is the exact absolute offset difference", () => {
    const a = spatial({ offset: fromRatio(1, 4) }); // 0.25 → 16384
    const d = spatial({ offset: fromInt(1) }); // 65536
    expect(lateralGap(a, d) as number).toBe(49152); // 0.75
  });
});

describe("range whiff (spec §1.2)", () => {
  it("connects inside [minRange, maxRange] and whiffs beyond it", () => {
    const a = spatial({ pos: fromInt(0), facing: 1 });
    const r = reach({ minRange: fromInt(0), maxRange: fromInt(2), advance: fromInt(0) });
    expect(inLaneRange(a, spatial({ pos: fromInt(2) }), r)).toBe(true); // dist 2 == maxRange
    expect(inLaneRange(a, spatial({ pos: fromRatio(5, 2) }), r)).toBe(false); // dist 2.5 > 2 → whiff
    expect(doesHit(a, spatial({ pos: fromRatio(5, 2) }), r, "MID")).toBe(false);
  });

  it("whiffs when the defender is closer than minRange (too close / over the head)", () => {
    const a = spatial({ pos: fromInt(0), facing: 1 });
    const r = reach({ minRange: fromInt(1), maxRange: fromInt(3) });
    expect(inLaneRange(a, spatial({ pos: fromRatio(1, 2) }), r)).toBe(false); // dist 0.5 < 1
    expect(doesHit(a, spatial({ pos: fromRatio(1, 2) }), r, "MID")).toBe(false);
  });
});

describe("height miss (spec §1.2)", () => {
  it("whiffs when defender height is outside [heightLow, heightHigh]", () => {
    const r = reach({ heightLow: fromInt(1), heightHigh: fromInt(2) });
    expect(inHeightBand(spatial({ height: fromRatio(1, 2) }), r)).toBe(false); // 0.5 below low
    expect(inHeightBand(spatial({ height: fromInt(1) }), r)).toBe(true);
    expect(doesHit(spatial(), spatial({ pos: fromInt(1), height: fromInt(3) }), r, "MID")).toBe(
      false,
    );
  });
});

describe("sidestep dodges LINEAR; tracking/homing realign (spec §1.1, §2.6; audit C-8/C-11)", () => {
  it("a LINEAR move whiffs once the defender steps beyond the lateral band", () => {
    const a = spatial({ offset: fromInt(0) });
    const onAxis = spatial({ pos: fromInt(1), offset: fromRatio(1, 4) }); // gap 0.25 ≤ 0.5
    const stepped = spatial({ pos: fromInt(1), offset: fromInt(1) }); // gap 1.0 > 0.5
    const linear = reach({ stepIn: fromInt(0), trackSide: 0 });
    expect(doesHit(a, onAxis, linear, "MID")).toBe(true);
    expect(doesHit(a, stepped, linear, "MID")).toBe(false); // sidestep dodged the linear move
  });

  it("a HOMING move (trackSide 0, large stepIn) realigns through the sidestep on both sides", () => {
    const a = spatial({ offset: fromInt(0) });
    const homing = reach({ stepIn: fromInt(2), trackSide: 0 }); // allowance 0.5 + 2 = 2.5
    expect(doesHit(a, spatial({ pos: fromInt(1), offset: fromInt(1) }), homing, "MID")).toBe(true);
    expect(doesHit(a, spatial({ pos: fromInt(1), offset: fromInt(-1) }), homing, "MID")).toBe(true);
  });

  it("track_side asymmetry: a TRACKING move covers one side and is dodged on the other", () => {
    const a = spatial({ offset: fromInt(0) });
    const tracksRight = reach({ stepIn: fromInt(1), trackSide: 1 }); // covered side allowance 1.5
    const steppedRight = spatial({ pos: fromInt(1), offset: fromInt(1) }); // side +1 → covered
    const steppedLeft = spatial({ pos: fromInt(1), offset: fromInt(-1) }); // side −1 → uncovered
    expect(inLateralBand(a, steppedRight, tracksRight)).toBe(true);
    expect(inLateralBand(a, steppedLeft, tracksRight)).toBe(false);
    expect(doesHit(a, steppedRight, tracksRight, "MID")).toBe(true);
    expect(doesHit(a, steppedLeft, tracksRight, "MID")).toBe(false);
  });
});

describe("throws ignore offset (decision: throws beat sidestep — spec §2.6)", () => {
  it("a throw connects regardless of how far the defender stepped, while a strike whiffs", () => {
    const a = spatial({ offset: fromInt(0), facing: 1 });
    const wayOffAxis = spatial({ pos: fromInt(1), offset: fromInt(10) }); // gap 10 ≫ band
    const throwReach = reach({ minRange: fromInt(0), maxRange: fromInt(2), lateralBand: fromRatio(1, 2) });
    expect(attackTypeOf("THROW")).toBe("THROW");
    expect(doesHit(a, wayOffAxis, throwReach, "THROW")).toBe(true); // throw ignores the sidestep
    expect(doesHit(a, wayOffAxis, throwReach, "MID")).toBe(false); // same spacing, a strike whiffs
  });

  it("a throw still whiffs if out of lane range (throws are short-range)", () => {
    const a = spatial({ facing: 1 });
    const r = reach({ minRange: fromInt(0), maxRange: fromInt(1) });
    expect(doesHit(a, spatial({ pos: fromInt(3), offset: fromInt(5) }), r, "THROW")).toBe(false);
  });
});

describe("typed invincibility (spec §0.3, §2.4)", () => {
  it("STRIKE invuln stops a strike but not a throw; THROW invuln the reverse", () => {
    const a = spatial({ facing: 1 });
    const d = spatial({ pos: fromInt(1) });
    const r = reach();
    expect(doesHit(a, d, r, "MID", invuln("STRIKE"))).toBe(false);
    expect(doesHit(a, d, r, "THROW", invuln("STRIKE"))).toBe(true);
    expect(doesHit(a, d, r, "THROW", invuln("THROW"))).toBe(false);
    expect(doesHit(a, d, r, "MID", invuln("THROW"))).toBe(true);
  });

  it("ALL invuln stops everything", () => {
    const a = spatial({ facing: 1 });
    const d = spatial({ pos: fromInt(1) });
    const r = reach();
    expect(doesHit(a, d, r, "MID", invuln("ALL"))).toBe(false);
    expect(doesHit(a, d, r, "THROW", invuln("ALL"))).toBe(false);
  });
});
