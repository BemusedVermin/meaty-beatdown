//! A move is **authored, composable qualities** — never a generic template. A `FrameProfile` is
//! `timing` + a list of windowed [`Quality`]s; each quality carries *its own* magnitudes. There is
//! **no engine-wide value** for counter-hit, parry, block, knockdown, etc.: a move that parries
//! authors its own freeze/recover, a move that counter-hits authors its own bonus, and one move can
//! hold **several** qualities at once (e.g. a `Block` window *and* a `Hitbox` that strikes on hit).
//!
//! A `Hitbox`'s geometry is **sourced from the body** ([`HitboxSource`]) so it fits the fighter, with
//! a `Custom` box for anything bespoke. The L4 RPG layer (deferred) is the compiler that emits these.

use super::space::{BodyPart, Box3, Vec3};
use std::collections::HashMap;
use super::sim::Tick;

/// Stable identifier for an authored move.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct MoveId(pub u32);

/// The authored move set the engine runs: id → profile. (No move is implicit — all are content.)
pub type MoveTable = HashMap<MoveId, FrameProfile>;

/// startup / active / recovery, in ticks. `total` is derived.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Timing {
    pub startup: Tick,
    pub active: Tick,
    pub recovery: Tick,
}
impl Timing {
    pub fn total(&self) -> Tick {
        self.startup + self.active + self.recovery
    }
}

/// The phase of an in-flight move, derived from elapsed ticks.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Phase {
    Startup,
    Active,
    Recovery,
    Done,
}

pub fn phase_at(elapsed: Tick, t: &Timing) -> Phase {
    if elapsed < t.startup {
        Phase::Startup
    } else if elapsed < t.startup + t.active {
        Phase::Active
    } else if elapsed < t.total() {
        Phase::Recovery
    } else {
        Phase::Done
    }
}

/// Strike vs throw — an orthogonal axis (not a `MoveLevel` variant).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AttackKind {
    Strike,
    Throw,
}
impl AttackKind {
    pub fn as_invuln(self) -> InvulnType {
        match self {
            AttackKind::Strike => InvulnType::Strike,
            AttackKind::Throw => InvulnType::Throw,
        }
    }
}

/// Which guard stance must block the attack — orthogonal to the *spatial* height (the box's Y).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GuardHeight {
    High,
    Mid,
    Low,
    Overhead,
}

/// Category of invincibility a window grants.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum InvulnType {
    All,
    Strike,
    Throw,
    Projectile,
}

/// What a clean hit does. Every magnitude is **authored on the move** — `knockdown` carries its own
/// down-duration, not an engine constant.
#[derive(Clone, Copy, Debug)]
pub struct HitEffect {
    pub damage: Health,
    pub hitstun: Tick,
    pub blockstun: Tick,
    /// Chip — destined for Poise (a deferred resource); tracked, not yet spent.
    pub chip: Health,
    /// Lane (X) pushback applied along the attacker's facing.
    pub knockback: f32,
    pub launches: bool,          // → AIRBORNE (juggle)
    pub knockdown: Option<Tick>, // Some(down_ticks) → DOWN/okizeme for the authored duration
}
use super::entity::Health;

/// The authored counter-hit bonus *of an attack*. A move "is a counter-hit move" by carrying this.
#[derive(Clone, Copy, Debug)]
pub struct CounterBonus {
    pub dmg_num: Health,
    pub dmg_den: Health,
    pub hitstun_bonus: Tick,
}

/// Where an attack's hitbox geometry comes from. A `Part` box is sourced from the fighter's body so
/// it fits the body type; `Custom` is an explicit box (weapons, projectiles, odd shapes).
#[derive(Clone, Copy, Debug)]
pub enum HitboxSource {
    Part(BodyPart),
    Custom(Box3),
}

/// How the hitbox behaves vs a sidestep (the Z axis). LINEAR can be stepped; TRACKING covers one
/// side; HOMING realigns to the defender's Z (mechanics §5.4).
#[derive(Clone, Copy, Debug)]
pub enum Tracking {
    Linear,
    Tracking(i8), // the covered side: -1 | +1
    Homing,
}

