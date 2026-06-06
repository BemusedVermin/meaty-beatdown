/**
 * engine.ts — the resolution loop and the synchronous Agent interface (spec §2.1-2.10) [L2].
 *
 * Conceptually the engine is `step(state, decisions) → state'`: pure, synchronous, no RNG, no clock,
 * no ambient state, no I/O (decision 12). Same inputs ⇒ byte-identical outputs (determinism, audit
 * C-3). Decisions enter only through a synchronous, value-based `Agent`. Everything keys off
 * `ready_tick`: the regime (NEUTRAL vs PRESSURE) and the whole neutral/pressure/punish loop emerge
 * from that single rule with no special-casing (spec §2.1, audit C-2).
 *
 * §2.10 information rules are enforced structurally: in NEUTRAL both agents' `chooseAction` are
 * collected against the SAME pre-reveal state (neither can see the other's pending choice), then
 * both commit. Cancels are offered only at active/recovery cancel windows — never during startup
 * unless the move is `startupCancelable` (decision 6) — closing the react-to-reveal exploit (C-6).
 */
import { type Fixed, ZERO, add, sub, mul, fromInt, toIntRound, compare } from "./fixed";
import { type Tick } from "./tick";
import {
  type FrameProfile,
  type MoveLevel,
  type InvulnType,
  attackTypeOf,
  isPropertyActive,
  totalFrames,
} from "./frameprofile";
import { type SpatialState, type Facing } from "./spatial-types";
import {
  type Entity,
  type EntityState,
  type MoveInstance,
  type MoveId,
  type MoveContact,
  type Resources,
  stateMove,
  phaseAt,
  entityStateTag,
} from "./entity";
import { type Regime, computeRegime } from "./regime";
import {
  type ContactResult,
  type DefenderContext,
  classifyContact,
  counterHitDamage,
  counterHitHitstun,
  juggleScaledDamage,
  effectiveHitstun,
} from "./resolver";
import { type CancelWindow, type CancelGate, type ApGain } from "./cost";
import {
  canAfford,
  spend,
  combineCost,
  gainAp,
  gainFocus,
  refillAp,
  regenStamina,
} from "./resource-ops";
import { CONFIG } from "./config";
import { assertNever } from "./assert-never";
import { doesHit } from "../spatial/lane";

// ---------------------------------------------------------------------------
// Match state & move tables
// ---------------------------------------------------------------------------

export type EntityIndex = 0 | 1;

export interface MatchState {
  readonly t: Tick;
  readonly entities: readonly [Entity, Entity];
}

/** A resolved move table: moveId → the frame data the engine runs. (The RPG layer produces these.) */
export type MoveTable = ReadonlyMap<MoveId, FrameProfile>;

const other = (i: EntityIndex): EntityIndex => (i === 0 ? 1 : 0);

function setEntity(s: MatchState, i: EntityIndex, e: Entity): MatchState {
  const entities: readonly [Entity, Entity] = i === 0 ? [e, s.entities[1]] : [s.entities[0], e];
  return { ...s, entities };
}

// ---------------------------------------------------------------------------
// Agent interface (synchronous, value-based — decision 12)
// ---------------------------------------------------------------------------

export type Action =
  | { readonly kind: "MOVE"; readonly moveId: MoveId }
  | { readonly kind: "WAIT" };

export type DecisionResult = Action | { readonly kind: "DECLINE" };

/** One recorded decision, in the order the engine asked for it (for record/replay golden vectors). */
export type Decision =
  | { readonly kind: "action"; readonly action: Action }
  | { readonly kind: "cancel"; readonly result: DecisionResult };

export interface EntitySnapshot {
  readonly id: string;
  readonly stateTag: string;
  readonly readyTick: Tick;
  readonly resources: Resources;
  readonly spatial: SpatialState;
}

/** The opponent as the actor may see them. Crucially carries NO pending action — neutral is hidden. */
export interface OpponentSnapshot {
  readonly id: string;
  readonly stateTag: string;
  readonly readyTick: Tick;
  readonly spatial: SpatialState;
  /** In PRESSURE the locked opponent's committed move is visible (full info); null when free. */
  readonly currentMove: { readonly moveId: MoveId; readonly recoverAt: Tick } | null;
}

export interface PlayerView {
  readonly t: Tick;
  readonly regime: Regime;
  readonly self: EntitySnapshot;
  readonly opponent: OpponentSnapshot;
  readonly availableMoves: readonly MoveId[];
}

