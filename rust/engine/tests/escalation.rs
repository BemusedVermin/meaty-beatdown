//! Phase 5 escalation tests: Heat, Rage, EX/supers, missiles, beams, and hazards.

mod common;

use common::*;
use engine::combat::agents::{Agent, ReadProfile, fallback};
use engine::combat::sim::{CombatSim, EntitySetup, SimConfig, SimStatus};
use engine::core::fx::{FxVec2, fx};
use engine::core::ids::{EntityId, SideId};
use engine::core::tick::Tick;
use engine::data::{ArenaDef, HazardSpec, HazardTrigger, Reaction, Walls};
use engine::trace::TraceEvent;
use std::collections::BTreeMap;

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

fn sim_with(entities: Vec<EntitySetup>, arena: ArenaDef, max_ticks: u64) -> CombatSim {
    CombatSim::new(SimConfig {
        arena,
        ruleset: ruleset(),
        entities,
        max_ticks,
        knowledge: BTreeMap::new(),
    })
}

fn open_arena() -> ArenaDef {
    ArenaDef {
        half_extents: FxVec2::new(fx(12), fx(8)),
        walls: Walls::default(),
        hazards: vec![],
    }
}

#[test]
fn heat_burst_latches_and_unlocks_heat_only_moves() {
    let mut sim = duel(150);
    run(
        &mut sim,
        &Script::new([
            (
                0,
                A,
                engine::combat::schedule::Choice::Move { id: HEAT_BURST },
            ),
            (
                12,
                A,
                engine::combat::schedule::Choice::Move { id: HEAT_CLEAVE },
            ),
        ]),
        40,
    );
    assert!(sim.trace().iter().any(|e| {
        matches!(e, TraceEvent::HeatStarted { actor, until, .. } if *actor == A && until.0 == 300)
    }));
    assert!(sim.trace().iter().any(|e| {
        matches!(e, TraceEvent::Contact { attacker, mv, .. } if *attacker == A && *mv == HEAT_CLEAVE)
    }));
}

#[test]
fn heat_engager_latches_on_clean_hit() {
    let mut sim = duel(150);
    run(
        &mut sim,
        &Script::new([(
            0,
            A,
            engine::combat::schedule::Choice::Move { id: HEAT_ENGAGER },
        )]),
        30,
    );
    assert!(sim.trace().iter().any(|e| {
        matches!(e, TraceEvent::HeatStarted { t, actor, .. } if t.0 == 9 && *actor == A)
    }));
}

#[test]
fn rage_latches_per_actor_and_rage_art_is_once_only() {
    let mut frail = defense();
    frail.hp_max = 200;
    frail.rage_threshold_hp = 150;
    let mut sim = sim_with(
        vec![
            EntitySetup {
                id: A,
                side: SIDE_A,
                pos: FxVec2::new(fx(-1), fx(0)),
                target: B,
                ready_at: Tick::ZERO,
                defense: frail,
                moves: kit(),
            },
            setup(B, SIDE_B, 100, 0, A),
        ],
        open_arena(),
        120,
    );
    run(
        &mut sim,
        &Script::new([
            (
                0,
                B,
                engine::combat::schedule::Choice::Move { id: MID_POKE },
            ),
            (
                32,
                A,
                engine::combat::schedule::Choice::Move { id: RAGE_ART },
            ),
        ]),
        70,
    );
    assert!(
        sim.trace()
            .iter()
            .any(|e| matches!(e, TraceEvent::RageStarted { actor, .. } if *actor == A))
    );
    assert!(sim.debug_entity(A).unwrap().rage_art_used);
    assert!(!sim.debug_entity(A).unwrap().rage);
    assert!(sim.trace().iter().any(|e| {
        matches!(e, TraceEvent::Contact { attacker, mv, .. } if *attacker == A && *mv == RAGE_ART)
    }));
}

#[test]
fn missile_spawns_as_independent_entity_and_hits_later() {
    let mut sim = duel(400);
    run(
        &mut sim,
        &Script::new([(
            0,
            A,
            engine::combat::schedule::Choice::Move { id: FIREBALL },
        )]),
        30,
    );
    assert!(sim.trace().iter().any(|e| {
        matches!(e, TraceEvent::ProjectileSpawned { t, owner, source, .. }
            if t.0 == 6 && *owner == A && *source == FIREBALL)
    }));
    assert!(sim.trace().iter().any(|e| {
        matches!(e, TraceEvent::ProjectileContact { attacker, victim, source, .. }
            if *attacker == A && *victim == B && *source == FIREBALL)
    }));
}

