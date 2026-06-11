//! The Observation API — the ONLY window into a live fight (spec §7.1, charter C-FOG).
//!
//! Everything anyone learns flows through `CombatSim::observe(side)`, consumed
//! identically by the player UI and by every AI agent. If a fact is not in an
//! Observation, neither the player nor the AI can act on it — the fog is enforced by
//! architecture, not discipline. Enemy INTENT is hidden (committed choices never appear
//! here); resolved FACTS are public, permanent, and exact.
//!
//! Knowledge tiers (spec §7.3) gate enrichment of cue views; the reveals at T2/T3
//! (candidate sets, exact tick readouts, throw break keys) are deliberate, authored
//! features of mastery — knowledge sharpens the read, it never removes it.

use crate::core::fx::{Fx, FxVec2};
use crate::core::ids::{EntityId, SideId};
use crate::core::tick::Tick;
use crate::data::movedef::CueClass;
use crate::data::{ArenaDef, KnowledgeTier, MoveId, Ruleset, ThrowBreakKey};
use crate::trace::TraceEvent;
use serde::{Deserialize, Serialize};

use super::entity::{ActorState, Entity, MovePhase, Stance};

/// The coarse state class (spec §7.1): you can SEE that someone is locked, just not
/// what they're locked into.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StateClass {
    /// Actionable (free, or holding a non-guard stance).
    Free,
    /// Committed to something — see the cue.
    Committed,
    /// Reeling: hitstun, crumple, juggled, splatted, guard-broken.
    Reeling,
    /// Visibly guarding (holding a guard, or stuck in blockstun behind it).
    Blocking,
    Down,
    Grabbed,
    Ko,
}

/// The phase tag of an in-flight cue (spec §7.1): coarse by design — exact remaining
/// ticks are NOT shown until knowledge supplies them.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CuePhase {
    WindUp,
    Swinging,
    Recovering,
}

/// T3's exact readout (spec §7.3): the mastered matchup reads the animation
/// frame-perfectly.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExactReadout {
    pub elapsed: u32,
    pub remaining: u32,
}

/// What an enemy's in-flight move looks like through the fog.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CueView {
    pub cue: CueClass,
    pub phase: CuePhase,
    /// T2+: every enemy move you have STUDIED that shares this cue — the candidate set.
    /// Sharpened, not solved: two entries is still a guess.
    pub candidates: Vec<MoveId>,
    /// T3 only (the in-flight move itself is mastered): the exact tick readout.
    pub exact: Option<ExactReadout>,
    /// T3 only, grabs only: the break key shows on the cue — studied opponents get
    /// their throws broken (spec §5.4).
    pub break_key: Option<ThrowBreakKey>,
}

/// One enemy actor through the fog (spec §7.1).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnemyView {
    pub id: EntityId,
    pub side: SideId,
    // ── physical state: visible body language ──
    pub pos: FxVec2,
    pub facing: FxVec2,
    /// Who they're squared up against is visible.
    pub target: EntityId,
    pub stance: Stance,
    pub height_off: Fx,
    // ── the coarse state class ──
    pub state_class: StateClass,
    /// The cue of any in-flight move (None while not committed).
    pub cue: Option<CueView>,
    // ── meters, per visibility flags (default: HP only) ──
    pub hp: Option<u32>,
    pub guard: Option<u32>,
    pub breath: Option<u32>,
    pub ap: Option<u32>,
    pub focus: Option<u32>,
}

/// Everything a side may know at a decision point (spec §7.1). Own side: full
/// information (you are one mind commanding it). The public event log carries only
/// RESOLVED facts plus your own side's commitments — enemy `Committed` events are
/// exactly the intent the fog hides.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Observation {
    pub t: Tick,
    pub side: SideId,
    /// Full truth about your own actors, movelists included.
    pub allies: Vec<Entity>,
    pub enemies: Vec<EnemyView>,
    /// Resolved facts (never fogged) + own-side commits. Permanent and exact.
    pub events: Vec<TraceEvent>,
    /// The fight's public physics (content is not a secret; intent is).
    pub arena: ArenaDef,
    pub ruleset: Ruleset,
}

