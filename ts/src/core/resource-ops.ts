/**
 * resource-ops.ts — pure operations on the Resources pool (spec §3.1, §3.5) [L2 helper].
 *
 * The engine charges/refunds resources inline (it interprets cost data, which lives in core), so these
 * helpers live in core. The L3 `moves/` layer re-exports them for authoring/audit use. All integer
 * arithmetic; every result is clamped to its cap.
 */
import { type Resources } from "./entity";
import { type ResourceCost } from "./cost";
import { CONFIG } from "./config";

/** True iff the pool can pay the cost's AP, Stamina, and Focus. */
export function canAfford(r: Resources, cost: ResourceCost): boolean {
  return r.ap >= cost.ap && r.stamina >= cost.stamina && r.focus >= cost.focus;
}

/** Deduct a cost's AP/Stamina/Focus (call only after canAfford). */
export function spend(r: Resources, cost: ResourceCost): Resources {
  return {
    ...r,
    ap: r.ap - cost.ap,
    stamina: r.stamina - cost.stamina,
    focus: r.focus - cost.focus,
  };
}

/** The combined cost of two actions (AP/Stamina/Focus added; apGain prefers the second). */
export function combineCost(a: ResourceCost, b: ResourceCost): ResourceCost {
  return {
    ap: a.ap + b.ap,
    stamina: a.stamina + b.stamina,
    focus: a.focus + b.focus,
    apGain: b.apGain ?? a.apGain,
  };
}

export function gainAp(r: Resources, amount: number): Resources {
  return { ...r, ap: Math.min(r.ap + amount, r.apMax) };
}

export function gainFocus(r: Resources, amount: number): Resources {
  return { ...r, focus: Math.min(r.focus + amount, r.focusMax) };
}

/** Refill AP to the cap — done when an entity (re)gains initiative (spec §3.5.1; decision 4). */
export function refillAp(r: Resources): Resources {
  return CONFIG.ap.REFILL_TO_MAX ? { ...r, ap: r.apMax } : r;
}

/** Regenerate Stamina one tick's worth (only while not attacking — spec §3.1). */
export function regenStamina(r: Resources): Resources {
  return { ...r, stamina: Math.min(r.stamina + CONFIG.resources.STAMINA_REGEN_PER_TICK, r.staminaMax) };
}
