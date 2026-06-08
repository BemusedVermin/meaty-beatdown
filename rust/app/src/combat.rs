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
use engine::content::{arena, fighter_for_encounter, generate_fighter};
use engine::fighting::{
    Action, Decision, EndReason, EntityId, Health, MoveId, Outcome, QualityKind, Reaction, Sim,
};

use crate::debuglog::DebugLog;
use crate::exploration::EngagedEncounter;
use crate::state::{ActorState, CombatState, FightState};

/// The live fight: the engine [`Sim`] wrapped as a Bevy resource. Present **only** while a fight is
/// running — inserted at `InitializeFight`, removed when the `Combat` overlay lowers. The engine is
/// the authority; this is just where the shell holds a handle to it.
#[derive(Resource)]
pub struct ActiveFight(pub Sim);

/// Paces the auto-advancing of a fight so each exchange is watchable instead of resolving at 60 fps.
/// One engine **tick** per fire — so a move plays out startup → active → recovery on screen.
#[derive(Resource)]
struct Tempo(Timer);

/// Wall-clock seconds per simulated tick (a ~12-tick punch animates over ~0.7 s).
const SECONDS_PER_TICK: f32 = 0.06;

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
            .add_systems(Update, project_actor_states.run_if(in_state(CombatState::Fight)))
            // On an outcome screen, wait for the player to dismiss it, then lower the overlay.
            .add_systems(
                Update,
                dismiss_outcome.run_if(
                    in_state(CombatState::Victory)
                        .or(in_state(CombatState::Defeat))
                        .or(in_state(CombatState::Escape)),
                ),
            );
    }
}

/// `InitializeFight`: compile the engaged encounter into engine fighters + a move table via the
/// content layer, build the [`Sim`], and insert it as [`ActiveFight`].
fn build_fight(
    mut commands: Commands,
    engaged: Option<Res<EngagedEncounter>>,
    log: Res<DebugLog>,
    mut next_combat: ResMut<NextState<CombatState>>,
) {
    let sim = build_sim(engaged.as_deref());
    log_fight_setup(&log, &sim); // dump the live frame data — the ground truth for "did the fix land?"
    commands.insert_resource(ActiveFight(sim));
    commands.insert_resource(Tempo(Timer::from_seconds(SECONDS_PER_TICK, TimerMode::Repeating)));
    next_combat.set(CombatState::Fight); // skip Introductions for now — straight into the fight
}

/// Dump both fighters + every move's frame data to the trace. Crucially it flags any move whose
/// `hitstun ≥ total` — the exact authoring flaw that produced the infinite-combo lockout — so the log
/// proves whether the running binary actually carries the fix.
fn log_fight_setup(log: &DebugLog, sim: &Sim) {
    log.line("combat", "──────── fight start ────────");
    for (i, e) in sim.entities.iter().enumerate() {
        log.line(
            "combat",
            format!(
                "entity {i}: side {} | {} HP | pos ({:.2},{:.2},{:.2}) facing {}",
                e.side.0, e.health, e.pos.x, e.pos.y, e.pos.z, e.facing
            ),
        );
    }
    let mut ids: Vec<MoveId> = sim.moves.keys().copied().collect();
    ids.sort_by_key(|m| m.0);
    for id in ids {
        let p = &sim.moves[&id];
        let t = p.timing;
        let detail = p
            .qualities
            .iter()
            .find_map(|q| match &q.kind {
                QualityKind::Hitbox(a) => Some(format!(
                    "dmg {} | hitstun {} | blockstun {} | total {}{}",
                    a.hit.damage,
                    a.hit.hitstun,
                    a.hit.blockstun,
                    t.total(),
                    if a.hit.hitstun >= t.total() { "   <<< CAN INFINITE-COMBO" } else { "" },
                )),
                QualityKind::Block { covers } => Some(format!("GUARD covers {covers:?}")),
                _ => None,
            })
            .unwrap_or_else(|| "(no hitbox)".to_string());
        log.line("combat", format!("  move {:>2}: {}/{}/{} | {detail}", id.0, t.startup, t.active, t.recovery));
    }
}

/// `OnExit(Fight)`: drop the simulation handle. The overlay lowering is what ends the fight.
fn teardown_fight(mut commands: Commands) {
    commands.remove_resource::<ActiveFight>();
}

