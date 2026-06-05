/**
 * compiler.ts — stats + equipment → resolved FrameProfile / Resources / MoveList [L4].
 *
 * THE SINGLE L4→engine bridge (spec App. A; audit C-5): the ONLY file in rpg/ that may import core/,
 * spatial/, or moves/. dependency-cruiser fails the build if any other rpg/ file does. "Stats and
 * equipment are compilers that emit frame data; the engine is an interpreter that runs it" (spec L0
 * intro) — so the engine never sees a stat, only the resolved FrameProfile this module produces.
 *
 * Every curve constant comes from CONFIG.rpg (the playtest-tuned tables). R-2: each attribute drives
 * one lever. R-3: bonuses for exceeding a requirement are CAPPED. Advantage stays derived (I-1): we
 * only ever set startup/active/recovery + hitstun/blockstun, never on_hit/on_block.
 */
import { fromInt, add } from "../core/fixed";
import {
  type FrameProfile,
  type Property,
  type HitEffect,
  type Timing,
} from "../core/frameprofile";
import { type ReachProfile } from "../core/spatial-types";
import { type Resources } from "../core/entity";
import { CONFIG } from "../core/config";
import { type Move, type MoveClass, type MoveList } from "../moves/move";
import { type Sheet, tempoMod } from "./sheet";
import { type Weapon, meetsRequirements } from "./equipment";

const R = CONFIG.rpg;

/** AP tiers cleared by a tempo modifier (decision 5 curve; CONFIG.ap.TEMPO_TIER_THRESHOLDS). */
export function tempoTier(tm: number): number {
  return CONFIG.ap.TEMPO_TIER_THRESHOLDS.filter((threshold) => tm >= threshold).length;
}

/** AP_max = AP_BASE + tempoTier(tempoMod) (decision 5). */
export function apMaxFor(sheet: Sheet): number {
  return CONFIG.ap.AP_BASE + tempoTier(tempoMod(sheet.attributes));
}

/** Compile the entity's resource pools from attributes (spec §4.2). Pools start full. */
export function compileResources(sheet: Sheet): Resources {
  const a = sheet.attributes;
  const hpMax = R.BASE_HP + a.con * R.CON_HP_PER_MOD;
  const staminaMax = R.BASE_STAMINA + a.con * R.CON_STAMINA_PER_MOD;
  const poiseMax = R.BASE_POISE + a.con * R.CON_POISE_PER_MOD;
  const focusMax = R.BASE_FOCUS + a.int * R.INT_FOCUS_PER_MOD;
  const apMax = apMaxFor(sheet);
  return {
    hp: hpMax,
    hpMax,
    stamina: staminaMax,
    staminaMax,
    poise: poiseMax,
    poiseMax,
    focus: focusMax,
    focusMax,
    ap: apMax,
    apMax,
  };
}

/** Whether the wielder can use a weapon at all (R-3: a floor, not a multiplier). */
export function canUseWeapon(sheet: Sheet, weapon: Weapon): boolean {
  return meetsRequirements(weapon.requirements, sheet.attributes.str, sheet.attributes.dex);
}

const clampLow = (n: number, lo: number): number => (n < lo ? lo : n);

/** STR damage bonus applies to heavies and throws only (R-2: STR's offensive lever). */
function strDamageBonus(moveClass: MoveClass, level: FrameProfile["level"], str: number): number {
  const applies = moveClass === "HEAVY" || moveClass === "THROW" || level === "THROW";
  return applies ? clampLow(str, 0) * R.STR_DAMAGE_PER_MOD : 0;
}

function compileReach(base: ReachProfile, weapon: Weapon, dex: number): ReachProfile {
  // Weapon = spacing identity: it sets the lane range. DEX adds reach advance (R-2: DEX movement lever).
  return {
    ...base,
    minRange: fromInt(weapon.minRange),
    maxRange: fromInt(weapon.maxRange),
    advance: add(base.advance, fromInt(clampLow(dex, 0) * R.DEX_ADVANCE_PER_MOD)),
  };
}

function compileProperties(base: readonly Property[], str: number): readonly Property[] {
  const armorBonus = Math.min(clampLow(str, 0) * R.STR_ARMOR_HITS_PER_MOD, R.STR_ARMOR_HITS_CAP);
  return base.map((p) => (p.kind === "ARMOR" ? { ...p, armorHits: p.armorHits + armorBonus } : p));
}

/**
 * Compile a base FrameProfile against a sheet + weapon into the resolved profile the engine runs.
 * DEX lowers startup (capped); the weapon shifts startup/recovery/damage/range; STR adds damage to
 * heavies/throws and armor budget. Hitstun/blockstun are untouched, so advantage stays I-1-consistent.
 */
export function compileProfile(
  base: FrameProfile,
  moveClass: MoveClass,
  sheet: Sheet,
  weapon: Weapon,
): FrameProfile {
  const a = sheet.attributes;
  const dexReduction = Math.min(
    clampLow(a.dex, 0) * R.DEX_STARTUP_REDUCTION_PER_MOD,
    R.DEX_STARTUP_REDUCTION_CAP,
  );
  const timing: Timing = {
    startup: clampLow(base.timing.startup - dexReduction + weapon.startupDelta, R.MIN_STARTUP),
    active: base.timing.active,
    recovery: clampLow(base.timing.recovery + weapon.recoveryDelta, 0),
  };
  const hitEffect: HitEffect = {
    ...base.hitEffect,
    damage: clampLow(
      base.hitEffect.damage + strDamageBonus(moveClass, base.level, a.str) + weapon.damageDelta,
      0,
    ),
  };
  return {
    ...base,
    timing,
    hitEffect,
    reach: compileReach(base.reach, weapon, a.dex),
    properties: compileProperties(base.properties, a.str),
  };
}

/** Compile a base Move (carrying its class) into a resolved Move. */
export function compileMove(base: Move, sheet: Sheet, weapon: Weapon): Move {
  return { ...base, profile: compileProfile(base.profile, base.moveClass, sheet, weapon) };
}

/**
 * Compile a base MoveList for a wielder + weapon. If the weapon's requirements are unmet the wielder
 * cannot use it at all (R-3) → empty list. Otherwise every move is compiled to its resolved form.
 */
export function compileMoveList(base: MoveList, sheet: Sheet, weapon: Weapon): MoveList {
  if (!canUseWeapon(sheet, weapon)) return [];
  return base.map((m) => compileMove(m, sheet, weapon));
}
