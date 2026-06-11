//! `CombatSim` — the advance loop of spec §4.3.
//!
//! ```text
//! loop:
//!   pump_decisions(T)   # gather same-tick decisions, side-blind commit, apply
//!   step_tick(T)        # move phases, contacts via does_hit -> resolve, motion, timers
//!   T += 1
//! ```
//!
//! The engine pauses (`SimStatus::AwaitingDecisions`) whenever any actor must decide and
//! resumes when every side has committed — the wall clock is irrelevant (spec §2.1).
//! The fight is a pure function of (initial state, content + Ruleset, decision log):
//! the `Committed` trace events ARE the decision log, and replaying them byte-identically
//! reproduces the trace (C-DET).
//!
//! Phase 2 adds the combo system (spec §6) and the meters (spec §9). The governors live
//! where they bind: hitstun decay and juggle damage decay in the hit application
//! (governors 1-2), the extender latches in the reaction application (governor 3), AP
//! and Focus pricing at commit/cancel time (governors 4-5), and the gravity floor at
//! juggle sustain (governor 7). Governor 6 (no positive cancel cycles) is the audit's.
//!
//! Phase 3 raises the fog boundary (C-FOG): `observe(side)` is the only gameplay read;
//! `debug_entity`/`trace` are replay & test surfaces, review-banned in the app.

use crate::core::fx::{Fx, FxVec2};
use crate::core::ids::{EntityId, SideId};
use crate::core::tick::Tick;
use crate::data::movedef::{
    CancelGate, CancelWindow, GainGate, GainResource, Move, MoveCategory, PropertyKind, StanceKind,
    StanceReq,
};
use crate::data::{ArenaDef, DefenseProfile, KnowledgeBook, MoveId, Reaction, Ruleset};
use crate::trace::{ThrowResolution, TraceEvent};

use super::entity::{ActorState, ComboTracker, Entity, MoveInstance, MovePhase, Stance};
use super::observe::{self, Observation};
use super::resolve::{self, ContactOutcome};
use super::schedule::{Choice, CommitBatch, CommitError, DecisionKind, PendingDecision};
use super::spatial;

/// Initial placement of one actor.
#[derive(Clone, Debug)]
pub struct EntitySetup {
    pub id: EntityId,
    pub side: SideId,
    pub pos: FxVec2,
    pub target: EntityId,
    /// When this actor first gets a free decision.
    pub ready_at: Tick,
    pub defense: DefenseProfile,
    pub moves: Vec<Move>,
}

#[derive(Clone, Debug)]
pub struct SimConfig {
    pub arena: ArenaDef,
    pub ruleset: Ruleset,
    pub entities: Vec<EntitySetup>,
    /// Hard termination cap (fsm.md): with no rounds and no timer, a bout must still
    /// provably end.
    pub max_ticks: u64,
    /// Each side's matchup knowledge (spec §7.3), read-only for the fight. Missing
    /// sides know nothing (T0 across the board).
    pub knowledge: std::collections::BTreeMap<SideId, KnowledgeBook>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SimStatus {
    /// One or more sides must commit: query `pending()`, then `commit_side()` per side.
    AwaitingDecisions,
    /// The fight is over. `winner` is None on a tick-cap stop or mutual wipe.
    Over { winner: Option<SideId> },
}

/// Where the per-tick pipeline resumes after a pause.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Stage {
    /// Expiries, auto-facing, then decision collection (pauses for commits).
    TickStart,
    /// Motion + contacts (pauses for break reads).
    World,
    /// Regen, KO/elimination, tick increment.
    TickEnd,
}

/// A grab that connected this tick and awaits its break read.
#[derive(Copy, Clone, Debug)]
struct PendingGrab {
    attacker: usize,
    victim: usize,
}

pub struct CombatSim {
    t: Tick,
    /// Sorted by id at construction: stable entity-id order is the same-tick
    /// determinism rule (spec §4.2).
    entities: Vec<Entity>,
    arena: ArenaDef,
    ruleset: Ruleset,
    max_ticks: u64,
    knowledge: std::collections::BTreeMap<SideId, KnowledgeBook>,
    stage: Stage,
    batch: Option<CommitBatch>,
    grabs: Vec<PendingGrab>,
    over: Option<Option<SideId>>,
    trace: Vec<TraceEvent>,
}

impl CombatSim {
    #[must_use]
    pub fn new(config: SimConfig) -> Self {
        let mut entities: Vec<Entity> = config
            .entities
            .into_iter()
            .map(|e| Entity {
                id: e.id,
                side: e.side,
                pos: e.pos,
                facing: FxVec2::new(Fx::ONE, Fx::ZERO),
                target: e.target,
                stance: Stance::Standing,
                state: ActorState::Free,
                ready_tick: e.ready_at,
                current: None,
                held: None,
                reevaluate_at: Tick::ZERO,
                height_off: Fx::ZERO,
                hp: e.defense.hp_max,
                guard: e.defense.guard_max,
                guard_regen_acc: 0,
                breath: e.defense.breath_max,
                breath_regen_acc: 0,
                ap: e.defense.ap_max,
                focus: 0,
                combo: ComboTracker::default(),
                moves: e.moves,
                defense: e.defense,
            })
            .collect();
        entities.sort_by_key(|e| e.id);
        let trace = vec![TraceEvent::SimStarted {
            entities: entities.iter().map(|e| e.id).collect(),
        }];
        let mut sim = Self {
            t: Tick::ZERO,
            entities,
            arena: config.arena,
            ruleset: config.ruleset,
            max_ticks: config.max_ticks,
            knowledge: config.knowledge,
            stage: Stage::TickStart,
            batch: None,
            grabs: Vec::new(),
            over: None,
            trace,
        };
        for i in 0..sim.entities.len() {
            sim.auto_face(i);
        }
        sim
    }

    /// Build a PROJECTION sim from already-shaped entities at an absolute tick — the
    /// forecast's engine-on-Observation path (spec §7.4; tech-plan §3). Same advance
    /// loop, same rules: the forecast can never drift from the fight.
    pub(crate) fn projection(
        t: Tick,
        mut entities: Vec<Entity>,
        arena: ArenaDef,
        ruleset: Ruleset,
    ) -> Self {
        entities.sort_by_key(|e| e.id);
        Self {
            t,
            entities,
            arena,
            ruleset,
            max_ticks: t.0 + 10_000,
            knowledge: std::collections::BTreeMap::new(),
            stage: Stage::TickStart,
            batch: None,
            grabs: Vec::new(),
            over: None,
            trace: Vec::new(),
        }
    }

    // -- the public pump ------------------------------------------------------

    /// Run until a decision is needed or the fight is over.
    pub fn advance(&mut self) -> SimStatus {
        loop {
            if let Some(winner) = self.over {
                return SimStatus::Over { winner };
            }
            match self.stage {
                Stage::TickStart => {
                    if let Some(batch) = &self.batch {
                        if !batch.complete() {
                            return SimStatus::AwaitingDecisions;
                        }
                        self.apply_choices();
                        self.stage = Stage::World;
                    } else {
                        self.upkeep_start();
                        if self.collect_decisions() {
                            return SimStatus::AwaitingDecisions;
                        }
                        self.stage = Stage::World;
                    }
                }
                Stage::World => {
                    if self.grabs.is_empty() {
                        self.integrate_motion();
                        self.run_contacts();
                    }
                    if !self.grabs.is_empty() {
                        if self.batch.as_ref().is_none_or(|b| !b.complete()) {
                            return SimStatus::AwaitingDecisions;
                        }
                        self.resolve_breaks();
                    }
                    self.stage = Stage::TickEnd;
                }
                Stage::TickEnd => {
                    self.upkeep_end();
                    if self.over.is_some() {
                        continue;
                    }
                    if self.t.0 + 1 >= self.max_ticks {
                        self.finish(None);
                        continue;
                    }
                    self.t.advance();
                    self.stage = Stage::TickStart;
                }
            }
        }
    }

