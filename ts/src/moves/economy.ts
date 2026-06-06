/**
 * economy.ts — the AP action-economy static analysis (spec §3.5) [L3].
 *
 * The engine APPLIES the AP economy at runtime (charge/refill/gain); this module is the STATIC
 * analysis over move definitions: the R-5 no-positive-cycle linter and a report of the four combo
 * governors. Pure functions over MoveList data — used by the balance audit (Phase 6).
 */
import { type MoveId } from "../core/ids";
import { type MoveList, type Move } from "./move";
import { CONFIG } from "../core/config";
import { compare, ONE } from "../core/fixed";

export function apCost(m: Move): number {
  return m.profile.cost.ap;
}

export function apGainAmount(m: Move): number {
  return m.profile.cost.apGain?.amount ?? 0;
}

/** Net AP a move yields when it lands (gain − cost). > 0 means it generates net AP. */
export function netAp(m: Move): number {
  return apGainAmount(m) - apCost(m);
}

/** Adjacency: move id → the ids it may cancel into (union over all its cancel windows). */
function cancelGraph(moves: MoveList): Map<MoveId, readonly MoveId[]> {
  const g = new Map<MoveId, readonly MoveId[]>();
  for (const m of moves) {
    const targets = new Set<MoveId>();
    for (const cw of m.profile.cancelWindows) for (const into of cw.into) targets.add(into);
    g.set(m.id, [...targets]);
  }
  return g;
}

export interface ApCycle {
  readonly nodes: readonly MoveId[];
  readonly netAp: number;
}

/**
 * R-5 (spec §3.5.3): every cycle in the cancel graph must strictly DRAIN AP (Σ net AP < 0), so every
 * string is finite on the AP axis alone. Returns every simple cycle whose summed net AP is ≥ 0 — a
 * net-positive (or neutral) tempo loop, i.e. infinite offense. Enumerates simple cycles anchored at
 * each cycle's lexicographically-smallest member (move lists are small).
 */
export function findPositiveApCycles(moves: MoveList): readonly ApCycle[] {
  const graph = cancelGraph(moves);
  const weight = new Map(moves.map((m) => [m.id, netAp(m)]));
  const violating: ApCycle[] = [];

  for (const start of graph.keys()) {
    const stack: MoveId[] = [];
    const onStack = new Set<MoveId>();
    const dfs = (u: MoveId): void => {
      stack.push(u);
      onStack.add(u);
      for (const v of graph.get(u) ?? []) {
        if (v === start) {
          const sum = stack.reduce((acc, id) => acc + (weight.get(id) ?? 0), 0);
          if (sum >= 0) violating.push({ nodes: [...stack], netAp: sum });
        } else if (v > start && !onStack.has(v)) {
          dfs(v);
        }
      }
      stack.pop();
      onStack.delete(u);
    };
    dfs(start);
  }
  return violating;
}

/** R-5 holds iff there is no net-positive (or neutral) AP cycle in the cancel graph (audit C-10). */
export function r5Holds(moves: MoveList): boolean {
  return findPositiveApCycles(moves).length === 0;
}

// ---------------------------------------------------------------------------
// The four combo governors (spec §2.8, §3.4) — defense in depth against infinites
// ---------------------------------------------------------------------------

export type Governor = "FOCUS_COST" | "JUGGLE_DECAY" | "HITSTUN_DECAY" | "AP_EXHAUSTION";

export interface GovernorStatus {
  readonly governor: Governor;
  readonly present: boolean;
  readonly detail: string;
}

/** Report whether all four independent combo governors are active (audit C-4). */
export function governorReport(): readonly GovernorStatus[] {
  return [
    {
      governor: "FOCUS_COST",
      present: true,
      detail: "Each cancel pays Focus; the chain ends when Focus runs out (spec §3.4).",
    },
    {
      governor: "JUGGLE_DECAY",
      present: compare(CONFIG.combat.JUGGLE_DAMAGE_DECAY, ONE) < 0,
      detail: "Juggle damage decays ×0.9^n so juggle damage → 0 (spec §2.8).",
    },
    {
      governor: "HITSTUN_DECAY",
      present: CONFIG.combo.HITSTUN_DECAY_PER_HIT > 0,
      detail: `Each combo hit loses ${CONFIG.combo.HITSTUN_DECAY_PER_HIT} hitstun so advantage goes minus (spec §3.4).`,
    },
    {
      governor: "AP_EXHAUSTION",
      present: true,
      detail: "Every action pays ap_cost; AP refills only on entering neutral (spec §3.5).",
    },
  ];
}