export interface CancelView {
  readonly t: Tick;
  readonly self: EntitySnapshot;
  /** The contact fact for hit-confirm — by now the result is a fact, not a read (spec §2.10). */
  readonly contact: MoveContact;
  readonly cancelInto: readonly MoveId[];
}

export interface Agent {
  chooseAction(view: PlayerView): Action;
  chooseCancel(view: CancelView): DecisionResult;
}

// ---------------------------------------------------------------------------
// Trace events (integers/ids only → comparable across languages; Phase 8 canonicalizes)
// ---------------------------------------------------------------------------

/** A compact, integers-only snapshot of one entity (pos/offset are fixed-point raw). */
export interface TraceEntity {
  readonly id: string;
  readonly stateTag: string;
  readonly readyTick: Tick;
  readonly hp: number;
  readonly stamina: number;
  readonly poise: number;
  readonly focus: number;
  readonly ap: number;
  readonly pos: number;
  readonly offset: number;
}

export type TraceEvent =
  | { readonly kind: "COMMIT"; readonly t: Tick; readonly entity: EntityIndex; readonly moveId: MoveId }
  | { readonly kind: "WAIT"; readonly t: Tick; readonly entity: EntityIndex }
  | {
      readonly kind: "CONTACT";
      readonly t: Tick;
      readonly attacker: EntityIndex;
      readonly defender: EntityIndex;
      readonly result: ContactResult["kind"];
      readonly counter: boolean;
    }
  | { readonly kind: "CANCEL"; readonly t: Tick; readonly entity: EntityIndex; readonly into: MoveId }
  /** A chosen move could not be paid for and degraded to WAIT — AP/Stamina/Focus exhaustion (§3.5). */
  | { readonly kind: "DENIED"; readonly t: Tick; readonly entity: EntityIndex; readonly moveId: MoveId }
  | { readonly kind: "KO"; readonly t: Tick; readonly loser: EntityIndex }
  /** Both entities' state at a decision point (a turn boundary) — the entity-state stream. */
  | { readonly kind: "STATE"; readonly t: Tick; readonly regime: Regime["kind"]; readonly entities: readonly [TraceEntity, TraceEntity] };

export interface MatchResult {
  readonly finalState: MatchState;
  readonly trace: readonly TraceEvent[];
  readonly winner: EntityIndex | null;
}

export interface MatchOptions {
  readonly maxTicks: number;
  readonly maxDecisions: number;
}

export const DEFAULT_OPTIONS: MatchOptions = { maxTicks: 100_000, maxDecisions: 100_000 };

// ---------------------------------------------------------------------------
// Spatial helpers
// ---------------------------------------------------------------------------

function faceToward(self: SpatialState, oppPos: Fixed): Facing {
  const c = compare(oppPos, self.pos);
  if (c > 0) return 1;
  if (c < 0) return -1;
  return self.facing;
}

/** Push the defender away from the attacker along the lane by `knockback` (spec §2.7). */
function applyKnockback(defenderPos: Fixed, attackerPos: Fixed, knockback: Fixed): Fixed {
  const away = compare(defenderPos, attackerPos) >= 0 ? knockback : sub(ZERO, knockback);
  return add(defenderPos, away);
}

// ---------------------------------------------------------------------------
// Move execution state helpers
// ---------------------------------------------------------------------------

function moveExecState(move: MoveInstance, t: Tick): EntityState {
  switch (phaseAt(move, t)) {
    case "STARTUP":
      return { kind: "STARTUP", move };
    case "ACTIVE":
      return { kind: "ACTIVE", move };
    case "RECOVERY":
      return { kind: "RECOVERY", move };
    case "DONE":
      return { kind: "NEUTRAL" };
  }
}

/** Rebuild a move-execution state around an updated MoveInstance, preserving the phase kind. */
function rebuildExec(state: EntityState, move: MoveInstance): EntityState {
  switch (state.kind) {
    case "STARTUP":
      return { kind: "STARTUP", move };
    case "ACTIVE":
      return { kind: "ACTIVE", move };
    case "RECOVERY":
      return { kind: "RECOVERY", move };
    case "NEUTRAL":
    case "HITSTUN":
    case "BLOCKSTUN":
    case "AIRBORNE":
    case "DOWN":
    case "GUARDBROKEN":
      return state;
    default:
      return assertNever(state);
  }
}

/** Tick at which a stun/down state ends (the entity's ready_tick). */
function stunUntil(state: EntityState): Tick {
  switch (state.kind) {
    case "HITSTUN":
    case "BLOCKSTUN":
    case "AIRBORNE":
    case "GUARDBROKEN":
      return state.until;
    case "DOWN":
      return state.wakeupTick;
    case "NEUTRAL":
    case "STARTUP":
    case "ACTIVE":
    case "RECOVERY":
      return 0;
    default:
      return assertNever(state);
  }
}

