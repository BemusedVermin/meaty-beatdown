//! The Ruleset — where "universal" numbers live (spec §2.4, C-AUTH). Cross-cutting curves
//! can't live on any one move, but they may not live in engine code either: this object is
//! loaded with the fight — swappable, versioned, auditable. The engine holds ZERO.

use crate::core::fx::Fx;
use serde::{Deserialize, Serialize};

/// Counter-hit default when a hit authors no `ch_reaction` (spec §5.6).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChDefault {
    /// Damage multiplier on counter-hit.
    pub damage_mult: Fx,
    /// Extra hitstun ticks added to the hit's reaction.
    pub stun_bonus: u32,
}

/// Phase 1 subset (duel core). The decay schedules, extender latches, and the gravity
/// floor (governors 1–3, 7) join in Phase 2.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ruleset {
    pub ch_default: ChDefault,
    /// The anti-turtle terminus: a long, fully punishable stun (spec §5.3). ⚠️ tuning.
    pub guard_break_stun: u32,
    /// Both actors' recovery after a throw tech (spec §5.4). ⚠️ tuning.
    pub throw_tech_recovery: u32,
    /// Separation applied between both actors on a throw tech.
    pub throw_tech_push: Fx,
    /// While holding guard you re-decide at this interval, so turtling never stalls the
    /// turn flow (spec §5.3). ⚠️ tuning.
    pub block_reevaluate_every: u32,
}
