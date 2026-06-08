//! # `exploration` — the flooded-world hexcrawl (the overworld)
//!
//! A pure (no-Bevy) model + procedural generator for the game's exploration layer: a **95%-water**
//! hex ocean dotted with **master-anchored archipelagos**, ports, sects, ruins, and Crossing
//! thresholds — drawn from `docs/the-promise-plot-bible.md` (a flooded xianxia world with a
//! Pirates-of-the-Caribbean surface). The Bevy `state::ExplorationState` systems drive this data.
//!
//! World generation is **seeded** (`hexx` for the hex grid, `fastrand` for the RNG), so a world seed
//! reproduces the same world. The method is WWN's sandbox approach — seed-and-spread terrain, then
//! scale locations by land area — adapted so land only exists where a master holds the deluge back.
//!
//! ## Scope
//! The hex world **model** + **generator** + **ocean travel** and **visible encounters** (sailing
//! into one hands it off to combat). Deferred: faction simulation and the in-fight content layer.

pub mod hex;
pub mod terrain;
pub mod poi;
pub mod encounter;
pub mod world;
pub mod travel;
pub mod worldgen;

pub use encounter::Encounter;
pub use hex::{distance, map_hexes, Hex};
pub use poi::{Faction, Poi};
pub use terrain::Terrain;
pub use travel::{navigable_neighbors, sail, visible_encounters, Party, TravelResult};
pub use world::{Tile, World};
pub use worldgen::{generate, GenConfig};
