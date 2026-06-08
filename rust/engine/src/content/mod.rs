//! # `content` — the L4 RPG / content layer (the compiler)
//!
//! Stats, skills, and equipment are **compilers**; the **move** is the basic unit of combat they
//! produce. This layer takes a [`Build`] (attributes + morphology + modifiers + skills + weapon) and
//! compiles it into a [`Fighter`] — a body (3D stick-figure part-boxes) + the moves it can use —
//! which the [`crate::fighting`] engine then runs. This is the single bridge **L4 → L2**; the engine
//! never sees a stat (one-way: `content` imports `fighting`/`exploration`, never the reverse).
//!
//! Procedural, **Worlds Without Number**–style: a character/monster is rolled from a seed
//! ([`generate_build`] / [`generate_fighter`]), and an exploration [`crate::exploration::Encounter`]
//! compiles into fighters ([`fighter_for_encounter`]).
//!
//! ## Scope (first pass)
//! Morphology (biped / quadruped) → stick-figure body; modifiers (fanged / clawed) that attach to a
//! base part; STR→damage, DEX→startup, CON→HP, skill→safety levers; weapons → reach-sized moves; the
//! `arena` assembler (encounter → runnable `Sim`). Deferred: foci, the AP/Focus/Poise resources
//! (the engine doesn't run them yet), the budget linter, and loot.

pub mod attributes;
pub mod morphology;
pub mod equipment;
pub mod sheet;
pub mod moves;
pub mod compile;
pub mod generate;

pub use attributes::{modifier, Attributes};
pub use compile::{arena, compile, Fighter};
pub use equipment::{Weapon, WeaponClass};
pub use generate::{fighter_for_encounter, generate_build, generate_fighter, roll_attributes};
pub use morphology::{Modifier, Morphology};
pub use moves::Levers;
pub use sheet::{Build, Skills};
