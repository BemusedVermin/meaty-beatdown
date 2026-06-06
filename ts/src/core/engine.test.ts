import { describe, it, expect } from "vitest";
import { fromInt, fromRatio } from "./fixed";
import { type FrameProfile, type Property } from "./frameprofile";
import { type Regime } from "./regime";
import {
  type Action,
  type Agent,
  type CancelView,
  type DecisionResult,
  type EntityIndex,
  type MatchState,
  type MoveTable,
  type PlayerView,
  type TraceEvent,
  runMatch,
} from "./engine";
import { makeProfile, makeEntity, makeCost } from "../test-support/fixtures";

// ---------------------------------------------------------------------------
// A scripted agent that records the regime it sees at each chooseAction call.
// ---------------------------------------------------------------------------

class ScriptAgent implements Agent {
  readonly regimes: Regime["kind"][] = [];
  readonly actors: (EntityIndex | null)[] = [];
  readonly views: PlayerView[] = [];
  private i = 0;
  constructor(
    private readonly script: readonly Action[],
    private readonly cancels: readonly DecisionResult[] = [],
  ) {}
  private ci = 0;

  chooseAction(view: PlayerView): Action {
    this.regimes.push(view.regime.kind);
    this.actors.push(view.regime.kind === "PRESSURE" ? view.regime.actor : null);
    this.views.push(view);
    return this.script[this.i++] ?? { kind: "WAIT" };
  }

  chooseCancel(_view: CancelView): DecisionResult {
    return this.cancels[this.ci++] ?? { kind: "DECLINE" };
  }
}

const table = (entries: Record<string, FrameProfile>): MoveTable => new Map(Object.entries(entries));
const MOVE = (moveId: string): Action => ({ kind: "MOVE", moveId });
const WAIT: Action = { kind: "WAIT" };
const OPTS = { maxTicks: 400, maxDecisions: 80 };

// ===========================================================================
// Regime flips (spec §2.1, audit C-2) + determinism (audit C-3)
// ===========================================================================

describe("regime flips and determinism", () => {
  // e0 jab: fast, hits e1 on a counter-hit; e1 slow: gets interrupted.
  const jab = makeProfile({
    timing: { startup: 2, active: 2, recovery: 4 },
    hitEffect: { damage: 10, hitstun: 12 },
    reach: { maxRange: fromInt(3) },
  });
  const slow = makeProfile({
    timing: { startup: 10, active: 3, recovery: 10 },
    reach: { maxRange: fromInt(3) },
  });

  const setup = (): {
    state: MatchState;
    tables: readonly [MoveTable, MoveTable];
  } => ({
    state: {
      t: 0,
      entities: [
        makeEntity({ id: "e0", spatial: { pos: fromInt(0), offset: fromInt(0), height: fromInt(1), facing: 1 } }),
        makeEntity({ id: "e1", spatial: { pos: fromInt(1), offset: fromInt(0), height: fromInt(1), facing: -1 } }),
      ],
    },
    tables: [table({ jab }), table({ slow })],
  });

  it("starts NEUTRAL then flips to PRESSURE for the faster fighter", () => {
    const { state, tables } = setup();
    const a0 = new ScriptAgent([MOVE("jab"), WAIT, WAIT, WAIT]);
    const a1 = new ScriptAgent([MOVE("slow"), WAIT]);
    const result = runMatch(state, tables, [a0, a1], OPTS);

    expect(a0.regimes[0]).toBe("NEUTRAL"); // both free at t=0
    expect(a1.regimes[0]).toBe("NEUTRAL");
    expect(a0.regimes[1]).toBe("PRESSURE"); // e0 recovers first, e1 stuck in counter-hit stun
    expect(a0.actors[1]).toBe(0);

    // The jab counter-hit landed (e1 took 13 = round(10×1.25) and was knocked out of its move).
    expect(result.trace.some((e) => e.kind === "CONTACT" && e.result === "HIT" && e.counter)).toBe(true);
    expect(result.finalState.entities[1]!.resources.hp).toBe(87);
  });

  it("is byte-identical across two runs (same inputs ⇒ same trace + final state)", () => {
    const run = (): { trace: readonly TraceEvent[]; finalState: MatchState } => {
      const { state, tables } = setup();
      const r = runMatch(state, tables, [new ScriptAgent([MOVE("jab"), WAIT, WAIT]), new ScriptAgent([MOVE("slow"), WAIT])], OPTS);
      return { trace: r.trace, finalState: r.finalState };
    };
    const r1 = run();
    const r2 = run();
    expect(r2.trace).toEqual(r1.trace);
    expect(r2.finalState).toEqual(r1.finalState);
  });
});