/// An offensive quality: a live hitbox. Geometry comes from `source` (placed by `placement`); the
/// strike/throw axis, guard height, blockability, hit effect, and an optional authored counter bonus
/// + throw-tech recover all live here.
#[derive(Clone, Copy, Debug)]
pub struct Attack {
    pub kind: AttackKind,
    pub guard: GuardHeight,
    pub blockable: bool,
    pub source: HitboxSource,
    /// Local offset/extension of the box during the move (e.g. a thrust reaches forward in +X).
    pub placement: Vec3,
    pub tracking: Tracking,
    pub hit: HitEffect,
    pub counter: Option<CounterBonus>,
    /// Throws only: ticks both fighters recover after a mutual-throw clash (authored, not fixed).
    pub tech_recover: Tick,
}

/// The kinds of quality a move can carry. A move composes any number of these over tick windows.
#[derive(Clone, Debug)]
pub enum QualityKind {
    /// A live attack (hitbox).
    Hitbox(Attack),
    /// A held guard stance covering these heights.
    Block { covers: Vec<GuardHeight> },
    /// A parry/sabaki window with its own authored magnitudes.
    Parry { freeze: Tick, recover: Tick },
    /// Hyper-armor: absorb up to `hits` strikes, taking `dmg_num/dmg_den` of the damage, no hitstun.
    Armor { hits: u8, dmg_num: Health, dmg_den: Health },
    /// Typed invincibility.
    Invuln(InvulnType),
    /// Extra counter-hit vulnerability beyond the default startup/recovery.
    CounterVulnerable,
    /// Juggle state marker.
    Airborne,
}

/// A quality live on an inclusive `[from, to]` elapsed-tick window.
#[derive(Clone, Debug)]
pub struct Quality {
    pub from: Tick,
    pub to: Tick,
    pub kind: QualityKind,
}

/// One-shot repositioning applied when a movement move reaches its active phase (spec §1.3).
#[derive(Clone, Copy, Debug, Default)]
pub struct Motion {
    /// Local displacement (X lane mirrored by facing, Y vertical, Z lateral).
    pub delta: Vec3,
}

/// An authored move: shape (`timing`) + the composable qualities that give it behaviour.
#[derive(Clone, Debug)]
pub struct FrameProfile {
    pub timing: Timing,
    pub qualities: Vec<Quality>,
    pub motion: Option<Motion>,
    /// **Morphology gate.** Body parts the entity must have for this move to be usable — a punch
    /// requires a `Fist`, a tail-whip a `Tail`. A fighter whose body lacks them can't use the move.
    /// This is what makes a move character-specific (alongside its body-sourced hitboxes).
    pub requires: Vec<BodyPart>,
}

impl FrameProfile {
    fn at(&self, e: Tick) -> impl Iterator<Item = &QualityKind> {
        self.qualities
            .iter()
            .filter(move |q| e >= q.from && e <= q.to)
            .map(|q| &q.kind)
    }
    /// The live attack (first hitbox quality) at `elapsed`, if any.
    pub fn active_hitbox(&self, e: Tick) -> Option<&Attack> {
        self.at(e).find_map(|k| match k {
            QualityKind::Hitbox(a) => Some(a),
            _ => None,
        })
    }
    pub fn active_block(&self, e: Tick) -> Option<&[GuardHeight]> {
        self.at(e).find_map(|k| match k {
            QualityKind::Block { covers } => Some(covers.as_slice()),
            _ => None,
        })
    }
    /// `(freeze, recover)` of an active parry window.
    pub fn active_parry(&self, e: Tick) -> Option<(Tick, Tick)> {
        self.at(e).find_map(|k| match k {
            QualityKind::Parry { freeze, recover } => Some((*freeze, *recover)),
            _ => None,
        })
    }
    pub fn active_armor(&self, e: Tick) -> Option<(u8, Health, Health)> {
        self.at(e).find_map(|k| match k {
            QualityKind::Armor { hits, dmg_num, dmg_den } => Some((*hits, *dmg_num, *dmg_den)),
            _ => None,
        })
    }
    pub fn active_invuln(&self, e: Tick) -> Option<InvulnType> {
        self.at(e).find_map(|k| match k {
            QualityKind::Invuln(t) => Some(*t),
            _ => None,
        })
    }
    pub fn is_throwing(&self, e: Tick) -> bool {
        self.active_hitbox(e).map(|a| a.kind == AttackKind::Throw).unwrap_or(false)
    }
    /// Counter-vulnerable = in this move's own startup/recovery, or an authored `CounterVulnerable`
    /// window. (A structural rule, not a magnitude.)
    pub fn counter_vulnerable(&self, e: Tick) -> bool {
        matches!(phase_at(e, &self.timing), Phase::Startup | Phase::Recovery)
            || self.at(e).any(|k| matches!(k, QualityKind::CounterVulnerable))
    }
}
