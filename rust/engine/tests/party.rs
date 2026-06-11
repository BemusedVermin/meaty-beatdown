//! Phase 4 party-combat tests: N-per-side scheduling, target switching, bystander
//! hitboxes, facing-relative defense, KO/revive, Burst, Rescue, and 3v3 termination.

mod common;

use common::*;
use engine::combat::agents::{Agent, ReadProfile, fallback};
use engine::combat::entity::ActorState;
use engine::combat::resolve::ContactOutcome;
use engine::combat::schedule::Choice;
use engine::combat::sim::{CombatSim, EntitySetup, SimConfig, SimStatus};
use engine::core::fx::{FxVec2, fx};
use engine::core::ids::{EntityId, SideId};
use engine::core::tick::Tick;
use engine::data::{ArenaDef, Walls};
use engine::trace::TraceEvent;
use std::collections::BTreeMap;

const C: EntityId = EntityId(3);
const D: EntityId = EntityId(4);
const E: EntityId = EntityId(5);
const F: EntityId = EntityId(6);

fn setup(id: EntityId, side: SideId, x: i32, y: i32, target: EntityId) -> EntitySetup {
    EntitySetup {
        id,
        side,
        pos: FxVec2::new(fxf(x, 100), fxf(y, 100)),
        target,
        ready_at: Tick::ZERO,
        defense: defense(),
        moves: kit(),
    }
}

fn party(entities: Vec<EntitySetup>, max_ticks: u64) -> CombatSim {
    CombatSim::new(SimConfig {
        arena: ArenaDef {
            half_extents: FxVec2::new(fx(12), fx(8)),
            walls: Walls::default(),
            hazards: vec![],
        },
        ruleset: ruleset(),
        entities,
        max_ticks,
        knowledge: BTreeMap::new(),
    })
}

fn contacts(sim: &CombatSim) -> Vec<(u64, u32, u32, engine::data::MoveId, ContactOutcome)> {
    sim.trace()
        .iter()
        .filter_map(|e| match e {
            TraceEvent::Contact {
                t,
                attacker,
                victim,
                mv,
                outcome,
                ..
            } => Some((t.0, attacker.0, victim.0, *mv, *outcome)),
            _ => None,
        })
        .collect()
}

#[test]
fn scripted_2v2_runs_the_pre_heat_section_14_beats() {
    let mut sim = party(
        vec![
            setup(A, SIDE_A, -100, 0, B),
            setup(C, SIDE_A, -100, -220, D),
            setup(B, SIDE_B, 100, 0, A),
            setup(D, SIDE_B, 100, -220, C),
        ],
        240,
    );
    let script = Script::new([
        // T0: all four actors commit side-blind. Reza waits for the cue, Borin is
        // exposed, the Duelist sweeps, and the Brute starts a launcher.
        (0, A, Choice::Wait { ticks: 4 }),
        (0, C, Choice::Wait { ticks: 40 }),
        (0, B, Choice::Move { id: SWEEP }),
        (0, D, Choice::Move { id: LAUNCHER }),
        // Reza reads the linear sweep and vacates the lane.
        (4, A, Choice::Move { id: SIDESTEP_L }),
        // The ally is launched at T15, so the Rescue gate opens for Reza's next decision.
        (
            17,
            A,
            Choice::MoveAt {
                id: RESCUE_STRIKE,
                target: D,
            },
        ),
    ]);

    run(&mut sim, &script, 80);
    let cs = contacts(&sim);
    assert!(
        !cs.iter().any(|(_, attacker, victim, mv, _)| {
            *attacker == B.0 && *victim == A.0 && *mv == SWEEP
        }),
        "the Duelist sweep should whiff through the sidestep"
    );
    assert!(
        cs.iter()
            .any(|c| { matches!(c, (15, 4, 3, _, ContactOutcome::Hit { counter: false })) })
    );
    assert!(sim.trace().iter().any(|e| {
        matches!(
            e,
            TraceEvent::Contact {
                t,
                attacker,
                victim,
                reaction: Some(engine::data::Reaction::Launch { .. }),
                ..
            } if t.0 == 15 && *attacker == D && *victim == C
        )
    }));
    assert!(
        cs.iter()
            .any(|c| { matches!(c, (23, 1, 4, _, ContactOutcome::Hit { counter: true })) })
    );
    assert!(
        sim.trace().iter().any(|e| {
            matches!(
                e,
                TraceEvent::Contact {
                    t,
                    attacker,
                    victim,
                    outcome: ContactOutcome::Hit { counter: true },
                    ..
                } if t.0 == 23 && *attacker == A && *victim == D
            )
        }),
        "the rescuer counter-hits the comboer with no bespoke interruption path"
    );
}