function commitMove(
  s: MatchState,
  i: EntityIndex,
  moveId: MoveId,
  table: MoveTable,
  t: Tick,
): MatchState {
  const profile = table.get(moveId);
  if (!profile) throw new Error(`unknown move "${moveId}" for entity ${i}`);
  const move: MoveInstance = {
    moveId,
    profile,
    startTick: t,
    connected: false,
    contact: "NONE",
    armorHitsUsed: 0,
  };
  const e = s.entities[i];
  return setEntity(s, i, {
    ...e,
    state: moveExecState(move, t),
    readyTick: t + totalFrames(profile.timing),
  });
}

/**
 * Reduce an agent's chosen action to what actually happens: a MOVE the entity cannot afford (or that
 * is not in its table) degrades to WAIT. This is how AP/Stamina/Focus exhaustion ends a string
 * (governors 1 & 4) — the next action simply can't be paid for. Resolved against the pre-commit state.
 */
function resolveAction(s: MatchState, i: EntityIndex, action: Action, table: MoveTable): Action {
  if (action.kind === "WAIT") return action;
  const profile = table.get(action.moveId);
  if (!profile || !canAfford(s.entities[i].resources, profile.cost)) return { kind: "WAIT" };
  return action;
}

function commitAction(
  s: MatchState,
  i: EntityIndex,
  action: Action,
  table: MoveTable,
  t: Tick,
): MatchState {
  switch (action.kind) {
    case "WAIT": {
      const e = s.entities[i];
      return setEntity(s, i, { ...e, state: { kind: "NEUTRAL" }, readyTick: t + 1 });
    }
    case "MOVE": {
      const profile = table.get(action.moveId);
      if (!profile) throw new Error(`unknown move "${action.moveId}" for entity ${i}`);
      const e = s.entities[i];
      const spent = setEntity(s, i, { ...e, resources: spend(e.resources, profile.cost) });
      return commitMove(spent, i, action.moveId, table, t);
    }
    default:
      return assertNever(action);
  }
}

/** Mark the attacker's in-flight move as connected with the given contact (single-hit; hit-confirm). */
function markConnected(s: MatchState, ai: EntityIndex, contact: MoveContact): MatchState {
  const e = s.entities[ai];
  const mv = stateMove(e.state);
  if (!mv) return s; // attacker was interrupted (e.g. a trade) — nothing to mark
  const updated: MoveInstance = { ...mv, connected: true, contact };
  return setEntity(s, ai, { ...e, state: rebuildExec(e.state, updated) });
}

// ---------------------------------------------------------------------------
// Defender context (read active properties on the defender's in-flight move)
// ---------------------------------------------------------------------------

function defenderContextAt(d: Entity, t: Tick): DefenderContext {
  const invulnTo = new Set<InvulnType>();
  let guardPointActive = false;
  let blockCovers: readonly MoveLevel[] | null = null;
  let armorRemaining = 0;
  let armorDamageMult: Fixed = fromInt(1);
  let throwTeching = false;
  let counterHitState = false;

  const mv = stateMove(d.state);
  if (mv) {
    const elapsed = t - mv.startTick;
    for (const p of mv.profile.properties) {
      if (!isPropertyActive(p, elapsed)) continue;
      switch (p.kind) {
        case "INVULN":
          invulnTo.add(p.invulnType);
          break;
        case "GUARD_POINT":
          guardPointActive = true;
          break;
        case "BLOCK":
          blockCovers = p.covers;
          break;
        case "ARMOR": {
          const remaining = p.armorHits - mv.armorHitsUsed;
          if (remaining > 0) {
            armorRemaining = remaining;
            armorDamageMult = p.armorDamageMult;
          }
          break;
        }
        case "COUNTER_HIT_STATE":
          counterHitState = true;
          break;
        case "AIRBORNE":
        case "PROJECTILE_SPAWN":
          break;
        default:
          assertNever(p);
      }
    }
    const phase = phaseAt(mv, t);
    // Startup and recovery are counter-hit windows by default (spec §2.7).
    if (phase === "STARTUP" || phase === "RECOVERY") counterHitState = true;
    // A defender actively throwing this tick techs an incoming throw (spec §2.6).
    if (attackTypeOf(mv.profile.level) === "THROW" && phase === "ACTIVE") throwTeching = true;
  }

  return {
    invulnTo,
    guardPointActive,
    blockCovers,
    armorRemaining,
    armorDamageMult,
    throwTeching,
    counterHitState,
  };
}

