//! The arena (spec §3.1): a bounded region of the 2D ground plane. Phase 1: a rectangular
//! floor boundary (positions clamp to it). Wall segments with per-segment properties
//! (splat-able / breakable) and hazard volumes join in Phase 2 — hazards are arena DATA,
//! never engine rules.

use crate::core::fx::FxVec2;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArenaDef {
    /// Rectangle centered on the origin: positions clamp to `[-half_extents, +half_extents]`
    /// on each axis.
    pub half_extents: FxVec2,
}
