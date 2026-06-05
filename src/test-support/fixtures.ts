/**
 * test-support/fixtures.ts — builders for engine data, used only by *.test.ts files.
 *
 * Centralizes default-laden constructors so adding a field to a core record updates one place, not
 * every test. This is test scaffolding (not gameplay), so it is exempt from the async/toNumber lint
 * globs; it imports only core data shapes (no upward deps).
 */
import { type Fixed, fromInt } from "../core/fixed";
import { type Tick } from "../core/tick";
import {
  type FrameProfile,
  type HitEffect,
  type Property,
  type Timing,
  type MoveLevel,
} from "../core/frameprofile";
import {
  type ReachProfile,
  type SpatialState,
  type Motion,
  type Facing,
} from "../core/spatial-types";
import {
  type Entity,
  type EntityState,
  type MoveInstance,
  type Resources,
  type MoveId,
} from "../core/entity";

export function makeResources(o: Partial<Resources> = {}): Resources {
  return {
    hp: 100,
    hpMax: 100,
    stamina: 50,
    staminaMax: 50,
    poise: 30,
    poiseMax: 30,
    focus: 10,
    focusMax: 10,
    ap: 5,
    apMax: 5,
    ...o,
  };
}

export function makeSpatial(o: Partial<SpatialState> = {}): SpatialState {
  return { pos: fromInt(0), offset: fromInt(0), height: fromInt(1), facing: 1 as Facing, ...o };
}

export function makeReach(o: Partial<ReachProfile> = {}): ReachProfile {
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

export function makeHitEffect(o: Partial<HitEffect> = {}): HitEffect {
  return {
    damage: 10,
    hitstun: 9,
    blockstun: 5,
    chipDamage: 1,
    knockback: fromInt(1),
    launches: false,
    knockdown: false,
    ...o,
  };
}

export interface ProfileOverrides {
  readonly timing?: Partial<Timing>;
  readonly hitEffect?: Partial<HitEffect>;
  readonly reach?: Partial<ReachProfile>;
  readonly properties?: readonly Property[];
  readonly level?: MoveLevel;
  readonly startupCancelable?: boolean;
  readonly motion?: Motion;
}

export function makeProfile(o: ProfileOverrides = {}): FrameProfile {
  const base: FrameProfile = {
    timing: { startup: 4, active: 2, recovery: 6, ...o.timing },
    hitEffect: makeHitEffect(o.hitEffect),
    reach: makeReach(o.reach),
    properties: o.properties ?? [],
    level: o.level ?? "MID",
    startupCancelable: o.startupCancelable ?? false,
  };
  return o.motion ? { ...base, motion: o.motion } : base;
}

export interface MoveOverrides {
  readonly moveId?: MoveId;
  readonly profile?: FrameProfile;
  readonly startTick?: Tick;
  readonly connected?: boolean;
  readonly armorHitsUsed?: number;
}

export function makeMove(o: MoveOverrides = {}): MoveInstance {
  return {
    moveId: o.moveId ?? "move",
    profile: o.profile ?? makeProfile(),
    startTick: o.startTick ?? 0,
    connected: o.connected ?? false,
    contact: "NONE",
    armorHitsUsed: o.armorHitsUsed ?? 0,
  };
}

export interface EntityOverrides {
  readonly id?: string;
  readonly state?: EntityState;
  readonly readyTick?: Tick;
  readonly resources?: Partial<Resources>;
  readonly spatial?: Partial<SpatialState>;
}

export function makeEntity(o: EntityOverrides = {}): Entity {
  return {
    id: o.id ?? "e",
    state: o.state ?? { kind: "NEUTRAL" },
    readyTick: o.readyTick ?? 0,
    resources: makeResources(o.resources),
    spatial: makeSpatial(o.spatial),
  };
}

/** Re-export a couple of constructors used widely in tests. */
export const fx = (n: number): Fixed => fromInt(n);
