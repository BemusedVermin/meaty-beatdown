//! Visible encounters — enemy patrols/ships placed on the ocean. There are **no random encounters**:
//! every encounter is a token you can *see* and choose to engage by sailing into it (RPG-first; no
//! preview step — walking into one starts the fight).

use super::poi::Faction;

/// A visible encounter token sitting on a water hex.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Encounter {
    /// Who you fight — a faction's patrol / pirates.
    pub faction: Faction,
    /// Difficulty tier (0 = trivial). The content layer turns this into an actual fighter roster.
    pub strength: u8,
}
