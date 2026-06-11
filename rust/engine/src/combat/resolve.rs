//! Contact resolution (spec §5): the priority table, read top to bottom — the defender's
//! state decides the branch. Pure decision logic: the sim applies the outcomes.

use crate::core::tick::Tick;
use crate::data::HitEvent;
use crate::data::movedef::{InvulnCover, Move, MoveCategory, PropertyKind};
use serde::{Deserialize, Serialize};

use super::entity::{ActorState, Entity, MovePhase};

/// What a resolved contact turned out to be (the priority table's verdict, spec §5.1).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum ContactOutcome {
    /// Invulnerability (or a non-grabbable throw target): passes clean through.
    Whiff,
    /// Mutual same-tick throws clash (spec §5.4): both reset.
    ThrowTech,
    /// A grab connected: opens the directional break window (spec §5.4).
    GrabConnected,
    /// A GUARD_POINT deflected the strike (spec §5.5); magnitudes are on the parry
    /// move's data (C-AUTH).
    Parried {
        freeze_attacker: u32,
        parry_recovery: u32,
    },
    Blocked,
    /// Armor absorbed it: scaled damage, no stun, the armored move continues.
    Armored,
    Hit {
        counter: bool,
    },
}

/// A property window of the defender's in-flight move active at `t`.
fn active_properties(defender: &Entity, t: Tick) -> impl Iterator<Item = PropertyKind> + '_ {
    let elapsed = defender.move_elapsed(t);
    defender
        .current_move()
        .into_iter()
        .flat_map(|mv| mv.properties.iter())
        .filter(move |w| elapsed.is_some_and(|e| e >= w.from && e <= w.to))
        .map(|w| w.kind)
}

fn invuln_covers(cover: InvulnCover, category: MoveCategory) -> bool {
    match cover {
        InvulnCover::All => true,
        InvulnCover::Strike => category == MoveCategory::Strike,
        InvulnCover::Throw => category == MoveCategory::Throw,
    }
}

/// Is the defender counter-hittable right now (spec §5.6)? Struck during its own move's
/// startup or recovery, or inside an explicit CH_STATE window.
#[must_use]
pub fn in_counter_hit_state(defender: &Entity, t: Tick) -> bool {
    if active_properties(defender, t).any(|p| matches!(p, PropertyKind::ChState)) {
        return true;
    }
    defender.state == ActorState::Acting
        && matches!(
            defender.move_phase(t),
            Some(MovePhase::Startup | MovePhase::Recovery)
        )
}

/// Is the defender throwing with an active window at `t` (the mutual-throw clause)?
#[must_use]
pub fn throwing_now(defender: &Entity, t: Tick) -> bool {
    defender.state == ActorState::Acting
        && defender
            .current_move()
            .is_some_and(|mv| mv.category == MoveCategory::Throw)
        && defender.move_phase(t) == Some(MovePhase::Active)
}

/// The priority table (spec §5.1). `does_hit` has already passed spatially; this decides
/// what the touch IS. Returns the outcome plus the armor damage multiplier when armored.
#[must_use]
pub fn resolve_contact(
    mv: &Move,
    _hit: &HitEvent,
    defender: &Entity,
    t: Tick,
    back_hit: bool,
) -> ContactOutcome {
    // 1. INVULN to this category -> WHIFF.
    for prop in active_properties(defender, t) {
        if let PropertyKind::Invuln { covers } = prop
            && invuln_covers(covers, mv.category)
        {
            return ContactOutcome::Whiff;
        }
    }

    // 2. Throws: tech on mutual throws; connect on a grabbable victim; whiff otherwise
    //    (crouching / reeling / downed victims are not standing-grabbable — spec §5.4).
    if mv.category == MoveCategory::Throw {
        if throwing_now(defender, t) {
            return ContactOutcome::ThrowTech;
        }
        if defender.grabbable() {
            return ContactOutcome::GrabConnected;
        }
        return ContactOutcome::Whiff;
    }

    // 3. GUARD_POINT covering this strike -> PARRIED (parry beats even unblockables;
    //    it loses to throws — already branched — and to the uncovered height).
    if !back_hit {
        for prop in active_properties(defender, t) {
            if let PropertyKind::GuardPoint {
                covers,
                freeze_attacker,
                parry_recovery,
            } = prop
                && covers.covers(mv.height)
            {
                return ContactOutcome::Parried {
                    freeze_attacker,
                    parry_recovery,
                };
            }
        }
    }

    // 4. Guarding and the held mask covers the height -> BLOCKED (the mixup: an
    //    uncovered height falls through). Unblockables skip this branch entirely.
    if !back_hit
        && mv.blockable
        && defender.guarding()
        && defender
            .held
            .and_then(|s| s.guard)
            .is_some_and(|mask| mask.covers(mv.height))
    {
        return ContactOutcome::Blocked;
    }

    // 5. ARMOR covering it with hits left -> ARMORED (throws never reach here; LOWs go
    //    through unless the authored mask covers them).
    if let Some(inst) = defender.current
        && inst.armor_hits_left > 0
    {
        for prop in active_properties(defender, t) {
            if let PropertyKind::Armor { covers, .. } = prop
                && covers.covers(mv.height)
            {
                return ContactOutcome::Armored;
            }
        }
    }

    // 6. HIT — counter-hit if the defender is in a CH state (spec §5.6).
    ContactOutcome::Hit {
        counter: in_counter_hit_state(defender, t),
    }
}
