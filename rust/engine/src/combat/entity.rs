//! The runtime actor (spec §2.3). There is no separate Fighter type: one Entity carries
//! the compiled offensive kit (`moves`), the compiled defensive profile, and all runtime
//! state. Phase 1 subset: meters beyond HP/Guard, combo tracking, and latches join in
//! Phase 2; height_off (airborne) with the juggle grammar.

use crate::core::fx::{Fx, FxVec2};
use crate::core::ids::{EntityId, SideId};
use crate::core::tick::Tick;
use crate::data::movedef::{Move, StanceSpec, Timing};
use crate::data::{DefenseProfile, MoveId};
use serde::{Deserialize, Serialize};

/// Body position (spec §2.3 `stance`).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Stance {
    Standing,
    Crouching,
    Airborne,
    Down,
}

/// What the actor is locked into right now (spec §2.3 `state`).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActorState {
    /// Fully actionable; decides at `ready_tick`.
    Free,
    /// Committed to a move; the phase derives from `MoveInstance` + elapsed ticks.
    Acting,
    /// Holding a STANCE move (guard / crouch) past its startup; re-decides at the
    /// Ruleset's reevaluate interval and whenever an event touches it (spec §5.3).
    HoldingStance,
    Hitstun {
        until: Tick,
    },
    /// Staggering toward collapse (spec §6.1): standing, juggleable, collapses to Down
    /// at `until` if not picked up.
    Crumple {
        until: Tick,
    },
    /// Launched into a JUGGLE (spec §6.2): re-hittable until the air stun expires, then
    /// lands into a knockdown.
    Airborne {
        stun_until: Tick,
    },
    /// Stuck on a splat-able wall, juggleable, gravity-suspended for an authored window
    /// (spec §3.7) — once per combo.
    WallSplat {
        until: Tick,
    },
    /// Reeling from a blocked hit; guard is still considered held.
    Blockstun {
        until: Tick,
    },
    /// Guard meter hit zero: long, fully punishable (spec §5.3).
    GuardBroken {
        until: Tick,
    },
    /// A grab connected; awaiting / resolving the break read (spec §5.4).
    Grabbed {
        by: EntityId,
    },
    /// On the ground; the wake-up decision opens at `until` (spec §6.3).
    Down {
        until: Tick,
    },
    Ko,
}

/// An in-flight move (spec §2.3 `current`).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MoveInstance {
    pub move_id: MoveId,
    /// Index into the owner's `moves` (stable; the movelist never changes mid-fight).
    pub move_index: usize,
    pub started_at: Tick,
    /// Armor uses left in the current ARMOR window, if any.
    pub armor_hits_left: u32,
    /// THROW: the victim held since the grab connected.
    pub grabbed_victim: Option<EntityId>,
    /// THROW: when the grab connected. The throw's hit `at` offsets are measured from
    /// this tick (the slam sequence starts when the hands touch).
    pub connected_at: Option<Tick>,
    /// Contact bookkeeping for cancel gates (spec §11, lock-then-confirm): the gates
    /// react to FACTS, never to the opponent's hidden input.
    pub hit_landed: bool,
    pub blocked: bool,
    /// Bitmask of cancel-window indices already prompted (a decline is final for that
    /// window; delay windows are a flagged refinement ⚠️).
    pub cancels_prompted: u32,
}

/// Per-victim combo bookkeeping (spec §2.3): decay indices and the extender latches
/// (governor 3). Reset when the victim regains freedom or hits the ground.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComboTracker {
    /// Hits taken this combo (indexes the decay schedules — governors 1 & 2).
    pub hits: u32,
    pub screw_used: u32,
    pub bound_used: u32,
    pub splat_used: u32,
}

/// Which timing phase a move is in at a given elapsed tick.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum MovePhase {
    Startup,
    Active,
    Recovery,
    Done,
}

