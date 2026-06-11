//! The compiled defensive profile (spec §2.3). Authored/compiled data — a defender's
//! susceptibility lives here (rule of placement, spec §2.4). block_arc joins in Phase 4,
//! visibility flags in Phase 3.

use crate::core::fx::Fx;
use serde::{Deserialize, Serialize};

/// What enemies may observe of this actor's meters (spec §7.1). Locked decision: only
/// HP at first; every flag tunable per compiled fighter. ✅
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MeterVisibility {
    pub hp: bool,
    pub guard: bool,
    pub breath: bool,
    pub ap: bool,
    pub focus: bool,
}

impl Default for MeterVisibility {
    fn default() -> Self {
        Self {
            hp: true,
            guard: false,
            breath: false,
            ap: false,
            focus: false,
        }
    }
}

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
    /// Heat duration once latched by Heat Burst or a Heat Engager hit.
    pub heat_duration: u32,
    /// HP threshold at or below which Rage latches. Zero disables Rage.
    pub rage_threshold_hp: u32,
    /// Passive damage scalar while Rage is latched.
    pub rage_damage_mult: Fx,
    /// Per-meter visibility to enemies (spec §7.1).
    pub visibility: MeterVisibility,
}
