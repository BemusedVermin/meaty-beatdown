//! The game binary: assemble the Bevy `App` from the shell plugins and run it.
//!
//! `DefaultPlugins` brings the window / render / input / `StatesPlugin`; [`StatePlugin`] registers
//! the FSM axes; [`CombatDriverPlugin`] wires the seam that drives the [`engine`] simulation.

use app::{CombatDriverPlugin, DebugLogPlugin, ExplorationDriverPlugin, RenderPlugin, StatePlugin};
use bevy::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(DebugLogPlugin) // opens the trace file early so every driver can write to it
        .add_plugins(StatePlugin)
        .add_plugins(CombatDriverPlugin)
        .add_plugins(ExplorationDriverPlugin)
        .add_plugins(RenderPlugin)
        .run();
}