// ---------------------------------------------------------------------------
// Contact application (the consequences of classifyContact)
// ---------------------------------------------------------------------------

interface PendingContact {
  readonly ai: EntityIndex;
  readonly di: EntityIndex;
  readonly am: MoveInstance;
  readonly dctx: DefenderContext;
  readonly result: ContactResult;
}

interface Applied {
  readonly state: MatchState;
  readonly events: readonly TraceEvent[];
}

function damage(e: Entity, amount: number): Entity {
  return { ...e, resources: { ...e.resources, hp: e.resources.hp - amount } };
}

function applyContact(s: MatchState, c: PendingContact, t: Tick): Applied {
  const { ai, di, am, result } = c;
  const he = am.profile.hitEffect;
  const postActive = am.startTick + am.profile.timing.startup + am.profile.timing.active;
  const attackerPos = s.entities[ai].spatial.pos;

  const contactEvent = (counter: boolean): TraceEvent => ({
    kind: "CONTACT",
    t,
    attacker: ai,
    defender: di,
    result: result.kind,
    counter,
  });
  const koEvents = (def: Entity): readonly TraceEvent[] =>
    def.resources.hp <= 0 ? [{ kind: "KO", t, loser: di }] : [];

  switch (result.kind) {
    case "WHIFF":
      return { state: markConnected(s, ai, "NONE"), events: [] };

    case "PARRIED": {
      // Attacker frozen in a long recovery; defender plus + Focus/AP refund (decision 7).
      let ns = setEntity(s, ai, { ...s.entities[ai], readyTick: t + CONFIG.combat.PARRY_FREEZE_TICKS });
      ns = markConnected(ns, ai, "NONE");
      const d = ns.entities[di];
      // Decision 7 / spec §3.5.2: a read-based parry refunds both Focus and AP, turning defense into
      // a long punish turn.
      const refunded = gainAp(
        gainFocus(d.resources, CONFIG.combat.PARRY_FOCUS_REFUND),
        CONFIG.combat.PARRY_AP_REFUND,
      );
      ns = setEntity(ns, di, {
        ...d,
        state: { kind: "NEUTRAL" },
        readyTick: t + CONFIG.combat.PARRY_RECOVER_TICKS,
        resources: refunded,
      });
      return { state: ns, events: [contactEvent(false)] };
    }

    case "THROWN": {
      const wakeup = postActive + he.hitstun;
      let d = s.entities[di];
      d = damage(d, he.damage);
      d = { ...d, state: { kind: "DOWN", wakeupTick: wakeup }, readyTick: wakeup };
      let ns = setEntity(s, di, d);
      ns = markConnected(ns, ai, "HIT");
      return { state: ns, events: [contactEvent(false), ...koEvents(d)] };
    }

    case "THROW_TECH": {
      const recover = t + CONFIG.combat.THROW_TECH_RECOVER_TICKS;
      let ns = setEntity(s, ai, { ...s.entities[ai], state: { kind: "NEUTRAL" }, readyTick: recover });
      ns = setEntity(ns, di, { ...ns.entities[di], state: { kind: "NEUTRAL" }, readyTick: recover });
      return { state: ns, events: [contactEvent(false)] };
    }

    case "BLOCKED": {
      const d = s.entities[di];
      const poiseAfter = d.resources.poise - he.chipDamage;
      let nd: Entity;
      if (poiseAfter <= 0) {
        const until = postActive + CONFIG.combat.GUARD_BREAK_STUN_TICKS;
        nd = {
          ...d,
          state: { kind: "GUARDBROKEN", until },
          readyTick: until,
          resources: { ...d.resources, poise: d.resources.poiseMax }, // reset on break (spec §2.5/§3.1)
        };
      } else {
        const until = postActive + he.blockstun;
        nd = {
          ...d,
          state: { kind: "BLOCKSTUN", until },
          readyTick: until,
          resources: { ...d.resources, poise: poiseAfter },
        };
      }
      let ns = setEntity(s, di, nd);
      ns = markConnected(ns, ai, "BLOCK");
      return { state: ns, events: [contactEvent(false)] };
    }

    case "ARMORED": {
      // Reduced damage, NO hitstun, defender continues its move; consume one armor hit (decision 1).
      const reduced = toIntRound(mul(fromInt(he.damage), c.dctx.armorDamageMult));
      let d = s.entities[di];
      const dmv = stateMove(d.state);
      if (dmv) {
        const bumped: MoveInstance = { ...dmv, armorHitsUsed: dmv.armorHitsUsed + 1 };
        d = { ...d, state: rebuildExec(d.state, bumped) };
      }
      d = damage(d, reduced);
      let ns = setEntity(s, di, d);
      ns = markConnected(ns, ai, "HIT");
      return { state: ns, events: [contactEvent(false), ...koEvents(d)] };
    }

    case "HIT": {
      let d = s.entities[di];
      const wasInStun =
        d.state.kind === "HITSTUN" || d.state.kind === "BLOCKSTUN" || d.state.kind === "AIRBORNE";
      const combo = wasInStun ? d.comboCount + 1 : 1; // governor 3: hitstun decays as combo grows
      let dmg: number;
      let nextState: EntityState;

      if (d.state.kind === "AIRBORNE") {
        const jc = d.state.juggleCount;
        dmg = juggleScaledDamage(he.damage, jc); // governor 2: juggle damage decay
        const stun = effectiveHitstun(he.hitstun, combo);
        nextState = { kind: "AIRBORNE", until: postActive + stun, juggleCount: jc + 1 };
      } else {
        const counter = result.counter;
        const baseStun = counter ? counterHitHitstun(he.hitstun) : he.hitstun;
        const stun = effectiveHitstun(baseStun, combo);
        dmg = counter ? counterHitDamage(he.damage) : he.damage;
        const until = postActive + stun;
        nextState = he.launches
          ? { kind: "AIRBORNE", until, juggleCount: 0 }
          : he.knockdown
            ? { kind: "DOWN", wakeupTick: until }
            : { kind: "HITSTUN", until };
      }

      const pushedPos = applyKnockback(d.spatial.pos, attackerPos, he.knockback);
      d = damage(d, dmg);
      d = {
        ...d,
        state: nextState,
        readyTick: stunUntil(nextState),
        spatial: { ...d.spatial, pos: pushedPos },
        comboCount: combo,
      };
      let ns = setEntity(s, di, d);
      ns = markConnected(ns, ai, "HIT");
      return { state: ns, events: [contactEvent(result.counter), ...koEvents(d)] };
    }

    default:
      return assertNever(result);
  }
}

