//! Move templates — **the basic unit of combat**. Stats/skills/weapon compile into these
//! [`FrameProfile`]s (authored composable qualities; the engine runs them verbatim). Natural strikes
//! source their hitbox from a body part; weapon strikes use a `Custom` box sized by the weapon reach.

use super::equipment::{Weapon, WeaponClass};
use crate::fighting::{
    Attack, AttackKind, BodyPart, Box3, FrameProfile, GuardHeight, HitEffect, HitboxSource, Quality,
    QualityKind, Timing, Tracking, Vec3,
};

/// The compiled per-fighter adjustments the templates apply (the wired stat/skill levers).
#[derive(Clone, Copy, Debug)]
pub struct Levers {
    /// Ticks shaved off startup (DEX), already clamped ≥0.
    pub startup_cut: i32,
    /// Ticks shaved off recovery (skill rank), already clamped ≥0.
    pub recovery_cut: i32,
    /// Flat damage bonus (STR), may be negative.
    pub damage_bonus: i32,
}

const MIN_STARTUP: i32 = 2;
const MIN_RECOVERY: i32 = 2;

/// Ticks a stun must end *before* the same move could re-connect — the safety window that turns an
/// otherwise frame-tight loop into an escapable one.
const ESCAPE_MARGIN: u32 = 2;

/// The **no-infinite-combo invariant**, enforced here in *authoring* — the engine deliberately does
/// not police it (a stun is just a number it honours). A hit-/block-stun may never outlast the move
/// that inflicted it: the victim must always reach a decision before the attacker can land the
/// *identical* move again (the next same-move hit is one full `total` later). So we cap every stun a
/// margin below the move's own duration. A single authored move therefore can't lock its target
/// forever — and because the cap reads the *tuned* `total`, it holds however the levers shorten the
/// move. (Cross-move cancel chains are a separate concern — the deferred "combo governors".)
fn escapable(stun: u32, total: u32) -> u32 {
    stun.min(total.saturating_sub(ESCAPE_MARGIN))
}

fn tune(t: Timing, lv: &Levers) -> Timing {
    Timing {
        startup: (t.startup as i32 - lv.startup_cut).max(MIN_STARTUP) as u32,
        active: t.active,
        recovery: (t.recovery as i32 - lv.recovery_cut).max(MIN_RECOVERY) as u32,
    }
}

fn dmg(base: i32, lv: &Levers) -> u32 {
    (base + lv.damage_bonus).max(1) as u32
}

fn hit(damage: u32, hitstun: u32, blockstun: u32) -> HitEffect {
    HitEffect {
        damage,
        hitstun,
        blockstun,
        chip: damage / 4,
        knockback: 0.0,
        launches: false,
        knockdown: None,
    }
}

/// Wrap a single hitbox `attack` (live over the tuned active frames) into a one-move profile.
fn one_hit(timing: Timing, attack: Attack, requires: Vec<BodyPart>) -> FrameProfile {
    let from = timing.startup;
    let to = timing.startup + timing.active - 1;
    FrameProfile {
        timing,
        qualities: vec![Quality { from, to, kind: QualityKind::Hitbox(attack) }],
        motion: None,
        requires,
    }
}

/// A natural strike sourced from a body part (punch / kick / bite / claw).
fn natural(
    part: BodyPart,
    guard: GuardHeight,
    timing: Timing,
    base_dmg: i32,
    hitstun: u32,
    blockstun: u32,
    lv: &Levers,
) -> FrameProfile {
    let t = tune(timing, lv);
    let total = t.total();
    let attack = Attack {
        kind: AttackKind::Strike,
        guard,
        blockable: true,
        source: HitboxSource::Part(part),
        placement: Vec3::new(0.35, 0.0, 0.0), // reaches forward off the limb
        tracking: Tracking::Linear,
        // Stuns are clamped escapable so no natural strike can loop into itself (the authoring bug).
        hit: hit(dmg(base_dmg, lv), escapable(hitstun, total), escapable(blockstun, total)),
        counter: None,
        tech_recover: 0,
    };
    one_hit(t, attack, vec![part])
}

pub fn punch(lv: &Levers) -> FrameProfile {
    // hitstun 9 on a total-12 move → the victim recovers ~3 ticks before a re-punch could land.
    natural(BodyPart::Fist, GuardHeight::Mid, Timing { startup: 4, active: 2, recovery: 6 }, 8, 9, 6, lv)
}
pub fn kick(lv: &Levers) -> FrameProfile {
    natural(BodyPart::Foot, GuardHeight::Low, Timing { startup: 7, active: 2, recovery: 10 }, 12, 14, 8, lv)
}
pub fn bite(lv: &Levers) -> FrameProfile {
    natural(BodyPart::Fangs, GuardHeight::High, Timing { startup: 8, active: 2, recovery: 12 }, 14, 16, 8, lv)
}
pub fn claw(lv: &Levers) -> FrameProfile {
    natural(BodyPart::Claws, GuardHeight::Mid, Timing { startup: 5, active: 2, recovery: 8 }, 11, 10, 6, lv)
}

/// The universal **guard** — every fighter's defensive option (`requires` nothing). A held stance
/// covering High/Mid; a Low or Overhead beats it (the high/low mixup the fighting-game layer keeps).
/// Its own recovery stops it being a free, spammable wall.
pub fn guard(lv: &Levers) -> FrameProfile {
    let t = tune(Timing { startup: 1, active: 22, recovery: 8 }, lv);
    let to = t.startup + t.active - 1;
    FrameProfile {
        timing: t,
        qualities: vec![Quality {
            from: t.startup,
            to,
            kind: QualityKind::Block { covers: vec![GuardHeight::High, GuardHeight::Mid] },
        }],
        motion: None,
        requires: vec![], // anyone can guard
    }
}

/// A weapon strike: a `Custom` hitbox sized by the weapon's lane reach (the spacing identity). The
/// weapon's frame deltas fold into the levers so the same template yields a fast dagger or a slow,
/// long, heavy greatsword purely from data.
pub fn weapon_moves(weapon: &Weapon, lv: &Levers) -> Vec<FrameProfile> {
    let wlv = Levers {
        startup_cut: lv.startup_cut - weapon.startup_delta,
        recovery_cut: lv.recovery_cut - weapon.recovery_delta,
        damage_bonus: lv.damage_bonus + weapon.damage_delta,
    };
    let t = tune(Timing { startup: 8, active: 3, recovery: 12 }, &wlv);
    let total = t.total();
    let mid = (weapon.min_range + weapon.max_range) / 2.0;
    let half_x = ((weapon.max_range - weapon.min_range) / 2.0).max(0.1);
    let reach_box = Box3::new(Vec3::new(mid, 1.0, 0.0), Vec3::new(half_x, 0.5, 0.2));
    let attack = Attack {
        kind: AttackKind::Strike,
        guard: match weapon.class {
            WeaponClass::Greatsword => GuardHeight::High, // heavy overheads
            _ => GuardHeight::Mid,
        },
        blockable: true,
        source: HitboxSource::Custom(reach_box),
        placement: Vec3::ZERO,
        tracking: Tracking::Linear,
        hit: hit(dmg(16, &wlv), escapable(14, total), escapable(8, total)),
        counter: None,
        tech_recover: 0,
    };
    vec![one_hit(t, attack, vec![BodyPart::Fist])] // must have a hand to wield
}
