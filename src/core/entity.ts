/**
 * entity.ts — the Entity record, its state machine, and move-phase derivation (spec §0.4) [L0].
 *
 * `ready_tick` is the mechanism that turns a continuous fight into turns: the engine always asks the
 * entity with the lower `ready_tick` to choose next (spec §0.4, §2.1). Everything downstream is
 * bookkeeping on that one idea.
 *
 * Portability notes:
 *  - Entities and moves are referenced by stable IDs, never object identity (portability contract).
 *  - The core Entity holds a plain integer `Resources` pool (the resource ECONOMY lives in moves/,
 *    Phase 4) and does NOT hold an RPGSheet (the engine runs resolved frame data; see NOTES.md).
 *  - Spatial fields (pos/offset/height/facing) are added in Phase 2 once spatial/lane.ts exists.
 */
import { type Tick, type Ticks } from "./tick";
import { type FrameProfile, totalFrames } from "./frameprofile";
import { type SpatialState } from "./spatial-types";
import { assertNever } from "./assert-never";

/** Stable identifiers (strings keep golden vectors human-readable; a port may use ints). */
export type EntityId = string;
export type MoveId = string;

// ---------------------------------------------------------------------------
// Resources (spec §3.1) — the pools the entity owns. Economy/regen logic lives in moves/ (Phase 4).
// ---------------------------------------------------------------------------

export interface Resources {
  readonly hp: number;
  readonly hpMax: number;
  readonly stamina: number;
  readonly staminaMax: number;
  readonly poise: number;
  readonly poiseMax: number;
  readonly focus: number;
  readonly focusMax: number;
  readonly ap: number;
  readonly apMax: number;
}

// ---------------------------------------------------------------------------
// Move instance — an in-flight move on the timeline
// ---------------------------------------------------------------------------

export interface MoveInstance {
  /** Which move this is (stable ID — portability: ID-based references). */
  readonly moveId: MoveId;
  /** The resolved frame data the engine runs (already compiled by the RPG layer). */
  readonly profile: FrameProfile;
  /** The tick `T` at which the move began; all phase boundaries derive from this. */
  readonly startTick: Tick;
  /** Whether this instance has already registered a contact (single-hit; multi-hit is out of scope). */
  readonly connected: boolean;
  /** The kind of contact this move made (for hit-confirm cancels — spec §2.10). */
  readonly contact: MoveContact;
  /** Armor hits absorbed so far by this instance (vs the ARMOR property's armorHits budget). */
  readonly armorHitsUsed: number;
}

/** What a move connected as, for hit-confirm gating (a local union to keep entity free of resolver). */
export type MoveContact = "NONE" | "HIT" | "BLOCK";

/** A move's phase, derived from elapsed ticks (spec §2.2 "update phase based on elapsed ticks"). */
export type MovePhase = "STARTUP" | "ACTIVE" | "RECOVERY" | "DONE";

/**
 * Phase of `move` at absolute tick `t`. Active frames are elapsed ∈ [startup, startup+active−1];
 * the entity becomes actionable at elapsed = total (phase DONE). This derivation is the single
 * source of truth the engine uses to drive STARTUP→ACTIVE→RECOVERY transitions.
 */
export function phaseAt(move: MoveInstance, t: Tick): MovePhase {
  const elapsed: Ticks = t - move.startTick;
  const { startup, active } = move.profile.timing;
  const total = totalFrames(move.profile.timing);
  if (elapsed < startup) return "STARTUP";
  if (elapsed < startup + active) return "ACTIVE";
  if (elapsed < total) return "RECOVERY";
  return "DONE";
}

/** The tick at which an in-flight move makes its entity actionable (before any advantage adjust). */
export function moveReadyTick(move: MoveInstance): Tick {
  return move.startTick + totalFrames(move.profile.timing);
}

// ---------------------------------------------------------------------------
// Entity state machine (spec §0.4) — tagged union (decision 11)
// ---------------------------------------------------------------------------

export type EntityState =
  | { readonly kind: "NEUTRAL" }
  | { readonly kind: "STARTUP"; readonly move: MoveInstance }
  | { readonly kind: "ACTIVE"; readonly move: MoveInstance }
  | { readonly kind: "RECOVERY"; readonly move: MoveInstance }
  | { readonly kind: "HITSTUN"; readonly until: Tick }
  | { readonly kind: "BLOCKSTUN"; readonly until: Tick }
  | { readonly kind: "AIRBORNE"; readonly until: Tick; readonly juggleCount: number }
  | { readonly kind: "DOWN"; readonly wakeupTick: Tick }
  | { readonly kind: "GUARDBROKEN"; readonly until: Tick };

/** A short stable tag per state (for traces/printing). Exhaustive switch documents the union. */
export function entityStateTag(s: EntityState): string {
  switch (s.kind) {
    case "NEUTRAL":
      return "NEUTRAL";
    case "STARTUP":
      return "STARTUP";
    case "ACTIVE":
      return "ACTIVE";
    case "RECOVERY":
      return "RECOVERY";
    case "HITSTUN":
      return "HITSTUN";
    case "BLOCKSTUN":
      return "BLOCKSTUN";
    case "AIRBORNE":
      return "AIRBORNE";
    case "DOWN":
      return "DOWN";
    case "GUARDBROKEN":
      return "GUARDBROKEN";
    default:
      return assertNever(s);
  }
}

/** The in-flight move for the move-execution states, else null. Exhaustive over the union. */
export function stateMove(s: EntityState): MoveInstance | null {
  switch (s.kind) {
    case "STARTUP":
    case "ACTIVE":
    case "RECOVERY":
      return s.move;
    case "NEUTRAL":
    case "HITSTUN":
    case "BLOCKSTUN":
    case "AIRBORNE":
    case "DOWN":
    case "GUARDBROKEN":
      return null;
    default:
      return assertNever(s);
  }
}

// ---------------------------------------------------------------------------
// Entity (spec §0.4). Spatial fields land in Phase 2.
// ---------------------------------------------------------------------------

export interface Entity {
  readonly id: EntityId;
  readonly state: EntityState;
  /** The tick at which this entity becomes actionable (spec §0.4). */
  readonly readyTick: Tick;
  readonly resources: Resources;
  /** Position on the lane + sidestep offset + height + facing (spec §0.4, §1.1). */
  readonly spatial: SpatialState;
  // readonly statusEffects: ...      ← added Phase 3+
}

/** True iff the entity is free to choose an action at tick `t` (its ready_tick has arrived). */
export function isActionable(e: Entity, t: Tick): boolean {
  return t >= e.readyTick;
}