// ===========================================================================
// Throws beat armor (decision 1) — integration through the resolution loop
// ===========================================================================

describe("throws beat armor through the engine (decision 1)", () => {
  const armorProp: Property = {
    kind: "ARMOR",
    armorHits: 3,
    armorDamageMult: fromRatio(1, 2),
    window: { from: 0, to: 46 },
  };
  // e1 sits in an armored stance whose own attack never reaches (maxRange 0).
  const armorStance = makeProfile({
    timing: { startup: 2, active: 40, recovery: 5 },
    reach: { maxRange: fromInt(0) },
    properties: [armorProp],
  });
  const grab = makeProfile({
    timing: { startup: 3, active: 2, recovery: 6 },
    level: "THROW",
    hitEffect: { damage: 20, hitstun: 20 },
    reach: { maxRange: fromInt(2), lateralBand: fromRatio(1, 2) },
  });
  const poke = makeProfile({
    timing: { startup: 3, active: 2, recovery: 6 },
    level: "MID",
    hitEffect: { damage: 20, hitstun: 12 },
    reach: { maxRange: fromInt(2), lateralBand: fromRatio(1, 2) },
  });

  const run = (e0Move: string, e0Table: MoveTable) => {
    const state: MatchState = {
      t: 0,
      entities: [
        makeEntity({ id: "e0", spatial: { pos: fromInt(0), offset: fromInt(0), height: fromInt(1), facing: 1 } }),
        makeEntity({ id: "e1", spatial: { pos: fromInt(1), offset: fromInt(0), height: fromInt(1), facing: -1 } }),
      ],
    };
    return runMatch(state, [e0Table, table({ armorStance })], [
      new ScriptAgent([MOVE(e0Move), WAIT, WAIT]),
      new ScriptAgent([MOVE("armorStance"), WAIT]),
    ], OPTS);
  };

  it("a throw connects through armor (THROWN), while the same-spaced strike is ARMORED", () => {
    const thrown = run("grab", table({ grab }));
    expect(thrown.trace.some((e) => e.kind === "CONTACT" && e.result === "THROWN")).toBe(true);
    expect(thrown.trace.some((e) => e.kind === "CONTACT" && e.result === "ARMORED")).toBe(false);

    const armored = run("poke", table({ poke }));
    expect(armored.trace.some((e) => e.kind === "CONTACT" && e.result === "ARMORED")).toBe(true);
    expect(armored.trace.some((e) => e.kind === "CONTACT" && e.result === "THROWN")).toBe(false);
  });
});

// ===========================================================================
// §2.10 information rules: hidden neutral commit (audit C-6)
// ===========================================================================

describe("§2.10 — the neutral commit is hidden (no react-to-reveal)", () => {
  it("each agent's neutral view shows no pending opponent action", () => {
    const move = makeProfile({ reach: { maxRange: fromInt(0) } }); // out of range; no contacts
    const state: MatchState = {
      t: 0,
      entities: [
        makeEntity({ id: "e0", spatial: { pos: fromInt(0), offset: fromInt(0), height: fromInt(1), facing: 1 } }),
        makeEntity({ id: "e1", spatial: { pos: fromInt(5), offset: fromInt(0), height: fromInt(1), facing: -1 } }),
      ],
    };
    const a0 = new ScriptAgent([MOVE("m"), WAIT]);
    const a1 = new ScriptAgent([MOVE("m"), WAIT]);
    runMatch(state, [table({ m: move }), table({ m: move })], [a0, a1], { maxTicks: 50, maxDecisions: 4 });

    const v0 = a0.views[0]!;
    const v1 = a1.views[0]!;
    expect(v0.regime.kind).toBe("NEUTRAL");
    expect(v1.regime.kind).toBe("NEUTRAL");
    // Both views are built from the same pre-reveal state: neither opponent has committed yet.
    expect(v0.opponent.currentMove).toBeNull();
    expect(v1.opponent.currentMove).toBeNull();
    // The snapshot has no field that could leak the opponent's chosen action.
    expect(Object.keys(v0.opponent).sort()).toEqual(
      ["currentMove", "id", "readyTick", "spatial", "stateTag"].sort(),
    );
  });
});