impl MovePhase {
    /// Phase at `elapsed` ticks since the move's first tick.
    #[must_use]
    pub fn at(timing: Timing, elapsed: u32) -> Self {
        if elapsed < timing.startup {
            Self::Startup
        } else if elapsed < timing.startup + timing.active {
            Self::Active
        } else if elapsed < timing.total() {
            Self::Recovery
        } else {
            Self::Done
        }
    }
}

/// The runtime actor.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entity {
    pub id: EntityId,
    pub side: SideId,
    pub pos: FxVec2,
    /// Unit vector; auto-faces the target while actionable, frozen while committed or
    /// reeling (spec §3.2 — what makes flanking possible from Phase 4).
    pub facing: FxVec2,
    /// Every actor targets exactly one other actor; the target creates the lane.
    pub target: EntityId,
    pub stance: Stance,
    pub state: ActorState,
    /// When this actor next gets a free decision (meaningful while `Free`).
    pub ready_tick: Tick,
    pub current: Option<MoveInstance>,
    /// The stance spec being held while `HoldingStance` / `Blockstun`.
    pub held: Option<StanceSpec>,
    /// Next scheduled re-decision while holding a stance.
    pub reevaluate_at: Tick,
    /// Vertical offset while airborne (juggle arcs). Zero on the ground.
    pub height_off: Fx,
    pub hp: u32,
    pub guard: u32,
    /// Ticks accumulated toward the next guard regen point.
    pub guard_regen_acc: u32,
    /// Breath (exertion): regenerates while not executing (spec §9).
    pub breath: u32,
    pub breath_regen_acc: u32,
    /// AP: the tempo budget; refills to max on regaining freedom (spec §9.4).
    pub ap: u32,
    /// Focus: the earned super gauge (spec §9).
    pub focus: u32,
    /// Combo bookkeeping while this actor is the VICTIM (governors 1–3).
    pub combo: ComboTracker,
    /// The compiled movelist (opaque data to the engine — emitted by L4 from Phase 6).
    pub moves: Vec<Move>,
    pub defense: DefenseProfile,
}

impl Entity {
    /// The in-flight move's definition, if any.
    #[must_use]
    pub fn current_move(&self) -> Option<&Move> {
        self.current.map(|inst| &self.moves[inst.move_index])
    }

    /// Elapsed ticks of the in-flight move at `t` (0 on its first tick).
    #[must_use]
    pub fn move_elapsed(&self, t: Tick) -> Option<u32> {
        self.current
            .map(|inst| u32::try_from(t.0 - inst.started_at.0).expect("move ticks fit u32"))
    }

    /// Phase of the in-flight move at `t`.
    #[must_use]
    pub fn move_phase(&self, t: Tick) -> Option<MovePhase> {
        let timing = self.current_move()?.timing;
        self.move_elapsed(t)
            .map(|elapsed| MovePhase::at(timing, elapsed))
    }

    /// Is the actor holding an active guard (HoldingStance with a guard mask, or stuck in
    /// blockstun behind it)?
    #[must_use]
    pub fn guarding(&self) -> bool {
        matches!(
            self.state,
            ActorState::HoldingStance | ActorState::Blockstun { .. }
        ) && self.held.is_some_and(|s| s.guard.is_some())
    }

    /// Can this actor be grabbed (spec §5.4)? Phase 1: standing throws only — crouching
    /// and downed victims, reeling victims, and mid-grab victims are not grabbable.
    #[must_use]
    pub fn grabbable(&self) -> bool {
        self.stance == Stance::Standing
            && matches!(
                self.state,
                ActorState::Free
                    | ActorState::Acting
                    | ActorState::HoldingStance
                    | ActorState::GuardBroken { .. }
            )
    }

    /// Is the actor currently a combo victim (the states a combo rides on)? A hit on an
    /// actor in one of these continues the combo; a hit on anyone else starts one.
    #[must_use]
    pub fn in_combo_state(&self) -> bool {
        matches!(
            self.state,
            ActorState::Hitstun { .. }
                | ActorState::Crumple { .. }
                | ActorState::Airborne { .. }
                | ActorState::WallSplat { .. }
        )
    }
}
