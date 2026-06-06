/**
 * cost.ts — resource costs, AP economy, and cancel windows (spec §3.1, §3.4, §3.5) [L3 data in core].
 *
 * These are engine-INTERPRETED data shapes (the engine charges costs and offers gated cancels), so
 * per the established split they live in core; the L3 `moves/` layer owns the authoring wrappers and
 * the static economy LOGIC (the R-5 cancel-graph linter, governor checks). See NOTES.md.
 *
 * All values are integer resource amounts (no fixed-point here — these are meters, not positions).
 */
import { type Ticks } from "./tick";
import { type MoveId } from "./ids";

/** When a move's conditional AP generation fires (spec §3.5.1). */
export type ApGate = "ON_HIT" | "ON_CH" | "ON_BLOCK" | "ON_PARRY" | "ALWAYS";

/** Conditional AP generation — "moves that generate AP" (spec §3.5.2). Earned by success, not mashing. */
export interface ApGain {
  readonly amount: number;
  readonly gate: ApGate;
}

/**
 * What an action costs and (conditionally) refunds. AP is the tempo/turn-budget (decision 4); Stamina
 * and Focus gate access (spec §3.1). Movement/cancels also use this shape.
 */
export interface ResourceCost {
  readonly ap: number; // ap_cost (≥ 0)
  readonly apGain: ApGain | null; // conditional AP generation (null = none)
  readonly stamina: number;
  readonly focus: number;
}

/** The zero cost (most basic actions). */
export const FREE_COST: ResourceCost = { ap: 0, apGain: null, stamina: 0, focus: 0 };

/** Which contact result opens a cancel window (spec §3.4). */
export type CancelGate = "ON_HIT" | "ON_BLOCK" | "ON_CONTACT" | "ALWAYS" | "ON_WHIFF";

/**
 * A window during a move where it may cancel into another move (spec §3.4). Cancels are why combos
 * are finite (each pays `cost`, usually Focus + AP). `into` is an explicit move-id list (the spec's
 * CATEGORY shorthand is deferred — DECISION in NOTES.md).
 */
export interface CancelWindow {
  readonly from: Ticks; // relative to move start (like Property windows)
  readonly to: Ticks;
  readonly gate: CancelGate;
  readonly into: readonly MoveId[];
  readonly cost: ResourceCost;
}
