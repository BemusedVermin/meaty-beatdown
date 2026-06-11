//! The trace — the engine's behavioral contract (tech-plan §2.4).
//!
//! Every commit, contact, reaction, meter change, and state transition is a
//! serde-serializable tagged event. Replays, tests, and (from Phase 6) the frozen golden
//! vectors v2 all consume this stream. The schema grows with each phase and is free to
//! change until the vectors freeze (after which schema changes re-freeze the vectors with
//! a changelog — standing rule 4). Resolved events are PUBLIC by charter (C-FOG: facts
//! are never fogged, only intent).

use crate::combat::resolve::ContactOutcome;
use crate::combat::schedule::Choice;
use crate::core::ids::{EntityId, SideId};
use crate::core::tick::Tick;
use crate::data::MoveId;
use serde::{Deserialize, Serialize};

/// How a connected grab resolved after the break read (spec §5.4).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThrowResolution {
    /// Correct break guess: clash, both reset, small separation.
    Teched,
    /// Wrong guess or declined: the throw's hit events run.
    Thrown,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum TraceEvent {
    /// Sim constructed; the participating actors in stable id order.
    SimStarted { entities: Vec<EntityId> },
    /// An actor's owner committed a choice at a decision point (spec §4.1). The decision
    /// log IS the sequence of these events — replaying them reproduces the fight (C-DET).
    Committed {
        t: Tick,
        actor: EntityId,
        choice: Choice,
    },
    /// A contact resolved through the priority table (spec §5.1).
    Contact {
        t: Tick,
        attacker: EntityId,
        victim: EntityId,
        mv: MoveId,
        outcome: ContactOutcome,
        /// HP damage actually applied (post CH/armor scaling); 0 for non-damaging outcomes.
        damage: u32,
    },
    /// The break read resolved a connected grab (spec §5.4).
    ThrowResolved {
        t: Tick,
        attacker: EntityId,
        victim: EntityId,
        resolution: ThrowResolution,
    },
    /// Guard meter hit zero: the anti-turtle terminus (spec §5.3).
    GuardBroken { t: Tick, actor: EntityId },
    /// HP reached zero; the actor is out of the fight.
    Ko { t: Tick, actor: EntityId },
    /// The fight ended. `winner` is None on a `max_ticks` cap or a mutual wipe.
    SimEnded { t: Tick, winner: Option<SideId> },
}
