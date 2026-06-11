//! The combat sim (tech-plan §3): `entity` (runtime actors), `schedule` (decision
//! points + side-blind commits), `spatial` (target-lane math, `does_hit`), `resolve`
//! (the contact priority table), `sim` (the advance loop), `observe` (★ the fog
//! boundary — the only read path out of a live fight), `forecast` (projection-replay),
//! `agents` (Observation-only AI read-profiles).

pub mod agents;
pub mod entity;
pub mod forecast;
pub mod observe;
pub mod resolve;
pub mod schedule;
pub mod sim;
pub mod spatial;
