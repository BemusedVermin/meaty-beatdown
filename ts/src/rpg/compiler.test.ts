import { describe, it, expect } from "vitest";
import { toInt } from "../core/fixed";
import { onHit, onBlock, checkFrameProfile } from "../core/frameprofile";
import { type Move } from "../moves/move";
import { type Sheet, type Attributes } from "./sheet";
import { type Weapon } from "./equipment";
import {
  compileProfile,
  compileMove,
  compileMoveList,
  compileResources,
  apMaxFor,
  tempoTier,
  canUseWeapon,
} from "./compiler";
import { makeProfile } from "../test-support/fixtures";

const attrs = (o: Partial<Attributes> = {}): Attributes => ({
  str: 0,
  dex: 0,
  con: 0,
  int: 0,
  wis: 0,
  cha: 0,
  ...o,
});
const sheet = (o: Partial<Attributes> = {}): Sheet => ({ attributes: attrs(o), skills: {}, foci: [] });
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

// Heavy Cleave base: st10 a3 r18, hitstun 23, blockstun 12, damage 40.
const heavyBase = makeProfile({
  timing: { startup: 10, active: 3, recovery: 18 },
  hitEffect: { damage: 40, hitstun: 23, blockstun: 12 },
});

describe("compileProfile keeps resolved profiles I-1-consistent (audit C-1)", () => {
  it("a compiled profile validates and its advantage is still derived from stun − recovery", () => {
    const greatsword = weapon({ startupDelta: 6, recoveryDelta: 0, damageDelta: 10, maxRange: 4 });
    const resolved = compileProfile(heavyBase, "HEAVY", sheet({ str: 3, dex: 2 }), greatsword);

    expect(checkFrameProfile(resolved)).toEqual([]);
    // I-1: on_hit = hitstun − recovery = 23 − 18; the compiler never set advantage by hand.
    expect(onHit(resolved)).toBe(resolved.hitEffect.hitstun - resolved.timing.recovery);
    expect(onHit(resolved)).toBe(5);
    expect(onBlock(resolved)).toBe(resolved.hitEffect.blockstun - resolved.timing.recovery);
  });

  it("DEX lowers startup (capped) and the weapon shifts it; floored at MIN_STARTUP", () => {
    const gs = weapon({ startupDelta: 6 });
    // dex 2 → −2; +6 weapon ⇒ 10 − 2 + 6 = 14.
    expect(compileProfile(heavyBase, "HEAVY", sheet({ dex: 2 }), gs).timing.startup).toBe(14);
    // dex 9 → reduction capped at 3 (R-3): 10 − 3 + 6 = 13.
    expect(compileProfile(heavyBase, "HEAVY", sheet({ dex: 9 }), gs).timing.startup).toBe(13);
  });

  it("STR adds damage to heavies/throws but not to lights, plus weapon damageDelta", () => {
    const gs = weapon({ damageDelta: 10 });
    const heavy = compileProfile(heavyBase, "HEAVY", sheet({ str: 3 }), gs);
    expect(heavy.hitEffect.damage).toBe(40 + 3 * 2 + 10); // STR_DAMAGE_PER_MOD = 2
    const light = compileProfile(heavyBase, "LIGHT", sheet({ str: 3 }), gs);
    expect(light.hitEffect.damage).toBe(40 + 0 + 10); // no STR bonus on a light
  });

  it("the weapon sets the lane range (spacing identity, §4.4)", () => {
    const spear = weapon({ minRange: 1, maxRange: 5 });
    const resolved = compileProfile(heavyBase, "HEAVY", sheet(), spear);
    expect(toInt(resolved.reach.minRange)).toBe(1);
    expect(toInt(resolved.reach.maxRange)).toBe(5);
  });
});

describe("compileResources — pools derived from attributes (spec §4.2)", () => {
  it("CON drives HP/Stamina/Poise, INT drives Focus, tempo drives AP", () => {
    const r = compileResources(sheet({ con: 2, int: 3, dex: 2, wis: 3 }));
    expect(r.hpMax).toBe(100 + 2 * 10);
    expect(r.staminaMax).toBe(50 + 2 * 5);
    expect(r.poiseMax).toBe(30 + 2 * 3);
    expect(r.focusMax).toBe(10 + 3 * 2);
    // tempoMod(dex2,wis3) = 3 → tempoTier([1,3,5]) = 2 → AP_max = AP_BASE(3) + 2 = 5.
    expect(tempoTier(3)).toBe(2);
    expect(apMaxFor(sheet({ dex: 2, wis: 3 }))).toBe(5);
    expect(r.apMax).toBe(5);
    expect(r.hp).toBe(r.hpMax); // pools start full
  });
});

describe("weapon gating (R-3) and MoveList compilation", () => {
  const baseMove: Move = { id: "cleave", name: "Cleave", moveClass: "HEAVY", profile: heavyBase };

  it("canUseWeapon enforces the requirement floor", () => {
    const gs = weapon({ requirements: { str: 3 } });
    expect(canUseWeapon(sheet({ str: 3 }), gs)).toBe(true);
    expect(canUseWeapon(sheet({ str: 2 }), gs)).toBe(false);
  });

  it("compileMoveList yields resolved moves, or nothing if the weapon is unusable", () => {
    const gs = weapon({ requirements: { str: 3 }, damageDelta: 10 });
    const usable = compileMoveList([baseMove], sheet({ str: 3 }), gs);
    expect(usable).toHaveLength(1);
    expect(usable[0]!.profile.hitEffect.damage).toBe(40 + 3 * 2 + 10);
    expect(compileMoveList([baseMove], sheet({ str: 2 }), gs)).toEqual([]);
  });

  it("compileMove preserves id/name/class", () => {
    const m = compileMove(baseMove, sheet({ dex: 1 }), weapon());
    expect(m.id).toBe("cleave");
    expect(m.name).toBe("Cleave");
    expect(m.moveClass).toBe("HEAVY");
  });
});