    /// All decisions pending at the current tick. Exposes only the prompts — never any
    /// side's already-committed choices (side-blind, spec §4.2).
    #[must_use]
    pub fn pending(&self) -> Vec<PendingDecision> {
        self.batch
            .as_ref()
            .map(|b| b.pending.clone())
            .unwrap_or_default()
    }

    /// Commit one side's choices for all of its pending actors, without seeing the other
    /// side's same-tick commitments.
    pub fn commit_side(
        &mut self,
        side: SideId,
        choices: &[(EntityId, Choice)],
    ) -> Result<(), CommitError> {
        for &(actor, choice) in choices {
            self.validate_choice(side, actor, choice)?;
        }
        let batch = self.batch.as_mut().expect("validated non-empty batch");
        for &(actor, choice) in choices {
            batch.committed.insert(actor, choice);
        }
        Ok(())
    }

    #[must_use]
    pub fn tick(&self) -> Tick {
        self.t
    }

    /// THE fog boundary (spec §7.1, C-FOG): everything a side may know right now.
    /// The UI and every AI agent consume this and nothing else.
    #[must_use]
    pub fn observe(&self, side: SideId) -> Observation {
        let book = self.knowledge.get(&side);
        let tier_of = |id: MoveId| book.map(|b| b.tier(id)).unwrap_or_default();
        let allies: Vec<Entity> = self
            .entities
            .iter()
            .filter(|e| e.side == side)
            .cloned()
            .collect();
        let enemies: Vec<observe::EnemyView> = self
            .entities
            .iter()
            .filter(|e| e.side != side)
            .map(|e| observe::project_enemy(e, self.t, tier_of))
            .collect();
        let side_of = |id: EntityId| self.entities.iter().find(|e| e.id == id).map(|e| e.side);
        let events: Vec<TraceEvent> = self
            .trace
            .iter()
            .filter(|ev| observe::event_public_for(ev, side, side_of))
            .cloned()
            .collect();
        Observation {
            t: self.t,
            side,
            allies,
            enemies,
            events,
            arena: self.arena.clone(),
            ruleset: self.ruleset.clone(),
        }
    }

    /// TRUE state — debug/test surface ONLY. Everything gameplay-facing goes through
    /// `observe()`; the app may never call this (driver review rule, tech-plan §4).
    #[doc(hidden)]
    #[must_use]
    pub fn debug_entity(&self, id: EntityId) -> Option<&Entity> {
        self.entities.iter().find(|e| e.id == id)
    }

    /// The full trace — the replay/golden-vector contract (C-DET). Contains BOTH sides'
    /// commitments: a replay artifact and debug log, not a gameplay read (the fogged
    /// event view lives on `Observation::events`).
    #[must_use]
    pub fn trace(&self) -> &[TraceEvent] {
        &self.trace
    }

    // -- stage: tick start ----------------------------------------------------

    fn upkeep_start(&mut self) {
        let t = self.t;
        for i in 0..self.entities.len() {
            // Move completion / stance-hold entry.
            if self.entities[i].state == ActorState::Acting {
                let mv = self.entities[i].current_move().expect("acting has a move");
                let timing = mv.timing;
                let category = mv.category;
                let stance_spec = mv.stance_spec;
                let elapsed = self.entities[i]
                    .move_elapsed(t)
                    .expect("acting has elapsed");
                // Exact-tick transition: a stance enters its hold when startup elapses.
                // (Release instances re-enter the move already past this point, so they
                // pass through to Done normally. Stance startup must be >= 1.)
                if category == MoveCategory::Stance && elapsed == timing.startup {
                    let spec = stance_spec.expect("stance move has a spec");
                    let e = &mut self.entities[i];
                    e.current = None;
                    e.state = ActorState::HoldingStance;
                    e.held = Some(spec);
                    e.stance = match spec.stance {
                        StanceKind::Standing => Stance::Standing,
                        StanceKind::Crouching => Stance::Crouching,
                    };
                    e.reevaluate_at = t + u64::from(self.ruleset.block_reevaluate_every);
                } else if MovePhase::at(timing, elapsed) == MovePhase::Done {
                    let victim = self.entities[i].current.and_then(|c| c.grabbed_victim);
                    self.entities[i].current = None;
                    if self.entities[i].held.is_none() {
                        self.entities[i].stance = Stance::Standing;
                    }
                    self.set_free(i, t);
                    if let Some(v) = victim {
                        self.release_grabbed(v);
                    }
                }
            }
            // Stun / juggle / down expiries.
            match self.entities[i].state {
                ActorState::Hitstun { until } | ActorState::GuardBroken { until } if until == t => {
                    self.entities[i].stance = Stance::Standing;
                    self.set_free(i, t);
                    self.end_combo(i);
                }
                ActorState::Crumple { until } if until == t => {
                    // Nobody picked them up: collapse.
                    self.floor(i, "collapse");
                }
                ActorState::Airborne { stun_until } if stun_until == t => {
                    self.trace.push(TraceEvent::Landed {
                        t,
                        victim: self.entities[i].id,
                    });
                    self.floor(i, "landed");
                }
                ActorState::WallSplat { until } if until == t => {
                    self.trace.push(TraceEvent::Landed {
                        t,
                        victim: self.entities[i].id,
                    });
                    self.floor(i, "splat fell");
                }
                ActorState::Blockstun { until } if until == t => {
                    // Still holding guard; an event touched you -> re-decide now (§5.3).
                    self.entities[i].state = ActorState::HoldingStance;
                    self.entities[i].reevaluate_at = t;
                }
                _ => {}
            }
        }
        for i in 0..self.entities.len() {
            self.auto_face(i);
        }
    }

    /// Regaining freedom refills AP to max (spec §9.4): your turn's tempo budget resets
    /// when the string is over.
    fn set_free(&mut self, i: usize, ready: Tick) {
        let e = &mut self.entities[i];
        e.state = ActorState::Free;
        e.ready_tick = ready;
        e.ap = e.defense.ap_max;
    }

    /// The victim hits the ground: knockdown into the wake-up flow, combo over.
    fn floor(&mut self, i: usize, _why: &str) {
        let t = self.t;
        let e = &mut self.entities[i];
        e.state = ActorState::Down {
            until: t + u64::from(self.ruleset.landing_down_ticks.max(1)),
        };
        e.stance = Stance::Down;
        e.height_off = Fx::ZERO;
        e.current = None;
        e.held = None;
        self.end_combo(i);
    }

    /// Close out a combo on this victim, if one was running.
    fn end_combo(&mut self, i: usize) {
        let hits = self.entities[i].combo.hits;
        if hits > 0 {
            let victim = self.entities[i].id;
            self.trace.push(TraceEvent::ComboEnded {
                t: self.t,
                victim,
                hits,
            });
            self.entities[i].combo = ComboTracker::default();
        }
    }

