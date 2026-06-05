/**
 * sheet.ts — attributes, skills, foci, and the tempo derivation (spec §4.2, §4.3) [L4].
 *
 * PURE DATA + derivations that need no engine. This file must NOT import core/, spatial/, or moves/ —
 * only rpg/compiler.ts (the single bridge) may. dependency-cruiser enforces this (audit C-5).
 *
 * Attributes are stored as WWN-style MODIFIERS (the low ≈ −2..+3 range that keeps frame-data swings
 * small). Each maps to exactly one major frame-data lever in the compiler (R-2, no double-dip).
 */

export type AttributeName = "str" | "dex" | "con" | "int" | "wis" | "cha";

export interface Attributes {
  readonly str: number; // damage on heavies/throws; armor budget; heavy-weapon gate; throw-tech
  readonly dex: number; // −startup on lights; +movement advance; fast-weapon gate; cancel windows
  readonly con: number; // +max HP / Stamina / Poise
  readonly int: number; // +max Focus; technique specials; −Focus cost on cancels
  readonly wis: number; // defensive reads: parry window, Focus refund on parry/CH, wakeup
  readonly cha: number; // feints/intimidate (content/foci); NOT the tempo stat (decision 5)
}

export interface Sheet {
  readonly attributes: Attributes;
  /** Per weapon-class proficiency, rank 0–4 (spec §4.3). Rank unlocks moves + improves frame data. */
  readonly skills: Readonly<Record<string, number>>;
  /** Build-defining unlocks — the modular content slots (spec §4.3). */
  readonly foci: readonly string[];
}

/**
 * Tempo modifier (decision 5): a derived blend of DEX and WIS, NOT a 7th attribute.
 *   tempoMod = roundHalfUp((dexMod + wisMod) / 2)
 * Implemented as the integer-equivalent floor((dex + wis + 1) / 2) so this file needs no core import;
 * it reproduces core/fixed.roundHalfUp exactly for the half-integer input (see fixed.ts).
 */
export function tempoMod(a: Attributes): number {
  return Math.floor((a.dex + a.wis + 1) / 2);
}

/** Weapon-class proficiency rank (0 if untrained). */
export function skillRank(sheet: Sheet, weaponClass: string): number {
  return sheet.skills[weaponClass] ?? 0;
}

/** Whether the sheet has a given build-defining Focus. */
export function hasFocus(sheet: Sheet, focusId: string): boolean {
  return sheet.foci.includes(focusId);
}
