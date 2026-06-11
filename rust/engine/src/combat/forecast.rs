//! The honest forecast (spec §7.4) as PROJECTION-REPLAY (tech-plan §3): the prediction
//! is the REAL sim run on a state built only from an Observation — one simulation
//! codebase, zero reimplementation drift, and the forecast physically cannot use hidden
//! facts because the projection type doesn't contain them.
//!
//! Unknowns are held at last-observed values: enemies are statues frozen in their
//! observed posture (their in-flight moves contribute NOTHING — the forecast is exact
//! about your move and silent about their intent; the gap between those two is the
//! game). A visibly blocking enemy is projected guarding the standard mask for their
//! stance. A committed enemy projects as non-counter-hittable: where reality would CH,
//! the forecast under-promises rather than peeking.
//!
//! Phase 3 scope: forecasts at Ready decision points (composing a commitment). Cancel
//! and break-window forecasts join with the timeline-ribbon UI work (Phase 7).

use crate::core::ids::EntityId;
use crate::core::tick::Tick;
use crate::data::movedef::HeightMask;
use crate::data::{DefenseProfile, MeterVisibility, Reaction, StanceKind, StanceSpec};
use crate::trace::TraceEvent;
use serde::{Deserialize, Serialize};

use super::entity::{ActorState, ComboTracker, Entity, Stance};
use super::observe::{Observation, StateClass};
use super::resolve::ContactOutcome;
use super::schedule::Choice;
use super::sim::{CombatSim, SimStatus};

/// A projected touch of the composed move.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectedContact {
    pub t: Tick,
    pub victim: EntityId,
    pub outcome: ContactOutcome,
    pub damage: u32,
    pub reaction: Option<Reaction>,
}

/// What the engine projects for one composed choice, against the world as last observed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Forecast {
    /// Every projected contact of the composing actor's move, in order.
    pub contacts: Vec<ProjectedContact>,
    /// When the composing actor is next actionable on the projection (frame advantage
    /// derives from this against each contact's reaction — I-1: derived, never stored).
    pub free_at: Tick,
}

/// Statues never act: a ready tick no fight reaches.
const NEVER: Tick = Tick(u64::MAX / 2);

fn statue_defense(hp: u32) -> DefenseProfile {
    DefenseProfile {
        hp_max: hp.max(1),
        guard_max: u32::MAX / 2,
        guard_regen_interval: u32::MAX / 2,
        weight: crate::core::fx::Fx::ONE,
        breath_max: 0,
        breath_regen_interval: u32::MAX / 2,
        ap_max: 0,
        focus_max: 0,
        heat_duration: 0,
        rage_threshold_hp: 0,
        rage_damage_mult: crate::core::fx::Fx::ONE,
        visibility: MeterVisibility::default(),
    }
}

/// Build the projection sim from nothing but an Observation.
fn projection(obs: &Observation) -> CombatSim {
    let mut entities: Vec<Entity> = obs.allies.clone();
    for view in &obs.enemies {
        // Held at last-observed values; no movelist — no intent.
        let hp = view.hp.unwrap_or(u32::MAX / 2);
        let (state, held) = match view.state_class {
            StateClass::Blocking => {
                // The standard guard mask for their visible stance (a projection
                // assumption — their authored coverage is their secret).
                let mask = if view.stance == Stance::Crouching {
                    HeightMask::CROUCHING_GUARD
                } else {
                    HeightMask::STANDING_GUARD
                };
                let kind = if view.stance == Stance::Crouching {
                    StanceKind::Crouching
                } else {
                    StanceKind::Standing
                };
                (
                    ActorState::HoldingStance,
                    Some(StanceSpec {
                        stance: kind,
                        guard: Some(mask),
                    }),
                )
            }
            StateClass::Down => (ActorState::Down { until: NEVER }, None),
            StateClass::Ko => (ActorState::Ko, None),
            // Free / Committed / Reeling / Grabbed: a statue in that posture.
            _ => (ActorState::Free, None),
        };
        entities.push(Entity {
            id: view.id,
            side: view.side,
            pos: view.pos,
            facing: view.facing,
            target: view.target,
            stance: view.stance,
            state,
            ready_tick: NEVER,
            current: None,
            held,
            reevaluate_at: NEVER,
            height_off: view.height_off,
            hp,
            guard: view.guard.unwrap_or(u32::MAX / 2),
            guard_regen_acc: 0,
            breath: view.breath.unwrap_or(0),
            breath_regen_acc: 0,
            ap: view.ap.unwrap_or(0),
            focus: view.focus.unwrap_or(0),
            heat_until: None,
            heat_used: false,
            rage: false,
            rage_art_used: false,
            burst_used: false,
            combo: ComboTracker::default(),
            moves: Vec::new(),
            defense: statue_defense(hp),
        });
    }
    CombatSim::projection(obs.t, entities, obs.arena.clone(), obs.ruleset.clone())
}

/// Project a composed Ready choice (spec §7.4): run the real engine on the
/// Observation-only world and report what the move does.
#[must_use]
pub fn forecast(obs: &Observation, actor: EntityId, choice: Choice) -> Forecast {
    let mut sim = projection(obs);
    let horizon = match (&choice, obs.ally(actor)) {
        (Choice::Move { id } | Choice::MoveAt { id, .. }, Some(me)) => me
            .moves
            .iter()
            .find(|m| m.id == *id)
            .map(|m| u64::from(m.timing.total()) + 2)
            .unwrap_or(2),
        _ => 2,
    };
    let stop_at = obs.t + horizon;
    let mut committed = false;
    loop {
        match sim.advance() {
            SimStatus::Over { .. } => break,
            SimStatus::AwaitingDecisions => {
                if sim.tick() >= stop_at {
                    break;
                }
                for p in sim.pending() {
                    let c = if p.actor == actor && !committed {
                        committed = true;
                        choice
                    } else {
                        // Other allies hold still on the projection: the forecast is
                        // about THIS commitment.
                        match p.kind {
                            super::schedule::DecisionKind::Ready => Choice::Wait {
                                ticks: u32::try_from(horizon).unwrap_or(u32::MAX),
                            },
                            super::schedule::DecisionKind::StanceReevaluate => Choice::HoldStance,
                            super::schedule::DecisionKind::ThrowBreak { .. } => {
                                Choice::ThrowBreak { guess: None }
                            }
                            super::schedule::DecisionKind::Cancel => Choice::Cancel { into: None },
                            super::schedule::DecisionKind::WakeUp => Choice::Rise,
                            super::schedule::DecisionKind::Burst => Choice::Wait { ticks: 1 },
                        }
                    };
                    let _ = sim.commit_side(p.side, &[(p.actor, c)]);
                }
            }
        }
    }
    let contacts: Vec<ProjectedContact> = sim
        .trace()
        .iter()
        .filter_map(|e| match e {
            TraceEvent::Contact {
                t,
                attacker,
                victim,
                outcome,
                damage,
                reaction,
                ..
            } if *attacker == actor => Some(ProjectedContact {
                t: *t,
                victim: *victim,
                outcome: *outcome,
                damage: *damage,
                reaction: *reaction,
            }),
            _ => None,
        })
        .collect();
    let free_at = sim
        .debug_entity(actor)
        .map(|e| {
            if e.state == ActorState::Free {
                e.ready_tick
            } else {
                stop_at
            }
        })
        .unwrap_or(stop_at);
    Forecast { contacts, free_at }
}
