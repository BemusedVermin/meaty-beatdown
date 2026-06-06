/**
 * regime.ts — the NEUTRAL/PRESSURE decision, keyed entirely off ready_tick (spec §2.1) [L2].
 *
 * This is the consistency keystone: the SAME timeline produces both the neutral mind-read and
 * offense/pressure with no special-casing. The engine always asks the entity with the lower
 * ready_tick to choose next; ties mean both are free → the simultaneous hidden commit.
 */
import { type Tick } from "./tick";
import { type Entity } from "./entity";

export type Regime =
  /** Both entities are actionable at the same tick → both commit simultaneously and hidden. */
  | { readonly kind: "NEUTRAL" }
  /** One entity is free while the other is locked → the actor chooses with full information. */
  | { readonly kind: "PRESSURE"; readonly actor: 0 | 1 };

/** Decide the regime from the two entities' ready_ticks (spec §2.1). */
export function computeRegime(a: Entity, b: Entity): Regime {
  if (a.readyTick === b.readyTick) return { kind: "NEUTRAL" };
  return { kind: "PRESSURE", actor: a.readyTick < b.readyTick ? 0 : 1 };
}

/** The next tick at which some entity becomes actionable (the next decision point). */
export function nextDecisionTick(a: Entity, b: Entity): Tick {
  return a.readyTick < b.readyTick ? a.readyTick : b.readyTick;
}
