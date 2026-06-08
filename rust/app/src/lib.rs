//! # `app` — the Bevy game shell
//!
//! The host layer: it owns the window, render, input, audio, assets, and the game-wide **finite
//! state machines** (`docs/fsm.md`), and it *drives* the deterministic [`engine`] core — it never
//! lives inside it. The dependency arrow runs strictly `app → engine`; the engine has no idea Bevy
//! exists, which is what keeps it portable and golden-vector–testable.
//!
//! Two pieces:
//! - [`state`] — the `States` / `SubStates` axes (App / Pause / Game / Combat) + the per-actor
//!   `ActorState` component. Just the machines; mirrors `docs/fsm.md`.
//! - [`combat`] — the **driver**: the seam that pumps `engine::fighting::Sim` from inside the
//!   `Combat` overlay and projects engine state onto ECS for presentation.

pub mod combat;
pub mod state;

pub use combat::CombatDriverPlugin;
pub use state::StatePlugin;
