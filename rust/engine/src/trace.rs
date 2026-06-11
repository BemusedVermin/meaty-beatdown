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
use crate::data::{MoveId, Reaction};
use serde::{Deserialize, Serialize};

/// How a connected grab resolved after the break read (spec §5.4).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThrowResolution {
    /// Correct break guess: clash, both reset, small separation.
    Teched,
    /// Wrong guess or declined: the throw's hit events run.
    Thrown,
    /// The thrower was interrupted (a trade, or later an ally's hit) between the
    /// connect and the resolution: the grab dissolves and the victim goes free.
    Interrupted,
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
        /// HP damage actually applied (post CH/decay/armor scaling); 0 for
        /// non-damaging outcomes.
        damage: u32,
        /// The reaction actually applied to the victim (post CH override, post latch
        /// degradation) — what makes a §6.2 combo trace read as authored.
        reaction: Option<Reaction>,
        /// The victim's combo hit count including this hit (decay index + 1).
        combo_hits: u32,
    },
    /// A juggle carried the victim into a splat-able wall (spec §3.7) — once per combo.
    WallSplat { t: Tick, victim: EntityId },
    /// An airborne victim's stun expired (or the gravity floor fired): they land.
    Landed { t: Tick, victim: EntityId },
    /// The victim regained freedom or hit the ground: the combo is over.
    ComboEnded {
        t: Tick,
        victim: EntityId,
        hits: u32,
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
    /// A KO'd actor was restored by an authored utility move.
    Revived { t: Tick, actor: EntityId, hp: u32 },
    /// The fight ended. `winner` is None on a `max_ticks` cap or a mutual wipe.
    SimEnded { t: Tick, winner: Option<SideId> },
}