// ---------------------------------------------------------------------------
// Per-tick stepping
// ---------------------------------------------------------------------------

function guardNoProjectile(s: MatchState, t: Tick): void {
  for (const i of [0, 1] as const) {
    const mv = stateMove(s.entities[i].state);
    if (!mv) continue;
    const elapsed = t - mv.startTick;
    for (const p of mv.profile.properties) {
      if (p.kind === "PROJECTILE_SPAWN" && isPropertyActive(p, elapsed)) {
        // DEFERRED (spec §2.9; decision 8): the projectile entity is not implemented.
        throw new Error("DEFERRED (spec §2.9): projectile spawning is not implemented (decision 8).");
      }
    }
  }
}

function applyMotion(s: MatchState, t: Tick): MatchState {
  let ns = s;
  for (const i of [0, 1] as const) {
    const e = ns.entities[i];
    const mv = stateMove(e.state);
    if (!mv) continue;
    const motion = mv.profile.motion;
    if (!motion) continue;
    if (t - mv.startTick !== mv.profile.timing.startup) continue; // discrete hop at first active frame
    const laneDelta = e.spatial.facing === 1 ? motion.lane : sub(ZERO, motion.lane);
    ns = setEntity(ns, i, {
      ...e,
      spatial: {
        ...e.spatial,
        pos: add(e.spatial.pos, laneDelta),
        offset: add(e.spatial.offset, motion.offset),
      },
    });
  }
  return ns;
}

function collectContacts(s: MatchState, t: Tick): readonly PendingContact[] {
  const out: PendingContact[] = [];
  for (const ai of [0, 1] as const) {
    const di = other(ai);
    const attacker = s.entities[ai];
    const am = stateMove(attacker.state);
    if (!am || am.connected) continue;
    if (phaseAt(am, t) !== "ACTIVE") continue;
    const defender = s.entities[di];
    const dctx = defenderContextAt(defender, t);
    if (!doesHit(attacker.spatial, defender.spatial, am.profile.reach, am.profile.level, dctx.invulnTo))
      continue;
    const result = classifyContact(
      { type: attackTypeOf(am.profile.level), level: am.profile.level },
      dctx,
    );
    out.push({ ai, di, am, dctx, result });
  }
  return out;
}