#[test]
fn wide_hits_clip_bystanders_but_not_allies_by_default() {
    let mut sim = party(
        vec![
            setup(A, SIDE_A, 0, 0, B),
            setup(C, SIDE_A, 120, 50, B),
            setup(B, SIDE_B, 180, 0, A),
            setup(D, SIDE_B, 190, 90, A),
        ],
        80,
    );
    run(
        &mut sim,
        &Script::new([(0, A, Choice::Move { id: WIDE_CLEAVE })]),
        40,
    );
    let victims: Vec<u32> = contacts(&sim)
        .into_iter()
        .filter(|(_, attacker, _, mv, _)| *attacker == A.0 && *mv == WIDE_CLEAVE)
        .map(|(_, _, victim, _, _)| victim)
        .collect();
    assert_eq!(victims, vec![B.0, D.0]);
    assert_eq!(sim.debug_entity(C).unwrap().hp, 1000);
}

#[test]
fn back_hits_bypass_a_visible_guard() {
    let mut sim = party(
        vec![
            setup(A, SIDE_A, -100, 0, B),
            setup(C, SIDE_A, 250, 0, B),
            setup(B, SIDE_B, 50, 0, C),
        ],
        80,
    );
    run(
        &mut sim,
        &Script::new([
            (0, A, Choice::Move { id: MID_POKE }),
            (0, B, Choice::Move { id: STAND_GUARD }),
        ]),
        40,
    );
    assert!(
        contacts(&sim)
            .iter()
            .any(|c| { matches!(c, (12, 1, 2, _, ContactOutcome::Hit { counter: false })) })
    );
    assert_eq!(sim.debug_entity(B).unwrap().hp, 940);
}

#[test]
fn ko_does_not_end_until_full_side_wipe_and_revive_restores_timeline() {
    let mut frail = defense();
    frail.hp_max = 50;
    let mut sim = party(
        vec![
            setup(A, SIDE_A, -100, 0, B),
            EntitySetup {
                id: B,
                side: SIDE_B,
                pos: FxVec2::new(fx(0), fx(0)),
                target: A,
                ready_at: Tick::ZERO,
                defense: frail,
                moves: kit(),
            },
            setup(D, SIDE_B, 80, 200, A),
        ],
        120,
    );
    run(
        &mut sim,
        &Script::new([
            (0, A, Choice::Move { id: MID_POKE }),
            (0, D, Choice::Wait { ticks: 13 }),
            (
                13,
                D,
                Choice::MoveAt {
                    id: REVIVE,
                    target: B,
                },
            ),
        ]),
        60,
    );
    assert!(
        sim.trace()
            .iter()
            .any(|e| matches!(e, TraceEvent::Ko { actor, .. } if *actor == B))
    );
    assert!(sim.trace().iter().any(|e| {
        matches!(e, TraceEvent::Revived { actor, hp, .. } if *actor == B && *hp == 50)
    }));
    assert_ne!(sim.debug_entity(B).unwrap().state, ActorState::Ko);
    assert!(!matches!(
        sim.trace().last(),
        Some(TraceEvent::SimEnded { .. })
    ));
}

#[test]
fn solo_burst_is_a_once_per_fight_reaction_from_combo_states() {
    let mut sim = party(
        vec![setup(A, SIDE_A, -50, 0, B), setup(B, SIDE_B, 50, 0, A)],
        120,
    );
    run(
        &mut sim,
        &Script::new([
            (0, A, Choice::Move { id: MID_POKE }),
            (13, B, Choice::Move { id: BURST }),
        ]),
        40,
    );
    assert!(sim.debug_entity(B).unwrap().burst_used);
    assert!(
        contacts(&sim)
            .iter()
            .any(|c| { matches!(c, (13, 2, 1, _, ContactOutcome::Hit { counter: false })) })
    );
    assert_eq!(sim.debug_entity(B).unwrap().state, ActorState::Free);
}

#[test]
fn ai_3v3_fuzz_terminates_within_tick_bounds() {
    let mut sim = party(
        vec![
            setup(A, SIDE_A, -220, -140, B),
            setup(C, SIDE_A, -220, 0, D),
            setup(E, SIDE_A, -220, 140, F),
            setup(B, SIDE_B, 220, -140, A),
            setup(D, SIDE_B, 220, 0, C),
            setup(F, SIDE_B, 220, 140, E),
        ],
        900,
    );
    let mut agents = BTreeMap::from([
        (SIDE_A, Agent::new(ReadProfile::StepHappy, 10)),
        (SIDE_B, Agent::new(ReadProfile::Gambler, 20)),
    ]);
    let mut guard = 0u32;
    loop {
        guard += 1;
        assert!(guard < 200_000, "3v3 AI fuzz must not deadlock");
        match sim.advance() {
            SimStatus::Over { .. } => break,
            SimStatus::AwaitingDecisions => {
                for p in sim.pending() {
                    let obs = sim.observe(p.side);
                    let pick = agents.get_mut(&p.side).unwrap().decide(&obs, &p);
                    if sim.commit_side(p.side, &[(p.actor, pick)]).is_err() {
                        sim.commit_side(p.side, &[(p.actor, fallback(p.kind))])
                            .expect("fallback is legal");
                    }
                }
            }
        }
    }
    assert!(matches!(sim.trace().last(), Some(TraceEvent::SimEnded { t, .. }) if t.0 <= 900));
}
