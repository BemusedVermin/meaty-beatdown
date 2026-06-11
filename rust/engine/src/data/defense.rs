//! The compiled defensive profile (spec §2.3). Authored/compiled data — a defender's
//! susceptibility lives here (rule of placement, spec §2.4). block_arc joins in Phase 4,
//! visibility flags in Phase 3.

use crate::core::fx::Fx;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DefenseProfile {
    pub hp_max: u32,
    /// Guard meter: drained by blocked chip; zero means GUARDBROKEN (spec §5.3).
    pub guard_max: u32,
    /// Guard regenerates 1 point every this many ticks while not blocking. ⚠️ tuning.
    pub guard_regen_interval: u32,
    /// Juggle gravity/decay modifier: heavier bodies decay juggles faster (governor 2).
    /// 🔬 Tekken body weight. 1 = baseline.
    pub weight: Fx,
    /// Breath (exertion): the anti-mash pacing floor, deliberately light (spec §9).
    pub breath_max: u32,
    /// Breath regenerates 1 point every this many ticks while not executing a move.
    pub breath_regen_interval: u32,
    /// AP: the tempo budget — how long your turn runs (spec §9.4).
    pub ap_max: u32,
    /// Focus: the earned super gauge (spec §9).
    pub focus_max: u32,
}
