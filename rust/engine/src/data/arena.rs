//! The arena (spec §3.1): a bounded region of the 2D ground plane with wall segments.
//! Phase 2: the rectangular boundary's four walls carry per-segment properties
//! (splat-able). Breakable segments and hazard volumes join later — hazards are arena
//! DATA, never engine rules.

use crate::core::fx::FxVec2;
use serde::{Deserialize, Serialize};

/// Per-segment wall properties (spec §3.7).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WallSpec {
    /// A knocked-back or juggled victim carried into this wall WALL_SPLATs (once per
    /// combo) instead of clamping.
    pub splattable: bool,
}

/// The four boundary walls of the rectangular arena (±x = east/west, ±y = north/south).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Walls {
    pub east: WallSpec,
    pub west: WallSpec,
    pub north: WallSpec,
    pub south: WallSpec,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArenaDef {
    /// Rectangle centered on the origin: positions clamp to `[-half_extents, +half_extents]`
    /// on each axis.
    pub half_extents: FxVec2,
    pub walls: Walls,
}
