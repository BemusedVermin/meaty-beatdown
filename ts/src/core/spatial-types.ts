/**
 * spatial-types.ts — the spatial DATA shapes (spec §0.4, §1.1, §1.2) [L0/L1 data].
 *
 * These live in core because the Entity (core) owns its `SpatialState` and the FrameProfile (core)
 * embeds its `ReachProfile`, and core must import nothing upward. The spatial LOGIC — the single
 * `doesHit` contact predicate (audit C-7) — lives in `spatial/lane.ts`, which imports these shapes.
 *
 * All positional quantities are `Fixed` (16.16), never floats (decision 10).
 */
import { type Fixed } from "./fixed";

/** Which way an entity faces along the lane (±1). Auto-facing re-centers it when actionable (§1.1). */
export type Facing = 1 | -1;

/**
 * An entity's position in space (spec §0.4, §1.1).
 *  - `pos`    — the 1D distance lane: spacing, reach, whiff-by-range, knockback. The ONLY spacing axis.
 *  - `offset` — lateral/depth displacement for Tekken sidestep. Does ONE job: evasion (gates linear
 *               vs tracking). It is NOT a second spacing axis (spec §1.1).
 *  - `height` — hurtbox height (anti-air / low coverage).
 */
export interface SpatialState {
  readonly pos: Fixed;
  readonly offset: Fixed;
  readonly height: Fixed;
  readonly facing: Facing;
}

/**
 * A move's spatial footprint (spec §1.2). `pos`-axis fields (min/max range, advance) carry the
 * spacing identity; the `offset`-axis fields (lateralBand, stepIn, trackSide) encode Tekken
 * linear/tracking/homing evasion — the ONLY place tracking is modeled (not duplicated as a Property).
 *
 * Tracking modes are encoded by magnitudes, not an enum:
 *  - LINEAR   : stepIn = 0            → a sidestep beyond lateralBand whiffs it.
 *  - HOMING   : trackSide = 0, stepIn > 0 → realigns through a sidestep on BOTH sides.
 *  - TRACKING : trackSide = ±1, stepIn > 0 → realigns only on the covered side; the other side dodges.
 */
export interface ReachProfile {
  readonly minRange: Fixed; // closer than this and the move whiffs (too close / over the head)
  readonly maxRange: Fixed; // outer edge of the hitbox along the lane
  readonly heightLow: Fixed; // vertical coverage (low edge)
  readonly heightHigh: Fixed; // vertical coverage (high edge)
  readonly advance: Fixed; // how far the attacker's HITBOX reaches forward during startup+active
  readonly lateralBand: Fixed; // half-width of the hitbox on the offset axis
  readonly stepIn: Fixed; // lateral realign during the move (TRACKING/HOMING > 0; LINEAR = 0)
  readonly trackSide: -1 | 0 | 1; // which sidestep direction the move covers better (0 = both/none)
}

/**
 * Repositioning a move applies to the ENTITY (distinct from ReachProfile.advance, which only
 * extends the hitbox). Movement moves (step/dash/backdash/sidestep) carry a Motion; most attacks do
 * not. Applied as a discrete hop at the first active frame (decision 9: sidestep is a hop, not a
 * continuous sidewalk); lateral `offset` re-centers on becoming actionable (auto-facing, §1.1).
 */
export interface Motion {
  readonly lane: Fixed; // signed displacement along the lane, relative to facing (+ = forward)
  readonly offset: Fixed; // lateral (sidestep) displacement off the shared axis
}
