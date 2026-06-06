/**
 * scenarios.ts — short, focused scenarios that become golden vectors [golden, edge].
 *
 * Each builds a tiny deterministic match that exercises one interaction (a defensive option, a combo
 * governor terminating, a counter-hit punish) plus the two worked examples. Built FRESH per call
 * (ScriptedAgent is stateful). These are the cross-language behavioral contract.
 */
import { fromInt, fromRatio } from "../core/fixed";
import { type Entity, type Resources } from "../core/entity";
import { type Facing } from "../core/spatial-types";
import {
  type Action,
  type Agent,
  type DecisionResult,
  type MatchState,
  type MoveTable,
  type MatchOptions,
} from "../core/engine";
import { type FrameProfile } from "../core/frameprofile";
import { type CancelWindow } from "../core/cost";
import { frame } from "../content/builders";
import { ScriptedAgent } from "../cli/agents";
import { scenarioById } from "../cli/scenarios";

export interface RunnableScenario {
  readonly id: string;
  readonly initial: MatchState;
  readonly tables: readonly [MoveTable, MoveTable];
  readonly agents: readonly [Agent, Agent];
  readonly options: MatchOptions;
}

const MOVE = (id: string): Action => ({ kind: "MOVE", moveId: id });
const WAIT: Action = { kind: "WAIT" };
const SHORT: MatchOptions = { maxTicks: 150, maxDecisions: 10 };