    /// An actionable actor auto-faces its target (spec §3.2); committed or reeling actors
    /// keep their facing — which is what makes whiffing-by-sidestep (and later flanking)
    /// possible.
    fn auto_face(&mut self, i: usize) {
        if !matches!(
            self.entities[i].state,
            ActorState::Free | ActorState::HoldingStance
        ) {
            return;
        }
        self.auto_face_forced(i);
    }

    fn auto_face_forced(&mut self, i: usize) {
        let target = self.entities[i].target;
        let Some(target_pos) = self.entities.iter().find(|e| e.id == target).map(|e| e.pos) else {
            return;
        };
        let dir = (target_pos - self.entities[i].pos).normalize_or_zero();
        if dir != FxVec2::ZERO {
            self.entities[i].facing = dir;
        }
    }

    /// Cancel windows currently open, gate-satisfied, unprompted, and affordable
    /// (unaffordable windows auto-pass silently — no prompt spam, no information).
    fn open_cancels(&self, i: usize) -> Vec<(u32, CancelWindow)> {
        let t = self.t;
        let e = &self.entities[i];
        let Some(inst) = e.current else {
            return Vec::new();
        };
        let Some(mv) = e.current_move() else {
            return Vec::new();
        };
        let Some(elapsed) = e.move_elapsed(t) else {
            return Vec::new();
        };
        let past_active = elapsed >= mv.timing.startup + mv.timing.active;
        mv.cancels
            .iter()
            .enumerate()
            .filter(|&(idx, w)| {
                let idx32 = u32::try_from(idx).expect("few windows");
                if inst.cancels_prompted & (1 << idx32) != 0 {
                    return false;
                }
                if elapsed < w.from || elapsed > w.to {
                    return false;
                }
                let satisfied = match w.gate {
                    CancelGate::OnHit => inst.hit_landed,
                    CancelGate::OnBlock => inst.blocked,
                    CancelGate::OnContact => inst.hit_landed || inst.blocked,
                    CancelGate::OnWhiff => past_active && !inst.hit_landed && !inst.blocked,
                    CancelGate::Always => true,
                };
                if !satisfied {
                    return false;
                }
                let Some(target) = e.moves.iter().find(|m| m.id == w.into) else {
                    return false;
                };
                e.ap >= w.ap_cost + target.cost.ap
                    && e.focus >= w.focus_cost + target.cost.focus
                    && e.breath >= target.cost.breath
            })
            .map(|(idx, w)| (u32::try_from(idx).expect("few windows"), *w))
            .collect()
    }

    /// Collect this tick's prompts. Returns true if a batch is now awaiting commits.
    fn collect_decisions(&mut self) -> bool {
        let t = self.t;
        let mut pending: Vec<PendingDecision> = Vec::new();
        for i in 0..self.entities.len() {
            let e = &self.entities[i];
            match e.state {
                // <= catches actors freed mid-tick (e.g. a dissolved grab at TickEnd):
                // nobody starves on a stale ready_tick.
                ActorState::Free if e.ready_tick <= t => pending.push(PendingDecision {
                    actor: e.id,
                    side: e.side,
                    kind: DecisionKind::Ready,
                }),
                ActorState::HoldingStance if e.reevaluate_at == t => {
                    pending.push(PendingDecision {
                        actor: e.id,
                        side: e.side,
                        kind: DecisionKind::StanceReevaluate,
                    });
                }
                ActorState::Down { until } if until == t => pending.push(PendingDecision {
                    actor: e.id,
                    side: e.side,
                    kind: DecisionKind::WakeUp,
                }),
                ActorState::Acting if !self.open_cancels(i).is_empty() => {
                    pending.push(PendingDecision {
                        actor: e.id,
                        side: e.side,
                        kind: DecisionKind::Cancel,
                    });
                }
                _ => {}
            }
        }
        if pending.is_empty() {
            return false;
        }
        self.batch = Some(CommitBatch {
            pending,
            committed: std::collections::BTreeMap::new(),
        });
        true
    }

    fn affordable(e: &Entity, mv: &Move) -> bool {
        e.breath >= mv.cost.breath && e.ap >= mv.cost.ap && e.focus >= mv.cost.focus
    }

    fn validate_choice(
        &self,
        side: SideId,
        actor: EntityId,
        choice: Choice,
    ) -> Result<(), CommitError> {
        let Some(batch) = &self.batch else {
            return Err(CommitError::NotPending { actor });
        };
        let Some(pending) = batch
            .pending
            .iter()
            .find(|p| p.actor == actor && p.side == side)
        else {
            return Err(CommitError::NotPending { actor });
        };
        if batch.committed.contains_key(&actor) {
            return Err(CommitError::AlreadyCommitted { actor });
        }
        let entity = self.debug_entity(actor).expect("pending actor exists");
        let i = self.index_of(actor);
        match (pending.kind, choice) {
            (DecisionKind::Ready, Choice::Wait { .. }) => Ok(()),
            (DecisionKind::Ready, Choice::Move { id }) => {
                let Some(mv) = entity.moves.iter().find(|m| m.id == id) else {
                    return Err(CommitError::UnknownOrUnmetMove { actor });
                };
                if mv.req_down {
                    return Err(CommitError::UnknownOrUnmetMove { actor });
                }
                if !Self::affordable(entity, mv) {
                    return Err(CommitError::Denied { actor });
                }
                // Free actors are standing; crouch-required moves are only reachable
                // from a held crouching stance.
                match mv.req_stance {
                    None | Some(StanceReq::Standing) => Ok(()),
                    Some(StanceReq::Crouching) => Err(CommitError::UnknownOrUnmetMove { actor }),
                }
            }
            (DecisionKind::StanceReevaluate, Choice::HoldStance | Choice::Release) => Ok(()),
            (DecisionKind::StanceReevaluate, Choice::Move { id }) => {
                // Direct moves from a held stance: only from a pure body stance (no
                // guard commitment) whose kind the move requires — the while-crouching
                // idiom. Guarded holds must Release first (spec §5.3).
                let held = entity.held.expect("holding");
                if held.guard.is_some() {
                    return Err(CommitError::IllegalChoice {
                        actor,
                        why: "attacks from a guarding hold require Release",
                    });
                }
                let Some(mv) = entity.moves.iter().find(|m| m.id == id) else {
                    return Err(CommitError::UnknownOrUnmetMove { actor });
                };
                if mv.req_down {
                    return Err(CommitError::UnknownOrUnmetMove { actor });
                }
                if !Self::affordable(entity, mv) {
                    return Err(CommitError::Denied { actor });
                }
                let matches_stance = matches!(
                    (held.stance, mv.req_stance),
                    (StanceKind::Crouching, Some(StanceReq::Crouching))
                        | (StanceKind::Standing, Some(StanceReq::Standing) | None)
                );
                if matches_stance {
                    Ok(())
                } else {
                    Err(CommitError::UnknownOrUnmetMove { actor })
                }
            }
            (DecisionKind::ThrowBreak { .. }, Choice::ThrowBreak { .. }) => Ok(()),
            (DecisionKind::Cancel, Choice::Cancel { into }) => match into {
                None => Ok(()),
                Some(id) => {
                    if self.open_cancels(i).iter().any(|(_, w)| w.into == id) {
                        Ok(())
                    } else {
                        Err(CommitError::UnknownOrUnmetMove { actor })
                    }
                }
            },
            (DecisionKind::WakeUp, Choice::Rise | Choice::BackRise) => Ok(()),
            (DecisionKind::WakeUp, Choice::DelayRise { ticks }) => {
                if ticks >= 1 && ticks <= self.ruleset.wake_delay_max {
                    Ok(())
                } else {
                    Err(CommitError::IllegalChoice {
                        actor,
                        why: "delay outside wake_delay_max",
                    })
                }
            }
            (DecisionKind::WakeUp, Choice::Move { id }) => {
                let Some(mv) = entity.moves.iter().find(|m| m.id == id) else {
                    return Err(CommitError::UnknownOrUnmetMove { actor });
                };
                if !mv.req_down {
                    return Err(CommitError::UnknownOrUnmetMove { actor });
                }
                if !Self::affordable(entity, mv) {
                    return Err(CommitError::Denied { actor });
                }
                Ok(())
            }
            _ => Err(CommitError::IllegalChoice {
                actor,
                why: "choice does not fit prompt",
            }),
        }
    }

