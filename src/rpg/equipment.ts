/**
 * equipment.ts — weapons / armor / accessories, and the R-4 weapon tradeoff lint (spec §4.4) [L4].
 *
 * PURE DATA (no core import — see sheet.ts). Ranges are plain integer "lane units"; the compiler (the
 * bridge) converts them to fixed-point when it builds the resolved ReachProfile. Weapon = your spacing
 * identity (spear long / dagger short / greatsword slow-huge).
 */

export interface AttrRequirement {
  readonly str?: number;
  readonly dex?: number;
}

export interface Weapon {
  readonly id: string;
  readonly weaponClass: string; // dagger / sword / greatsword / spear / fists / bow ...
  readonly minRange: number; // lane units (compiler → fixed-point)
  readonly maxRange: number;
  /** Global frame deltas applied to this weapon's moves (spec §4.4). */
  readonly startupDelta: number; // + = slower
  readonly recoveryDelta: number;
  readonly damageDelta: number;
  readonly requirements: AttrRequirement;
  /** Move ids this weapon contributes to the MoveList (content). */
  readonly grantsMoves: readonly string[];
}

export interface Armor {
  readonly id: string;
  readonly poiseBonus: number;
  readonly hpBonus: number;
  readonly speedPenalty: number; // + startup / − movement (the tradeoff)
  readonly damageResist: number;
}

/** Whether the wielder meets a weapon's attribute gate (R-3: a floor, not a multiplier). */
export function meetsRequirements(req: AttrRequirement, str: number, dex: number): boolean {
  return (req.str === undefined || str >= req.str) && (req.dex === undefined || dex >= req.dex);
}

// ---------------------------------------------------------------------------
// R-4 — the universal tradeoff triangle: range ↔ speed ↔ damage (spec §4.4, §4.5)
// ---------------------------------------------------------------------------

/** A weapon's position on the three tradeoff axes (higher = better on each). */
export interface TradeoffScore {
  readonly reach: number; // maxRange (more reach = better)
  readonly speed: number; // −startupDelta (lower startup = faster = better)
  readonly damage: number; // damageDelta
}

export function tradeoffScore(w: Weapon): TradeoffScore {
  return { reach: w.maxRange, speed: -w.startupDelta, damage: w.damageDelta };
}

export interface Domination {
  readonly dominator: string;
  readonly dominated: string;
}

/** True iff `a` is ≥ `b` on all three axes and strictly greater on at least one (Pareto dominance). */
function dominates(a: TradeoffScore, b: TradeoffScore): boolean {
  const ge = a.reach >= b.reach && a.speed >= b.speed && a.damage >= b.damage;
  const gt = a.reach > b.reach || a.speed > b.speed || a.damage > b.damage;
  return ge && gt;
}

/**
 * R-4 lint: no weapon may be top-tier in all of range/speed/damage. Returns every ordered pair where
 * one weapon Pareto-dominates another (better-or-equal on all three, strictly better on one) — a
 * violation of the tradeoff triangle (spec §4.4; audit R-4).
 */
export function paretoDominations(weapons: readonly Weapon[]): readonly Domination[] {
  const out: Domination[] = [];
  for (const a of weapons) {
    for (const b of weapons) {
      if (a.id === b.id) continue;
      if (dominates(tradeoffScore(a), tradeoffScore(b))) {
        out.push({ dominator: a.id, dominated: b.id });
      }
    }
  }
  return out;
}

/** R-4 holds iff no weapon Pareto-dominates another (audit R-4). */
export function r4Holds(weapons: readonly Weapon[]): boolean {
  return paretoDominations(weapons).length === 0;
}
