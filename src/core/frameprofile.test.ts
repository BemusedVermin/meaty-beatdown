import { describe, it, expect } from "vitest";
import { fromInt } from "./fixed";
import {
  type FrameProfile,
  type Property,
  totalFrames,
  onHit,
  onBlock,
  onWhiff,
  windowContains,
  propertyWindow,
  isPropertyActive,
  checkFrameProfile,
} from "./frameprofile";
import { makeProfile } from "../test-support/fixtures";

describe("timing", () => {
  it("totalFrames = startup + active + recovery", () => {
    expect(totalFrames({ startup: 4, active: 2, recovery: 6 })).toBe(12);
    expect(totalFrames({ startup: 14, active: 3, recovery: 18 })).toBe(35);
  });
});

describe("invariant I-1 — advantage is DERIVED from stun − recovery, never stored", () => {
  it("there are no on_hit/on_block fields to set (compile-time enforcement)", () => {
    const fp = makeProfile();
    // @ts-expect-error — FrameProfile has no `on_hit` field; advantage is computed, not stored.
    void fp.on_hit;
  });

  it("reproduces the worked example: Light Slash (st4 a2 r6) → on_hit +3, on_block −1", () => {
    // I-1: on_hit = hitstun − recovery = 9 − 6 = +3; on_block = blockstun − recovery = 5 − 6 = −1.
    const lightSlash = makeProfile({
      timing: { recovery: 6 },
      hitEffect: { hitstun: 9, blockstun: 5 },
    });
    expect(onHit(lightSlash)).toBe(3);
    expect(onBlock(lightSlash)).toBe(-1);
    expect(onWhiff()).toBe(0);
  });

  it("reproduces the worked example: Heavy Cleave (st14 a3 r18) → on_hit +5, on_block −6", () => {
    const heavyCleave = makeProfile({
      timing: { startup: 14, active: 3, recovery: 18 },
      hitEffect: { hitstun: 23, blockstun: 12 },
    });
    expect(onHit(heavyCleave)).toBe(5);
    expect(onBlock(heavyCleave)).toBe(-6);
  });

  it("holds across generated profiles", () => {
    for (const recovery of [0, 5, 10, 18, 25]) {
      for (const hitstun of [0, 1, 5, 9, 23, 40]) {
        for (const blockstun of [0, 5, 12, 30]) {
          const fp = makeProfile({ timing: { recovery }, hitEffect: { hitstun, blockstun } });
          expect(onHit(fp)).toBe(hitstun - recovery);
          expect(onBlock(fp)).toBe(blockstun - recovery);
        }
      }
    }
  });
});

describe("property windows resolve at the correct ticks", () => {
  it("windowContains is inclusive on both ends", () => {
    const w = { from: 4, to: 14 };
    expect(windowContains(w, 3)).toBe(false);
    expect(windowContains(w, 4)).toBe(true);
    expect(windowContains(w, 9)).toBe(true);
    expect(windowContains(w, 14)).toBe(true);
    expect(windowContains(w, 15)).toBe(false);
  });

  it("Heavy Cleave armor[4..14] is live exactly on elapsed 4..14", () => {
    const armor: Property = {
      kind: "ARMOR",
      armorHits: 4,
      armorDamageMult: fromInt(1),
      window: { from: 4, to: 14 },
    };
    expect(propertyWindow(armor)).toEqual({ from: 4, to: 14 });
    expect(isPropertyActive(armor, 3)).toBe(false);
    expect(isPropertyActive(armor, 4)).toBe(true);
    expect(isPropertyActive(armor, 14)).toBe(true);
    expect(isPropertyActive(armor, 15)).toBe(false);
  });

  it("each property variant exposes its window via the exhaustive accessor", () => {
    const props: readonly Property[] = [
      { kind: "INVULN", invulnType: "STRIKE", window: { from: 0, to: 3 } },
      { kind: "ARMOR", armorHits: 1, armorDamageMult: fromInt(1), window: { from: 1, to: 2 } },
      { kind: "COUNTER_HIT_STATE", window: { from: 0, to: 1 } },
      { kind: "GUARD_POINT", window: { from: 2, to: 5 } },
      { kind: "BLOCK", covers: ["HIGH", "MID"], window: { from: 0, to: 30 } },
      { kind: "AIRBORNE", window: { from: 0, to: 9 } },
      { kind: "PROJECTILE_SPAWN", window: { from: 3, to: 3 } },
    ];
    for (const p of props) {
      expect(propertyWindow(p)).toBe(p.window);
    }
  });
});

describe("checkFrameProfile — surrounding consistency invariants", () => {
  it("passes a well-formed profile", () => {
    const fp = makeProfile({
      timing: { startup: 14, active: 3, recovery: 18 },
      properties: [
        { kind: "ARMOR", armorHits: 4, armorDamageMult: fromInt(1), window: { from: 4, to: 14 } },
      ],
    });
    expect(checkFrameProfile(fp)).toEqual([]);
  });

  it("flags a property window that exceeds the move's frame span", () => {
    const fp = makeProfile({
      timing: { startup: 4, active: 2, recovery: 6 }, // total 12 → last index 11
      properties: [{ kind: "AIRBORNE", window: { from: 4, to: 20 } }],
    });
    const problems = checkFrameProfile(fp);
    expect(problems.some((p) => p.includes("exceeds last frame index 11"))).toBe(true);
  });

  it("flags non-positive active and inverted windows", () => {
    const fp = makeProfile({
      timing: { startup: 4, active: 0, recovery: 6 },
      properties: [{ kind: "AIRBORNE", window: { from: 5, to: 2 } }],
    });
    const problems = checkFrameProfile(fp);
    expect(problems.some((p) => p.includes("active must be ≥ 1"))).toBe(true);
    expect(problems.some((p) => p.includes("must be ≥ from"))).toBe(true);
  });

  it("flags an inverted reach band", () => {
    const fp: FrameProfile = {
      ...makeProfile(),
      reach: { ...makeProfile().reach, minRange: fromInt(3), maxRange: fromInt(1) },
    };
    expect(checkFrameProfile(fp).some((p) => p.includes("minRange must be ≤ maxRange"))).toBe(true);
  });
});