function refreshPhaseLabels(s: MatchState, t: Tick): MatchState {
  let ns = s;
  for (const i of [0, 1] as const) {
    const e = ns.entities[i];
    const mv = stateMove(e.state);
    if (!mv) continue;
    ns = setEntity(ns, i, { ...e, state: moveExecState(mv, t) });
  }
  return ns;
}

/** Whether a cancel window's gate is satisfied by the move's settled contact (hit-confirm — §2.10). */
function cancelGateSatisfied(gate: CancelGate, contact: MoveContact): boolean {
  switch (gate) {
    case "ALWAYS":
      return true;
    case "ON_HIT":
      return contact === "HIT";
    case "ON_BLOCK":
      return contact === "BLOCK";
    case "ON_CONTACT":
      return contact === "HIT" || contact === "BLOCK";
    case "ON_WHIFF":
      return contact === "NONE";
    default:
      return assertNever(gate);
  }
}

/** Whether a move's conditional AP gain fires for a given contact result (spec §3.5.2). */
function apGainTriggered(gain: ApGain, result: ContactResult): boolean {
  switch (gain.gate) {
    case "ALWAYS":
      return true;
    case "ON_HIT":
      return result.kind === "HIT";
    case "ON_CH":
      return result.kind === "HIT" && result.counter;
    case "ON_BLOCK":
      return result.kind === "BLOCKED";
    case "ON_PARRY":
      return false; // the parrier's refund is applied on PARRIED via config (decision 7), not here
    default:
      return assertNever(gain.gate);
  }
}

function applyApGain(s: MatchState, ai: EntityIndex, am: MoveInstance, result: ContactResult): MatchState {
  const gain = am.profile.cost.apGain;
  if (!gain || !apGainTriggered(gain, result)) return s;
  const e = s.entities[ai];
  return setEntity(s, ai, { ...e, resources: gainAp(e.resources, gain.amount) });
}

/** The cancel window an entity has just entered at tick `t` (edge-triggered, gate satisfied), or null. */
function cancelEligible(e: Entity, t: Tick): CancelWindow | null {
  const mv = stateMove(e.state);
  if (!mv) return null;
  const elapsed = t - mv.startTick;
  const startup = mv.profile.timing.startup;
  for (const cw of mv.profile.cancelWindows) {
    // No-startup-cancel (decision 6): unless flagged, the window's eligible start clamps to active.
    const firstEligible = mv.profile.startupCancelable ? cw.from : Math.max(cw.from, startup);
    if (firstEligible > cw.to) continue; // window lies entirely in startup → never offered
    if (elapsed === firstEligible && cancelGateSatisfied(cw.gate, mv.contact)) return cw;
  }
  return null;
}

/** Stamina regen for entities not currently executing a move (spec §3.1). */
function applyRegen(s: MatchState): MatchState {
  let ns = s;
  for (const i of [0, 1] as const) {
    const e = ns.entities[i];
    if (stateMove(e.state)) continue;
    ns = setEntity(ns, i, { ...e, resources: regenStamina(e.resources) });
  }
  return ns;
}

interface StepResult {
  readonly state: MatchState;
  readonly events: readonly TraceEvent[];
  readonly cancelCheckpoint: { readonly entity: EntityIndex; readonly window: CancelWindow } | null;
}

function stepOneTick(s: MatchState, t: Tick): StepResult {
  guardNoProjectile(s, t);
  let ns = applyMotion(s, t);

  let events: TraceEvent[] = [];
  for (const c of collectContacts(ns, t)) {
    const applied = applyContact(ns, c, t);
    ns = applied.state;
    ns = applyApGain(ns, c.ai, c.am, c.result); // conditional AP generation (spec §3.5.2)
    events = events.concat(applied.events);
  }

  ns = refreshPhaseLabels(ns, t);
  ns = applyRegen(ns);

  let cancelCheckpoint: StepResult["cancelCheckpoint"] = null;
  for (const i of [0, 1] as const) {
    const window = cancelEligible(ns.entities[i], t);
    if (window) {
      cancelCheckpoint = { entity: i, window };
      break;
    }
  }

  return { state: { ...ns, t: t + 1 }, events, cancelCheckpoint };
}

// ---------------------------------------------------------------------------
// Views & actionability
// ---------------------------------------------------------------------------

function isMatchOver(s: MatchState): boolean {
  return s.entities[0].resources.hp <= 0 || s.entities[1].resources.hp <= 0;
}

function winnerOf(s: MatchState): EntityIndex | null {
  const a = s.entities[0].resources.hp <= 0;
  const b = s.entities[1].resources.hp <= 0;
  if (a && b) return null; // double KO
  if (b) return 0;
  if (a) return 1;
  return null;
}

