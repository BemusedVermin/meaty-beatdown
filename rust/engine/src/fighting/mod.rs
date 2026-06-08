//! # `fighting` — the combat engine (turn-based, *based on* fighting-game concepts)
//!
//! RPG combat **inspired by** fighting games — not a real-time fighter. **100% turn-based**: no
//! rounds, no timer, no real-time input. A shared tick counter only *orders* actions; the sim
//! **pauses** whenever an entity must choose. Pure, headless logic (it leans on `bevy_math` only for
//! AABB geometry — math, no ECS/App).
//!
//! ## Two orthogonal hit layers
//! - **The high/mid/low RPS** — `Attack::guard` (High/Mid/Low/Overhead) vs a defender's `Block`
//!   `covers`. A *discrete* mixup decided in [`resolver`], independent of geometry — the readable
//!   fighting-game core that always holds.
//! - **The 3D spatial layer** — hitboxes vs hurtboxes as AABBs on 3 axes (X lane, Y vertical,
//!   Z lateral). Decides whether a hit *reaches* (range, anti-air, sidestep). Richer, and can be kept
//!   generous if it proves too fiddly — the RPS still carries the game because the two are decoupled.
//!
//! ## No generic moves, no engine constants
//! Every fighter acts only through **authored moves**; every magnitude is a **quality of the move**
//! (see [`frame`], [`config`]). Moves are **morphology-gated** — a move's `requires` body parts must
//! exist on the fighter — and a `Hitbox`'s geometry is **sourced from the body** ([`HitboxSource`]),
//! so both hits and applicability are character-specific.
//!
//! ## Run/pause ↔ the `Fight` FSM
//! | engine                            | FSM (`state::FightState`) |
//! |-----------------------------------|---------------------------|
//! | [`Sim::advance`] running ticks    | `Advancing`               |
//! | an [`Outcome::Decision`] returned | `AwaitInput`              |
//!
//! ## Scope (this pass = "L2 engine core")
//! Implemented: tick scheduler, NEUTRAL/PRESSURE regime, advance→resolve loop, the contact-priority
//! resolver, 3D AABB overlap, morphology gating, entity runtime state.
//! **Deferred:** resources & the AP/tempo economy, cancels / hit-confirm, the combo governors, the
//! L3 move taxonomy, and the L4 stats→frame-data compiler.

pub mod config;
pub mod frame;
pub mod space;
pub mod entity;
pub mod regime;
pub mod resolver;
pub mod sim;

pub use entity::{Body, Entity, EntityId, Health, MoveInstance, Reaction, SideId};
pub use frame::{
    phase_at, Attack, AttackKind, CounterBonus, FrameProfile, GuardHeight, HitEffect, HitboxSource,
    InvulnType, Motion, MoveId, MoveTable, Phase, Quality, QualityKind, Timing, Tracking,
};
pub use regime::Regime;
pub use resolver::{classify_contact, ContactResult};
pub use space::{overlaps, place, BodyPart, Box3, Vec3};
pub use sim::{Action, Decision, EndReason, Outcome, Sim};
