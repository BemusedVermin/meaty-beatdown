//! What a hit IS and what it DOES (spec §2.2 `HitEvent`, §6.1 `Reaction`).

use crate::core::fx::Fx;
use serde::{Deserialize, Serialize};

/// What a clean hit does to its victim — an authored value, exhaustively matched by the
/// engine (spec §6.1). The combo grammar's states are all here; each extender is latched
/// once per combo (governor 3) and degrades to a plain (still decayed) hit when its
/// latch is spent.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Reaction {
    /// Standard reel for `ticks` (hitstun decay applies per combo hit — governor 1).
    Hitstun { ticks: u32 },
    /// Slow stagger for `ticks`, then collapse: a standing, juggleable pickup window —
    /// hit them before they fall (spec §6.1).
    Crumple { ticks: u32 },
    /// Airborne: enters JUGGLE (the combo starter). `rise` sets the arc height,
    /// `carry` the authored launch drift, `stun` the (decaying) air stun.
    Launch { rise: Fx, carry: Fx, stun: u32 },
    /// Juggle extender: flattens the arc and extends carry — ONCE per combo (🔬 T7
    /// tailspin). On the ground or with the latch spent: behaves as Hitstun.
    Screw { carry: Fx, stun: u32 },
    /// Juggle extender: slams to a re-juggleable bounce — ONCE per combo (🔬 T6 bound).
    /// On the ground or with the latch spent: behaves as Hitstun.
    Bound { stun: u32 },
    /// On the ground for `down_ticks`, then the wake-up decision (spec §6.3). `hard`
    /// knockdowns are untechable (full oki); soft ones reach the wake decision at once.
    Knockdown { hard: bool, down_ticks: u32 },
    /// Pure separation along the attacker's lane (also: juggle carry without stun).
    Push { dist: Fx },
}

/// One contact within a move's active window (spec §2.2). Multi-hit moves author several.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HitEvent {
    /// Tick offset within the active window (0 = the first active tick). For THROWs,
    /// offsets are measured from the grab's connect tick.
    pub at: u32,
    pub damage: u32,
    /// Drains the blocker's Guard meter on block (spec §5.3); HP is never chipped.
    pub chip_guard: u32,
    /// Blockstun inflicted on block. Frame advantage is DERIVED, never stored (I-1):
    /// `on_block = blockstun - attacker_remaining`.
    pub blockstun: u32,
    /// Pushback separation applied to the blocker on block.
    pub block_push: Fx,
    /// Displacement applied to an AIRBORNE victim this hit juggles (wall carry — the
    /// reason to take juggles toward walls, spec §3.7).
    pub juggle_carry: Fx,
    /// What a clean hit does.
    pub reaction: Reaction,
    /// Counter-hit override (the CH-launcher idiom, spec §5.6); falls back to the
    /// Ruleset's CH default when absent.
    pub ch_reaction: Option<Reaction>,
}