function anyActionable(s: MatchState): boolean {
  return s.entities[0].readyTick <= s.t || s.entities[1].readyTick <= s.t;
}

/**
 * Transition every now-actionable entity to NEUTRAL, clear its combo counter, re-center offset, and
 * auto-face the opponent (§1.1). AP is NOT refilled here — that happens only on entering NEUTRAL (see
 * runMatch): a fresh exchange refills the turn budget, but in PRESSURE you act with your current AP,
 * so conditional ap_gain (parry, ON_HIT links — spec §3.5.2; decision 7) genuinely extends offense.
 */
function normalizeActionable(s: MatchState): MatchState {
  let ns = s;
  for (const i of [0, 1] as const) {
    const e = ns.entities[i];
    if (e.readyTick > ns.t) continue;
    const opp = ns.entities[other(i)];
    ns = setEntity(ns, i, {
      ...e,
      state: { kind: "NEUTRAL" },
      comboCount: 0,
      spatial: { ...e.spatial, offset: ZERO, facing: faceToward(e.spatial, opp.spatial.pos) },
    });
  }
  return ns;
}

/** Refill both entities' AP to the cap — done on entering NEUTRAL (regaining initiative, §3.5.1). */
function refillBothAp(s: MatchState): MatchState {
  return {
    ...s,
    entities: [
      { ...s.entities[0], resources: refillAp(s.entities[0].resources) },
      { ...s.entities[1], resources: refillAp(s.entities[1].resources) },
    ],
  };
}

function snapshot(e: Entity): EntitySnapshot {
  return {
    id: e.id,
    stateTag: entityStateTag(e.state),
    readyTick: e.readyTick,
    resources: e.resources,
    spatial: e.spatial,
  };
}

function traceEntity(e: Entity): TraceEntity {
  return {
    id: e.id,
    stateTag: entityStateTag(e.state),
    readyTick: e.readyTick,
    hp: e.resources.hp,
    stamina: e.resources.stamina,
    poise: e.resources.poise,
    focus: e.resources.focus,
    ap: e.resources.ap,
    pos: e.spatial.pos as number,
    offset: e.spatial.offset as number,
  };
}

function stateEvent(s: MatchState, regime: Regime): TraceEvent {
  return {
    kind: "STATE",
    t: s.t,
    regime: regime.kind,
    entities: [traceEntity(s.entities[0]), traceEntity(s.entities[1])],
  };
}

function opponentSnapshot(e: Entity): OpponentSnapshot {
  const mv = stateMove(e.state);
  return {
    id: e.id,
    stateTag: entityStateTag(e.state),
    readyTick: e.readyTick,
    spatial: e.spatial,
    currentMove: mv ? { moveId: mv.moveId, recoverAt: e.readyTick } : null,
  };
}

function playerView(
  s: MatchState,
  i: EntityIndex,
  regime: Regime,
  tables: readonly [MoveTable, MoveTable],
): PlayerView {
  return {
    t: s.t,
    regime,
    self: snapshot(s.entities[i]),
    opponent: opponentSnapshot(s.entities[other(i)]),
    availableMoves: [...tables[i].keys()],
  };
}

function cancelView(s: MatchState, i: EntityIndex, window: CancelWindow): CancelView {
  const mv = stateMove(s.entities[i].state);
  return {
    t: s.t,
    self: snapshot(s.entities[i]),
    contact: mv ? mv.contact : "NONE",
    cancelInto: window.into,
  };
}

// ---------------------------------------------------------------------------
// The resolution loop (spec §2.2)
// ---------------------------------------------------------------------------

type Pause =
  | { readonly kind: "ACTION" }
  | { readonly kind: "CANCEL"; readonly entity: EntityIndex; readonly window: CancelWindow }
  | { readonly kind: "OVER" };

function advanceUntilNextDecision(
  s: MatchState,
  opts: MatchOptions,
): { readonly state: MatchState; readonly pause: Pause; readonly events: readonly TraceEvent[] } {
  if (isMatchOver(s)) return { state: s, pause: { kind: "OVER" }, events: [] };
  if (anyActionable(s)) return { state: s, pause: { kind: "ACTION" }, events: [] };

  let state = s;
  let events: TraceEvent[] = [];
  while (state.t < opts.maxTicks) {
    const stepped = stepOneTick(state, state.t);
    state = stepped.state;
    events = events.concat(stepped.events);
    if (isMatchOver(state)) return { state, pause: { kind: "OVER" }, events };
    if (stepped.cancelCheckpoint !== null)
      return {
        state,
        pause: { kind: "CANCEL", entity: stepped.cancelCheckpoint.entity, window: stepped.cancelCheckpoint.window },
        events,
      };
    if (anyActionable(state)) return { state, pause: { kind: "ACTION" }, events };
  }
  return { state, pause: { kind: "OVER" }, events };
}

