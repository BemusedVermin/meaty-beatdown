//! WWN attributes + the modifier table. Scores are ~3–18; modifiers are deliberately **low**
//! (−2..+2, WWN-style), so frame-data swings stay small and the engine stays the star (spec §4.2).

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Attributes {
    pub strength: i32,
    pub dexterity: i32,
    pub constitution: i32,
    pub intelligence: i32,
    pub wisdom: i32,
    pub charisma: i32,
}

/// The WWN attribute modifier for a score.
pub fn modifier(score: i32) -> i32 {
    match score {
        s if s <= 3 => -2,
        4..=7 => -1,
        8..=13 => 0,
        14..=17 => 1,
        _ => 2, // 18+
    }
}

impl Attributes {
    pub fn str_mod(&self) -> i32 {
        modifier(self.strength)
    }
    pub fn dex_mod(&self) -> i32 {
        modifier(self.dexterity)
    }
    pub fn con_mod(&self) -> i32 {
        modifier(self.constitution)
    }
    pub fn int_mod(&self) -> i32 {
        modifier(self.intelligence)
    }
    pub fn wis_mod(&self) -> i32 {
        modifier(self.wisdom)
    }
    pub fn cha_mod(&self) -> i32 {
        modifier(self.charisma)
    }
}
