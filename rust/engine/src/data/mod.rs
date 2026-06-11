//! The content SCHEMA (tech-plan §3): pure types, no behavior — the language both engine
//! and content speak. Every combat magnitude lives here or in authored data, never in
//! engine code (charter C-AUTH: the engine is an interpreter with no numbers of its own).
//!
//! Phase 1 carries the duel-core subset of spec §2; cancel windows, cue classes, and the
//! decay schedules join in Phases 2–3. Pre-golden-vector schema is free to move.

pub mod arena;
pub mod defense;
pub mod hit;
pub mod ids;
pub mod movedef;
pub mod ruleset;

pub use arena::{ArenaDef, WallSpec, Walls};
pub use defense::DefenseProfile;
pub use hit::{HitEvent, Reaction};
pub use ids::{FormId, MoveId};
pub use movedef::{
    CancelGate, CancelWindow, GainGate, GainResource, Height, HeightMask, InvulnCover, Move,
    MoveCategory, MoveCost, PhaseMotion, PropertyKind, PropertyWindow, ReachEnvelope, ResourceGain,
    SelfMotion, StanceKind, StanceReq, StanceSpec, ThrowBreakKey, Timing, Tracking,
};
pub use ruleset::{ChDefault, ExtenderLatches, FocusGains, Ruleset};