function res(o: Partial<Resources> = {}): Resources {
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

function ent(id: string, pos: number, facing: Facing, o: Partial<Resources> = {}): Entity {
  return {
    id,
    state: { kind: "NEUTRAL" },
    readyTick: 0,
    resources: res(o),
    spatial: { pos: fromInt(pos), offset: fromInt(0), height: fromInt(1), facing },
    comboCount: 0,
  };
}

const tbl = (r: Record<string, FrameProfile>): MoveTable => new Map(Object.entries(r));
const onHit = (into: string, from: number, to: number, c: Partial<{ ap: number; focus: number }> = {}): CancelWindow => ({
  from,
  to,
  gate: "ON_HIT",
  into: [into],
  cost: { ap: c.ap ?? 0, apGain: null, stamina: 0, focus: c.focus ?? 0 },
});

function sc(
  id: string,
  e0: Entity,
  e1: Entity,
  t0: Record<string, FrameProfile>,
  t1: Record<string, FrameProfile>,
  a0: Action[],
  a1: Action[],
  c0: DecisionResult[] = [],
  options: MatchOptions = SHORT,
): RunnableScenario {
  return {
    id,
    initial: { t: 0, entities: [e0, e1] },
    tables: [tbl(t0), tbl(t1)],
    agents: [new ScriptedAgent(a0, c0), new ScriptedAgent(a1)],
    options,
  };
}

// ---- reusable little moves --------------------------------------------------------------------

const poke = frame({
  timing: { startup: 3, active: 2, recovery: 6 },
  hitEffect: { damage: 10, hitstun: 9, blockstun: 5, knockback: fromInt(1) },
  reach: { maxRange: fromInt(2), lateralBand: fromRatio(1, 2) },
  cost: { stamina: 4, ap: 1 },
});
const grab = frame({
  timing: { startup: 3, active: 2, recovery: 6 },
  level: "THROW",
  hitEffect: { damage: 18, hitstun: 20, knockdown: true },
  reach: { maxRange: fromInt(1) },
  cost: { stamina: 6, ap: 1 },
});

// ---- the focused scenarios --------------------------------------------------------------------

const FOCUSED: Readonly<Record<string, () => RunnableScenario>> = {
  block: () => {
    const guard = frame({
      timing: { startup: 1, active: 20, recovery: 4 },
      reach: { maxRange: fromInt(0) },
      properties: [{ kind: "BLOCK", covers: ["HIGH", "MID"], window: { from: 0, to: 24 } }],
      cost: { stamina: 3 },
    });
    return sc("block", ent("A", 0, 1), ent("B", 1, -1), { poke }, { guard }, [MOVE("poke")], [MOVE("guard")]);
  },

  parry: () => {
    const parry = frame({
      timing: { startup: 1, active: 6, recovery: 12 },
      reach: { maxRange: fromInt(0) },
      properties: [{ kind: "GUARD_POINT", window: { from: 1, to: 6 } }],
      cost: { focus: 3 },
    });
    return sc("parry", ent("A", 0, 1), ent("B", 1, -1), { poke }, { parry }, [MOVE("poke")], [MOVE("parry")]);
  },

  throw: () =>
    sc("throw", ent("A", 0, 1), ent("B", 1, -1), { grab }, { poke }, [MOVE("grab")], [WAIT, WAIT, WAIT]),

  "throw-tech": () =>
    sc("throw-tech", ent("A", 0, 1), ent("B", 1, -1), { grab }, { grab }, [MOVE("grab")], [MOVE("grab")]),

  "sidestep-linear": () => {
    const slowPoke = frame({
      timing: { startup: 6, active: 2, recovery: 6 },
      hitEffect: { damage: 10, hitstun: 9, knockback: fromInt(1) },
      reach: { maxRange: fromInt(2), lateralBand: fromRatio(1, 2) }, // LINEAR
      cost: { stamina: 4 },
    });
    const sidestep = frame({
      timing: { startup: 3, active: 2, recovery: 7 },
      reach: { maxRange: fromInt(0) },
      motion: { lane: fromInt(0), offset: fromInt(-1) },
      cost: { stamina: 4 },
    });
    return sc(
      "sidestep-linear",
      ent("A", 0, 1),
      ent("B", 1, -1),
      { slowPoke },
      { sidestep },
      [MOVE("slowPoke")],
      [MOVE("sidestep")],
    );
  },

  "sidestep-homing": () => {
    const homing = frame({
      timing: { startup: 6, active: 2, recovery: 6 },
      hitEffect: { damage: 12, hitstun: 12, knockback: fromInt(1) },
      reach: { maxRange: fromInt(2), stepIn: fromInt(2), trackSide: 0 }, // HOMING realigns
      cost: { stamina: 5, ap: 1 },
    });
    const sidestep = frame({
      timing: { startup: 3, active: 2, recovery: 7 },
      reach: { maxRange: fromInt(0) },
      motion: { lane: fromInt(0), offset: fromInt(-1) },
      cost: { stamina: 4 },
    });
    return sc(
      "sidestep-homing",
      ent("A", 0, 1),
      ent("B", 1, -1),
      { homing },
      { sidestep },
      [MOVE("homing")],
      [MOVE("sidestep")],
    );
  },

  "counter-hit": () => {
    const slow = frame({
      timing: { startup: 12, active: 3, recovery: 12 },
      hitEffect: { damage: 20, hitstun: 16 },
      reach: { maxRange: fromInt(2) },
      cost: { stamina: 6, ap: 1 },
    });
    return sc("counter-hit", ent("A", 0, 1), ent("B", 1, -1), { poke }, { slow }, [MOVE("poke")], [MOVE("slow")]);
  },

  "governor-ap": () => {
    const opener = frame({
      timing: { startup: 4, active: 2, recovery: 12 },
      hitEffect: { damage: 8, hitstun: 10 },
      reach: { maxRange: fromInt(2) },
      cost: { stamina: 4 },
      cancelWindows: [onHit("follow", 6, 17, { ap: 5 })], // costs more AP than the 3-AP attacker has
    });
    const follow = frame({ reach: { maxRange: fromInt(0) }, cost: { stamina: 4 } });
    const stance = frame({ timing: { startup: 2, active: 40, recovery: 5 }, reach: { maxRange: fromInt(0) } });
    return sc(
      "governor-ap",
      ent("A", 0, 1, { ap: 3, apMax: 3 }),
      ent("B", 1, -1),
      { opener, follow },
      { stance },
      [MOVE("opener")],
      [MOVE("stance")],
      [MOVE("follow")], // tries to cancel; can't afford → DENIED
    );
  },

  "governor-focus": () => {
    const opener = frame({
      timing: { startup: 4, active: 2, recovery: 12 },
      hitEffect: { damage: 8, hitstun: 10 },
      reach: { maxRange: fromInt(2) },
      cost: { stamina: 4 },
      cancelWindows: [onHit("follow", 6, 17, { focus: 5 })], // costs more Focus than available
    });
    const follow = frame({ reach: { maxRange: fromInt(0) }, cost: { stamina: 4 } });
    const stance = frame({ timing: { startup: 2, active: 40, recovery: 5 }, reach: { maxRange: fromInt(0) } });
    return sc(
      "governor-focus",
      ent("A", 0, 1, { focus: 3, focusMax: 3 }),
      ent("B", 1, -1),
      { opener, follow },
      { stance },
      [MOVE("opener")],
      [MOVE("stance")],
      [MOVE("follow")],
    );
  },

  "governor-juggle": () => {
    const launcher = frame({
      timing: { startup: 3, active: 2, recovery: 10 },
      hitEffect: { damage: 20, hitstun: 18, launches: true },
      reach: { maxRange: fromInt(2) },
      cost: { stamina: 5, ap: 1 },
      cancelWindows: [onHit("juggle1", 5, 14, { ap: 1 })],
    });
    const juggle1 = frame({
      timing: { startup: 2, active: 2, recovery: 8 },
      hitEffect: { damage: 20, hitstun: 16 },
      reach: { maxRange: fromInt(2) },
      cost: { stamina: 5, ap: 1 },
      cancelWindows: [onHit("juggle2", 4, 11, { ap: 1 })],
    });
    const juggle2 = frame({
      timing: { startup: 2, active: 2, recovery: 8 },
      hitEffect: { damage: 20, hitstun: 14 },
      reach: { maxRange: fromInt(2) },
      cost: { stamina: 5, ap: 1 },
    });
    return sc(
      "governor-juggle",
      ent("A", 0, 1),
      ent("B", 1, -1),
      { launcher, juggle1, juggle2 },
      { poke },
      [MOVE("launcher")],
      [WAIT, WAIT, WAIT],
      [MOVE("juggle1"), MOVE("juggle2")],
    );
  },

  "governor-hitstun": () => {
    const rekka = frame({
      timing: { startup: 2, active: 2, recovery: 4 },
      hitEffect: { damage: 6, hitstun: 12 },
      reach: { maxRange: fromInt(2) },
      cost: { stamina: 3, ap: 1 },
      cancelWindows: [onHit("rekka", 4, 7, { ap: 0 })], // self-cancel (net-negative AP)
    });
    return sc(
      "governor-hitstun",
      ent("A", 0, 1),
      ent("B", 1, -1),
      { rekka },
      { poke },
      [MOVE("rekka")],
      [WAIT, WAIT, WAIT, WAIT, WAIT],
      [MOVE("rekka"), MOVE("rekka"), MOVE("rekka")], // chained; hitstun decays each hit
    );
  },
};

function workedExamples(): readonly RunnableScenario[] {
  return ["reza-borin", "sidestep-ap"].map((id) => {
    const s = scenarioById(id)!;
    return { id, initial: s.initial, tables: s.tables, agents: s.agents, options: s.options };
  });
}

/** Build all golden scenarios fresh (worked examples first, then the focused interactions). */
export function goldenScenarios(): readonly RunnableScenario[] {
  return [...workedExamples(), ...Object.values(FOCUSED).map((build) => build())];
}
