//! `ExplorationState` (sub of `GameState::Exploration`) and `DungeonState` (sub of
//! `ExplorationState::Dungeon`).
//!
//! The hexgrid is the **navigation layer only**. Note there is no `Combat` variant here: engaging
//! a visible encounter raises the orthogonal `CombatState` overlay (the 1D-lane fighting engine)
//! while this sub-tree is **frozen, not destroyed** — so it resumes exactly where it was when the
//! fight ends. A victory then routes the retained layer to `Loot`. (The engage/return trigger
//! systems are mechanics, later.)
//!
//! Mirrors **Exploration State Diagram** and **Dungeon State Diagram** in `docs/fsm.md`.

use bevy::prelude::*;

use super::game::GameState;

/// The hexgrid overworld. Active while `GameState::Exploration`.
#[derive(SubStates, Clone, PartialEq, Eq, Hash, Debug, Default)]
#[source(GameState = GameState::Exploration)]
pub enum ExplorationState {
    /// The hexgrid map and entry point; visible encounter tokens + POIs occupy hexes.
    #[default]
    Overworld,
    /// Inventory / character sheet / move loadout / map overlay (the orthogonal-freeze pattern).
    Menu,
    /// A shopkeeper hex: buy / sell.
    Shop,
    /// A skill-trainer hex: rank weapon skills, choose foci.
    Trainer,
    /// Inside a dungeon instance — expanded by [`DungeonState`].
    Dungeon,
    /// The reward beat after a won encounter: the pickup moment (drop generation is a data system).
    Loot,
}

/// A single dungeon instance. Active while `ExplorationState::Dungeon`.
#[derive(SubStates, Clone, PartialEq, Eq, Hash, Debug, Default)]
#[source(ExplorationState = ExplorationState::Dungeon)]
pub enum DungeonState {
    /// The threshold: `descend` into the dungeon, or `turn back` to the overworld.
    #[default]
    Entrance,
    /// Traverse the interior — the dungeon's `Overworld` equivalent.
    Delve,
    /// A loot container → `Loot`.
    Chest,
    /// The reward beat after a won encounter or opened chest.
    Loot,
    /// The culminating encounter (also the fighting engine).
    Boss,
    /// Dungeon complete: claim the boss reward and leave.
    Cleared,
}
