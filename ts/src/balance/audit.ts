/**
 * audit.ts — the automated consistency & fun audit (spec Appendix B) [tooling].
 *
 * Runs every C-1..C-11 and R-1..R-5 check over the sample content and prints a pass/fail row per id.
 * Some invariants are enforced structurally (by dependency-cruiser / the engine / config) — those rows
 * cite their enforcement mechanism. Run with `npm run audit`; exits non-zero if any check fails.
 *
 * This file is a top-level tooling edge: it uses console/process (globals, not imports) and imports
 * only data + pure engine functions. It is not imported by any test (the rule functions are unit-
 * tested in budget.test.ts), so its top-level print runs only when executed as a script.
 */
import { fromInt } from "../core/fixed";
import { onHit, checkFrameProfile, totalFrames } from "../core/frameprofile";
import { type SpatialState } from "../core/spatial-types";
import { computeRegime } from "../core/regime";
import {
  type Action,
  type Agent,
  type MatchState,
  type PlayerView,
  type CancelView,
  type DecisionResult,
  runMatch,
} from "../core/engine";
import { doesHit } from "../spatial/lane";
import { CONFIG } from "../core/config";
import { toMoveTable } from "../moves/move";
import { governorReport } from "../moves/economy";
import { checkAllRules, budgetReport } from "./budget";
import { ALL_MOVES, REZA_MOVES, BORIN_MOVES, WEAPONS, moveById } from "../content/sample";

export interface AuditRow {
  readonly id: string;
  readonly pass: boolean;
  readonly detail: string;
}

const spatial = (pos: number, offset: number, facing: 1 | -1): SpatialState => ({
  pos: fromInt(pos),
  offset: fromInt(offset),
  height: fromInt(1),
  facing,
});

// A scripted agent for the determinism match (C-3).
class Scripted implements Agent {
  private i = 0;
  constructor(private readonly script: readonly Action[]) {}
  chooseAction(_v: PlayerView): Action {
    return this.script[this.i++] ?? { kind: "WAIT" };
  }
  chooseCancel(_v: CancelView): DecisionResult {
    return { kind: "DECLINE" };
  }
}

function determinismMatch(): MatchState {
  return {
    t: 0,
    entities: [
      {
        id: "reza",
        state: { kind: "NEUTRAL" },
        readyTick: 0,
        resources: { hp: 100, hpMax: 100, stamina: 99, staminaMax: 99, poise: 30, poiseMax: 30, focus: 20, focusMax: 20, ap: 5, apMax: 5 },
        spatial: spatial(0, 0, 1),
        comboCount: 0,
      },
      {
        id: "borin",
        state: { kind: "NEUTRAL" },
        readyTick: 0,
        resources: { hp: 120, hpMax: 120, stamina: 99, staminaMax: 99, poise: 36, poiseMax: 36, focus: 10, focusMax: 10, ap: 5, apMax: 5 },
        spatial: spatial(1, 0, -1),
        comboCount: 0,
      },
    ],
  };
}

function runDeterminismCheck(): boolean {
  const tables = [toMoveTable(REZA_MOVES), toMoveTable(BORIN_MOVES)] as const;
  const run = () =>
    runMatch(
      determinismMatch(),
      tables,
      [new Scripted([{ kind: "MOVE", moveId: "light_jab" }, { kind: "WAIT" }]), new Scripted([{ kind: "MOVE", moveId: "tempo_jab" }, { kind: "WAIT" }])],
      { maxTicks: 300, maxDecisions: 60 },
    );
  return JSON.stringify(run().trace) === JSON.stringify(run().trace);
}

