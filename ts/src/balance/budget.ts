/**
 * budget.ts — the frame-data budget identity (MOVE_VALUE) + the R-1..R-5 linter (spec §4.5, §B) [tooling].
 *
 * Balance is a checkable PROPERTY, not a wish: a move's strengths must be paid for by weaknesses. This
 * is tooling, so it is allowed floats (the weights `w_*` and scoring) — but it reads only DATA, never
 * mutates engine state. The weights are the master tuning knobs (spec §4.5 ⚠️: a starting set).
 */
import { toNumber } from "../core/fixed";
import { type FrameProfile, type Property, totalFrames, onBlock } from "../core/frameprofile";
import { assertNever } from "../core/assert-never";
import { CONFIG } from "../core/config";
import { type MoveList } from "../moves/move";
import { r5Holds, findPositiveApCycles } from "../moves/economy";
import { type Weapon, r4Holds, paretoDominations } from "../rpg/equipment";

// ---------------------------------------------------------------------------
// MOVE_VALUE — the points identity (spec §4.5)
// ---------------------------------------------------------------------------

export interface BudgetWeights {
  readonly speed: number;
  readonly safety: number;
  readonly reward: number;
  readonly range: number;
  readonly props: number;
  readonly cost: number;
  readonly commit: number;
}

/** A starting weight set (spec §4.5 ⚠️ — playtest-tuned). */
export const DEFAULT_WEIGHTS: BudgetWeights = {
  speed: 1,
  safety: 1.5,
  reward: 0.5,
  range: 1,
  props: 1,
  cost: 0.4,
  commit: 0.2,
};

export const BASELINE_STARTUP = 8;

/** Point value of a frame property (i-frames/armor cost points; CH-state is a vulnerability). */
function propertyValue(p: Property): number {
  switch (p.kind) {
    case "INVULN":
      return 4;
    case "ARMOR":
      return 3 * p.armorHits;
    case "GUARD_POINT":
      return 4;
    case "BLOCK":
      return 1;
    case "COUNTER_HIT_STATE":
      return -1;
    case "AIRBORNE":
      return 0;
    case "PROJECTILE_SPAWN":
      return 5;
    default:
      return assertNever(p);
  }
}

/**
 * MOVE_VALUE(move) (spec §4.5): fast / safe / damaging / long / propertied COSTS points; paying
 * resources and being punishable on whiff REFUND points. A balanced move nets near the archetype budget.
 */
export function moveValue(p: FrameProfile, w: BudgetWeights = DEFAULT_WEIGHTS): number {
  const resourceCost = p.cost.stamina + p.cost.focus + p.cost.ap * 2;
  const propSum = p.properties.reduce((s, pr) => s + propertyValue(pr), 0);
  return (
    w.speed * (BASELINE_STARTUP - p.timing.startup) +
    w.safety * onBlock(p) +
    w.reward * p.hitEffect.damage +
    w.range * toNumber(p.reach.maxRange) +
    w.props * propSum -
    w.cost * resourceCost -
    w.commit * totalFrames(p.timing)
  );
}

export interface BudgetReport {
  readonly perMove: ReadonlyArray<{ readonly id: string; readonly value: number; readonly deviation: number }>;
  readonly budget: number;
  readonly epsilon: number;
  readonly outliers: readonly string[];
}

/**
 * Score a MoveList against its own mean (the implicit ARCHETYPE_BUDGET) and flag moves whose value
 * deviates by more than ±ε — i.e. over- or under-budget moves that need a downside (or an upside).
 */
export function budgetReport(moves: MoveList, epsilon = 10, w: BudgetWeights = DEFAULT_WEIGHTS): BudgetReport {
  const scored = moves.map((m) => ({ id: m.id, value: moveValue(m.profile, w) }));
  const budget = scored.reduce((s, x) => s + x.value, 0) / Math.max(1, scored.length);
  const perMove = scored.map((x) => ({ ...x, deviation: x.value - budget }));
  return {
    perMove,
    budget,
    epsilon,
    outliers: perMove.filter((x) => Math.abs(x.deviation) > epsilon).map((x) => x.id),
  };
}

