//! The Move — orthogonal axes (spec §2.2): each axis answers exactly one question, any
//! combination is expressible. v1's conflated `MoveLevel` (and its THROW duplication bug)
//! is design history; v2 decomposes.

use super::hit::HitEvent;
use super::ids::{FormId, MoveId};
use crate::core::fx::Fx;
use serde::{Deserialize, Serialize};

/// What kind of thing this move fundamentally is. (PROJECTILE joins in Phase 5 — the
/// schema deliberately cannot express what the engine cannot yet honor.)
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MoveCategory {
    Strike,
    Throw,
    Motion,
    Stance,
    Utility,
}

/// Strike height — the Tekken triangle (spec §5.2). HIGH whiffs entirely over a crouching
/// victim; MID is blocked only standing; LOW is blocked only crouching. `None` for
/// motion/stance/utility moves.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Height {
    High,
    Mid,
    Low,
    None,
}

/// Behavior vs lateral movement (spec §3.5). Lateral sign convention: positive = the
/// attacker's left (`facing.perp()`, counter-clockwise).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Tracking {
    /// Narrow band; a sidestep either way evades. Cheap on the budget.
    Linear,
    /// Realigns against steps to the attacker's left only.
    TrackL,
    /// Realigns against steps to the attacker's right only.
    TrackR,
    /// Realigns both ways; beats stepping outright, pays for it on the budget.
    Homing,
}

/// Frame data: the move's life in ticks (spec §2.2). `on_whiff = 0` is structural — you
/// always eat the full recovery.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Timing {
    pub startup: u32,
    pub active: u32,
    pub recovery: u32,
}

impl Timing {
    #[must_use]
    pub const fn total(self) -> u32 {
        self.startup + self.active + self.recovery
    }
}

/// Where the move can touch (spec §3.4). Ranges run along the attacker's facing axis;
/// the arc is a lateral band about it. Tracking widens coverage to `track_halfwidth` on
/// the tracked side(s).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReachEnvelope {
    /// `> 0` means the move whiffs point-blank.
    pub min_range: Fx,
    pub max_range: Fx,
    /// Lateral half-width of the base band (narrow = LINEAR-feeling).
    pub arc_halfwidth: Fx,
    /// Lateral coverage on a tracked side (TRACK_L/TRACK_R/HOMING), per §3.5.
    pub track_halfwidth: Fx,
}

/// Authored self-displacement per phase (spec §3.6): `forward` runs along facing
/// (negative = backward), `lateral` along `facing.perp()` (positive = the actor's left).
/// Each value is the total displacement spread evenly across that phase's ticks.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PhaseMotion {
    pub forward: Fx,
    pub lateral: Fx,
}

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelfMotion {
    pub startup: PhaseMotion,
    pub active: PhaseMotion,
    pub recovery: PhaseMotion,
}

/// Height coverage mask (guard coverage, armor coverage).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeightMask {
    pub high: bool,
    pub mid: bool,
    pub low: bool,
}

impl HeightMask {
    pub const STANDING_GUARD: Self = Self {
        high: true,
        mid: true,
        low: false,
    };
    pub const CROUCHING_GUARD: Self = Self {
        high: false,
        mid: false,
        low: true,
    };

    #[must_use]
    pub const fn covers(self, height: Height) -> bool {
        match height {
            Height::High => self.high,
            Height::Mid => self.mid,
            Height::Low => self.low,
            Height::None => false,
        }
    }
}

/// What an INVULN window lets pass through (spec §2.5).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum InvulnCover {
    All,
    Strike,
    Throw,
}

/// A frame flag live during an inclusive tick window relative to the move's start
/// (spec §2.5). Phase 1 subset; CANCELABLE / HEAT_ENGAGER / PROJECTILE_SPAWN join in
/// their phases.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PropertyKind {
    /// Matching attacks pass through (reversals, backdash i-frames).
    Invuln { covers: InvulnCover },
    /// Absorb `hits` covered strikes without stun; still take `dmg_mult`-scaled damage.
    /// Throws and (by default) LOWs go through. 🔬 Tekken Power Crush.
    Armor {
        hits: u32,
        dmg_mult: Fx,
        covers: HeightMask,
    },
    /// Auto-deflect one covered strike -> parry outcome (spec §5.5). 🔬 Tekken sabaki.
    GuardPoint {
        covers: HeightMask,
        freeze_attacker: u32,
        parry_recovery: u32,
    },
    /// Being struck here is a counter-hit (extends the startup/recovery default).
    ChState,
}

/// A property window: `kind` is live during move ticks `[from, to]` (inclusive, relative
/// to the move's first startup tick).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PropertyWindow {
    pub from: u32,
    pub to: u32,
    pub kind: PropertyKind,
}

/// Throw break key (spec §5.4): the defender's 2-way directional read. A THROW with
/// `break_key: None` is an unbreakable command grab (pays heavily on the budget).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThrowBreakKey {
    L,
    R,
}

/// Body stance while a STANCE move is held (spec §5.2–5.3).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum StanceKind {
    Standing,
    Crouching,
}

/// What holding a STANCE move does: body position, plus an optional guard (with authored
/// height coverage — C-AUTH: standing/crouching guard coverage is data, not engine rule).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StanceSpec {
    pub stance: StanceKind,
    pub guard: Option<HeightMask>,
}

/// Stance requirement for committing a move (spec §2.2 `reqs`). A move with
/// `StanceReq::Crouching` may be committed directly from a held crouching stance (the
/// while-rising idiom) without paying the stance's release recovery.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum StanceReq {
    Standing,
    Crouching,
}

/// The Move (spec §2.2). Phase 1 subset of the full schema: costs/gains (P2 meters),
/// cancels (P2), cue (P3), and tags (P2 economy) join in their phases.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Move {
    pub id: MoveId,
    pub name: String,
    /// Every move belongs to a Form (C-AUTH: no generic moves).
    pub form: FormId,

    // ── the orthogonal type axes ─────────────────────────────────────────
    pub category: MoveCategory,
    pub height: Height,
    /// Orthogonal to height: unblockables exist at any height.
    pub blockable: bool,
    pub tracking: Tracking,

    // ── timing & space ───────────────────────────────────────────────────
    pub timing: Timing,
    /// Multi-hit moves are first-class; empty for motion/stance moves.
    pub hits: Vec<HitEvent>,
    pub region: ReachEnvelope,
    pub motion: SelfMotion,

    // ── windows ──────────────────────────────────────────────────────────
    pub properties: Vec<PropertyWindow>,

    // ── interaction ──────────────────────────────────────────────────────
    /// Required stance to commit (None = any grounded stance).
    pub req_stance: Option<StanceReq>,
    /// THROW only: the directional break read. None on a throw = unbreakable.
    pub break_key: Option<ThrowBreakKey>,
    /// STANCE only: what holding it does.
    pub stance_spec: Option<StanceSpec>,
}
