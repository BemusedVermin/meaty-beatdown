/**
 * resources.ts — the four-meter resource model + the L3 facade over core resource ops (spec §3.1) [L3].
 *
 * The pool data (Resources) and the arithmetic (spend/gain/regen) live in core because the engine
 * applies them inline; this module documents the resource model (what each meter prevents) and
 * re-exports the ops for authoring/audit consumers, so moves/ is the L3 home of "resources" per the
 * file map without duplicating the engine's logic.
 */
export {
  canAfford,
  spend,
  combineCost,
  gainAp,
  gainFocus,
  refillAp,
  regenStamina,
} from "../core/resource-ops";
export { type ResourceCost, type ApGain, type ApGate, FREE_COST } from "../core/cost";

export type ResourceName = "stamina" | "poise" | "focus" | "ap" | "hp";

export interface ResourceDescriptor {
  readonly name: ResourceName;
  /** Regenerates per tick while not attacking? (spec §3.1) */
  readonly regenWhileIdle: boolean;
  /** One-line "what it prevents" (spec §3.1 table). */
  readonly purpose: string;
}

/** The four meters + HP (spec §3.1). Adding/removing a meter does not touch the engine (it reads by name). */
export const RESOURCE_MODEL: readonly ResourceDescriptor[] = [
  { name: "stamina", regenWhileIdle: true, purpose: "Prevents mashing; spacing to recover is a decision." },
  { name: "poise", regenWhileIdle: false, purpose: "Guard-break system; caps pure turtling." },
  { name: "focus", regenWhileIdle: false, purpose: "Earned offense — specials, cancels, reversals." },
  { name: "ap", regenWhileIdle: false, purpose: "Tempo / turn-budget; caps actions per turn (§3.5)." },
  { name: "hp", regenWhileIdle: false, purpose: "The lose condition." },
];