    /// Apply a complete batch (everything commits at once — spec §4.2). BTreeMap
    /// iteration = entity-id order = deterministic.
    fn apply_choices(&mut self) {
        let batch = self.batch.take().expect("complete batch");
        debug_assert!(batch.complete());
        let t = self.t;
        for (&actor, &choice) in &batch.committed {
            self.trace.push(TraceEvent::Committed { t, actor, choice });
            let i = self.index_of(actor);
            match choice {
                Choice::Wait { ticks } => {
                    self.entities[i].ready_tick = t + u64::from(ticks.max(1));
                }
                Choice::Move { id } => self.start_move(i, id),
                Choice::HoldStance => {
                    self.entities[i].reevaluate_at =
                        t + u64::from(self.ruleset.block_reevaluate_every);
                }
                Choice::Release => self.release_stance(i),
                Choice::Cancel { into } => match into {
                    Some(id) => self.take_cancel(i, id),
                    None => {
                        // Decline every window open this tick — final for those windows.
                        let open: Vec<u32> =
                            self.open_cancels(i).iter().map(|&(idx, _)| idx).collect();
                        if let Some(inst) = &mut self.entities[i].current {
                            for idx in open {
                                inst.cancels_prompted |= 1 << idx;
                            }
                        }
                    }
                },
                Choice::Rise => self.rise(i, self.ruleset.wake_rise_ticks, Fx::ZERO),
                Choice::BackRise => {
                    self.rise(
                        i,
                        self.ruleset.wake_back_rise_ticks,
                        self.ruleset.wake_back_rise_push,
                    );
                }
                Choice::DelayRise { ticks } => {
                    self.entities[i].state = ActorState::Down {
                        until: t + u64::from(ticks.max(1)),
                    };
                }
                // Break batches never reach here (resolve_breaks consumes them).
                Choice::ThrowBreak { .. } => debug_assert!(false, "break in tick-start batch"),
            }
        }
    }

    /// Wake-up rise: standing and hittable immediately (the meaty window), actionable
    /// after the rise ticks.
    fn rise(&mut self, i: usize, rise_ticks: u32, back_push: Fx) {
        let t = self.t;
        let back = self.entities[i].facing * back_push;
        let e = &mut self.entities[i];
        e.stance = Stance::Standing;
        e.height_off = Fx::ZERO;
        let pos = e.pos - back;
        e.pos = spatial::clamp_to_arena(&self.arena, pos).0;
        self.set_free(i, t + u64::from(rise_ticks.max(1)));
    }

    fn start_move(&mut self, i: usize, id: MoveId) {
        let t = self.t;
        self.auto_face_forced(i);
        let entity = &self.entities[i];
        let (index, mv) = entity
            .moves
            .iter()
            .enumerate()
            .find(|(_, m)| m.id == id)
            .expect("validated move id");
        let armor = mv
            .properties
            .iter()
            .find_map(|w| match w.kind {
                PropertyKind::Armor { hits, .. } => Some(hits),
                _ => None,
            })
            .unwrap_or(0);
        let keeps_crouch = mv.req_stance == Some(StanceReq::Crouching);
        let cost = mv.cost;
        let always_gains: Vec<(GainResource, u32)> = mv
            .gains
            .iter()
            .filter(|g| g.gate == GainGate::Always)
            .map(|g| (g.resource, g.amount))
            .collect();
        let e = &mut self.entities[i];
        e.breath = e.breath.saturating_sub(cost.breath);
        e.ap = e.ap.saturating_sub(cost.ap);
        e.focus = e.focus.saturating_sub(cost.focus);
        e.current = Some(MoveInstance {
            move_id: id,
            move_index: index,
            started_at: t,
            armor_hits_left: armor,
            grabbed_victim: None,
            connected_at: None,
            hit_landed: false,
            blocked: false,
            cancels_prompted: 0,
        });
        e.state = ActorState::Acting;
        e.held = None;
        e.height_off = Fx::ZERO;
        e.stance = if keeps_crouch {
            Stance::Crouching
        } else {
            Stance::Standing
        };
        for (resource, amount) in always_gains {
            self.gain(i, resource, amount);
        }
    }

    /// Pay the window + target costs and chain (spec §11): the combo's links are bought.
    fn take_cancel(&mut self, i: usize, into: MoveId) {
        let open = self.open_cancels(i);
        let (_, window) = open
            .iter()
            .find(|(_, w)| w.into == into)
            .copied()
            .expect("validated cancel");
        let e = &mut self.entities[i];
        e.ap = e.ap.saturating_sub(window.ap_cost);
        e.focus = e.focus.saturating_sub(window.focus_cost);
        self.start_move(i, into);
    }

    fn gain(&mut self, i: usize, resource: GainResource, amount: u32) {
        let e = &mut self.entities[i];
        match resource {
            GainResource::Breath => e.breath = (e.breath + amount).min(e.defense.breath_max),
            GainResource::Ap => e.ap = (e.ap + amount).min(e.defense.ap_max),
            GainResource::Focus => e.focus = (e.focus + amount).min(e.defense.focus_max),
        }
    }

    /// Apply an actor's authored gains for a gate from its live in-flight move (the
    /// parrier's path — its move survives the contact).
    fn move_gains(&mut self, i: usize, gate: GainGate) {
        let mv = self.entities[i].current_move().cloned();
        if let Some(mv) = mv {
            self.move_gains_from(i, &mv, gate);
        }
    }

    /// Apply authored gains from an explicit move (the attacker's path — a trade may
    /// have interrupted the move by application time, but the landed hit still pays).
    fn move_gains_from(&mut self, i: usize, mv: &Move, gate: GainGate) {
        let gains: Vec<(GainResource, u32)> = mv
            .gains
            .iter()
            .filter(|g| g.gate == gate)
            .map(|g| (g.resource, g.amount))
            .collect();
        for (resource, amount) in gains {
            self.gain(i, resource, amount);
        }
    }

    /// Releasing a held stance pays the stance move's authored release recovery: re-enter
    /// the move directly in its recovery phase.
    fn release_stance(&mut self, i: usize) {
        let t = self.t;
        let held = self.entities[i].held;
        let found = self.entities[i]
            .moves
            .iter()
            .enumerate()
            .find(|(_, m)| m.category == MoveCategory::Stance && m.stance_spec == held)
            .map(|(idx, m)| (idx, m.id, u64::from(m.timing.startup + m.timing.active)));
        let e = &mut self.entities[i];
        e.held = None;
        e.stance = Stance::Standing;
        match found {
            Some((index, move_id, recovery_offset)) => {
                e.state = ActorState::Acting;
                e.current = Some(MoveInstance {
                    move_id,
                    move_index: index,
                    started_at: Tick(t.0.saturating_sub(recovery_offset)),
                    armor_hits_left: 0,
                    grabbed_victim: None,
                    connected_at: None,
                    hit_landed: false,
                    blocked: false,
                    cancels_prompted: 0,
                });
            }
            None => {
                // Held spec without a matching move is authoring rot; release instantly.
                self.set_free(i, t);
            }
        }
    }

