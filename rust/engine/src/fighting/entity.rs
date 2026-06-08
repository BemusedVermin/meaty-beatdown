//! Entity runtime state — the per-fighter state the engine mutates (spec §0.4). The Entity holds its
//! own offensive action *and* defensive / runtime state (the "no separate `Fighter` type" decision):
//! a 3D position, its **body** (the part-boxes that are its hurtboxes and attack-box sources), the
//! in-flight move, and its reaction state.

use super::frame::MoveId;
use super::resolver::ContactResult;
use super::space::{place, Box3, BodyPart, Vec3};
use super::sim::Tick;

pub type Health = u32;
pub type EntityId = usize;

/// Which team an entity fights for. A side is eliminated when all its entities are `KO`; the fight
/// ends when one side remains (fsm.md: sides / elimination).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SideId(pub u8);

/// The entity's reaction state while it is *not* executing a move. While a move is in flight the
/// phase (startup/active/recovery) is derived from elapsed ticks instead. This is the engine's
/// authoritative state; [`crate::state::ActorState`] is the ECS-facing view a later system projects.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Reaction {
    #[default]
    Neutral,
    Hitstun,
    Blockstun,
    GuardBroken,
    Airborne,
    Down,
    Parried,
    KO,
}

/// A move in flight on an entity.
#[derive(Clone, Debug)]
pub struct MoveInstance {
    pub move_id: MoveId,
    pub start_tick: Tick,
    pub armor_used: u8,
    /// Has this active window already resolved a contact? (prevents an N-active-frame move from
    /// dealing N hits — multi-hit moves are deferred).
    pub connected: bool,
    /// The resolved contact, retained for (deferred) hit-confirm cancels.
    pub contact: Option<ContactResult>,
}

/// A fighter's body: the local-space part boxes that are its **hurtboxes** and its **attack-box
/// sources**. Sized to the fighter — so a punch with `HitboxSource::Part(Fist)` "fits the body type".
#[derive(Clone, Debug)]
pub struct Body {
    pub parts: Vec<(BodyPart, Box3)>,
}
impl Body {
    pub fn part(&self, p: BodyPart) -> Option<Box3> {
        self.parts.iter().find(|(q, _)| *q == p).map(|(_, b)| *b)
    }
    pub fn has(&self, p: BodyPart) -> bool {
        self.parts.iter().any(|(q, _)| *q == p)
    }
    /// Does this body meet a move's morphology requirements (have every required part)?
    pub fn satisfies(&self, required: &[BodyPart]) -> bool {
        required.iter().all(|p| self.has(*p))
    }
}

/// A single fighter's runtime state.
#[derive(Clone, Debug)]
pub struct Entity {
    pub side: SideId,
    /// World position — X lane, Y vertical, Z lateral.
    pub pos: Vec3,
    pub facing: i8, // +1 / -1
    pub body: Body,
    pub health: Health,
    pub ready_tick: Tick,
    pub action: Option<MoveInstance>,
    pub reaction: Reaction,
}

impl Entity {
    pub fn is_alive(&self) -> bool {
        self.reaction != Reaction::KO
    }
    /// Actionable = free to choose: alive, not mid-move, in neutral, and its ready tick has arrived.
    pub fn is_actionable(&self, now: Tick) -> bool {
        self.is_alive()
            && self.action.is_none()
            && self.reaction == Reaction::Neutral
            && self.ready_tick <= now
    }
    /// Ticks elapsed into the current move, if any.
    pub fn elapsed(&self, now: Tick) -> Option<Tick> {
        self.action.as_ref().map(|m| now.saturating_sub(m.start_tick))
    }
    /// World-space hurtboxes — every body part placed by this entity's position + facing.
    pub fn hurtboxes(&self) -> Vec<Box3> {
        self.body.parts.iter().map(|(_, b)| place(*b, self.pos, self.facing)).collect()
    }
}
