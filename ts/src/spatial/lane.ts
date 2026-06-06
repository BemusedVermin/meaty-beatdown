/**
 * lane.ts — the spatial contact model: pos (lane) + offset (sidestep) + height (spec §1.1, §1.2) [L1].
 *
 * This file owns ALL contact math behind the single `doesHit` predicate (audit C-7). No other module
 * computes range / lateral / height. Because everything routes through one predicate, sidestep
 * evasion is a spatial fact identical in structure to a backdash whiff — it flows down the same
 * `on_whiff = 0 → eat full recovery → exposed` path (spec §1.2).
 *
 * Every quantity is `Fixed` (16.16), never a float (decision 10). The data shapes (SpatialState,
 * ReachProfile) live in core/; this module is the pure spatial LOGIC over them.
 *
 * Precondition: callers invoke `doesHit` only while the attacker's move is on an ACTIVE frame — the
 * active-frame gate is engine timing (Phase 3), so it stays out of the spatial predicate.
 */
import { type Fixed, ZERO, add, sub, abs, compare } from "../core/fixed";
import { type SpatialState, type ReachProfile } from "../core/spatial-types";
import {
  type MoveLevel,
  type InvulnType,
  type AttackType,
  attackTypeOf,
} from "../core/frameprofile";

export { type AttackType, attackTypeOf };

const NO_INVULN: ReadonlySet<InvulnType> = new Set<InvulnType>();

/** True iff the defender's active invuln set stops an attack of `type` (ALL stops everything). */
export function invulnBlocks(invuln: ReadonlySet<InvulnType>, type: AttackType): boolean {
  return invuln.has("ALL") || invuln.has(type);
}

/**
 * Lane distance between attacker and defender along the 1D `pos` axis, AFTER the attacker's `advance`
 * (it closes ground in its facing direction during startup+active). This is the spacing scalar.
 */
export function laneDistance(
  attacker: SpatialState,
  defender: SpatialState,
  reach: ReachProfile,
): Fixed {
  const facingAdvance: Fixed = attacker.facing === 1 ? reach.advance : sub(ZERO, reach.advance);
  const effectiveAttackerPos = add(attacker.pos, facingAdvance);
  return abs(sub(defender.pos, effectiveAttackerPos));
}

/** Signed lateral offset of the defender relative to the attacker (defender.offset − attacker.offset). */
export function relativeOffset(attacker: SpatialState, defender: SpatialState): Fixed {
  return sub(defender.offset, attacker.offset);
}

/** Absolute lateral gap on the offset axis. */
export function lateralGap(attacker: SpatialState, defender: SpatialState): Fixed {
  return abs(relativeOffset(attacker, defender));
}

/** (range) defender within [minRange, maxRange] along the lane after advance. */
export function inLaneRange(
  attacker: SpatialState,
  defender: SpatialState,
  reach: ReachProfile,
): boolean {
  const dist = laneDistance(attacker, defender, reach);
  return compare(dist, reach.minRange) >= 0 && compare(dist, reach.maxRange) <= 0;
}

/** (height) defender's hurtbox height within [heightLow, heightHigh]. */
export function inHeightBand(defender: SpatialState, reach: ReachProfile): boolean {
  return (
    compare(defender.height, reach.heightLow) >= 0 &&
    compare(defender.height, reach.heightHigh) <= 0
  );
}

/**
 * (lateral) the sidestep clause. The covered side gets `lateralBand + stepIn`; the uncovered side
 * gets only `lateralBand`. Which side is "covered" follows trackSide:
 *  - trackSide = 0  → both sides covered (HOMING when stepIn > 0; LINEAR is just stepIn = 0).
 *  - trackSide = ±1 → only the matching side is covered (TRACKING); stepping the other way dodges.
 * On-axis (offset gap 0) is always within the band.
 */
export function inLateralBand(
  attacker: SpatialState,
  defender: SpatialState,
  reach: ReachProfile,
): boolean {
  const gap = lateralGap(attacker, defender);
  const side = compare(relativeOffset(attacker, defender), ZERO); // -1 | 0 | 1
  const covered = reach.trackSide === 0 || side === 0 || side === reach.trackSide;
  const allowance: Fixed = covered ? add(reach.lateralBand, reach.stepIn) : reach.lateralBand;
  return compare(gap, allowance) <= 0;
}

/**
 * The single spatial contact predicate (spec §1.2). True iff, on an active frame, the attack lines up
 * with the defender:
 *   (type)    defender not invuln to this attack's category   — i-frames win
 *   (range)   defender within [minRange, maxRange] after advance
 *   (height)  defender height within [heightLow, heightHigh]
 *   (lateral) within the sidestep band — SKIPPED for throws, which ignore offset (decision: throws
 *             beat sidestep; they are short-range and realign on auto-facing — spec §2.6).
 */
export function doesHit(
  attacker: SpatialState,
  defender: SpatialState,
  reach: ReachProfile,
  level: MoveLevel,
  defenderInvuln: ReadonlySet<InvulnType> = NO_INVULN,
): boolean {
  const type = attackTypeOf(level);
  if (invulnBlocks(defenderInvuln, type)) return false;
  if (!inLaneRange(attacker, defender, reach)) return false;
  if (!inHeightBand(defender, reach)) return false;
  if (type !== "THROW" && !inLateralBand(attacker, defender, reach)) return false;
  return true;
}