    // -- stage: world ---------------------------------------------------------

    /// Authored self-displacement, spread evenly across the current phase's ticks
    /// (spec §3.6).
    fn integrate_motion(&mut self) {
        let t = self.t;
        for i in 0..self.entities.len() {
            if self.entities[i].state != ActorState::Acting {
                continue;
            }
            let mv = self.entities[i].current_move().expect("acting");
            let timing = mv.timing;
            let motion = mv.motion;
            let elapsed = self.entities[i].move_elapsed(t).expect("acting");
            let (phase_motion, phase_len) = match MovePhase::at(timing, elapsed) {
                MovePhase::Startup => (motion.startup, timing.startup),
                MovePhase::Active => (motion.active, timing.active),
                MovePhase::Recovery => (motion.recovery, timing.recovery),
                MovePhase::Done => continue,
            };
            if phase_len == 0 {
                continue;
            }
            let len = Fx::from_num(phase_len);
            let e = &self.entities[i];
            let step = e.facing * (phase_motion.forward / len)
                + e.facing.perp() * (phase_motion.lateral / len);
            let pos = spatial::clamp_to_arena(&self.arena, e.pos + step).0;
            self.entities[i].pos = pos;
        }
    }

    /// Resolve this tick's contacts: snapshot states, evaluate every active hit through
    /// the priority table, then apply — so trades resolve fairly and order never decides
    /// a winner where the rules define a clash (spec §4.2).
    fn run_contacts(&mut self) {
        let t = self.t;
        let snapshot = self.entities.clone();

        struct Resolved {
            attacker: usize,
            victim: usize,
            /// Cloned from the snapshot: a trade may interrupt the attacker before this
            /// contact applies, but the contact already happened (spec §4.2 trades).
            mv: Move,
            hit_index: usize,
            outcome: ContactOutcome,
        }
        let mut resolved: Vec<Resolved> = Vec::new();
        let mut connects: Vec<(usize, usize)> = Vec::new();

        for (ai, attacker) in snapshot.iter().enumerate() {
            if attacker.state != ActorState::Acting {
                continue;
            }
            let Some(mv) = attacker.current_move() else {
                continue;
            };
            let inst = attacker.current.expect("acting");
            let elapsed = attacker.move_elapsed(t).expect("acting");

            // Held-victim throw hits: fire at offsets from the connect tick.
            if let (Some(victim_id), Some(connected_at)) = (inst.grabbed_victim, inst.connected_at)
            {
                let offset = u32::try_from(t.0 - connected_at.0).expect("offsets fit");
                let vi = self.index_of(victim_id);
                if self.entities[vi].state == ActorState::Ko {
                    continue;
                }
                for (hi, hit) in mv.hits.iter().enumerate() {
                    if hit.at == offset {
                        resolved.push(Resolved {
                            attacker: ai,
                            victim: vi,
                            mv: mv.clone(),
                            hit_index: hi,
                            outcome: ContactOutcome::Hit { counter: false },
                        });
                    }
                }
                continue;
            }

            if MovePhase::at(mv.timing, elapsed) != MovePhase::Active {
                continue;
            }
            let active_offset = elapsed - mv.timing.startup;

            match mv.category {
                MoveCategory::Throw => {
                    // Grabs realign on auto-facing: spacing escapes them, sidesteps
                    // don't (spec §5.4) — so the reach test is pure distance.
                    let Some(victim) = snapshot.iter().find(|e| e.id == attacker.target) else {
                        continue;
                    };
                    let vi = self.index_of(victim.id);
                    let dist = attacker.pos.distance(victim.pos);
                    if dist < mv.region.min_range || dist > mv.region.max_range {
                        continue;
                    }
                    let Some(first_hit) = mv.hits.first() else {
                        debug_assert!(false, "throw authored without hits");
                        continue;
                    };
                    match resolve::resolve_contact(mv, first_hit, victim, t) {
                        ContactOutcome::ThrowTech => {
                            // Mutual throws clash exactly once per pair.
                            if !connects.iter().any(|&(a, v)| a == vi && v == ai) {
                                resolved.push(Resolved {
                                    attacker: ai,
                                    victim: vi,
                                    mv: mv.clone(),
                                    hit_index: 0,
                                    outcome: ContactOutcome::ThrowTech,
                                });
                            }
                            connects.push((ai, vi));
                        }
                        ContactOutcome::GrabConnected => {
                            resolved.push(Resolved {
                                attacker: ai,
                                victim: vi,
                                mv: mv.clone(),
                                hit_index: 0,
                                outcome: ContactOutcome::GrabConnected,
                            });
                            connects.push((ai, vi));
                        }
                        ContactOutcome::Whiff => {}
                        other => {
                            debug_assert!(false, "throw resolved to {other:?}");
                        }
                    }
                }
                MoveCategory::Strike | MoveCategory::Motion | MoveCategory::Utility => {
                    for (hi, hit) in mv.hits.iter().enumerate() {
                        if hit.at != active_offset {
                            continue;
                        }
                        for (vi, victim) in snapshot.iter().enumerate() {
                            if vi == ai
                                || victim.side == attacker.side
                                || victim.state == ActorState::Ko
                            {
                                continue;
                            }
                            if !spatial::does_hit_spatially(attacker, mv, victim) {
                                continue;
                            }
                            let outcome = resolve::resolve_contact(mv, hit, victim, t);
                            resolved.push(Resolved {
                                attacker: ai,
                                victim: vi,
                                mv: mv.clone(),
                                hit_index: hi,
                                outcome,
                            });
                        }
                    }
                }
                MoveCategory::Stance => {}
            }
        }

        for r in resolved {
            self.apply_outcome(r.attacker, r.victim, &r.mv, r.hit_index, r.outcome);
        }

        // Grab connects open break prompts — one batch, defender side(s) commit blind.
        if !self.grabs.is_empty() {
            let pending: Vec<PendingDecision> = self
                .grabs
                .iter()
                .map(|g| PendingDecision {
                    actor: self.entities[g.victim].id,
                    side: self.entities[g.victim].side,
                    kind: DecisionKind::ThrowBreak {
                        attacker: self.entities[g.attacker].id,
                    },
                })
                .collect();
            self.batch = Some(CommitBatch {
                pending,
                committed: std::collections::BTreeMap::new(),
            });
        }
    }

    /// Governor 1 — hitstun decay: combo hit `n` loses `n * step` ticks of stun.
    fn decayed_stun(&self, combo_hits: u32, stun: u32) -> u32 {
        stun.saturating_sub(combo_hits * self.ruleset.hitstun_decay_step)
    }

    /// Governor 2 — juggle damage decay (× defender weight).
    fn juggle_damage(&self, combo_hits: u32, weight: Fx, damage: u32) -> u32 {
        let step = self.ruleset.juggle_decay_step * weight;
        let mult = Fx::ONE - step * Fx::from_num(combo_hits);
        if mult <= Fx::ZERO {
            0
        } else {
            scale_damage(damage, mult)
        }
    }

