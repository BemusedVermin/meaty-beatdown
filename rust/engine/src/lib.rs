//! TICK engine — the deterministic core.
//!
//! No Bevy, no floats, no wall clock (tech-plan §1): the engine compiles headless and runs
//! full fights in tests with zero graphics. A fight is a pure function of
//! (initial state, content + Ruleset, decision log) — charter C-DET.
//!
//! Module map (tech-plan §3): `core` (ids, tick, fx, rng), `trace` (the behavioral
//! contract), `combat` (the sim). `data`, `content`, and `exploration` arrive in their
//! phases (implementation-plan).

pub mod combat;
pub mod core;
pub mod trace;

/// Engine crate version, exposed so the app stub can prove the `app -> engine` arrow links.
#[must_use]
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
