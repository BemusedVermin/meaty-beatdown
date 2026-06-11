//! The compiled defensive profile (spec §2.3). Authored/compiled data — a defender's
//! susceptibility lives here (rule of placement, spec §2.4). Phase 1 subset: weight
//! (juggle gravity) joins in Phase 2, block_arc in Phase 4, visibility flags in Phase 3.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DefenseProfile {
    pub hp_max: u32,
    /// Guard meter: drained by blocked chip; zero means GUARDBROKEN (spec §5.3).
    pub guard_max: u32,
    /// Guard regenerates 1 point every this many ticks while not blocking. ⚠️ tuning.
    pub guard_regen_interval: u32,
}
