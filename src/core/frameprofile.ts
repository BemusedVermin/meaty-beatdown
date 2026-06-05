/**
 * frameprofile.ts — the FrameProfile and the data shapes it aggregates (spec §0.2, §0.3) [L0].
 *
 * This is the spec's central object: almost everything else exists to produce, modify, or consume
 * it. The engine is an interpreter that runs a *resolved* FrameProfile; the RPG layer is a compiler
 * that emits one. Per decision 11 every sum type here is a tagged union with a literal discriminant.
 *
 * Frame advantage is the load-bearing concept, and it is DERIVED, never stored (invariant I-1):
 *   on_hit   = defender_hitstun  − attacker_recovery
 *   on_block = defender_blockstun − attacker_recovery
 * There are no on_hit/on_block fields to set inconsistently — `onHit`/`onBlock` compute them. The
 * authoring tool/compiler thus *computes* advantage, never accepts it (spec §0.2, audit C-1).
 *
 * Built up across phases: `reach: ReachProfile` is added in Phase 2 (L1); `cancelWindows` + `cost`
 * (AP) in Phase 4 (L3). Keeping those out of L0 for now keeps core a pure foundation. See NOTES.md.
 */
import { type Fixed, compare } from "./fixed";
import { type Ticks } from "./tick";
import { type ReachProfile, type Motion } from "./spatial-types";

// ---------------------------------------------------------------------------
// Levels & directions (fieldless enumerations → string-literal unions; map to Rust fieldless enums)
// ---------------------------------------------------------------------------

/**
 * A move's blockability/height class (spec §0.2, §2.5). HIGH/MID are blocked standing, LOW crouching,
 * OVERHEAD only standing, THROW is unblockable (beats block — spec §2.6), UNBLOCKABLE must be evaded.
 */
export type MoveLevel = "HIGH" | "MID" | "LOW" | "OVERHEAD" | "THROW" | "UNBLOCKABLE";

/** Typed invincibility categories (spec §0.3). ALL = full i-frames; the rest are type-specific. */
export type InvulnType = "ALL" | "STRIKE" | "THROW" | "PROJECTILE";

/** The category an attack matches against typed invincibility. Projectiles are deferred (decision 8). */
export type AttackType = "STRIKE" | "THROW";

/** A move's attack type derives from its level: only THROW-level moves are throws (spec §2.6). */
export function attackTypeOf(level: MoveLevel): AttackType {
  return level === "THROW" ? "THROW" : "STRIKE";
}

// ---------------------------------------------------------------------------
// Property windows (spec §0.3) — each property is live during a tick range relative to move start
// ---------------------------------------------------------------------------

/** A closed tick range `[from, to]` (INCLUSIVE both ends), measured in `elapsed = T − startTick`. */
export interface Window {
  readonly from: Ticks;
  readonly to: Ticks;
}

/** True iff `elapsed` falls within the inclusive window. */
export function windowContains(w: Window, elapsed: Ticks): boolean {
  return elapsed >= w.from && elapsed <= w.to;
}

/**
 * A frame property attached to a tick range of a move (spec §0.3). Tagged union (decision 11): every
 * variant carries a `window`. Note: TRACKING is intentionally NOT here — it is encoded in ReachProfile
 * (lateral_band/step_in/track_side, §1.2) so all contact math stays in spatial/lane.ts (audit C-7).
 */
export type Property =
  /** Typed invincibility: hitboxes of the matching category pass through (spec §0.3). */
  | { readonly kind: "INVULN"; readonly invulnType: InvulnType; readonly window: Window }
  /**
   * Hyper armor: absorbs up to `armorHits` strikes without hitstun, still taking `armorDamageMult`
   * of the damage. Throws beat armor (decision 1) — handled by the resolver, not here.
   */
  | {
      readonly kind: "ARMOR";
      readonly armorHits: number;
      readonly armorDamageMult: Fixed;
      readonly window: Window;
    }
  /** If struck during this window the defender takes a counter-hit (spec §2.7). Startup/recovery are
   *  counter-hit windows by default (engine, Phase 3); this property extends CH to other frames. */
  | { readonly kind: "COUNTER_HIT_STATE"; readonly window: Window }
  /** Auto-blocks one hit during the window, then the move continues (sabaki/parry — spec §0.3, §2.6). */
  | { readonly kind: "GUARD_POINT"; readonly window: Window }
  /** A held blocking stance covering the listed levels (spec §2.5). A strike whose level is covered
   *  is BLOCKED; an uncovered level (the mixup) is a clean hit. Throws beat block (spec §2.6). */
  | { readonly kind: "BLOCK"; readonly covers: readonly MoveLevel[]; readonly window: Window }
  /** Marks a window where cancels are legal (gating handled in L3 — spec §0.3, §3.4). */
  | { readonly kind: "CANCELABLE"; readonly window: Window }
  /** Entity is launched / juggle-state during the window (spec §0.3, §2.8). */
  | { readonly kind: "AIRBORNE"; readonly window: Window }
  /** Emits an independent timeline entity. DEFERRED (decision 8 / spec §2.9): data slot kept; the
   *  engine stub throws if a spawn is actually invoked (Phase 3). */
  | { readonly kind: "PROJECTILE_SPAWN"; readonly window: Window };

/** Every Property carries a window; this exhaustive switch documents the union (decision 11). */
export function propertyWindow(p: Property): Window {
  switch (p.kind) {
    case "INVULN":
    case "ARMOR":
    case "COUNTER_HIT_STATE":
    case "GUARD_POINT":
    case "BLOCK":
    case "CANCELABLE":
    case "AIRBORNE":
    case "PROJECTILE_SPAWN":
      return p.window;
  }
}

