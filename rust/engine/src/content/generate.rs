//! Worlds Without Number–style procedural generation — roll a character/monster from a seed, so
//! fighters (and exploration encounters) are reproducible content rather than hand-authored.

use super::attributes::Attributes;
use super::compile::{compile, Fighter};
use super::equipment::{Weapon, WeaponClass};
use super::morphology::{Modifier, Morphology};
use super::sheet::{Build, Skills};
use crate::exploration::Encounter;
use crate::fighting::BodyPart;

/// Roll 3d6 per attribute, in order (WWN).
pub fn roll_attributes(rng: &mut fastrand::Rng) -> Attributes {
    let mut d3d6 = || (rng.u32(1..=6) + rng.u32(1..=6) + rng.u32(1..=6)) as i32;
    Attributes {
        strength: d3d6(),
        dexterity: d3d6(),
        constitution: d3d6(),
        intelligence: d3d6(),
        wisdom: d3d6(),
        charisma: d3d6(),
    }
}

const WEAPONS: [WeaponClass; 4] =
    [WeaponClass::Dagger, WeaponClass::Sword, WeaponClass::Greatsword, WeaponClass::Spear];
const MODIFIERS: [Modifier; 2] = [Modifier::Fanged, Modifier::Clawed];

/// Roll a full build from a seed.
pub fn generate_build(seed: u64) -> Build {
    let mut rng = fastrand::Rng::with_seed(seed);
    let attributes = roll_attributes(&mut rng);
    let morphology = if rng.bool() { Morphology::Biped } else { Morphology::Quadruped };
    let parts = morphology.parts();

    let modifiers: Vec<Modifier> =
        MODIFIERS.iter().copied().filter(|m| m.compatible_with(parts) && rng.bool()).collect();

    // Only an armed morphology (one with a Fist) may carry a weapon.
    let weapon = if parts.contains(&BodyPart::Fist) && rng.bool() {
        Some(Weapon::of(WEAPONS[rng.usize(0..WEAPONS.len())]))
    } else {
        None
    };

    let skills = Skills { unarmed: rng.u8(0..=4), weapon: rng.u8(0..=4) };
    Build { attributes, morphology, modifiers, skills, weapon }
}

/// Roll + compile a fighter from a seed.
pub fn generate_fighter(seed: u64) -> Fighter {
    compile(&generate_build(seed))
}

/// Compile a fighter for an exploration encounter, scaled up by its strength tier.
pub fn fighter_for_encounter(enc: &Encounter, seed: u64) -> Fighter {
    let mut build = generate_build(seed);
    let bump = enc.strength as i32 * 2; // tougher foes hit harder and last longer
    build.attributes.strength += bump;
    build.attributes.constitution += bump;
    compile(&build)
}
