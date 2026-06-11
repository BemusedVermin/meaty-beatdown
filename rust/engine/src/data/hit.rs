//! What a hit IS and what it DOES (spec §2.2 `HitEvent`, §6.1 `Reaction`).

use crate::core::fx::Fx;
use serde::{Deserialize, Serialize};

/// What a clean hit does to its victim — an authored value, exhaustively matched by the
/// engine (spec §6.1). Phase 1 subset: Launch/Crumple/Screw/Bound (the juggle grammar)
/// join in Phase 2 with the governors that bound them.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Reaction {
    /// Standard reel for `ticks` (combo decay applies from Phase 2).
    Hitstun { ticks: u32 },
    /// On the ground for `down_ticks`, then rise. `hard` distinguishes techable (soft)
    /// knockdowns — the wake-up option menu joins in Phase 2; Phase 1 auto-rises.
    Knockdown { hard: bool, down_ticks: u32 },
    /// Pure separation along the attacker's lane.
    Push { dist: Fx },
}

/// One contact within a move's active window (spec §2.2). Multi-hit moves author several.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HitEvent {
    /// Tick offset within the active window (0 = the first active tick).
    pub at: u32,
    pub damage: u32,
    /// Drains the blocker's Guard meter on block (spec §5.3); HP is never chipped.
    pub chip_guard: u32,
    /// Blockstun inflicted on block. Frame advantage is DERIVED, never stored (I-1):
    /// `on_block = blockstun - attacker_remaining`.
    pub blockstun: u32,
    /// Pushback separation applied to the blocker on block.
    pub block_push: Fx,
    /// What a clean hit does.
    pub reaction: Reaction,
    /// Counter-hit override (the CH-launcher idiom, spec §5.6); falls back to the
    /// Ruleset's CH default when absent.
    pub ch_reaction: Option<Reaction>,
}
