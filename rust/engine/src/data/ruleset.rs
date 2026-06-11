//! The Ruleset — where "universal" numbers live (spec §2.4, C-AUTH). Cross-cutting curves
//! can't live on any one move, but they may not live in engine code either: this object is
//! loaded with the fight — swappable, versioned, auditable. The engine holds ZERO.
//!
//! The anti-infinite charter's curves (governors 1–3 and 7, spec §6.5) live here.

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

/// Per-combo extender allowances (governor 3): each is usable this many times per combo
/// (canonically 1) and degrades to a plain hit beyond that.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtenderLatches {
    pub screw: u32,
    pub bound: u32,
    pub wall_splat: u32,
}

/// The Focus gain table (spec §9): offense and skillful defense build the super gauge.
/// ⚠️ all tuning values.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FocusGains {
    /// Landing a clean hit.
    pub land_hit: u32,
    /// Having your hit blocked (pressure pays a little).
    pub hit_blocked: u32,
    /// Taking damage, per 100 HP lost (the small comeback factor 🔬).
    pub take_damage_per_100: u32,
    /// Executing a parry (skill pays — large).
    pub parry: u32,
    /// Landing a counter-hit on startup (the frame trap).
    pub counter_hit: u32,
    /// Landing a counter-hit on recovery (the whiff punish — the largest).
    pub whiff_punish: u32,
}

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

    // ── the combo governors (spec §6.5) ─────────────────────────────────────
    /// Governor 1 — hitstun decay: each consecutive combo hit loses `hit_index *
    /// hitstun_decay_step` ticks of stun; advantage trends negative, every chain drops.
    pub hitstun_decay_step: u32,
    /// Governor 2 — juggle damage decay: juggle hit `n` deals
    /// `damage * max(1 - n * juggle_decay_step * defender_weight, 0)`.
    pub juggle_decay_step: Fx,
    /// Governor 3 — per-combo extender allowances.
    pub extender_latches: ExtenderLatches,
    /// Governor 7 — the gravity floor: a juggle hit whose decayed stun falls below the
    /// minimum startup among the attacker's currently affordable follow-ups drops the
    /// victim instead of sustaining the juggle (spec §6.5.7).
    pub forced_landing: bool,

    // ── walls & okizeme ─────────────────────────────────────────────────────
    /// How long a WALL_SPLAT holds the victim stuck and juggleable (spec §3.7). ⚠️
    pub splat_duration: u32,
    /// Down-time after landing from a juggle, a splat fall, or a crumple collapse. ⚠️
    pub landing_down_ticks: u32,
    /// Ticks from a wake-up "rise" choice to actionable (meaty timing exists). ⚠️
    pub wake_rise_ticks: u32,
    /// Backward displacement of a back-rise (and its slightly longer rise). ⚠️
    pub wake_back_rise_push: Fx,
    pub wake_back_rise_ticks: u32,
    /// Maximum extra down-time a delayed rise may choose. ⚠️
    pub wake_delay_max: u32,

    // ── meters (spec §9) ────────────────────────────────────────────────────
    pub focus_gains: FocusGains,
}