/// On an outcome screen (`Victory`/`Defeat`/`Escape`), `Enter`/`Space` lowers the overlay back to
/// `Dormant` — the frozen overworld resumes.
fn dismiss_outcome(input: Res<ButtonInput<KeyCode>>, mut next_combat: ResMut<NextState<CombatState>>) {
    if input.just_pressed(KeyCode::Enter) || input.just_pressed(KeyCode::Space) {
        next_combat.set(CombatState::Dormant);
    }
}

/// `Advancing`: run the engine until it needs a decision or the fight ends, then route the outcome
/// onto the state machines. (Idempotent while a decision is pending — see [`Sim::advance`].)
fn advance_fight(
    time: Res<Time>,
    mut tempo: ResMut<Tempo>,
    fight: Option<ResMut<ActiveFight>>,
    log: Res<DebugLog>,
    mut next_fight: ResMut<NextState<FightState>>,
    mut next_combat: ResMut<NextState<CombatState>>,
) {
    if !tempo.0.tick(time.delta()).just_finished() {
        return; // pace the fight one tick at a time so moves animate (not resolved in one frame)
    }
    let Some(mut fight) = fight else { return };
    let before: Vec<(Health, Reaction)> =
        fight.0.entities.iter().map(|e| (e.health, e.reaction)).collect();
    match fight.0.step() {
        // A tick of the in-flight move(s) advanced — log any HP / reaction change so the trace shows
        // the exchange unfold; the renderer shows this frame.
        None => {
            let now = fight.0.tick;
            for (i, e) in fight.0.entities.iter().enumerate() {
                let (h0, r0) = before[i];
                if e.health != h0 {
                    log.line("combat", format!("t{now}: entity {i} HP {h0} → {} ({:?})", e.health, e.reaction));
                } else if e.reaction != r0 {
                    log.line("combat", format!("t{now}: entity {i} {r0:?} → {:?}", e.reaction));
                }
            }
        }
        // An actor is ready: hand off to input collection.
        Some(Outcome::Decision(decision)) => {
            log.line("combat", format!("t{}: DECISION {decision:?}", fight.0.tick));
            next_fight.set(FightState::AwaitInput);
        }
        // The bout resolved: raise the matching outcome phase on the overlay.
        Some(Outcome::Ended(reason)) => {
            log.line("combat", format!("t{}: ENDED {reason:?}", fight.0.tick));
            next_combat.set(combat_outcome(reason));
        }
    }
}

/// `AwaitInput`: resolve the ready actors' actions — **human input for the player**, AI for foes —
/// commit them, and return to `Advancing`. While the player hasn't chosen yet, this **stays** in
/// `AwaitInput` (the turn-based pause): nothing commits until their input arrives.
fn await_input(
    input: Res<ButtonInput<KeyCode>>,
    fight: Option<ResMut<ActiveFight>>,
    log: Res<DebugLog>,
    mut next_fight: ResMut<NextState<FightState>>,
) {
    let Some(mut fight) = fight else { return };
    let Some(choices) = collect_choices(&input, &fight.0) else { return }; // waiting on the player
    for &(id, action) in &choices {
        let who = if fight.0.entities[id].side.0 == PLAYER_SIDE { "player" } else { "ai" };
        log.line("combat", format!("t{}: commit {who} e{id} = {}", fight.0.tick, action_label(action)));
    }
    fight.0.commit(&choices);
    next_fight.set(FightState::Advancing);
}

/// A short, log-friendly label for a committed action.
fn action_label(a: Action) -> String {
    match a {
        Action::Wait => "Wait".to_string(),
        Action::Use(m) => format!("Use(move {})", m.0),
    }
}

/// Mirror the engine's authoritative per-entity state onto each fighter's [`ActorState`] component
/// (what animation / UI read). The engine `Reaction` + in-flight move phase is the source of truth;
/// `ActorState` is the projected view.
fn project_actor_states(_fight: Option<Res<ActiveFight>>, _actors: Query<&mut ActorState>) {
    // No fighter entities carry an `ActorState` yet, so there is nothing to project — a no-op until
    // fighters are spawned into the ECS for presentation. (Headless play doesn't need this at all.)
}

