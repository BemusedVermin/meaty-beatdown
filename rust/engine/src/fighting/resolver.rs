//! `classify_contact` — the interaction-priority table (mechanics §6). When [`super::lane::does_hit`]
//! is true, the defender's *currently-active qualities* decide the branch, read top-to-bottom as the
//! priority order: throw / tech → parry → block → armor → hit{counter}. This function only decides
//! the *outcome*; every magnitude (parry freeze, blockstun, counter bonus…) is read from the authored
//! move at apply time, never from an engine constant.

use super::entity::{Entity, Health};
use super::frame::{Attack, AttackKind, GuardHeight};
use super::sim::Tick;

/// The seven possible outcomes of a contact. `Whiff` is produced upstream by `does_hit`; it is kept
/// for completeness / apply-time no-ops.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ContactResult {
    Whiff,
    Parried,
    Thrown,
    ThrowTech,
    Blocked,
    Armored,
    Hit { counter: bool },
}

/// Decide the contact outcome from the attacker's live `attack` and the defender's live qualities.
/// `def_counter_vulnerable` couples with the attack *authoring* a counter — a hit on a vulnerable
/// defender only counters if the attacking move says so. Pure — no mutation.
#[allow(clippy::too_many_arguments)]
pub fn classify_contact(
    attack: &Attack,
    def: &Entity,
    def_block: Option<&[GuardHeight]>,
    def_parry: Option<(Tick, Tick)>,
    def_armor: Option<(u8, Health, Health)>,
    def_throwing: bool,
    def_counter_vulnerable: bool,
) -> ContactResult {
    let counters = def_counter_vulnerable && attack.counter.is_some();

    // Throws resolve first and ignore block / parry / armor.
    if attack.kind == AttackKind::Throw {
        return if def_throwing {
            ContactResult::ThrowTech
        } else {
            ContactResult::Thrown
        };
    }

    // Strikes: parry > block > armor > hit.
    if def_parry.is_some() {
        return ContactResult::Parried;
    }
    if let Some(covers) = def_block {
        let covered = attack.blockable && covers.contains(&attack.guard);
        return if covered {
            ContactResult::Blocked
        } else {
            ContactResult::Hit { counter: counters } // wrong stance / unblockable → the mixup landed
        };
    }
    if let Some((hits, _, _)) = def_armor {
        let used = def.action.as_ref().map(|m| m.armor_used).unwrap_or(0);
        if used < hits {
            return ContactResult::Armored;
        }
    }

    ContactResult::Hit { counter: counters }
}
