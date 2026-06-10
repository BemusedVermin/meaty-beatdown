//! The Phase 0 trivial sim: the advance-loop skeleton of spec §4.3 with WAIT as the only
//! decision. It exists to prove the determinism bedrock end-to-end — `ready_tick`
//! scheduling, stable entity-id commit order (spec §4.2), trace emission, and replay
//! equality — before any combat rule exists. Phase 1 grows this into the real `CombatSim`
//! (pump_decisions / commit / step / trace).

use crate::core::ids::{EntityId, SideId};
use crate::core::tick::Tick;
use crate::trace::TraceEvent;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// What an actor's owner chooses at a decision point. Phase 0: WAIT only (spec §4.1 lists
/// WAIT among the Ready choices; the rest of the union arrives in Phase 1).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Decision {
    Wait { ticks: u32 },
}

/// Initial placement of one actor on the timeline.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EntityInit {
    pub id: EntityId,
    pub side: SideId,
    /// When this actor first gets a free decision.
    pub ready_at: Tick,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SimConfig {
    pub entities: Vec<EntityInit>,
    /// Hard termination cap (fsm.md open-questions: with no rounds and no timer, a bout
    /// must still provably end; replays and AI-vs-AI runs rely on this bound).
    pub max_ticks: u64,
}

/// A recorded sequence of decisions, consumed in commit order. With the engine's
/// deterministic decision ordering (tick, then entity id), a flat log replays a fight
/// exactly — C-DET: a fight is a pure function of (initial state, content, decisions).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct DecisionLog(VecDeque<Decision>);

impl DecisionLog {
    #[must_use]
    pub fn new(decisions: impl Into<VecDeque<Decision>>) -> Self {
        Self(decisions.into())
    }

    fn pop(&mut self) -> Option<Decision> {
        self.0.pop_front()
    }
}

struct Actor {
    id: EntityId,
    #[expect(dead_code, reason = "sides drive win/loss from Phase 1")]
    side: SideId,
    ready_tick: Tick,
}

/// The fight simulation. True state is module-private from Phase 3 (the fog boundary); in
/// Phase 0 the only public surface is already just construction, `run`, and the trace.
pub struct CombatSim {
    t: Tick,
    /// Sorted by id at construction: stable entity-id order is the same-tick determinism
    /// rule (spec §4.2).
    actors: Vec<Actor>,
    max_ticks: u64,
    trace: Vec<TraceEvent>,
}

impl CombatSim {
    #[must_use]
    pub fn new(config: SimConfig) -> Self {
        let mut actors: Vec<Actor> = config
            .entities
            .into_iter()
            .map(|e| Actor {
                id: e.id,
                side: e.side,
                ready_tick: e.ready_at,
            })
            .collect();
        actors.sort_by_key(|a| a.id);
        let trace = vec![TraceEvent::SimStarted {
            entities: actors.iter().map(|a| a.id).collect(),
        }];
        Self {
            t: Tick::ZERO,
            actors,
            max_ticks: config.max_ticks,
            trace,
        }
    }

    /// Advance the loop of spec §4.3 until the decision log runs dry at a decision point or
    /// `max_ticks` is reached. (Side-blind same-tick commit grouping arrives in Phase 1;
    /// with WAIT as the only decision there is nothing to hide yet.)
    pub fn run(&mut self, mut log: DecisionLog) {
        while self.t.0 < self.max_ticks {
            if !self.pump_decisions(&mut log) {
                break; // a decision was needed and the log was dry: the scenario is over
            }
            self.step_tick();
            self.t.advance();
        }
        self.trace.push(TraceEvent::SimEnded { t: self.t });
    }

    /// Gather every actor due to decide at `T` (stable id order — the actors vec is sorted)
    /// and apply its scripted decision. Returns false if the log ran dry.
    fn pump_decisions(&mut self, log: &mut DecisionLog) -> bool {
        for i in 0..self.actors.len() {
            if self.actors[i].ready_tick != self.t {
                continue;
            }
            let Some(decision) = log.pop() else {
                return false;
            };
            let actor_id = self.actors[i].id;
            self.apply(i, decision);
            self.trace.push(TraceEvent::Committed {
                t: self.t,
                actor: actor_id,
                decision,
            });
        }
        true
    }

    fn apply(&mut self, actor_index: usize, decision: Decision) {
        match decision {
            Decision::Wait { ticks } => {
                self.actors[actor_index].ready_tick = self.t + u64::from(ticks);
            }
        }
    }

    /// Per-tick world update. Phase 0: nothing moves yet. Phase 1 fills this with move
    /// phases, contacts, and motion integration (spec §4.3).
    fn step_tick(&mut self) {}

    #[must_use]
    pub fn trace(&self) -> &[TraceEvent] {
        &self.trace
    }
}
