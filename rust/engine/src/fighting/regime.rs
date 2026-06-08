//! The NEUTRAL vs PRESSURE decision regime — derived *entirely* from `ready_tick`, with no
//! special-casing (spec §2.1, mechanics §1.3). The concrete derivation lives in [`super::sim`]
//! (`pending_decision`); this is just the label the engine reports.

/// Which decision regime the fight is in.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Regime {
    /// Every contender is free at once → commit hidden, simultaneously (the neutral mind-read).
    Neutral,
    /// One entity is free while ≥1 opponent is locked → it chooses with full information.
    Pressure,
}
