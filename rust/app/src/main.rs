//! The game binary: assemble the Bevy `App` from the shell plugins and run it.
//!
//! `DefaultPlugins` brings the window / render / input / `StatesPlugin`; [`StatePlugin`] registers
//! the FSM axes; [`CombatDriverPlugin`] wires the seam that drives the [`engine`] simulation.

use app::{CombatDriverPlugin, StatePlugin};
use bevy::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(StatePlugin)
        .add_plugins(CombatDriverPlugin)
        .run();
}
