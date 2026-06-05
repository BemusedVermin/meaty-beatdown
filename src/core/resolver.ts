/**
 * resolver.ts — resolve_contact: what happens when a hitbox meets a hurtbox (spec §2.4) [L2].
 *
 * `classifyContact` is the interaction-priority table as pure code. Read top to bottom it encodes
 * "invincibility > parry > block > armor > clean hit", with throws resolved separately (they ignore
 * block, beat parry/armor, and clash on throw-tech — spec §2.6; decision 1: throws beat armor).
 *
 * Damage scaling (counter-hit ×1.25, juggle ×0.9^n) is done in fixed-point then floored to an integer
 * (decision 10), so it is deterministic and reproducible across languages.
 */
import { type Fixed, mul, fromInt, toIntRound } from "./fixed";
import { type MoveLevel, type InvulnType, type AttackType } from "./frameprofile";
import { CONFIG } from "./config";

/** What the attacker is throwing at the defender. */
export interface AttackContext {
  readonly type: AttackType;
  readonly level: MoveLevel;
}

/** The defender's relevant state at the contact tick (computed by the engine from active properties). */
export interface DefenderContext {
  /** Typed invincibility currently active on the defender. */
  readonly invulnTo: ReadonlySet<InvulnType>;
  /** A parry (guard-point) window is live. */
  readonly guardPointActive: boolean;
  /** The defender is holding a block covering these levels, else null. */
  readonly blockCovers: readonly MoveLevel[] | null;
  /** Armor hits still available (> 0 ⇒ armor absorbs the strike). */
  readonly armorRemaining: number;
  /** Damage multiplier the active armor applies (used by the engine when ARMORED; ignored here). */
  readonly armorDamageMult: Fixed;
  /** The defender is ALSO actively throwing this tick → throw-tech clash. */
  readonly throwTeching: boolean;
  /** The defender is in a counter-hit state (own move's startup/recovery, or a CH-state window). */
  readonly counterHitState: boolean;
}

export type ContactResult =
  | { readonly kind: "WHIFF" } // i-frames win — no effect
  | { readonly kind: "PARRIED" } // attacker frozen, defender hugely plus
  | { readonly kind: "THROWN" } // throw connected (beats block/armor/parry)
  | { readonly kind: "THROW_TECH" } // both threw → clash, both reset, no damage
  | { readonly kind: "BLOCKED" } // chip + blockstun + on_block
  | { readonly kind: "ARMORED" } // armored: takes (reduced) damage, no hitstun, continues
  | { readonly kind: "HIT"; readonly counter: boolean }; // clean hit (counter = was in CH state)

/**
 * The ordered interaction-priority branch (spec §2.4). Precondition: the spatial `doesHit` predicate
 * already returned true (so range/height/lateral lined up); this decides the defender's outcome.
 */
export function classifyContact(att: AttackContext, def: DefenderContext): ContactResult {
  // i-frames win (doesHit usually filters this; classify defends in depth).
  if (def.invulnTo.has("ALL") || def.invulnTo.has(att.type)) return { kind: "WHIFF" };

  // Throws resolve separately: they ignore block and beat parry/armor (spec §2.6; decision 1).
  if (att.type === "THROW") {
    return def.throwTeching ? { kind: "THROW_TECH" } : { kind: "THROWN" };
  }

  // Parry beats strike.
  if (def.guardPointActive) return { kind: "PARRIED" };

  // Block: a covered level is blocked; an uncovered level is the mixup landing → clean hit.
  if (def.blockCovers !== null) {
    if (def.blockCovers.includes(att.level)) return { kind: "BLOCKED" };
    return { kind: "HIT", counter: def.counterHitState };
  }

  // Armor absorbs strikes (still takes damage, no hitstun).
  if (def.armorRemaining > 0) return { kind: "ARMORED" };

  // Clean hit (counter-hit if the defender was in a counter-hit state).
  return { kind: "HIT", counter: def.counterHitState };
}

// ---------------------------------------------------------------------------
// Damage scaling — fixed-point, rounded (half-up) to integer HP (decision 3 & 10)
// ---------------------------------------------------------------------------

/** Counter-hit damage: base × CH_DAMAGE_MULT (×1.25), rounded half-up (spec §2.7). */
export function counterHitDamage(baseDamage: number): number {
  return toIntRound(mul(fromInt(baseDamage), CONFIG.combat.CH_DAMAGE_MULT));
}

/** Counter-hit hitstun bonus in ticks (spec §2.7). */
export function counterHitHitstun(baseHitstun: number): number {
  return baseHitstun + CONFIG.combat.CH_HITSTUN_BONUS;
}

/**
 * Hitstun decay (combo governor 3, spec §3.4): each successive hit in a combo reduces the next hit's
 * effective hitstun, so the chained advantage eventually goes negative and the combo MUST end.
 * `comboCount` is 1 for the first hit (undecayed). Floored at MIN_HITSTUN so the hit still connects.
 */
export function effectiveHitstun(baseHitstun: number, comboCount: number): number {
  const decayed = baseHitstun - Math.max(0, comboCount - 1) * CONFIG.combo.HITSTUN_DECAY_PER_HIT;
  return Math.max(CONFIG.combo.MIN_HITSTUN, decayed);
}

/**
 * Juggle-scaled damage: base × (JUGGLE_DAMAGE_DECAY ^ juggleHitIndex), rounded half-up (spec §2.8).
 * The first juggle hit (index 0) is undecayed; each subsequent juggle hit decays by ×0.9 — so combos
 * terminate. The per-step `mul` floors; the final conversion rounds half-up (so 100×0.9 → 90, not 89).
 */
export function juggleScaledDamage(baseDamage: number, juggleHitIndex: number): number {
  let scaled: Fixed = fromInt(baseDamage);
  for (let i = 0; i < juggleHitIndex; i++) {
    scaled = mul(scaled, CONFIG.combat.JUGGLE_DAMAGE_DECAY);
  }
  return toIntRound(scaled);
}