export function runAudit(): readonly AuditRow[] {
  const rows: AuditRow[] = [];

  // C-1 — advantage is derived from stun − recovery (no on_hit field); all profiles valid.
  const attacks = ALL_MOVES.filter((m) => m.profile.hitEffect.hitstun > 0);
  const i1Holds = attacks.every(
    (m) => onHit(m.profile) === m.profile.hitEffect.hitstun - m.profile.timing.recovery,
  );
  const allValid = ALL_MOVES.every((m) => checkFrameProfile(m.profile).length === 0);
  rows.push({
    id: "C-1",
    pass: i1Holds && allValid,
    detail: i1Holds && allValid ? "on_hit = hitstun − recovery for every move; all profiles valid (I-1)" : "I-1 / validity violated",
  });

  // C-2 — neutral vs pressure both derive from ready_tick (no special-casing).
  const c2 =
    computeRegime(
      { ...determinismMatch().entities[0], readyTick: 5 },
      { ...determinismMatch().entities[1], readyTick: 5 },
    ).kind === "NEUTRAL" &&
    computeRegime(
      { ...determinismMatch().entities[0], readyTick: 3 },
      { ...determinismMatch().entities[1], readyTick: 9 },
    ).kind === "PRESSURE";
  rows.push({ id: "C-2", pass: c2, detail: "NEUTRAL/PRESSURE both come from a ready_tick comparison" });

  // C-3 — deterministic, single resolution (byte-identical replay).
  const c3 = runDeterminismCheck();
  rows.push({ id: "C-3", pass: c3, detail: c3 ? "same inputs ⇒ byte-identical trace (run twice)" : "non-determinism detected" });

  // C-4 — four independent combo governors present.
  const governors = governorReport();
  rows.push({
    id: "C-4",
    pass: governors.every((g) => g.present),
    detail: governors.map((g) => g.governor).join(", "),
  });

  // C-5 — single L4→engine bridge (enforced by dependency-cruiser).
  rows.push({ id: "C-5", pass: true, detail: "only rpg/compiler.ts bridges L4→core — enforced by `npm run depcruise`" });

  // C-6 — react-to-reveal closed (lock-then-confirm + no startup cancels by default).
  rows.push({
    id: "C-6",
    pass: CONFIG.features.STARTUP_CANCELABLE_BY_DEFAULT === false,
    detail: "neutral commits hidden; startup cancels disallowed by default (engine)",
  });

  // C-7 — contact via the single doesHit predicate (enforced: only spatial/lane.ts computes contact).
  rows.push({ id: "C-7", pass: true, detail: "all range/lateral/height math lives behind spatial/lane.ts doesHit" });

  // C-8 — lane + sidestep: a single sidestep (offset 1) dodges a LINEAR move.
  const linear = moveById("light_jab").profile;
  const onAxisHit = doesHit(spatial(0, 0, 1), spatial(1, 0, -1), linear.reach, linear.level);
  const steppedMiss = !doesHit(spatial(0, 0, 1), spatial(1, 1, -1), linear.reach, linear.level);
  rows.push({ id: "C-8", pass: onAxisHit && steppedMiss, detail: "a single sidestep whiffs a LINEAR move (on-axis it connects)" });

  // C-9 — WWN identity preserved where kept; deterministic combat (no d20 to-hit).
  rows.push({
    id: "C-9",
    pass: CONFIG.features.DAMAGE_VARIANCE === false,
    detail: "skills/foci/low-mods kept; combat hit/miss is deterministic (no d20), damage variance off",
  });

  // C-10 — no net-positive AP cycle (R-5).
  const r5 = checkAllRules(ALL_MOVES, WEAPONS).find((r) => r.rule === "R-5")!;
  rows.push({ id: "C-10", pass: r5.pass, detail: r5.detail });

  // C-11 — sidestep counterplay: LINEAR is dodged but a HOMING move connects through the step.
  const homing = moveById("homing_sweep").profile;
  const homingConnects = doesHit(spatial(0, 0, 1), spatial(1, 1, -1), homing.reach, homing.level);
  const linearDodged = !doesHit(spatial(0, 0, 1), spatial(1, 1, -1), linear.reach, linear.level);
  rows.push({ id: "C-11", pass: homingConnects && linearDodged, detail: "sidestep dodges LINEAR, HOMING realigns and connects" });

  // R-1..R-5 over the content.
  for (const r of checkAllRules(ALL_MOVES, WEAPONS)) {
    rows.push({ id: r.rule, pass: r.pass, detail: r.detail });
  }

  return rows;
}

function printAudit(rows: readonly AuditRow[]): boolean {
  const idW = Math.max(...rows.map((r) => r.id.length));
  const log = (s: string): void => console.log(s);
  log("");
  log("  TICK — consistency & fun audit (spec Appendix B)");
  log("  " + "─".repeat(72));
  for (const r of rows) {
    log(`  ${r.id.padEnd(idW)}  ${r.pass ? "PASS" : "FAIL"}  ${r.detail}`);
  }
  log("  " + "─".repeat(72));

  // Budget identity (informational): flag over/under-budget moves per archetype.
  for (const [name, list] of [["Reza", REZA_MOVES], ["Borin", BORIN_MOVES]] as const) {
    const rep = budgetReport(list);
    log(`  budget(${name}): mean ${rep.budget.toFixed(1)} — outliers: ${rep.outliers.length ? rep.outliers.join(", ") : "none"}`);
  }
  log(`  ${ALL_MOVES.length} moves, ${WEAPONS.length} weapons, total frames audited: ${ALL_MOVES.reduce((s, m) => s + totalFrames(m.profile.timing), 0)}`);

  const passed = rows.every((r) => r.pass);
  log("");
  log(`  RESULT: ${passed ? "ALL CHECKS PASS" : "SOME CHECKS FAILED"} (${rows.filter((r) => r.pass).length}/${rows.length})`);
  log("");
  return passed;
}

// --- script entry: print the table and exit non-zero on any failure ---
const ok = printAudit(runAudit());
if (!ok) process.exitCode = 1;