/**
 * Run a full match deterministically. The engine queries `agents` synchronously at each decision
 * point; given the same initial state, move tables, and agents, it produces a byte-identical trace.
 */
export function runMatch(
  initial: MatchState,
  tables: readonly [MoveTable, MoveTable],
  agents: readonly [Agent, Agent],
  options: MatchOptions = DEFAULT_OPTIONS,
): MatchResult {
  let state = initial;
  const trace: TraceEvent[] = [];

  for (let decisions = 0; decisions < options.maxDecisions; decisions++) {
    const advanced = advanceUntilNextDecision(state, options);
    state = advanced.state;
    trace.push(...advanced.events);

    switch (advanced.pause.kind) {
      case "OVER":
        return { finalState: state, trace, winner: winnerOf(state) };

      case "ACTION": {
        state = normalizeActionable(state);
        const regime = computeRegime(state.entities[0], state.entities[1]);
        if (regime.kind === "NEUTRAL") {
          // Fresh exchange ⇒ both refill their AP turn-budget (decision 4 / spec §3.5.1).
          state = refillBothAp(state);
        }
        trace.push(stateEvent(state, regime)); // entity-state stream snapshot at this turn boundary
        if (regime.kind === "NEUTRAL") {
          // §2.10 hidden simultaneous commit: both views are built from the SAME pre-reveal state,
          // and both actions are resolved for affordability against it before either commits.
          const a0 = agents[0].chooseAction(playerView(state, 0, regime, tables));
          const a1 = agents[1].chooseAction(playerView(state, 1, regime, tables));
          const eff0 = resolveAction(state, 0, a0, tables[0]);
          const eff1 = resolveAction(state, 1, a1, tables[1]);
          state = commitAction(state, 0, eff0, tables[0], state.t);
          state = commitAction(state, 1, eff1, tables[1], state.t);
          trace.push(commitTraceEvent(0, a0, eff0, state.t), commitTraceEvent(1, a1, eff1, state.t));
        } else {
          const i = regime.actor;
          const action = agents[i].chooseAction(playerView(state, i, regime, tables));
          const eff = resolveAction(state, i, action, tables[i]);
          state = commitAction(state, i, eff, tables[i], state.t);
          trace.push(commitTraceEvent(i, action, eff, state.t));
        }
        break;
      }

      case "CANCEL": {
        const { entity: i, window } = advanced.pause;
        const result = agents[i].chooseCancel(cancelView(state, i, window));
        if (result.kind === "MOVE" && window.into.includes(result.moveId)) {
          const target = tables[i].get(result.moveId);
          // A cancel pays the window's cost (the combo tax, usually Focus) PLUS the target move's own
          // cost; unaffordable ⇒ the cancel is refused and the string ends (governors 1 & 4).
          if (target) {
            const combined = combineCost(window.cost, target.cost);
            const e = state.entities[i];
            if (canAfford(e.resources, combined)) {
              state = setEntity(state, i, { ...e, resources: spend(e.resources, combined) });
              state = commitMove(state, i, result.moveId, tables[i], state.t);
              trace.push({ kind: "CANCEL", t: state.t, entity: i, into: result.moveId });
            } else {
              // Wanted to cancel but can't pay → exhaustion ends the string (governors 1 & 4).
              trace.push({ kind: "DENIED", t: state.t, entity: i, moveId: result.moveId });
            }
          }
        }
        // DECLINE / unaffordable / invalid target at a checkpoint: the move finishes (advance steps past).
        break;
      }

      default:
        return assertNever(advanced.pause);
    }
  }

  return { finalState: state, trace, winner: winnerOf(state) };
}

function commitTraceEvent(i: EntityIndex, chosen: Action, effective: Action, t: Tick): TraceEvent {
  // A chosen MOVE that resolved to WAIT was unaffordable → DENIED (exhaustion), not an idle wait.
  if (chosen.kind === "MOVE" && effective.kind === "WAIT") {
    return { kind: "DENIED", t, entity: i, moveId: chosen.moveId };
  }
  return effective.kind === "MOVE"
    ? { kind: "COMMIT", t, entity: i, moveId: effective.moveId }
    : { kind: "WAIT", t, entity: i };
}
