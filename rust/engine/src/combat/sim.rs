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
//! TODO(Phase 3): true state goes module-private behind the Observation API (C-FOG);
//! until then `entity()` exposes full state for tests.

use crate::core::fx::{Fx, FxVec2};
use crate::core::ids::{EntityId, SideId};
use crate::core::tick::Tick;
use crate::data::movedef::{Move, MoveCategory, PropertyKind, StanceKind, StanceReq};
use crate::data::{ArenaDef, DefenseProfile, MoveId, Reaction, Ruleset};
use crate::trace::{ThrowResolution, TraceEvent};

use super::entity::{ActorState, Entity, MoveInstance, MovePhase, Stance};
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
                hp: e.defense.hp_max,
                guard: e.defense.guard_max,
                guard_regen_acc: 0,
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

    // ── the public pump ──────────────────────────────────────────────────────

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

    /// Full actor state — test surface only. TODO(Phase 3): goes module-private behind
    /// `observe()`; the UI and AI never see this.
    #[must_use]
    pub fn entity(&self, id: EntityId) -> Option<&Entity> {
        self.entities.iter().find(|e| e.id == id)
    }

    #[must_use]
    pub fn trace(&self) -> &[TraceEvent] {
        &self.trace
    }

    // ── stage: tick start ────────────────────────────────────────────────────

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
                    let e = &mut self.entities[i];
                    // A thrown victim still held at move end is released standing.
                    if let Some(victim) = e.current.and_then(|c| c.grabbed_victim) {
                        e.current = None;
                        self.release_grabbed(victim);
                    } else {
                        e.current = None;
                    }
                    let e = &mut self.entities[i];
                    e.state = ActorState::Free;
                    e.ready_tick = t;
                    if e.held.is_none() {
                        e.stance = Stance::Standing;
                    }
                }
            }
            // Stun / down expiries.
            let e = &mut self.entities[i];
            match e.state {
                ActorState::Hitstun { until } | ActorState::GuardBroken { until } if until == t => {
                    e.state = ActorState::Free;
                    e.ready_tick = t;
                    e.stance = Stance::Standing;
                }
                ActorState::Blockstun { until } if until == t => {
                    // Still holding guard; an event touched you -> re-decide now (§5.3).
                    e.state = ActorState::HoldingStance;
                    e.reevaluate_at = t;
                }
                ActorState::Down { until } if until == t => {
                    e.stance = Stance::Standing;
                    e.state = ActorState::Free;
                    e.ready_tick = t;
                }
                _ => {}
            }
        }
        for i in 0..self.entities.len() {
            self.auto_face(i);
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
        let target = self.entities[i].target;
        let Some(target_pos) = self.entity(target).map(|e| e.pos) else {
            return;
        };
        let dir = (target_pos - self.entities[i].pos).normalize_or_zero();
        if dir != FxVec2::ZERO {
            self.entities[i].facing = dir;
        }
    }

    /// Collect this tick's Ready / StanceReevaluate prompts. Returns true if a batch is
    /// now awaiting commits.
    fn collect_decisions(&mut self) -> bool {
        let t = self.t;
        let pending: Vec<PendingDecision> = self
            .entities
            .iter()
            .filter_map(|e| match e.state {
                ActorState::Free if e.ready_tick == t => Some(PendingDecision {
                    actor: e.id,
                    side: e.side,
                    kind: DecisionKind::Ready,
                }),
                ActorState::HoldingStance if e.reevaluate_at == t => Some(PendingDecision {
                    actor: e.id,
                    side: e.side,
                    kind: DecisionKind::StanceReevaluate,
                }),
                _ => None,
            })
            .collect();
        if pending.is_empty() {
            return false;
        }
        self.batch = Some(CommitBatch {
            pending,
            committed: std::collections::BTreeMap::new(),
        });
        true
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
        let entity = self.entity(actor).expect("pending actor exists");
        match (pending.kind, choice) {
            (DecisionKind::Ready, Choice::Wait { .. }) => Ok(()),
            (DecisionKind::Ready, Choice::Move { id }) => {
                let Some(mv) = entity.moves.iter().find(|m| m.id == id) else {
                    return Err(CommitError::UnknownOrUnmetMove { actor });
                };
                // Free actors are standing in Phase 1; crouch-required moves are only
                // reachable from a held crouching stance.
                match mv.req_stance {
                    None | Some(StanceReq::Standing) => Ok(()),
                    Some(StanceReq::Crouching) => Err(CommitError::UnknownOrUnmetMove { actor }),
                }
            }
            (DecisionKind::StanceReevaluate, Choice::HoldStance | Choice::Release) => Ok(()),
            (DecisionKind::StanceReevaluate, Choice::Move { id }) => {
                // Direct moves from a held stance: only from a pure body stance (no
                // guard commitment) whose kind the move requires — the while-crouching
                // idiom. Guarded holds must Release first (spec §5.3: release is brief
                // but real).
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
                // Break batches never reach here (resolve_breaks consumes them).
                Choice::ThrowBreak { .. } => debug_assert!(false, "break in tick-start batch"),
            }
        }
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
        let e = &mut self.entities[i];
        e.current = Some(MoveInstance {
            move_id: id,
            move_index: index,
            started_at: t,
            armor_hits_left: armor,
            grabbed_victim: None,
            connected_at: None,
        });
        e.state = ActorState::Acting;
        e.held = None;
        e.stance = if keeps_crouch {
            Stance::Crouching
        } else {
            Stance::Standing
        };
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
                });
            }
            None => {
                // Held spec without a matching move is authoring rot; release instantly.
                e.state = ActorState::Free;
                e.ready_tick = t;
            }
        }
    }

    /// Commit-time facing (even from non-actionable-looking transitions).
    fn auto_face_forced(&mut self, i: usize) {
        let target = self.entities[i].target;
        let Some(target_pos) = self.entity(target).map(|e| e.pos) else {
            return;
        };
        let dir = (target_pos - self.entities[i].pos).normalize_or_zero();
        if dir != FxVec2::ZERO {
            self.entities[i].facing = dir;
        }
    }

    // ── stage: world ─────────────────────────────────────────────────────────

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
            let pos = spatial::clamp_to_arena(&self.arena, e.pos + step);
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
            self.apply_outcome(r.attacker, r.victim, r.hit_index, r.outcome);
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

    #[expect(
        clippy::too_many_lines,
        reason = "the priority-table application is one table"
    )]
    fn apply_outcome(&mut self, ai: usize, vi: usize, hit_index: usize, outcome: ContactOutcome) {
        let t = self.t;
        let attacker_id = self.entities[ai].id;
        let victim_id = self.entities[vi].id;
        let mv = self.entities[ai].current_move().expect("acting").clone();
        let hit = mv.hits.get(hit_index).cloned();

        let mut damage_applied = 0u32;
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
                let v = &mut self.entities[vi];
                v.current = None;
                v.state = ActorState::Free;
                v.ready_tick = t + u64::from(parry_recovery);
            }
            ContactOutcome::Blocked => {
                let hit = hit.expect("strike outcome has a hit");
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
                let pos = spatial::clamp_to_arena(&self.arena, self.entities[vi].pos + push);
                self.entities[vi].pos = pos;
            }
            ContactOutcome::Armored => {
                let hit = hit.expect("strike outcome has a hit");
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
                let (reaction, damage) = if counter {
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
                            self.apply_reaction(ai, vi, reaction);
                        }
                    } else {
                        self.apply_reaction(ai, vi, reaction);
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
        });
    }

    fn apply_reaction(&mut self, ai: usize, vi: usize, reaction: Reaction) {
        let t = self.t;
        match reaction {
            Reaction::Hitstun { ticks } => {
                let v = &mut self.entities[vi];
                v.current = None;
                v.held = None;
                v.state = ActorState::Hitstun {
                    until: t + u64::from(ticks.max(1)),
                };
            }
            Reaction::Knockdown {
                hard: _,
                down_ticks,
            } => {
                let v = &mut self.entities[vi];
                v.current = None;
                v.held = None;
                v.stance = Stance::Down;
                v.state = ActorState::Down {
                    until: t + u64::from(down_ticks.max(1)),
                };
            }
            Reaction::Push { dist } => {
                let push = self.entities[ai].facing * dist;
                let pos = spatial::clamp_to_arena(&self.arena, self.entities[vi].pos + push);
                self.entities[vi].pos = pos;
            }
        }
    }

    fn damage(&mut self, vi: usize, amount: u32) {
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
            let e = &mut self.entities[i];
            e.current = None;
            e.held = None;
            e.state = ActorState::Free;
            e.ready_tick = t + recovery;
            let pos = e.pos - back;
            self.entities[i].pos = spatial::clamp_to_arena(&self.arena, pos);
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
        let v = &mut self.entities[vi];
        if matches!(v.state, ActorState::Grabbed { .. }) {
            v.state = ActorState::Free;
            v.ready_tick = self.t;
            v.stance = Stance::Standing;
        }
    }

    // ── stage: tick end ──────────────────────────────────────────────────────

    fn upkeep_end(&mut self) {
        // Guard regen: slow, while not blocking (spec §5.3).
        for e in &mut self.entities {
            let blocking = e.guarding();
            if !blocking && e.state != ActorState::Ko && e.guard < e.defense.guard_max {
                e.guard_regen_acc += 1;
                if e.guard_regen_acc >= e.defense.guard_regen_interval {
                    e.guard_regen_acc = 0;
                    e.guard += 1;
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
