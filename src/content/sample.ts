/**
 * sample.ts — swappable sample content: two archetypes (DEX-frail dagger vs STR-armored greatsword),
 * three weapons, and ~12 base moves spanning the worked examples (spec "Worked example", §3.2) [content].
 *
 * This is DATA the engine never sees directly. The CLI compiles/uses it; the balance audit lints it.
 * Authored to satisfy the balance rules: every move pays a resource (R-1), no net-positive AP cycle
 * (R-5), the weapon roster has no Pareto-dominant weapon (R-4).
 */
import { fromInt, fromRatio } from "../core/fixed";
import { type CancelWindow } from "../core/cost";
import { type Move, type MoveList } from "../moves/move";
import { type Weapon } from "../rpg/equipment";
import { type Sheet } from "../rpg/sheet";
import { mv, cost } from "./builders";

const onHitInto = (into: readonly string[], from: number, to: number, focus: number): CancelWindow => ({
  from,
  to,
  gate: "ON_HIT",
  into,
  cost: cost({ focus }),
});

// --- Reza: DEX dagger, fast and frail ------------------------------------------------------------

export const REZA_MOVES: MoveList = [
  mv("light_jab", "Light Jab", "LIGHT", {
    timing: { startup: 3, active: 2, recovery: 5 },
    hitEffect: { damage: 8, hitstun: 8, blockstun: 4, knockback: fromInt(1) },
    // LINEAR: a narrow lateral band (< a 1-unit sidestep) so a sidestep dodges it (spec §1.1, §2.6).
    reach: { maxRange: fromInt(2), lateralBand: fromRatio(1, 2) },
    cost: { stamina: 4, ap: 1, apGain: { amount: 1, gate: "ON_HIT" } },
    cancelWindows: [onHitInto(["light_slash"], 5, 9, 2)],
  }),
  mv("light_slash", "Light Slash", "LIGHT", {
    timing: { startup: 4, active: 2, recovery: 6 }, // on_hit +3, on_block −1 (worked example)
    hitEffect: { damage: 10, hitstun: 9, blockstun: 5, knockback: fromInt(1) },
    reach: { maxRange: fromInt(2), lateralBand: fromRatio(1, 2) }, // LINEAR
    cost: { stamina: 5, ap: 1, apGain: { amount: 1, gate: "ON_HIT" } },
    cancelWindows: [onHitInto(["special_riposte"], 6, 11, 3)],
  }),
  mv("special_riposte", "Riposte", "SPECIAL", {
    timing: { startup: 6, active: 3, recovery: 10 },
    hitEffect: { damage: 20, hitstun: 18, knockback: fromInt(2), launches: true },
    reach: { maxRange: fromInt(2) },
    cost: { focus: 5, ap: 2 },
  }),
  mv("throw_grab", "Grab", "THROW", {
    timing: { startup: 5, active: 2, recovery: 8 },
    level: "THROW",
    hitEffect: { damage: 18, hitstun: 20, knockdown: true },
    reach: { maxRange: fromInt(1) }, // throws are short-range
    cost: { stamina: 8, ap: 1 },
  }),
  mv("backdash", "Backdash", "MOVEMENT", {
    timing: { startup: 3, active: 2, recovery: 8 },
    reach: { maxRange: fromInt(0) },
    properties: [{ kind: "INVULN", invulnType: "STRIKE", window: { from: 0, to: 4 } }],
    motion: { lane: fromInt(-2), offset: fromInt(0) },
    cost: { stamina: 5 },
  }),
  mv("sidestep_l", "Sidestep L", "MOVEMENT", {
    timing: { startup: 3, active: 2, recovery: 7 },
    reach: { maxRange: fromInt(0) },
    motion: { lane: fromInt(0), offset: fromInt(-1) },
    cost: { stamina: 4 },
  }),
  mv("sidestep_r", "Sidestep R", "MOVEMENT", {
    timing: { startup: 3, active: 2, recovery: 7 },
    reach: { maxRange: fromInt(0) },
    motion: { lane: fromInt(0), offset: fromInt(1) },
    cost: { stamina: 4 },
  }),
];

// --- Borin: STR greatsword, slow and armored -----------------------------------------------------

