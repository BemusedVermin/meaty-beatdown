/**
 * builders.ts — terse constructors for sample content (moves/weapons/sheets) [content].
 *
 * Content is swappable sample DATA consumed only at the top (cli/, balance/, golden/); the engine
 * never imports it (dependency-cruiser enforces this). These builders fill FrameProfile defaults so a
 * move definition states only what is interesting about it.
 */
import { fromInt } from "../core/fixed";
import {
  type FrameProfile,
  type Property,
  type HitEffect,
  type Timing,
  type MoveLevel,
} from "../core/frameprofile";
import { type ReachProfile, type Motion } from "../core/spatial-types";
import { type ResourceCost, type CancelWindow, FREE_COST } from "../core/cost";
import { type Move, type MoveClass } from "../moves/move";

export function reach(o: Partial<ReachProfile> = {}): ReachProfile {
  return {
    minRange: fromInt(0),
    maxRange: fromInt(2),
    heightLow: fromInt(0),
    heightHigh: fromInt(2),
    advance: fromInt(0),
    lateralBand: fromInt(1),
    stepIn: fromInt(0),
    trackSide: 0,
    ...o,
  };
}

export function cost(o: Partial<ResourceCost> = {}): ResourceCost {
  return { ...FREE_COST, ...o };
}

export function hit(o: Partial<HitEffect> = {}): HitEffect {
  return {
    damage: 0,
    hitstun: 0,
    blockstun: 0,
    chipDamage: 0,
    knockback: fromInt(0),
    launches: false,
    knockdown: false,
    ...o,
  };
}

export interface FrameOpts {
  readonly timing?: Partial<Timing>;
  readonly hitEffect?: Partial<HitEffect>;
  readonly reach?: Partial<ReachProfile>;
  readonly properties?: readonly Property[];
  readonly level?: MoveLevel;
  readonly cost?: Partial<ResourceCost>;
  readonly cancelWindows?: readonly CancelWindow[];
  readonly startupCancelable?: boolean;
  readonly motion?: Motion;
}

export function frame(o: FrameOpts = {}): FrameProfile {
  const base: FrameProfile = {
    timing: { startup: 4, active: 2, recovery: 6, ...o.timing },
    hitEffect: hit(o.hitEffect),
    reach: reach(o.reach),
    properties: o.properties ?? [],
    level: o.level ?? "MID",
    cost: cost(o.cost),
    cancelWindows: o.cancelWindows ?? [],
    startupCancelable: o.startupCancelable ?? false,
  };
  return o.motion ? { ...base, motion: o.motion } : base;
}

export function mv(id: string, name: string, moveClass: MoveClass, o: FrameOpts = {}): Move {
  return { id, name, moveClass, profile: frame(o) };
}