/** True iff property `p` is live at `elapsed` ticks into the move. */
export function isPropertyActive(p: Property, elapsed: Ticks): boolean {
  return windowContains(propertyWindow(p), elapsed);
}

// ---------------------------------------------------------------------------
// Hit effect (spec §0.2) — what a clean hit does to the defender
// ---------------------------------------------------------------------------

export interface HitEffect {
  /** Integer HP damage. Decision 3: fixed for the prototype (DAMAGE_VARIANCE off → no RNG). */
  readonly damage: number;
  /** Defender hitstun ticks → drives on_hit advantage (invariant I-1). */
  readonly hitstun: Ticks;
  /** Defender blockstun ticks → drives on_block advantage (invariant I-1). */
  readonly blockstun: Ticks;
  /** Chip damage on block — goes to Poise/guard, not HP (spec §2.5). Integer. */
  readonly chipDamage: number;
  /** Lane pushback distance on hit (spatial → Fixed, decision 10). */
  readonly knockback: Fixed;
  /** If true, the hit launches the defender into AIRBORNE (juggle — spec §2.8). */
  readonly launches: boolean;
  /** If true, the hit knocks the defender DOWN (okizeme/wakeup — spec §2.8). */
  readonly knockdown: boolean;
}

// ---------------------------------------------------------------------------
// Timing (spec §0.2)
// ---------------------------------------------------------------------------

export interface Timing {
  readonly startup: Ticks; // ticks before the first active tick
  readonly active: Ticks; // ticks the hitbox/effect is live
  readonly recovery: Ticks; // ticks after active before actionable again
}

/** total = startup + active + recovery (spec §0.2 derived field). */
export function totalFrames(t: Timing): Ticks {
  return t.startup + t.active + t.recovery;
}

// ---------------------------------------------------------------------------
// FrameProfile (spec §0.2) — the resolved frame data the engine runs
// ---------------------------------------------------------------------------

export interface FrameProfile {
  readonly timing: Timing;
  readonly hitEffect: HitEffect;
  readonly properties: readonly Property[];
  readonly level: MoveLevel;
  /** Spatial footprint the engine feeds to spatial/lane.ts `doesHit` (spec §1.2). */
  readonly reach: ReachProfile;
  /** Decision 6: a move is cancelable only from active/recovery unless this is true (spec §2.10). */
  readonly startupCancelable: boolean;
  /** Repositioning of the entity (movement moves). Omitted for moves that don't reposition. */
  readonly motion?: Motion;
  // readonly cancelWindows / cost     ← added Phase 4 (L3, moves/)
}

/** on_hit advantage (invariant I-1): defender hitstun − attacker recovery. Can be ± (spec §0.2). */
export function onHit(fp: FrameProfile): number {
  return fp.hitEffect.hitstun - fp.timing.recovery;
}

/** on_block advantage (invariant I-1): defender blockstun − attacker recovery. Usually − (spec §0.2). */
export function onBlock(fp: FrameProfile): number {
  return fp.hitEffect.blockstun - fp.timing.recovery;
}

/** on_whiff = 0 by definition: you eat full recovery and are exposed (spec §0.2, §1.2). */
export function onWhiff(): number {
  return 0;
}

// ---------------------------------------------------------------------------
// Consistency checks (spec §0.2 invariant-I-1 family) — returns problems, empty = OK
// ---------------------------------------------------------------------------

/**
 * Validate a FrameProfile's internal consistency. I-1 itself needs no check (advantage is derived),
 * so this verifies the surrounding invariants: non-negative timing, a real active window, integer
 * effects, and every property window inside the move's frame span [0, total − 1].
 */
export function checkFrameProfile(fp: FrameProfile): readonly string[] {
  const problems: string[] = [];
  const { startup, active, recovery } = fp.timing;
  const total = totalFrames(fp.timing);

  if (startup < 0) problems.push(`startup must be ≥ 0 (got ${startup})`);
  if (active < 1) problems.push(`active must be ≥ 1 (got ${active})`);
  if (recovery < 0) problems.push(`recovery must be ≥ 0 (got ${recovery})`);

  if (fp.hitEffect.hitstun < 0) problems.push(`hitstun must be ≥ 0 (got ${fp.hitEffect.hitstun})`);
  if (fp.hitEffect.blockstun < 0)
    problems.push(`blockstun must be ≥ 0 (got ${fp.hitEffect.blockstun})`);
  if (!Number.isInteger(fp.hitEffect.damage))
    problems.push(`damage must be an integer (got ${fp.hitEffect.damage})`);

  for (const p of fp.properties) {
    const w = propertyWindow(p);
    if (w.from < 0) problems.push(`${p.kind} window.from must be ≥ 0 (got ${w.from})`);
    if (w.to < w.from) problems.push(`${p.kind} window.to (${w.to}) must be ≥ from (${w.from})`);
    if (w.to > total - 1)
      problems.push(`${p.kind} window.to (${w.to}) exceeds last frame index ${total - 1}`);
  }

  const r = fp.reach;
  if (compare(r.minRange, r.maxRange) > 0) problems.push(`reach minRange must be ≤ maxRange`);
  if (compare(r.heightLow, r.heightHigh) > 0) problems.push(`reach heightLow must be ≤ heightHigh`);

  return problems;
}