// ===========================================================================
// No startup cancels by default (decision 6, spec §2.10) — closes react-to-reveal
// ===========================================================================

describe("no startup cancels unless flagged (decision 6)", () => {
  const poke2 = makeProfile({ timing: { startup: 4, active: 2, recovery: 4 }, reach: { maxRange: fromInt(0) } });
  const longMove = makeProfile({ timing: { startup: 20, active: 2, recovery: 20 }, reach: { maxRange: fromInt(0) } });

  const run = (startupCancelable: boolean): readonly TraceEvent[] => {
    const poke = makeProfile({
      timing: { startup: 6, active: 2, recovery: 6 }, // total 14; cancel window spans the whole move
      reach: { maxRange: fromInt(0) },
      // ALWAYS gate so contact (none, out of range) does not block the cancel.
      cancelWindows: [{ from: 0, to: 13, gate: "ALWAYS", into: ["poke2"], cost: makeCost() }],
      startupCancelable,
    });
    const state: MatchState = {
      t: 0,
      entities: [
        makeEntity({ id: "e0", spatial: { pos: fromInt(0), offset: fromInt(0), height: fromInt(1), facing: 1 } }),
        makeEntity({ id: "e1", spatial: { pos: fromInt(20), offset: fromInt(0), height: fromInt(1), facing: -1 } }),
      ],
    };
    // e0 always tries to cancel into poke2; e1 is locked in a long move out of range.
    const a0 = new ScriptAgent([MOVE("poke"), WAIT, WAIT, WAIT], [MOVE("poke2")]);
    const a1 = new ScriptAgent([MOVE("long"), WAIT]);
    return runMatch(state, [table({ poke, poke2 }), table({ long: longMove })], [a0, a1], OPTS).trace;
  };

  it("a cancel offered over startup fires only once active begins (elapsed ≥ startup)", () => {
    const cancel = run(false).find((e) => e.kind === "CANCEL");
    expect(cancel).toBeDefined();
    // startup is 6, so the cancel must not occur before that.
    expect(cancel!.t).toBeGreaterThanOrEqual(6);
  });

  it("a startupCancelable move CAN be canceled during startup", () => {
    const cancel = run(true).find((e) => e.kind === "CANCEL");
    expect(cancel).toBeDefined();
    expect(cancel!.t).toBeLessThan(6);
  });
});

// ===========================================================================
// Phase 4 — AP economy: exhaustion governors, conditional ap_gain, parry refund
// ===========================================================================

