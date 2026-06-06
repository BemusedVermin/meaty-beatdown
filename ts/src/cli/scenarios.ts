/**
 * scenarios.ts — scripted scenarios, incl. the spec's worked example + sidestep/AP addendum [edge].
 */
import { fromInt } from "../core/fixed";
import { type Entity, type Resources } from "../core/entity";
import { type SpatialState, type Facing } from "../core/spatial-types";
import { type Action, type Agent, type MatchState, type MoveTable, type MatchOptions } from "../core/engine";
import { toMoveTable } from "../moves/move";
import { REZA_MOVES, BORIN_MOVES, ALL_MOVES } from "../content/sample";
import { ScriptedAgent } from "./agents";

export interface Scenario {
  readonly id: string;
  readonly title: string;
  readonly initial: MatchState;
  readonly tables: readonly [MoveTable, MoveTable];
  readonly agents: readonly [Agent, Agent];
  readonly names: Readonly<Record<string, string>>;
  readonly fighters: readonly [string, string];
  readonly options: MatchOptions;
}

const MOVE = (moveId: string): Action => ({ kind: "MOVE", moveId });
const WAIT: Action = { kind: "WAIT" };

const NAMES: Readonly<Record<string, string>> = Object.fromEntries(ALL_MOVES.map((m) => [m.id, m.name]));

function spatial(pos: number, facing: Facing): SpatialState {
  return { pos: fromInt(pos), offset: fromInt(0), height: fromInt(1), facing };
}

function resources(o: Partial<Resources>): Resources {
  return {
    hp: 100,
    hpMax: 100,
    stamina: 99,
    staminaMax: 99,
    poise: 30,
    poiseMax: 30,
    focus: 20,
    focusMax: 20,
    ap: 5,
    apMax: 5,
    ...o,
  };
}

function fighter(id: string, pos: number, facing: Facing, res: Partial<Resources> = {}): Entity {
  return {
    id,
    state: { kind: "NEUTRAL" },
    readyTick: 0,
    resources: resources(res),
    spatial: spatial(pos, facing),
    comboCount: 0,
  };
}

const OPTS: MatchOptions = { maxTicks: 200, maxDecisions: 30 };

/** Worked example: neutral mind-read → armor absorbs Reza's poke → regime flips → counter-hit. */
function rezaVsBorin(): Scenario {
  return {
    id: "reza-borin",
    title: "Reza (dagger) vs Borin (greatsword) — neutral · armor · regime flip · counter-hit",
    initial: { t: 0, entities: [fighter("Reza", 0, 1), fighter("Borin", 1, -1, { hp: 120, poise: 36 })] },
    tables: [toMoveTable(REZA_MOVES), toMoveTable(BORIN_MOVES)],
    agents: [
      new ScriptedAgent([MOVE("light_slash"), MOVE("throw_grab"), WAIT, WAIT, WAIT]),
      new ScriptedAgent([MOVE("heavy_cleave"), WAIT, WAIT]),
    ],
    names: NAMES,
    fighters: ["Reza", "Borin"],
    options: OPTS,
  };
}

/** Addendum: AP tempo string — tempo jab → cancel homing → finisher cancel runs out of AP. */
function sidestepAp(): Scenario {
  return {
    id: "sidestep-ap",
    title: "Tempo / AP economy — jab → cancel homing → finisher denied (out of AP)",
    initial: {
      t: 0,
      // Borin plays a low-AP "tempo" build (apMax 3): jab(1) + homing-cancel(2) drains it, so the
      // heavy-finisher cancel (3 AP) can't be paid for → the string ends on the AP axis (governor 4).
      entities: [fighter("Borin", 0, 1, { ap: 3, apMax: 3 }), fighter("Reza", 1, -1)],
    },
    tables: [toMoveTable(BORIN_MOVES), toMoveTable(REZA_MOVES)],
    agents: [
      new ScriptedAgent([MOVE("tempo_jab"), WAIT, WAIT], [MOVE("homing_sweep"), MOVE("heavy_cleave")]),
      new ScriptedAgent([WAIT, WAIT, WAIT, WAIT, WAIT]),
    ],
    names: NAMES,
    fighters: ["Borin", "Reza"],
    options: OPTS,
  };
}

// Scenario builders, not instances: agents are stateful (ScriptedAgent holds an index), so every run
// must construct a FRESH scenario. allScenarios()/scenarioById() build on each call.
const REGISTRY: readonly (() => Scenario)[] = [rezaVsBorin, sidestepAp];

export function allScenarios(): readonly Scenario[] {
  return REGISTRY.map((build) => build());
}

export function scenarioById(id: string): Scenario | undefined {
  return allScenarios().find((s) => s.id === id);
}

export const SCENARIO_IDS: readonly string[] = allScenarios().map((s) => s.id);