// ── Stubs: engine / content logic, intentionally not implemented in the shell ────────────────────

/// Seeds for the placeholder builds. The real player comes from the save/sheet later; the foe is
/// rolled per encounter. Both go through the WWN content layer.
const PLAYER_SEED: u64 = 0x5EED_5EED;
const FOE_SEED: u64 = 0x0F0E_0F0E;
/// A safety cap so a headless bout always terminates even if nobody lands a KO (not a game timer).
const MAX_FIGHT_TICKS: u32 = 3600;

/// Compile a ready-to-run [`Sim`] via the content layer: a placeholder player build vs. a foe
/// compiled from the engaged [`crate::exploration::EngagedEncounter`] (scaled by its strength).
fn build_sim(engaged: Option<&EngagedEncounter>) -> Sim {
    let player = generate_fighter(PLAYER_SEED);
    let foe = match engaged {
        Some(e) => fighter_for_encounter(&e.0, FOE_SEED),
        None => generate_fighter(FOE_SEED),
    };
    let mut sim = arena(&player, &[foe]);
    sim.max_ticks = Some(MAX_FIGHT_TICKS);
    // Open in striking range — no movement moves are generated yet, so the AI can't close distance.
    if sim.entities.len() >= 2 {
        sim.entities[0].pos.x = -0.5;
        sim.entities[1].pos.x = 0.5;
    }
    sim
}

/// The player controls side 0 (see [`engine::content::arena`]).
const PLAYER_SIDE: u8 = 0;

/// Resolve every ready actor's [`Action`]: **human input** for the player's actor(s), **AI** for the
/// rest. Returns `None` while still waiting on the player to choose, so the caller stays paused in
/// `AwaitInput`.
fn collect_choices(input: &ButtonInput<KeyCode>, sim: &Sim) -> Option<Vec<(EntityId, Action)>> {
    let ids = match sim.pending_decision() {
        Some(Decision::Neutral(ids)) => ids,
        Some(Decision::Pressure(id)) => vec![id],
        None => return Some(Vec::new()),
    };
    let mut choices = Vec::with_capacity(ids.len());
    for id in ids {
        if sim.entities[id].side.0 == PLAYER_SIDE {
            choices.push((id, read_player_action(input, sim, id)?)); // `?` → keep waiting
        } else {
            choices.push((id, choose_action(sim, id)));
        }
    }
    Some(choices)
}

/// Read the player's chosen action this frame: `1..=9` select the actor's usable moves (in id order);
/// `Space` waits. `None` = nothing pressed yet (keep waiting).
fn read_player_action(input: &ButtonInput<KeyCode>, sim: &Sim, id: EntityId) -> Option<Action> {
    if input.just_pressed(KeyCode::Space) {
        return Some(Action::Wait);
    }
    for (slot, &mv) in usable_moves(sim, id).iter().enumerate() {
        if digit_key(slot).is_some_and(|k| input.just_pressed(k)) {
            return Some(Action::Use(mv));
        }
    }
    None
}

/// An actor's usable moves in stable id order — the order the number keys map to.
fn usable_moves(sim: &Sim, id: EntityId) -> Vec<MoveId> {
    let mut v: Vec<MoveId> = sim.moves.keys().copied().filter(|&m| sim.can_use(id, m)).collect();
    v.sort_by_key(|m| m.0);
    v
}

/// `KeyCode` for number-key slot `i` (0 → `Digit1` … 8 → `Digit9`).
fn digit_key(i: usize) -> Option<KeyCode> {
    Some(match i {
        0 => KeyCode::Digit1,
        1 => KeyCode::Digit2,
        2 => KeyCode::Digit3,
        3 => KeyCode::Digit4,
        4 => KeyCode::Digit5,
        5 => KeyCode::Digit6,
        6 => KeyCode::Digit7,
        7 => KeyCode::Digit8,
        8 => KeyCode::Digit9,
        _ => return None,
    })
}

/// The AI's choice for one (non-player) actor: its lowest-id usable move, else wait.
fn choose_action(sim: &Sim, id: EntityId) -> Action {
    usable_moves(sim, id).first().copied().map(Action::Use).unwrap_or(Action::Wait)
}

