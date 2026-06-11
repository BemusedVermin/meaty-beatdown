//! Decision scheduling (spec §4.1–4.2): the kinds of decision points, the choices, and
//! the side-blind same-tick commit batch.
//!
//! All decisions pending at tick `T` are collected and grouped by side. Each side commits
//! all of its actors' choices WITHOUT seeing the other side's same-tick commitments; the
//! tick then executes everything at once. The blindness is structural: nothing about a
//! committed-but-unexecuted batch is exposed by any public API.

use crate::core::ids::{EntityId, SideId};
use crate::data::MoveId;
use crate::data::movedef::ThrowBreakKey;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Why an actor must decide (spec §4.1 — Burst joins in Phase 4).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DecisionKind {
    /// A free actor at its `ready_tick`: any requirement-met move, WAIT, or stance.
    Ready,
    /// Holding a stance at the reevaluate interval (or after an event): keep holding or
    /// release (spec §5.3).
    StanceReevaluate,
    /// A grab connected (spec §5.4): guess the break key or decline. The key itself is
    /// NOT in the prompt — that read is the game (knowledge reveals it at T3, Phase 3).
    ThrowBreak { attacker: EntityId },
    /// One or more cancel windows are open with gates satisfied (spec §11): take one
    /// listed, affordable cancel (pay) or decline them all. Branch points are several
    /// windows open at once; the choice names the target.
    Cancel,
    /// The wake-up decision (spec §6.3): rise / back-rise / delayed rise / an authored
    /// `req_down` move (reversals).
    WakeUp,
    /// Phase 4 solo Burst window from combo-victim states.
    Burst,
}

/// One actor's pending decision.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingDecision {
    pub actor: EntityId,
    pub side: SideId,
    pub kind: DecisionKind,
}

/// What an owner commits for one actor.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Choice {
    /// Stay free and re-decide after `ticks` (the Phase 0 primitive, kept: spec §4.1
    /// lists WAIT among the Ready choices).
    Wait { ticks: u32 },
    /// Commit an authored move at the actor's current target.
    Move { id: MoveId },
    /// Retarget, then commit an authored move.
    MoveAt { id: MoveId, target: EntityId },
    /// Quick defensive re-facing without an authored attack.
    SwitchFocus { target: EntityId },
    /// Keep holding the current stance (StanceReevaluate only).
    HoldStance,
    /// Release the held stance, paying its authored release recovery (spec §5.3).
    Release,
    /// The throw-break read (ThrowBreak only): `None` declines.
    ThrowBreak { guess: Option<ThrowBreakKey> },
    /// The cancel decision (Cancel only): `Some(id)` pays and chains into a satisfied
    /// window's target; `None` declines every window open this tick (final for those
    /// windows; later windows prompt fresh).
    Cancel { into: Option<MoveId> },
    /// Wake-up: rise in place (WakeUp only).
    Rise,
    /// Wake-up: rise with a backward displacement (slower; creates space).
    BackRise,
    /// Wake-up: stay down for `ticks` more (≤ the Ruleset's wake_delay_max), then a
    /// fresh wake-up decision — the oki timing mixup.
    DelayRise { ticks: u32 },
}

/// A same-tick commit batch mid-collection. Sides commit independently; the sim executes
/// once every side with pending items has committed.
#[derive(Clone, Debug, Default)]
pub struct CommitBatch {
    pub pending: Vec<PendingDecision>,
    /// Committed choices per actor, filled side by side. BTreeMap: deterministic order.
    pub committed: BTreeMap<EntityId, Choice>,
}

impl CommitBatch {
    #[must_use]
    pub fn sides_outstanding(&self) -> Vec<SideId> {
        let mut sides: Vec<SideId> = self
            .pending
            .iter()
            .filter(|p| !self.committed.contains_key(&p.actor))
            .map(|p| p.side)
            .collect();
        sides.sort_unstable();
        sides.dedup();
        sides
    }

    #[must_use]
    pub fn complete(&self) -> bool {
        self.pending
            .iter()
            .all(|p| self.committed.contains_key(&p.actor))
    }
}

/// A rejected commit (driver bug or illegal script — tests treat these as failures).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CommitError {
    /// The actor has no pending decision this tick (or is not on the committing side).
    NotPending { actor: EntityId },
    /// The choice is not legal for the pending decision kind or actor state.
    IllegalChoice { actor: EntityId, why: &'static str },
    /// The side has already committed this actor.
    AlreadyCommitted { actor: EntityId },
    /// Unknown move id, or the move's requirements are not met.
    UnknownOrUnmetMove { actor: EntityId },
    /// DENIED (spec §9): the actor cannot afford this choice's costs. WAIT is always
    /// affordable, so a denial never deadlocks the fight.
    Denied { actor: EntityId },
}
