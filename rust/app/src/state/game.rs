//! `GameState` — the gameplay machine. A **separate top-level** `States` instance (default
//! `Inactive`), switched on at `OnEnter(AppState::InSession)` and frozen (not destroyed) by
//! [`super::pause::PauseState`]. Deliberately **not** a `SubState` of `AppState`, so a pause
//! leaves this machine — and its `Exploration` sub-tree — untouched. (`Combat` is no longer a
//! value here; it's the orthogonal `CombatState` overlay, frozen the same way.)
//!
//! Mirrors **Game State Diagram** in `docs/fsm.md`.

use bevy::prelude::*;

/// The world-context machine. `Inactive` until a session begins; the `Exploration` sub-tree hangs
/// off it. (Combat is the orthogonal [`super::combat::CombatState`] overlay, not a value here.)
#[derive(States, Clone, PartialEq, Eq, Hash, Debug, Default)]
pub enum GameState {
    /// No session running (boot / main menu). The default until `InSession` flips it to `Exploration`.
    #[default]
    Inactive,
    /// Free-roam hexgrid overworld — the session entry point. Expanded by [`super::exploration::ExplorationState`].
    Exploration,
    /// Conversation / scripted NPC interaction.
    Dialogue,
}