/// Map an engine end-reason onto the combat overlay's outcome phase. The player is always side 0
/// (see [`engine::content::arena`]).
fn combat_outcome(reason: EndReason) -> CombatState {
    match reason {
        EndReason::Victory(side) if side.0 == 0 => CombatState::Victory,
        EndReason::Victory(_) => CombatState::Defeat, // a hostile side was last standing
        EndReason::Draw => CombatState::Defeat,        // mutual KO — the player's side fell too
        EndReason::TickCap => CombatState::Escape,      // stalemate timed out → no contest
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// AI choices for every ready actor (for the headless, no-input run).
    fn ai_choices(sim: &Sim) -> Vec<(EntityId, Action)> {
        let ids = match sim.pending_decision() {
            Some(Decision::Neutral(ids)) => ids,
            Some(Decision::Pressure(id)) => vec![id],
            None => return Vec::new(),
        };
        ids.into_iter().map(|id| (id, choose_action(sim, id))).collect()
    }

    /// Drive a fight to completion with the AI on both sides — the headless loop, no Bevy.
    fn run_to_outcome(mut sim: Sim) -> CombatState {
        loop {
            match sim.advance() {
                Outcome::Decision(_) => {
                    let choices = ai_choices(&sim);
                    sim.commit(&choices);
                }
                Outcome::Ended(reason) => return combat_outcome(reason),
            }
        }
    }

    #[test]
    fn a_fight_runs_headless_to_an_outcome() {
        let outcome = run_to_outcome(build_sim(None));
        assert!(matches!(
            outcome,
            CombatState::Victory | CombatState::Defeat | CombatState::Escape
        ));
    }

    /// Regression for the reported "fight runs without asking for input" lockout: drive the *actual*
    /// in-game matchup with the player doing the worst possible thing (always `Wait`, never defend),
    /// and confirm the player is still offered a decision **after it has taken damage** — i.e. it
    /// recovers from stun rather than being locked out for the rest of the fight. This is the strong
    /// form: counting only pre-hit decisions hid the engine recovery bug; requiring a post-hit
    /// decision exercises it directly.
    #[test]
    fn player_is_never_locked_out_after_being_hit() {
        let mut sim = build_sim(None);
        let full_hp = sim.entities[0].health;
        let mut asked_after_damage = false;
        let mut steps = 0u32;
        loop {
            match sim.advance() {
                Outcome::Decision(d) => {
                    let player_hurt = sim.entities[0].health < full_hp;
                    let ids = match &d {
                        Decision::Neutral(ids) => ids.clone(),
                        Decision::Pressure(i) => vec![*i],
                    };
                    let choices: Vec<_> = ids
                        .into_iter()
                        .map(|id| {
                            if sim.entities[id].side.0 == PLAYER_SIDE {
                                if player_hurt {
                                    asked_after_damage = true;
                                }
                                (id, Action::Wait) // worst case: the player never defends
                            } else {
                                (id, choose_action(&sim, id))
                            }
                        })
                        .collect();
                    sim.commit(&choices);
                    steps += 1;
                    assert!(steps < 100_000, "fight did not terminate");
                }
                Outcome::Ended(_) => break,
            }
        }
        assert!(
            asked_after_damage,
            "player never got a turn after taking a hit — locked out by the stun-recovery bug"
        );
    }

    #[test]
    fn player_input_selects_a_move_and_ai_fills_the_rest() {
        let mut sim = build_sim(None);
        let _ = sim.advance(); // reach the opening neutral decision (both ready)

        // No key pressed yet → the turn waits (no commit).
        let idle = ButtonInput::<KeyCode>::default();
        assert!(collect_choices(&idle, &sim).is_none());

        // Press "1" → the player (entity 0) uses its first move; the foe gets an AI action.
        let mut input = ButtonInput::<KeyCode>::default();
        input.press(KeyCode::Digit1);
        let choices = collect_choices(&input, &sim).expect("a key was pressed → resolved");
        assert!(matches!(
            choices.iter().find(|(id, _)| *id == 0),
            Some((0, Action::Use(_)))
        ));
        assert!(choices.iter().any(|(id, _)| *id != 0)); // foe choice present
    }
}