impl Observation {
    #[must_use]
    pub fn ally(&self, id: EntityId) -> Option<&Entity> {
        self.allies.iter().find(|e| e.id == id)
    }

    #[must_use]
    pub fn enemy(&self, id: EntityId) -> Option<&EnemyView> {
        self.enemies.iter().find(|e| e.id == id)
    }
}

/// Is this trace event public to `side` (spec §7.1)? Resolved facts always; commitments
/// only for one's own actors — enemy intent never.
#[must_use]
pub fn event_public_for(
    event: &TraceEvent,
    side: SideId,
    side_of: impl Fn(EntityId) -> Option<SideId>,
) -> bool {
    match event {
        TraceEvent::Committed { actor, .. } => side_of(*actor) == Some(side),
        TraceEvent::SimStarted { .. }
        | TraceEvent::Contact { .. }
        | TraceEvent::ThrowResolved { .. }
        | TraceEvent::GuardBroken { .. }
        | TraceEvent::WallSplat { .. }
        | TraceEvent::Landed { .. }
        | TraceEvent::ComboEnded { .. }
        | TraceEvent::Ko { .. }
        | TraceEvent::Revived { .. }
        | TraceEvent::SimEnded { .. } => true,
    }
}

/// Project one entity into an enemy's fogged view. `tier_of` supplies the observer's
/// knowledge; the in-flight move's identity is used ONLY behind its own knowledge gate
/// (the deliberate T3 reveals).
#[must_use]
pub fn project_enemy(e: &Entity, t: Tick, tier_of: impl Fn(MoveId) -> KnowledgeTier) -> EnemyView {
    let state_class = match e.state {
        ActorState::Free => StateClass::Free,
        ActorState::HoldingStance => {
            if e.held.is_some_and(|s| s.guard.is_some()) {
                StateClass::Blocking
            } else {
                StateClass::Free
            }
        }
        ActorState::Acting => StateClass::Committed,
        ActorState::Hitstun { .. }
        | ActorState::Crumple { .. }
        | ActorState::Airborne { .. }
        | ActorState::WallSplat { .. }
        | ActorState::GuardBroken { .. } => StateClass::Reeling,
        ActorState::Blockstun { .. } => StateClass::Blocking,
        ActorState::Grabbed { .. } => StateClass::Grabbed,
        ActorState::Down { .. } => StateClass::Down,
        ActorState::Ko => StateClass::Ko,
    };

    let cue = e.current_move().map(|mv| {
        let elapsed = e.move_elapsed(t).expect("acting has elapsed");
        let phase = match MovePhase::at(mv.timing, elapsed) {
            MovePhase::Startup => CuePhase::WindUp,
            MovePhase::Active => CuePhase::Swinging,
            MovePhase::Recovery | MovePhase::Done => CuePhase::Recovering,
        };
        // T2+: the candidate set — every STUDIED move of theirs sharing this cue.
        let candidates: Vec<MoveId> = e
            .moves
            .iter()
            .filter(|m| m.cue == mv.cue && tier_of(m.id) >= KnowledgeTier::Studied)
            .map(|m| m.id)
            .collect();
        // T3: the mastered in-flight move reads exactly.
        let mastered = tier_of(mv.id) == KnowledgeTier::Mastered;
        let exact = mastered.then(|| ExactReadout {
            elapsed,
            remaining: mv.timing.total().saturating_sub(elapsed),
        });
        let break_key = if mastered { mv.break_key } else { None };
        CueView {
            cue: mv.cue,
            phase,
            candidates,
            exact,
            break_key,
        }
    });

    let vis = e.defense.visibility;
    EnemyView {
        id: e.id,
        side: e.side,
        pos: e.pos,
        facing: e.facing,
        target: e.target,
        stance: e.stance,
        height_off: e.height_off,
        state_class,
        cue,
        hp: vis.hp.then_some(e.hp),
        guard: vis.guard.then_some(e.guard),
        breath: vis.breath.then_some(e.breath),
        ap: vis.ap.then_some(e.ap),
        focus: vis.focus.then_some(e.focus),
    }
}
