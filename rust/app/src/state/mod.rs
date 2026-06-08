//! Game-wide finite state machines — Bevy `States` / `SubStates` mirroring `docs/fsm.md`.
//!
//! This module is *just the machines*: the state enums and their registration. Four
//! independent top-level axes —
//!
//! - [`AppState`]    — the application shell / lifecycle (`Logos … InSession`)
//! - [`PauseState`]  — an orthogonal freeze axis (`Running` / `Paused`)
//! - [`GameState`]   — the world context (`Inactive`, `Exploration`, `Dialogue`)
//! - [`CombatState`] — the combat overlay (`Dormant`, then the encounter phases), raised over a
//!   frozen world context the same way `Pause` is
//!
//! plus the sub-state trees:
//!
//! ```text
//! GameState::Exploration ── ExplorationState ── (Dungeon) ── DungeonState
//! CombatState (overlay)  ──────────────────────── (Fight) ── FightState
//! ```
//!
//! and one per-actor component FSM, [`ActorState`], that runs while `CombatState::Fight` is live.
//!
//! Transition systems, run-condition gating, and the engage/return routing between layers are
//! *mechanics* and live elsewhere — none of that is here.

use bevy::prelude::*;

pub mod app;
pub mod pause;
pub mod game;
pub mod exploration;
pub mod combat;
pub mod actor;

pub use actor::ActorState;
pub use app::AppState;
pub use combat::{CombatState, FightState};
pub use exploration::{DungeonState, ExplorationState};
pub use game::GameState;
pub use pause::PauseState;

/// Registers every state machine from `docs/fsm.md`. Add once to the `App`.
pub struct StatePlugin;

impl Plugin for StatePlugin {
    fn build(&self, app: &mut App) {
        app
            // ── Top-level axes (each its own independent `States`) ──
            .init_state::<AppState>()
            .init_state::<PauseState>()
            .init_state::<GameState>()
            .init_state::<CombatState>()
            // ── Sub-tree under `GameState::Exploration` ──
            .add_sub_state::<ExplorationState>()
            .add_sub_state::<DungeonState>()
            // ── Sub-tree under `CombatState::Fight` ──
            .add_sub_state::<FightState>();
        // `ActorState` is a per-actor *component*, not a global state — nothing to register here.
    }
}