    /// Governor 7 — the gravity floor: can the attacker even pick this stun up? If the
    /// decayed stun undercuts every affordable strike's startup, the juggle drops.
    fn gravity_floor_drops(&self, ai: usize, decayed: u32) -> bool {
        if !self.ruleset.forced_landing {
            return false;
        }
        let attacker = &self.entities[ai];
        let min_pickup = attacker
            .moves
            .iter()
            .filter(|m| m.category == MoveCategory::Strike && Self::affordable(attacker, m))
            .map(|m| m.timing.startup)
            .min();
        match min_pickup {
            Some(startup) => decayed < startup,
            None => true, // nothing affordable: it drops by definition
        }
    }

    #[expect(
        clippy::too_many_lines,
        reason = "the priority-table application is one table"
    )]
    fn apply_outcome(
        &mut self,
        ai: usize,
        vi: usize,
        mv: &Move,
        hit_index: usize,
        outcome: ContactOutcome,
    ) {
        let t = self.t;
        let attacker_id = self.entities[ai].id;
        let victim_id = self.entities[vi].id;
        let hit = mv.hits.get(hit_index).cloned();

        let mut damage_applied = 0u32;
        let mut reaction_applied: Option<Reaction> = None;
        let mut combo_hits = 0u32;
        match outcome {
            ContactOutcome::Whiff => {}
            ContactOutcome::ThrowTech => {
                self.reset_after_tech(ai, vi);
            }
            ContactOutcome::GrabConnected => {
                self.entities[vi].state = ActorState::Grabbed { by: attacker_id };
                self.entities[vi].current = None;
                self.entities[vi].held = None;
                if let Some(inst) = &mut self.entities[ai].current {
                    inst.grabbed_victim = Some(victim_id);
                    inst.connected_at = Some(t);
                }
                self.grabs.push(PendingGrab {
                    attacker: ai,
                    victim: vi,
                });
            }
            ContactOutcome::Parried {
                freeze_attacker,
                parry_recovery,
            } => {
                let a = &mut self.entities[ai];
                a.current = None;
                a.state = ActorState::Hitstun {
                    until: t + u64::from(freeze_attacker.max(1)),
                };
                // The parrier banks its authored gains + the Ruleset's parry Focus
                // (skill pays — spec §9).
                self.move_gains(vi, GainGate::OnParry);
                let parry_focus = self.ruleset.focus_gains.parry;
                self.gain(vi, GainResource::Focus, parry_focus);
                let v = &mut self.entities[vi];
                v.current = None;
                v.state = ActorState::Free;
                v.ready_tick = t + u64::from(parry_recovery);
                let vap = v.defense.ap_max;
                v.ap = vap;
            }
            ContactOutcome::Blocked => {
                let hit = hit.expect("strike outcome has a hit");
                if let Some(inst) = &mut self.entities[ai].current {
                    inst.blocked = true;
                }
                self.move_gains_from(ai, mv, GainGate::OnBlock);
                let blocked_focus = self.ruleset.focus_gains.hit_blocked;
                self.gain(ai, GainResource::Focus, blocked_focus);
                let push = self.entities[ai].facing * hit.block_push;
                let v = &mut self.entities[vi];
                v.guard = v.guard.saturating_sub(hit.chip_guard);
                if v.guard == 0 {
                    v.held = None;
                    v.state = ActorState::GuardBroken {
                        until: t + u64::from(self.ruleset.guard_break_stun),
                    };
                    self.trace.push(TraceEvent::GuardBroken {
                        t,
                        actor: victim_id,
                    });
                } else {
                    v.state = ActorState::Blockstun {
                        until: t + u64::from(hit.blockstun),
                    };
                }
                let pos = spatial::clamp_to_arena(&self.arena, self.entities[vi].pos + push).0;
                self.entities[vi].pos = pos;
            }
            ContactOutcome::Armored => {
                let hit = hit.expect("strike outcome has a hit");
                if let Some(inst) = &mut self.entities[ai].current {
                    inst.blocked = true; // contact happened: ON_CONTACT gates open
                }
                let mult = self.entities[vi]
                    .current_move()
                    .into_iter()
                    .flat_map(|m| m.properties.iter())
                    .find_map(|w| match w.kind {
                        PropertyKind::Armor { dmg_mult, .. } => Some(dmg_mult),
                        _ => None,
                    })
                    .unwrap_or(Fx::ONE);
                damage_applied = scale_damage(hit.damage, mult);
                if let Some(inst) = &mut self.entities[vi].current {
                    inst.armor_hits_left = inst.armor_hits_left.saturating_sub(1);
                }
                self.damage(vi, damage_applied);
            }
            ContactOutcome::Hit { counter } => {
                let hit = hit.expect("hit outcome has a hit");
                let was_grab_followthrough = self.entities[ai]
                    .current
                    .is_some_and(|c| c.grabbed_victim.is_some());
                if !was_grab_followthrough && let Some(inst) = &mut self.entities[ai].current {
                    inst.hit_landed = true;
                }

                // Whose combo is this? Fresh trackers for fresh victims.
                if !self.entities[vi].in_combo_state() {
                    self.entities[vi].combo = ComboTracker::default();
                }
                let combo_index = self.entities[vi].combo.hits;
                let airborne = matches!(
                    self.entities[vi].state,
                    ActorState::Airborne { .. } | ActorState::WallSplat { .. }
                );

                // Attacker gains: hit + the skillful-CH split (spec §9 gain table).
                self.move_gains_from(ai, mv, GainGate::OnHit);
                let land_focus = self.ruleset.focus_gains.land_hit;
                self.gain(ai, GainResource::Focus, land_focus);
                if counter {
                    self.move_gains_from(ai, mv, GainGate::OnCh);
                    let whiffed_recovery =
                        self.entities[vi].move_phase(t) == Some(MovePhase::Recovery);
                    if whiffed_recovery {
                        self.move_gains_from(ai, mv, GainGate::OnWhiffPunish);
                        let wp = self.ruleset.focus_gains.whiff_punish;
                        self.gain(ai, GainResource::Focus, wp);
                    } else {
                        let chf = self.ruleset.focus_gains.counter_hit;
                        self.gain(ai, GainResource::Focus, chf);
                    }
                }

                // Reaction + damage selection (CH override / Ruleset default / decay).
                let (mut reaction, mut damage) = if counter {
                    match hit.ch_reaction {
                        Some(ch) => (ch, hit.damage),
                        None => {
                            let boosted = match hit.reaction {
                                Reaction::Hitstun { ticks } => Reaction::Hitstun {
                                    ticks: ticks + self.ruleset.ch_default.stun_bonus,
                                },
                                other => other,
                            };
                            (
                                boosted,
                                scale_damage(hit.damage, self.ruleset.ch_default.damage_mult),
                            )
                        }
                    }
                } else {
                    (hit.reaction, hit.damage)
                };
                if airborne {
                    let weight = self.entities[vi].defense.weight;
                    damage = self.juggle_damage(combo_index, weight, damage);
                }
                damage_applied = damage;
                self.damage(vi, damage);
                if self.entities[vi].state != ActorState::Ko {
                    if was_grab_followthrough {
                        // Held victim: damage lands; the reaction applies on the LAST
                        // authored hit, releasing the victim into it.
                        let last_at = mv.hits.iter().map(|h| h.at).max().expect("throw has hits");
                        if hit.at == last_at {
                            if let Some(inst) = &mut self.entities[ai].current {
                                inst.grabbed_victim = None;
                            }
                            self.apply_reaction(ai, vi, &hit, reaction, combo_index);
                            reaction_applied = Some(reaction);
                        }
                    } else {
                        // Grounded Screw/Bound degrade before application (spec §6.1).
                        if !airborne {
                            reaction = match reaction {
                                Reaction::Screw { stun, .. } | Reaction::Bound { stun } => {
                                    Reaction::Hitstun { ticks: stun }
                                }
                                other => other,
                            };
                        }
                        // Count the hit BEFORE applying: an ender's end_combo must see it.
                        self.entities[vi].combo.hits = combo_index + 1;
                        combo_hits = combo_index + 1;
                        self.apply_reaction(ai, vi, &hit, reaction, combo_index);
                        reaction_applied = Some(reaction);
                    }
                }
            }
        }

        self.trace.push(TraceEvent::Contact {
            t,
            attacker: attacker_id,
            victim: victim_id,
            mv: mv.id,
            outcome,
            damage: damage_applied,
            reaction: reaction_applied,
            combo_hits,
        });
    }

    /// Interrupt an actor's in-flight move: the move is gone, any held stance drops,
    /// and a victim held by the now-interrupted throw goes free immediately (no path
    /// may strand a Grabbed actor).
    fn interrupt_actor(&mut self, i: usize) {
        let held_victim = self.entities[i].current.and_then(|c| c.grabbed_victim);
        self.entities[i].current = None;
        self.entities[i].held = None;
        if let Some(victim) = held_victim {
            self.release_grabbed(victim);
        }
    }

    /// Apply a (possibly decayed/degraded) reaction. The victim's current juggle state
    /// shapes the application; the extender latches (governor 3) and the gravity floor
    /// (governor 7) bind here.
    fn apply_reaction(
        &mut self,
        ai: usize,
        vi: usize,
        hit: &crate::data::HitEvent,
        reaction: Reaction,
        combo_index: u32,
    ) {
        let t = self.t;
        let latches = self.ruleset.extender_latches;
        let airborne = matches!(
            self.entities[vi].state,
            ActorState::Airborne { .. } | ActorState::WallSplat { .. }
        );
        // A sustained juggle keeps flying; everything else resolves below.
        if airborne {
            let carry = hit.juggle_carry;
            match reaction {
                Reaction::Hitstun { ticks } | Reaction::Crumple { ticks } => {
                    self.sustain_juggle(ai, vi, ticks, combo_index, carry);
                }
                Reaction::Launch {
                    rise,
                    carry: lcarry,
                    stun,
                } => {
                    let e = &mut self.entities[vi];
                    e.height_off = e.height_off.max(rise);
                    self.sustain_juggle(ai, vi, stun, combo_index, carry + lcarry);
                }
                Reaction::Screw {
                    carry: scarry,
                    stun,
                } => {
                    if self.entities[vi].combo.screw_used < latches.screw {
                        self.entities[vi].combo.screw_used += 1;
                        // Flattened arc, extended carry (🔬 T7 tailspin).
                        let e = &mut self.entities[vi];
                        e.height_off /= Fx::from_num(2);
                        self.sustain_juggle(ai, vi, stun, combo_index, carry + scarry);
                    } else {
                        self.sustain_juggle(ai, vi, stun, combo_index, carry);
                    }
                }
                Reaction::Bound { stun } => {
                    if self.entities[vi].combo.bound_used < latches.bound {
                        self.entities[vi].combo.bound_used += 1;
                        // Slammed to a re-juggleable bounce (🔬 T6 bound).
                        self.entities[vi].height_off = Fx::from_num(1) / Fx::from_num(2);
                        self.sustain_juggle(ai, vi, stun, combo_index, Fx::ZERO);
                    } else {
                        self.sustain_juggle(ai, vi, stun, combo_index, carry);
                    }
                }
                Reaction::Knockdown {
                    hard: _,
                    down_ticks,
                } => {
                    self.interrupt_actor(vi);
                    let e = &mut self.entities[vi];
                    e.state = ActorState::Down {
                        until: t + u64::from(down_ticks.max(1)),
                    };
                    e.stance = Stance::Down;
                    e.height_off = Fx::ZERO;
                    self.end_combo(vi);
                }
                Reaction::Push { dist } => {
                    self.displace_victim(ai, vi, dist);
                }
            }
            return;
        }

        match reaction {
            Reaction::Hitstun { ticks } => {
                let stun = self.decayed_stun(combo_index, ticks).max(1);
                self.interrupt_actor(vi);
                self.entities[vi].state = ActorState::Hitstun {
                    until: t + u64::from(stun),
                };
            }
            Reaction::Crumple { ticks } => {
                let stun = self.decayed_stun(combo_index, ticks).max(1);
                self.interrupt_actor(vi);
                self.entities[vi].state = ActorState::Crumple {
                    until: t + u64::from(stun),
                };
            }
            Reaction::Launch { rise, carry, stun } => {
                let decayed = self.decayed_stun(combo_index, stun).max(1);
                if self.gravity_floor_drops(ai, decayed) {
                    self.interrupt_actor(vi);
                    self.trace.push(TraceEvent::Landed {
                        t,
                        victim: self.entities[vi].id,
                    });
                    self.floor(vi, "gravity floor");
                    return;
                }
                self.interrupt_actor(vi);
                let e = &mut self.entities[vi];
                e.state = ActorState::Airborne {
                    stun_until: t + u64::from(decayed),
                };
                e.stance = Stance::Airborne;
                e.height_off = rise;
                self.displace_victim(ai, vi, carry);
            }
            // Screw/Bound were degraded to Hitstun by the caller on grounded victims.
            Reaction::Screw { .. } | Reaction::Bound { .. } => {
                debug_assert!(false, "grounded extender must be degraded by the caller");
            }
            Reaction::Knockdown {
                hard: _,
                down_ticks,
            } => {
                self.interrupt_actor(vi);
                let e = &mut self.entities[vi];
                e.state = ActorState::Down {
                    until: t + u64::from(down_ticks.max(1)),
                };
                e.stance = Stance::Down;
                self.end_combo(vi);
            }
            Reaction::Push { dist } => {
                self.displace_victim(ai, vi, dist);
            }
        }
    }

    /// Keep an airborne victim flying: decayed stun, carry, the gravity floor, and the
    /// splat check on the carry. WallSplat pickups return to Airborne here.
    fn sustain_juggle(&mut self, ai: usize, vi: usize, stun: u32, combo_index: u32, carry: Fx) {
        let t = self.t;
        let decayed = self.decayed_stun(combo_index, stun);
        if decayed == 0 || self.gravity_floor_drops(ai, decayed) {
            self.trace.push(TraceEvent::Landed {
                t,
                victim: self.entities[vi].id,
            });
            self.floor(vi, "gravity floor");
            return;
        }
        self.entities[vi].state = ActorState::Airborne {
            stun_until: t + u64::from(decayed),
        };
        self.entities[vi].stance = Stance::Airborne;
        self.displace_victim(ai, vi, carry);
    }

    /// Displace a hit victim along the attacker's facing; a splat-able wall catches
    /// airborne victims (once per combo) instead of clamping (spec §3.7).
    fn displace_victim(&mut self, ai: usize, vi: usize, dist: Fx) {
        let t = self.t;
        let push = self.entities[ai].facing * dist;
        let target = self.entities[vi].pos + push;
        let (clamped, wall) = spatial::clamp_to_arena(&self.arena, target);
        self.entities[vi].pos = clamped;
        let Some(wall) = wall else { return };
        let airborne = matches!(
            self.entities[vi].state,
            ActorState::Airborne { .. } | ActorState::WallSplat { .. }
        );
        if !airborne || !wall.splattable {
            return;
        }
        if self.entities[vi].combo.splat_used >= self.ruleset.extender_latches.wall_splat {
            return;
        }
        self.entities[vi].combo.splat_used += 1;
        let until = t + u64::from(self.ruleset.splat_duration.max(1));
        self.entities[vi].state = ActorState::WallSplat { until };
        self.trace.push(TraceEvent::WallSplat {
            t,
            victim: self.entities[vi].id,
        });
    }

    fn damage(&mut self, vi: usize, amount: u32) {
        // The comeback factor: taking damage banks a little Focus (spec §9).
        let comeback = amount.saturating_mul(self.ruleset.focus_gains.take_damage_per_100) / 100;
        if comeback > 0 {
            self.gain(vi, GainResource::Focus, comeback);
        }
        let v = &mut self.entities[vi];
        if v.state == ActorState::Ko {
            return;
        }
        v.hp = v.hp.saturating_sub(amount);
        if v.hp == 0 {
            let id = v.id;
            v.state = ActorState::Ko;
            v.current = None;
            v.held = None;
            v.stance = Stance::Down;
            self.trace.push(TraceEvent::Ko {
                t: self.t,
                actor: id,
            });
        }
    }

    fn reset_after_tech(&mut self, ai: usize, vi: usize) {
        let t = self.t;
        let recovery = u64::from(self.ruleset.throw_tech_recovery);
        let half_push = self.ruleset.throw_tech_push / Fx::from_num(2);
        for &i in &[ai, vi] {
            let back = self.entities[i].facing * half_push;
            self.entities[i].current = None;
            self.entities[i].held = None;
            let pos = self.entities[i].pos - back;
            self.entities[i].pos = spatial::clamp_to_arena(&self.arena, pos).0;
            self.set_free(i, t + recovery);
        }
    }

    /// Resolve the pending break reads against the committed guesses (spec §5.4).
    fn resolve_breaks(&mut self) {
        let batch = self.batch.take().expect("break batch");
        debug_assert!(batch.complete());
        let t = self.t;
        let grabs = std::mem::take(&mut self.grabs);
        for grab in grabs {
            let victim_id = self.entities[grab.victim].id;
            let attacker_id = self.entities[grab.attacker].id;
            let choice = batch.committed[&victim_id];
            self.trace.push(TraceEvent::Committed {
                t,
                actor: victim_id,
                choice,
            });
            let Choice::ThrowBreak { guess } = choice else {
                unreachable!("validated")
            };
            // A same-tick trade may have interrupted the thrower between the connect
            // and this resolution: the grab dissolves and the victim goes free.
            let still_holding = self.entities[grab.attacker].state == ActorState::Acting
                && self.entities[grab.attacker]
                    .current
                    .is_some_and(|c| c.grabbed_victim == Some(victim_id));
            if !still_holding {
                self.release_grabbed(victim_id);
                self.trace.push(TraceEvent::ThrowResolved {
                    t,
                    attacker: attacker_id,
                    victim: victim_id,
                    resolution: ThrowResolution::Interrupted,
                });
                continue;
            }
            let key = self.entities[grab.attacker]
                .current_move()
                .and_then(|m| m.break_key);
            let teched = key.is_some() && guess == key;
            if teched {
                // Attacker's throw is consumed by the tech.
                self.reset_after_tech(grab.attacker, grab.victim);
                self.trace.push(TraceEvent::ThrowResolved {
                    t,
                    attacker: attacker_id,
                    victim: victim_id,
                    resolution: ThrowResolution::Teched,
                });
            } else {
                self.trace.push(TraceEvent::ThrowResolved {
                    t,
                    attacker: attacker_id,
                    victim: victim_id,
                    resolution: ThrowResolution::Thrown,
                });
                // Offset-0 hits land this very tick.
                let mv = self.entities[grab.attacker]
                    .current_move()
                    .expect("throwing")
                    .clone();
                for (hi, hit) in mv.hits.iter().enumerate() {
                    if hit.at == 0 {
                        self.apply_outcome(
                            grab.attacker,
                            grab.victim,
                            &mv,
                            hi,
                            ContactOutcome::Hit { counter: false },
                        );
                    }
                }
            }
        }
    }

    fn release_grabbed(&mut self, victim: EntityId) {
        let vi = self.index_of(victim);
        if matches!(self.entities[vi].state, ActorState::Grabbed { .. }) {
            self.entities[vi].stance = Stance::Standing;
            let t = self.t;
            self.set_free(vi, t);
        }
    }

    // -- stage: tick end ------------------------------------------------------

    fn upkeep_end(&mut self) {
        // Orphaned-grab sweep: a victim held by an interrupted or KO'd thrower goes
        // free (no path may strand a Grabbed actor — the no-deadlock invariant).
        let orphaned: Vec<EntityId> = self
            .entities
            .iter()
            .filter_map(|v| match v.state {
                ActorState::Grabbed { by } => {
                    let held = self.entities.iter().find(|a| a.id == by).is_some_and(|a| {
                        a.state == ActorState::Acting
                            && a.current.is_some_and(|c| c.grabbed_victim == Some(v.id))
                    });
                    if held { None } else { Some(v.id) }
                }
                _ => None,
            })
            .collect();
        for id in orphaned {
            self.release_grabbed(id);
        }
        for e in &mut self.entities {
            if e.state == ActorState::Ko {
                continue;
            }
            // Guard regen: slow, while not blocking (spec §5.3).
            if !e.guarding() && e.guard < e.defense.guard_max {
                e.guard_regen_acc += 1;
                if e.guard_regen_acc >= e.defense.guard_regen_interval {
                    e.guard_regen_acc = 0;
                    e.guard += 1;
                }
            }
            // Breath regen: while not executing (spec §9).
            if e.state != ActorState::Acting && e.breath < e.defense.breath_max {
                e.breath_regen_acc += 1;
                if e.breath_regen_acc >= e.defense.breath_regen_interval {
                    e.breath_regen_acc = 0;
                    e.breath += 1;
                }
            }
        }
        // Side elimination -> outcome (spec §8.6 generalizes; 1v1 is the special case).
        let mut living: Vec<SideId> = self
            .entities
            .iter()
            .filter(|e| e.state != ActorState::Ko)
            .map(|e| e.side)
            .collect();
        living.sort_unstable();
        living.dedup();
        match living.len() {
            0 => self.finish(None),
            1 => self.finish(Some(living[0])),
            _ => {}
        }
    }

    fn finish(&mut self, winner: Option<SideId>) {
        self.over = Some(winner);
        self.trace.push(TraceEvent::SimEnded { t: self.t, winner });
    }

    fn index_of(&self, id: EntityId) -> usize {
        self.entities
            .iter()
            .position(|e| e.id == id)
            .expect("known entity")
    }
}

/// Integer damage scaled by an authored fixed-point multiplier (deterministic floor).
fn scale_damage(damage: u32, mult: Fx) -> u32 {
    (Fx::from_num(damage) * mult).to_num::<u32>()
}