export const BORIN_MOVES: MoveList = [
  mv("heavy_cleave", "Heavy Cleave", "HEAVY", {
    timing: { startup: 14, active: 3, recovery: 18 }, // on_hit +5, on_block −6 (worked example)
    hitEffect: { damage: 40, hitstun: 23, blockstun: 12, knockback: fromInt(3), knockdown: true },
    reach: { maxRange: fromInt(4), advance: fromInt(1) },
    properties: [
      { kind: "ARMOR", armorHits: 4, armorDamageMult: fromInt(1), window: { from: 4, to: 14 } },
    ],
    cost: { stamina: 12, ap: 3 },
  }),
  mv("tempo_jab", "Tempo Jab", "LIGHT", {
    timing: { startup: 5, active: 2, recovery: 6 },
    hitEffect: { damage: 8, hitstun: 10, blockstun: 4, knockback: fromInt(1) },
    reach: { maxRange: fromInt(3), lateralBand: fromRatio(1, 2) }, // LINEAR — sidesteppable
    cost: { stamina: 5, ap: 1, apGain: { amount: 2, gate: "ON_CH" } },
    cancelWindows: [onHitInto(["homing_sweep"], 7, 12, 4)],
  }),
  mv("homing_sweep", "Homing Sweep", "SPECIAL", {
    timing: { startup: 8, active: 3, recovery: 12 },
    hitEffect: { damage: 18, hitstun: 14, knockback: fromInt(2) },
    // HOMING: trackSide 0 + large stepIn realigns through a sidestep on both sides.
    reach: { maxRange: fromInt(3), stepIn: fromInt(2), trackSide: 0 },
    cost: { focus: 4, ap: 2 },
  }),
  mv("guard", "Guard", "MOVEMENT", {
    timing: { startup: 2, active: 20, recovery: 4 },
    reach: { maxRange: fromInt(0) },
    properties: [{ kind: "BLOCK", covers: ["HIGH", "MID"], window: { from: 0, to: 25 } }],
    cost: { stamina: 3 },
  }),
  mv("parry", "Parry", "SPECIAL", {
    timing: { startup: 2, active: 4, recovery: 12 },
    reach: { maxRange: fromInt(0) },
    properties: [{ kind: "GUARD_POINT", window: { from: 2, to: 5 } }],
    cost: { focus: 3, apGain: { amount: 2, gate: "ON_PARRY" } },
  }),
];

export const ALL_MOVES: MoveList = [...REZA_MOVES, ...BORIN_MOVES];

export const moveById = (id: string): Move => {
  const m = ALL_MOVES.find((x) => x.id === id);
  if (!m) throw new Error(`no sample move "${id}"`);
  return m;
};

// --- Weapons (R-4: no Pareto-dominant weapon) ----------------------------------------------------

export const WEAPONS: readonly Weapon[] = [
  {
    id: "dagger",
    weaponClass: "dagger",
    minRange: 0,
    maxRange: 2,
    startupDelta: -2,
    recoveryDelta: 0,
    damageDelta: -2,
    requirements: { dex: 2 },
    grantsMoves: REZA_MOVES.map((m) => m.id),
  },
  {
    id: "greatsword",
    weaponClass: "greatsword",
    minRange: 0,
    maxRange: 4,
    startupDelta: 4,
    recoveryDelta: 2,
    damageDelta: 8,
    requirements: { str: 3 },
    grantsMoves: BORIN_MOVES.map((m) => m.id),
  },
  {
    id: "spear",
    weaponClass: "spear",
    minRange: 1,
    maxRange: 5,
    startupDelta: 0,
    recoveryDelta: 0,
    damageDelta: 0,
    requirements: { dex: 1 },
    grantsMoves: [],
  },
];

// --- Archetypes ----------------------------------------------------------------------------------

export const REZA_SHEET: Sheet = {
  attributes: { str: 0, dex: 3, con: 1, int: 2, wis: 2, cha: 0 },
  skills: { dagger: 3 },
  foci: ["read_the_wind"],
};

export const BORIN_SHEET: Sheet = {
  attributes: { str: 3, dex: 0, con: 3, int: 0, wis: 1, cha: 0 },
  skills: { greatsword: 3 },
  foci: ["iron_guard"],
};
