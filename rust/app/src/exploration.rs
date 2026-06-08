//! # `exploration` — the overworld driver seam
//!
//! Drives the deterministic [`engine::exploration`] logic from the Bevy shell: it owns the generated
//! `World` + the `Party`, reads movement input, sails the hexcrawl, and — on sailing into a **visible
//! encounter** — raises the `CombatState` overlay (handing the encounter to [`crate::combat`]) with
//! **no preview**: the fight just begins. Mirrors `combat.rs` — structure here; input/content as
//! clearly-named stubs.
//!
//! | engine                       | shell (`state`)                          |
//! |------------------------------|------------------------------------------|
//! | [`sail`] → `TravelResult`    | a system gated by `ExplorationState::Overworld` |
//! | `Engaged { encounter }`      | raise `CombatState::InitializeFight`     |
//! | `Docked { poi }`             | route to `Shop` / `Trainer` / `Dungeon`  |

use bevy::prelude::*;
use engine::exploration::{generate, sail, Encounter, GenConfig, Hex, Party, Poi, TravelResult, World};

use crate::debuglog::DebugLog;
use crate::state::{CombatState, ExplorationState, GameState, PauseState};

/// The deterministic seed for this session's world.
const WORLD_SEED: u64 = 0xC0FFEE;

/// The generated overworld (the engine [`World`]), held by the shell while a session runs.
#[derive(Resource)]
pub struct Overworld(pub World);

/// The player's ship: position + heading (`0..=5` = index into the six hex directions).
#[derive(Resource)]
pub struct Voyage {
    pub party: Party,
    pub heading: u8,
}

/// The encounter the party just sailed into — handed to the combat driver to build the fight.
#[derive(Resource)]
pub struct EngagedEncounter(pub Encounter);

/// Registers the overworld driver: build the world on entering exploration, pump travel while in the
/// `Overworld` sub-state, and clear the beaten encounter on victory.
pub struct ExplorationDriverPlugin;

impl Plugin for ExplorationDriverPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Exploration), build_world)
            .add_systems(
                Update,
                sail_system
                    .run_if(in_state(ExplorationState::Overworld))
                    .run_if(in_state(CombatState::Dormant))
                    .run_if(in_state(PauseState::Running)),
            )
            .add_systems(OnEnter(CombatState::Victory), clear_beaten_encounter);
    }
}

/// `OnEnter(Exploration)`: generate the world + place the party, once (kept across pauses/fights).
fn build_world(mut commands: Commands, existing: Option<Res<Overworld>>, log: Res<DebugLog>) {
    if existing.is_some() {
        return;
    }
    let world = generate(WORLD_SEED, &GenConfig::default());
    let start = start_hex(&world);
    log.line(
        "explore",
        format!(
            "world built (seed {WORLD_SEED:#x}): {} tiles, {} encounters, {} POIs | start {start:?}",
            world.tiles.len(),
            world.encounters.len(),
            world.pois.len(),
        ),
    );
    commands.insert_resource(Voyage { party: Party { pos: start }, heading: 0 });
    commands.insert_resource(Overworld(world));
}

/// `Overworld`: read the player's chosen step, sail, and route the result. Sailing into an encounter
/// raises the combat overlay (no preview); docking routes to the destination sub-state.
fn sail_system(
    mut commands: Commands,
    input: Res<ButtonInput<KeyCode>>,
    world: Option<Res<Overworld>>,
    voyage: Option<ResMut<Voyage>>,
    log: Res<DebugLog>,
    mut next_combat: ResMut<NextState<CombatState>>,
) {
    let (Some(world), Some(mut voyage)) = (world, voyage) else { return };

    // Turn the ship (A/D or ←/→) — one hex direction per press; turning doesn't move.
    if input.just_pressed(KeyCode::KeyA) || input.just_pressed(KeyCode::ArrowLeft) {
        voyage.heading = (voyage.heading + 5) % 6;
        log.line("explore", format!("turn left → heading {}", voyage.heading));
        return;
    }
    if input.just_pressed(KeyCode::KeyD) || input.just_pressed(KeyCode::ArrowRight) {
        voyage.heading = (voyage.heading + 1) % 6;
        log.line("explore", format!("turn right → heading {}", voyage.heading));
        return;
    }

    // Sail forward (W or ↑) one hex in the current heading.
    if !(input.just_pressed(KeyCode::KeyW) || input.just_pressed(KeyCode::ArrowUp)) {
        return;
    }
    let to = voyage.party.pos.all_neighbors()[voyage.heading as usize];
    match sail(&world.0, &mut voyage.party, to) {
        TravelResult::Engaged { encounter, .. } => {
            log.line("explore", format!("ENGAGED at {to:?}: {encounter:?} → combat"));
            // Hand the encounter to the combat driver and raise the overlay — the fight just begins.
            commands.insert_resource(EngagedEncounter(encounter));
            next_combat.set(CombatState::InitializeFight);
        }
        TravelResult::Docked { .. } => {
            log.line("explore", format!("docked at {to:?}"));
            route_destination();
        }
        TravelResult::Sailed { .. } => log.line("explore", format!("sailed → {:?}", voyage.party.pos)),
        TravelResult::Blocked => log.line("explore", format!("blocked (can't enter {to:?})")),
    }
}

/// `OnEnter(CombatState::Victory)`: remove the just-beaten encounter from the world so it doesn't
/// re-trigger when the overlay lowers and the frozen overworld resumes.
fn clear_beaten_encounter(
    engaged: Option<Res<EngagedEncounter>>,
    mut world: Option<ResMut<Overworld>>,
    voyage: Option<Res<Voyage>>,
    mut commands: Commands,
) {
    if let (Some(_), Some(world), Some(voyage)) = (engaged.as_ref(), world.as_mut(), voyage.as_ref())
    {
        world.0.clear_encounter(voyage.party.pos);
    }
    commands.remove_resource::<EngagedEncounter>();
}

/// The party's starting hex: the lowest-coordinate port (the home harbour), else the map origin.
fn start_hex(world: &World) -> Hex {
    world
        .pois
        .iter()
        .filter(|(_, p)| matches!(p, Poi::Port { .. }))
        .map(|(h, _)| *h)
        .min_by_key(|h| (h.x, h.y))
        .unwrap_or(Hex::new(0, 0))
}

// ── Graceful stubs: input / content routing, wired later (no panic so the shell still boots) ──────

/// Route a dock onto the right destination sub-state by the POI under the party.
fn route_destination() {
    // TODO: set ExplorationState to Shop / Trainer / Dungeon based on the docked POI.
}
