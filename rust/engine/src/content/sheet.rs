//! The character sheet / build — everything that compiles into a fighter.

use super::attributes::Attributes;
use super::equipment::Weapon;
use super::morphology::{Modifier, Morphology};

/// WWN-style combat skills (rank 0–4). Rank does **not** add a to-hit (combat is deterministic — the
/// engine decides hits by spacing/timing); instead it improves frame safety and gates heavier moves.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Skills {
    pub unarmed: u8,
    pub weapon: u8,
}

/// A complete build: the input to [`super::compile::compile`].
#[derive(Clone, Debug)]
pub struct Build {
    pub attributes: Attributes,
    pub morphology: Morphology,
    pub modifiers: Vec<Modifier>,
    pub skills: Skills,
    pub weapon: Option<Weapon>,
}
