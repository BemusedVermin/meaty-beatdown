//! # `combat` — the driver seam
//!
//! The one place the Bevy shell and the deterministic [`engine::fighting`] core meet. The engine
//! *owns* the simulation (a pure `Sim`: tick clock, contact resolution, effect application); Bevy
//! *owns* presentation and input and merely **pumps** it. The mapping is:
//!
//! | engine                            | shell (`state::FightState`) |
//! |-----------------------------------|-----------------------------|
//! | [`Sim::advance`] running ticks    | `Advancing`                 |
//! | an [`Outcome::Decision`] returned | `AwaitInput`                |
//! | an [`Outcome::Ended`] returned    | `CombatState::{Victory,…}`  |
//!
//! ## Scope
//! This module wires the **structure** — the resource, the plugin, the state-gated systems and the
//! projection seam. The bodies that require *engine / content logic* (compiling an encounter into
//! fighters, gathering an actor's chosen action, mapping engine reaction → `ActorState`) are left as
//! clearly-named `todo!()` stubs: that logic belongs to the engine + content layers, not the shell.

use bevy::prelude::*;
use engine::fighting::{Action, EndReason, EntityId, Outcome, Sim};

use crate::state::{ActorState, CombatState, FightState};

/// The live fight: the engine [`Sim`] wrapped as a Bevy resource. Present **only** while a fight is
/// running — inserted at `InitializeFight`, removed when the `Combat` overlay lowers. The engine is
/// the authority; this is just where the shell holds a handle to it.
#[derive(Resource)]
pub struct ActiveFight(pub Sim);

/// Registers the driver: build/teardown on the `Combat` phase edges, and the per-frame pump gated by
/// the `FightState` tick loop.
pub struct CombatDriverPlugin;

impl Plugin for CombatDriverPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(CombatState::InitializeFight), build_fight)
            .add_systems(OnExit(CombatState::Fight), teardown_fight)
            // Pump the clock until the engine pauses for a decision or ends the fight.
            .add_systems(Update, advance_fight.run_if(in_state(FightState::Advancing)))
            // Collect the ready actors' choices, commit them, return to advancing.
            .add_systems(Update, await_input.run_if(in_state(FightState::AwaitInput)))
            // Project authoritative engine state onto each actor's ECS view, every frame of a fight.
            .add_systems(Update, project_actor_states.run_if(in_state(CombatState::Fight)));
    }
}

/// `InitializeFight`: compile the encounter into engine fighters + a move table, build the [`Sim`],
/// and insert it as [`ActiveFight`]. Fighter production is engine/content work — stubbed.
fn build_fight(mut commands: Commands) {
    commands.insert_resource(ActiveFight(build_sim()));
}

/// `OnExit(Fight)`: drop the simulation handle. The overlay lowering is what ends the fight.
fn teardown_fight(mut commands: Commands) {
    commands.remove_resource::<ActiveFight>();
}

/// `Advancing`: run the engine until it needs a decision or the fight ends, then route the outcome
/// onto the state machines. (Idempotent while a decision is pending — see [`Sim::advance`].)
fn advance_fight(
    fight: Option<ResMut<ActiveFight>>,
    mut next_fight: ResMut<NextState<FightState>>,
    mut next_combat: ResMut<NextState<CombatState>>,
) {
    let Some(mut fight) = fight else { return };
    match fight.0.advance() {
        // An actor is ready: hand off to input collection.
        Outcome::Decision(_decision) => next_fight.set(FightState::AwaitInput),
        // The bout resolved: raise the matching outcome phase on the overlay.
        Outcome::Ended(reason) => next_combat.set(combat_outcome(reason)),
    }
}

/// `AwaitInput`: gather the ready actors' chosen actions (player UI / AI), commit them to the engine,
/// and return to `Advancing`.
fn await_input(fight: Option<ResMut<ActiveFight>>, mut next_fight: ResMut<NextState<FightState>>) {
    let Some(mut fight) = fight else { return };
    let choices = gather_choices(&fight.0);
    fight.0.commit(&choices);
    next_fight.set(FightState::Advancing);
}

/// Mirror the engine's authoritative per-entity state onto each fighter's [`ActorState`] component
/// (what animation / UI read). The engine `Reaction` + in-flight move phase is the source of truth;
/// `ActorState` is the projected view.
fn project_actor_states(fight: Option<Res<ActiveFight>>, _actors: Query<&mut ActorState>) {
    let Some(_fight) = fight else { return };
    todo!("project each engine Entity's reaction / move-phase onto its ActorState component")
}

// ── Stubs: engine / content logic, intentionally not implemented in the shell ────────────────────

/// Compile the engaged encounter (fighters + their move table) into a ready-to-run [`Sim`].
fn build_sim() -> Sim {
    todo!("compile the encounter's fighters into engine Entities + a MoveTable, then Sim::new(...)")
}

/// Collect, for each actor the engine is waiting on, its chosen [`Action`] (from input or AI).
fn gather_choices(_sim: &Sim) -> Vec<(EntityId, Action)> {
    todo!("read the pending Decision and resolve each ready actor's action via UI / AI")
}

/// Map an engine end-reason onto the combat overlay's outcome phase.
fn combat_outcome(_reason: EndReason) -> CombatState {
    todo!("route engine Victory / Draw / TickCap onto CombatState Victory / Defeat / Escape")
}