// ---------------------------------------------------------------------------
// R-1..R-5 — the balance rules (spec §3.1, §4.2-4.5, §B)
// ---------------------------------------------------------------------------

export interface RuleResult {
  readonly rule: string;
  readonly pass: boolean;
  readonly detail: string;
}

/** R-1: no zero-cost action — every option pays a resource (spec §3.1). */
export function checkR1(moves: MoveList): RuleResult {
  const free = moves.filter(
    (m) => m.profile.cost.stamina + m.profile.cost.focus + m.profile.cost.ap === 0,
  );
  return {
    rule: "R-1",
    pass: free.length === 0,
    detail:
      free.length === 0
        ? "every move pays Stamina/Focus/AP (no zero-cost dominant action)"
        : `zero-cost moves: ${free.map((m) => m.id).join(", ")}`,
  };
}

/**
 * R-2: one major lever per attribute — no attribute drives BOTH a major offensive and a major
 * defensive lever (spec §4.2). Encodes the compiler's lever assignment.
 */
const LEVER_MAP: Readonly<Record<string, { offensive: boolean; defensive: boolean }>> = {
  str: { offensive: true, defensive: false }, // damage / armor budget
  dex: { offensive: true, defensive: false }, // speed / advance
  con: { offensive: false, defensive: true }, // HP / Stamina / Poise
  int: { offensive: true, defensive: false }, // Focus pool / specials access
  wis: { offensive: false, defensive: true }, // parry / Focus refund / wakeup
  cha: { offensive: false, defensive: false }, // content (feints/intimidate)
};

export function checkR2(): RuleResult {
  const doubleDippers = Object.entries(LEVER_MAP)
    .filter(([, v]) => v.offensive && v.defensive)
    .map(([k]) => k);
  return {
    rule: "R-2",
    pass: doubleDippers.length === 0,
    detail:
      doubleDippers.length === 0
        ? "each attribute drives one lever (no offensive+defensive double-dip)"
        : `double-dipping attributes: ${doubleDippers.join(", ")}`,
  };
}

/** R-3: gates are floors with CAPPED bonuses, never runaway multipliers (spec §4.3). */
export function checkR3(): RuleResult {
  const caps = [CONFIG.rpg.DEX_STARTUP_REDUCTION_CAP, CONFIG.rpg.STR_ARMOR_HITS_CAP];
  const allCapped = caps.every((c) => Number.isFinite(c) && c >= 0);
  return {
    rule: "R-3",
    pass: allCapped,
    detail: allCapped
      ? `bonuses capped (DEX startup ≤ ${CONFIG.rpg.DEX_STARTUP_REDUCTION_CAP}, STR armor ≤ ${CONFIG.rpg.STR_ARMOR_HITS_CAP})`
      : "an attribute bonus is uncapped",
  };
}

/** R-4: range ↔ speed ↔ damage tradeoff — no Pareto-dominant weapon (spec §4.4). */
export function checkR4(weapons: readonly Weapon[]): RuleResult {
  const doms = paretoDominations(weapons);
  return {
    rule: "R-4",
    pass: r4Holds(weapons),
    detail:
      doms.length === 0
        ? "no weapon is top-tier in range, speed, and damage"
        : doms.map((d) => `${d.dominator} ⪰ ${d.dominated}`).join("; "),
  };
}

/** R-5: no net-positive AP cycle in the cancel graph (spec §3.5.3). */
export function checkR5(moves: MoveList): RuleResult {
  const cycles = findPositiveApCycles(moves);
  return {
    rule: "R-5",
    pass: r5Holds(moves),
    detail:
      cycles.length === 0
        ? "every cancel cycle strictly drains AP"
        : cycles.map((c) => `[${c.nodes.join("→")}] net ${c.netAp}`).join("; "),
  };
}

export function checkAllRules(moves: MoveList, weapons: readonly Weapon[]): readonly RuleResult[] {
  return [checkR1(moves), checkR2(), checkR3(), checkR4(weapons), checkR5(moves)];
}
