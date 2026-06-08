//! `AppState` — the application shell / lifecycle. The top-level `States`; default `Logos`.
//! `Pause` is deliberately **not** here — it's an orthogonal axis ([`super::pause::PauseState`]).
//!
//! Mirrors **Overall State Diagram** in `docs/fsm.md`.

use bevy::prelude::*;

/// The application shell. One game-loaded value (`InSession`) under which the separate
/// [`super::game::GameState`] machine runs.
#[derive(States, Clone, PartialEq, Eq, Hash, Debug, Default)]
pub enum AppState {
    /// Boot splash (publisher / engine logos). Auto-advances to `MainMenu`.
    #[default]
    Logos,
    /// Title screen and root menu hub. Branches to `New` / `Load` / `Credits`, or quits.
    MainMenu,
    /// New-game flow (character creation, fresh save). → `InSession` on character created.
    New,
    /// Read a save and stream assets. → `InSession` on assets ready.
    Load,
    /// Scrolling credits; returns to `MainMenu`.
    Credits,
    /// A game is loaded and live — the `GameState` machine is running underneath.
    InSession,
}
