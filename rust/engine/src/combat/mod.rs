//! The combat sim (tech-plan §3). Phase 1 module map: `entity` (runtime actors),
//! `schedule` (decision points + side-blind commits), `spatial` (target-lane math,
//! `does_hit`), `resolve` (the contact priority table), `sim` (the advance loop).
//! `observe` and `agents` join in Phase 3 — until then the fog boundary is a TODO, not
//! a wall.

pub mod entity;
pub mod resolve;
pub mod schedule;
pub mod sim;
pub mod spatial;
