//! `PauseState` — an orthogonal axis, meaningful only during `AppState::InSession`.
//!
//! `Paused` freezes gameplay by gating systems with `run_if(in_state(PauseState::Running))`;
//! it never changes `AppState`/`GameState` and never tears anything down, so `resume` continues
//! exactly where it left off — no save/restore.
//!
//! Mirrors **Pause State Diagram** in `docs/fsm.md`.

use bevy::prelude::*;

/// The pause axis. Independent of `AppState`; only acted on while `AppState::InSession`.
#[derive(States, Clone, PartialEq, Eq, Hash, Debug, Default)]
pub enum PauseState {
    /// Normal play; gameplay systems tick. The default.
    #[default]
    Running,
    /// Gameplay frozen by run-conditions while every game-state value is retained; pause menu runs.
    Paused,
}
