//! The trace — the engine's behavioral contract (tech-plan §2.4).
//!
//! Every commit, contact, reaction, meter change, and state transition is a
//! serde-serializable tagged event. Replays, tests, and (from Phase 6) the frozen golden
//! vectors v2 all consume this stream. Phase 0 carries only the events the trivial sim
//! emits; the schema grows with each phase and is free to change until the vectors freeze
//! (after which schema changes re-freeze the vectors with a changelog — standing rule 4).

use crate::combat::sim::Decision;
use crate::core::ids::EntityId;
use crate::core::tick::Tick;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum TraceEvent {
    /// Sim constructed; lists the participating actors in stable id order.
    SimStarted { entities: Vec<EntityId> },
    /// An actor's owner committed a decision at its decision point (spec §4.1).
    Committed {
        t: Tick,
        actor: EntityId,
        decision: Decision,
    },
    /// The sim stopped advancing (decision log exhausted or `max_ticks` reached).
    SimEnded { t: Tick },
}