describe("AP/Focus economy ends a cancel string (governors 1 & 4, spec §3.5)", () => {
  type ChainOpts = {
    apMax?: number;
    focusMax?: number;
    windowCost?: Partial<import("./cost").ResourceCost>;
    openerCost?: Partial<import("./cost").ResourceCost>;
    apGain?: import("./cost").ApGain;
  };

  // e0 'opener' hits the locked e1, then offers an ON_HIT cancel (in recovery) into 'follow'.
  const runChain = (o: ChainOpts): readonly TraceEvent[] => {
    const openerCost = o.apGain
      ? makeCost({ ...o.openerCost, apGain: o.apGain })
      : makeCost(o.openerCost);
    const opener = makeProfile({
      timing: { startup: 4, active: 2, recovery: 10 }, // total 16; recovery starts at elapsed 6
      reach: { maxRange: fromInt(2) },
      cost: openerCost,
      cancelWindows: [{ from: 6, to: 15, gate: "ON_HIT", into: ["follow"], cost: makeCost(o.windowCost) }],
    });
    const follow = makeProfile({ reach: { maxRange: fromInt(0) } });
    const stance = makeProfile({ timing: { startup: 2, active: 40, recovery: 5 }, reach: { maxRange: fromInt(0) } });
    const res = { ap: o.apMax ?? 99, apMax: o.apMax ?? 99, focus: o.focusMax ?? 99, focusMax: o.focusMax ?? 99 };
    const state: MatchState = {
      t: 0,
      entities: [
        makeEntity({ id: "e0", resources: res, spatial: { pos: fromInt(0), offset: fromInt(0), height: fromInt(1), facing: 1 } }),
        makeEntity({ id: "e1", spatial: { pos: fromInt(1), offset: fromInt(0), height: fromInt(1), facing: -1 } }),
      ],
    };
    const a0 = new ScriptAgent([MOVE("opener"), WAIT, WAIT], [MOVE("follow")]);
    const a1 = new ScriptAgent([MOVE("stance"), WAIT]);
    return runMatch(state, [table({ opener, follow }), table({ stance })], [a0, a1], OPTS).trace;
  };

  const cancelHappened = (trace: readonly TraceEvent[]): boolean => trace.some((e) => e.kind === "CANCEL");

  it("AP exhaustion: the cancel is refused when AP can't pay (governor 4)", () => {
    expect(cancelHappened(runChain({ apMax: 3, windowCost: { ap: 5 } }))).toBe(false);
    expect(cancelHappened(runChain({ apMax: 10, windowCost: { ap: 5 } }))).toBe(true);
  });

  it("Focus exhaustion: the cancel is refused when Focus can't pay (governor 1)", () => {
    expect(cancelHappened(runChain({ focusMax: 3, windowCost: { focus: 5 } }))).toBe(false);
    expect(cancelHappened(runChain({ focusMax: 10, windowCost: { focus: 5 } }))).toBe(true);
  });

  it("conditional ap_gain ON_HIT restores enough AP to keep the string alive (spec §3.5.2)", () => {
    // opener costs 4 AP; window costs 6. Without a refund, 8 − 4 = 4 < 6 → no cancel.
    const base = { apMax: 8, openerCost: { ap: 4 }, windowCost: { ap: 6 } } as const;
    expect(cancelHappened(runChain(base))).toBe(false);
    // With +3 AP ON_HIT, 4 + 3 = 7 ≥ 6 → the cancel is affordable.
    expect(cancelHappened(runChain({ ...base, apGain: { amount: 3, gate: "ON_HIT" } }))).toBe(true);
  });
});

describe("parry refunds both Focus and AP (decision 7, spec §2.6/§3.5.2)", () => {
  it("a successful parry tops up the parrier's Focus and AP", () => {
    const jab = makeProfile({ timing: { startup: 2, active: 2, recovery: 10 }, reach: { maxRange: fromInt(2) } });
    const parry = makeProfile({
      timing: { startup: 0, active: 6, recovery: 10 },
      reach: { maxRange: fromInt(0) }, // a stance: it does not attack
      properties: [{ kind: "GUARD_POINT", window: { from: 0, to: 5 } }],
      cost: makeCost({ ap: 4 }), // e1 spends 4 AP raising the parry
    });
    const state: MatchState = {
      t: 0,
      entities: [
        makeEntity({ id: "e0", spatial: { pos: fromInt(0), offset: fromInt(0), height: fromInt(1), facing: 1 } }),
        makeEntity({ id: "e1", resources: { focus: 0, focusMax: 10, ap: 10, apMax: 10 }, spatial: { pos: fromInt(1), offset: fromInt(0), height: fromInt(1), facing: -1 } }),
      ],
    };
    const a0 = new ScriptAgent([MOVE("jab"), WAIT]);
    const a1 = new ScriptAgent([MOVE("parry"), WAIT, WAIT]);
    const r = runMatch(state, [table({ jab }), table({ parry })], [a0, a1], { maxTicks: 60, maxDecisions: 10 });

    expect(r.trace.some((e) => e.kind === "CONTACT" && e.result === "PARRIED")).toBe(true);
    // e1 refilled to 10 in neutral, spent 4 on the parry (→ 6), then the parry refunded +2 AP and +1 Focus.
    expect(r.finalState.entities[1]!.resources.ap).toBe(8);
    expect(r.finalState.entities[1]!.resources.focus).toBe(1);
  });
});