#[test]
fn opposing_missiles_annihilate_on_overlap() {
    let mut moves = kit();
    for m in moves.iter_mut().filter(|m| m.id == FIREBALL) {
        let spec = m.flags.projectile.as_mut().unwrap();
        spec.speed = fxf(50, 100);
        spec.half_len = fx(1);
        spec.half_width = fx(1);
    }
    let mut sim = sim_with(
        vec![
            EntitySetup {
                moves: moves.clone(),
                ..setup(A, SIDE_A, -200, 0, B)
            },
            EntitySetup {
                moves,
                ..setup(B, SIDE_B, 200, 0, A)
            },
        ],
        open_arena(),
        60,
    );
    run(
        &mut sim,
        &Script::new([
            (
                0,
                A,
                engine::combat::schedule::Choice::Move { id: FIREBALL },
            ),
            (
                0,
                B,
                engine::combat::schedule::Choice::Move { id: FIREBALL },
            ),
        ]),
        30,
    );
    assert!(
        sim.trace()
            .iter()
            .any(|e| matches!(e, TraceEvent::ProjectileClashed { .. }))
    );
}

#[test]
fn hazard_trigger_archetypes_are_authored_per_volume() {
    let arena = ArenaDef {
        half_extents: FxVec2::new(fx(12), fx(8)),
        walls: Walls::default(),
        hazards: vec![
            HazardSpec {
                id: 1,
                center: FxVec2::new(fx(-1), fx(0)),
                half_extents: FxVec2::new(fxf(50, 100), fxf(50, 100)),
                trigger: HazardTrigger::Once,
                damage: 1,
                reaction: None,
                affects_allies: true,
            },
            HazardSpec {
                id: 2,
                center: FxVec2::new(fx(-1), fx(0)),
                half_extents: FxVec2::new(fxf(50, 100), fxf(50, 100)),
                trigger: HazardTrigger::Cooldown { ticks: 3 },
                damage: 1,
                reaction: None,
                affects_allies: true,
            },
            HazardSpec {
                id: 3,
                center: FxVec2::new(fx(-1), fx(0)),
                half_extents: FxVec2::new(fxf(50, 100), fxf(50, 100)),
                trigger: HazardTrigger::Always,
                damage: 1,
                reaction: Some(Reaction::Hitstun { ticks: 1 }),
                affects_allies: true,
            },
        ],
    };
    let mut sim = sim_with(
        vec![setup(A, SIDE_A, -100, 0, B), setup(B, SIDE_B, 300, 0, A)],
        arena,
        20,
    );
    run(
        &mut sim,
        &Script::new([(0, A, engine::combat::schedule::Choice::Wait { ticks: 10 })]),
        8,
    );
    let count = |id| {
        sim.trace()
            .iter()
            .filter(|e| matches!(e, TraceEvent::HazardTriggered { hazard, .. } if *hazard == id))
            .count()
    };
    assert_eq!(count(1), 1);
    assert!(count(2) >= 2);
    assert!(count(3) >= 3);
}

#[test]
fn ai_trace_contains_escalation_arc_and_governors_still_hold() {
    let mut sim = duel(150);
    let mut agents = BTreeMap::from([
        (SIDE_A, Agent::new(ReadProfile::Aggressive, 100)),
        (SIDE_B, Agent::new(ReadProfile::Gambler, 200)),
    ]);
    let mut guard = 0u32;
    loop {
        guard += 1;
        assert!(guard < 100_000, "AI escalation fight must not deadlock");
        match sim.advance() {
            SimStatus::Over { .. } => break,
            SimStatus::AwaitingDecisions => {
                for p in sim.pending() {
                    let scripted = match (sim.tick().0, p.actor) {
                        (0, A) => Some(engine::combat::schedule::Choice::Move { id: MID_POKE }),
                        (30, A) => Some(engine::combat::schedule::Choice::Move { id: HEAT_BURST }),
                        (42, A) => Some(engine::combat::schedule::Choice::Move { id: BEAM_SUPER }),
                        _ => None,
                    };
                    let pick = scripted.unwrap_or_else(|| {
                        let obs = sim.observe(p.side);
                        agents
                            .get_mut(&p.side)
                            .map_or(fallback(p.kind), |a| a.decide(&obs, &p))
                    });
                    if sim.commit_side(p.side, &[(p.actor, pick)]).is_err() {
                        sim.commit_side(p.side, &[(p.actor, fallback(p.kind))])
                            .expect("fallback is legal");
                    }
                }
            }
        }
        if sim.tick().0 > 120 {
            break;
        }
    }
    assert!(
        sim.trace()
            .iter()
            .any(|e| matches!(e, TraceEvent::HeatStarted { .. }))
    );
    assert!(
        sim.trace()
            .iter()
            .any(|e| matches!(e, TraceEvent::Contact { mv, .. } if *mv == BEAM_SUPER))
    );
    let bound = engine::content::audit::audit(&kit(), &ruleset()).combo_bound;
    let max_combo = sim
        .trace()
        .iter()
        .filter_map(|e| match e {
            TraceEvent::Contact { combo_hits, .. }
            | TraceEvent::ProjectileContact { combo_hits, .. } => Some(*combo_hits),
            _ => None,
        })
        .max()
        .unwrap_or(0);
    assert!(max_combo <= bound);
}
