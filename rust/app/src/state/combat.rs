//! `CombatState` — the combat **overlay axis** — and `FightState` (sub of `CombatState::Fight`).
//!
//! `CombatState` is a top-level `States`, orthogonal to `GameState` (default `Dormant`). Engaging
//! an encounter raises it while the world context (`GameState::Exploration` / `Dialogue` and their
//! sub-trees) is **frozen, not destroyed** — the same overlay pattern as [`super::pause::PauseState`].
//! Its non-`Dormant` values are the phases of a single encounter; the `Fight` phase hosts the
//! shared-tick simulation, whose loop is `FightState`.
//!
//! Mirrors **Combat State Diagram** in `docs/fsm.md`.

use bevy::prelude::*;

/// The combat overlay. `Dormant` when no fight is running; otherwise the phases of one encounter.
#[derive(States, Clone, PartialEq, Eq, Hash, Debug, Default)]
pub enum CombatState {
    /// No fight in progress — the overlay is off and the world context runs. The default.
    #[default]
    Dormant,
    /// One-time setup: spawn actors, assign sides, place them in the arena, build runtime state.
    InitializeFight,
    /// Pre-fight presentation (character intros, "Fight!"). → `Fight` when intro finished.
    Introductions,
    /// The live exchange; the shared-tick simulation runs here. Expanded by [`FightState`].
    Fight,
    /// Outcome: the player's side is last standing (all hostile sides eliminated). → `Dormant`.
    Victory,
    /// Outcome: the player's side is eliminated. Soft loss → `Dormant`; the frozen `Exploration` resumes.
    Defeat,
    /// Outcome: the player's side disengaged. → `Dormant`.
    Escape,
}

/// The tick loop inside `Fight`. Active while `CombatState::Fight`.
#[derive(SubStates, Clone, PartialEq, Eq, Hash, Debug, Default)]
#[source(CombatState = CombatState::Fight)]
pub enum FightState {
    /// Advance the shared tick clock and apply any contacts resolving this tick.
    /// Self-loops while nobody is ready.
    #[default]
    Advancing,
    /// An actor's `ready_tick` is up: collect the ready actors' chosen actions
    /// ("actors produce frame"), then return to `Advancing`.
    AwaitInput,
}
