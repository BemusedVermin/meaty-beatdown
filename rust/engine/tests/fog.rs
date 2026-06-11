//! Phase 3 fog-honesty suite (the exit criterion): no agent or forecast can distinguish
//! two TRUE states that project to the same Observation. Plus the knowledge gradient
//! (what T2/T3 deliberately reveal), event-log privacy, forecast honesty, and
//! AI-vs-AI fights that complete and terminate on Observation alone.

mod common;

use common::*;
use engine::combat::agents::{Agent, ReadProfile, fallback};
use engine::combat::schedule::Choice;
use engine::combat::sim::{CombatSim, SimStatus};
use engine::core::ids::SideId;
use engine::data::{KnowledgeBook, KnowledgeTier, MoveId, ThrowBreakKey};
use engine::trace::TraceEvent;
use std::collections::BTreeMap;

fn obs_json(sim: &CombatSim, side: SideId) -> String {
    serde_json::to_string(&sim.observe(side)).expect("observation serializes")
}

/// Drive a duel to A's T4 decision with B having committed `b_move` at T0 (A waited).
fn b_committed(b_move: MoveId, knowledge_for_a: Option<KnowledgeBook>) -> CombatSim {
    let mut knowledge = BTreeMap::new();
    if let Some(book) = knowledge_for_a {
        knowledge.insert(SIDE_A, book);
    }
    let mut sim = duel_knowing(-100, 100, knowledge);
    let script = Script::new([
        (0, A, Choice::Wait { ticks: 4 }),
        (0, B, Choice::Move { id: b_move }),
    ]);
    run(&mut sim, &script, 3); // stop at A's T4 prompt
    assert_eq!(sim.tick().0, 4, "paused at A's decision with B mid-startup");
    sim
}

// ── THE EXIT CRITERION: indistinguishability under the fog ──────────────────

/// Spec §7.2/§14: the poke and the sweep share the "low coil" cue. At T0 knowledge the
/// two TRUE states must project to byte-identical Observations — and therefore to
/// identical agent decisions and identical forecasts. The read is a real guess.
#[test]
fn same_cue_commitments_are_indistinguishable_at_t0() {
    let poke_world = b_committed(MID_POKE, None);
    let sweep_world = b_committed(SWEEP, None);
    assert_eq!(
        obs_json(&poke_world, SIDE_A),
        obs_json(&sweep_world, SIDE_A),
        "two different commitments behind one cue must observe identically"
    );

    // Same observation -> same agent choice (any profile, same seed)...
    for profile in [
        ReadProfile::Aggressive,
        ReadProfile::Turtle,
        ReadProfile::Gambler,
        ReadProfile::StepHappy,
    ] {
        let prompt = poke_world.pending()[0];
        let c1 = Agent::new(profile, 7).decide(&poke_world.observe(SIDE_A), &prompt);
        let c2 = Agent::new(profile, 7).decide(&sweep_world.observe(SIDE_A), &prompt);
        assert_eq!(c1, c2, "{profile:?} acted on hidden state");
    }

    // ...and the same forecast (it takes nothing but the Observation).
    let f1 = engine::combat::forecast::forecast(
        &poke_world.observe(SIDE_A),
        A,
        Choice::Move { id: JAB },
    );
    let f2 = engine::combat::forecast::forecast(
        &sweep_world.observe(SIDE_A),
        A,
        Choice::Move { id: JAB },
    );
    assert_eq!(f1, f2);
}

/// Hidden content stays hidden: two fighters with different (invisible) guard pools
/// project identically.
#[test]
fn hidden_meters_do_not_distinguish() {
    let build = |guard_max: u32| {
        let mut moves_b = defense();
        moves_b.guard_max = guard_max;
        let mut sim = engine::combat::sim::CombatSim::new(engine::combat::sim::SimConfig {
            arena: engine::data::ArenaDef {
                half_extents: engine::core::fx::FxVec2::new(
                    engine::core::fx::fx(10),
                    engine::core::fx::fx(6),
                ),
                walls: engine::data::Walls::default(),
                hazards: vec![],
            },
            ruleset: ruleset(),
            entities: vec![
                engine::combat::sim::EntitySetup {
                    id: A,
                    side: SIDE_A,
                    pos: engine::core::fx::FxVec2::new(
                        engine::core::fx::fx(-1),
                        engine::core::fx::fx(0),
                    ),
                    target: B,
                    ready_at: engine::core::tick::Tick::ZERO,
                    defense: defense(),
                    moves: kit(),
                },
                engine::combat::sim::EntitySetup {
                    id: B,
                    side: SIDE_B,
                    pos: engine::core::fx::FxVec2::new(
                        engine::core::fx::fx(1),
                        engine::core::fx::fx(0),
                    ),
                    target: A,
                    ready_at: engine::core::tick::Tick::ZERO,
                    defense: moves_b,
                    moves: kit(),
                },
            ],
            max_ticks: 600,
            knowledge: BTreeMap::new(),
        });
        sim.advance();
        sim
    };
    let thick = build(50);
    let thin = build(10);
    assert_eq!(obs_json(&thick, SIDE_A), obs_json(&thin, SIDE_A));
}

// ── the knowledge gradient: what T2/T3 DELIBERATELY reveal (spec §7.3) ──────

#[test]
fn studied_knowledge_overlays_the_candidate_set_but_keeps_the_guess() {
    let book = KnowledgeBook::uniform([MID_POKE, SWEEP], KnowledgeTier::Studied);
    let poke_world = b_committed(MID_POKE, Some(book.clone()));
    let sweep_world = b_committed(SWEEP, Some(book));

    // Both candidates appear on the cue...
    let obs = poke_world.observe(SIDE_A);
    let cue = obs.enemy(B).unwrap().cue.as_ref().expect("B is committed");
    assert_eq!(
        cue.candidates,
        vec![MID_POKE, SWEEP],
        "the studied candidate set"
    );
    assert!(cue.exact.is_none(), "no exact readout below T3");

    // ...and the states STILL observe identically: sharpened, not solved.
    assert_eq!(
        obs_json(&poke_world, SIDE_A),
        obs_json(&sweep_world, SIDE_A)
    );
}

#[test]
fn mastery_reads_the_exact_ticks_and_legitimately_distinguishes() {
    let book = KnowledgeBook::uniform([MID_POKE, SWEEP], KnowledgeTier::Mastered);
    let poke_world = b_committed(MID_POKE, Some(book.clone()));
    let sweep_world = b_committed(SWEEP, Some(book));

    let obs = poke_world.observe(SIDE_A);
    let cue = obs.enemy(B).unwrap().cue.as_ref().expect("B is committed");
    let exact = cue.exact.expect("T3 reads the animation frame-perfectly");
    assert_eq!(exact.elapsed, 4);
    assert_eq!(exact.remaining, 26, "mid poke: 30 total - 4 elapsed");

    // The mastered readout differs between the two moves: the deliberate reveal.
    assert_ne!(
        obs_json(&poke_world, SIDE_A),
        obs_json(&sweep_world, SIDE_A)
    );
}

#[test]
fn break_keys_reveal_only_at_t3() {
    // Same lunging cue, opposite break keys: a pure guess until mastery.
    let l_world = b_committed(THROW_L, None);
    let r_world = b_committed(THROW_R, None);
    assert_eq!(obs_json(&l_world, SIDE_A), obs_json(&r_world, SIDE_A));

    let book = KnowledgeBook::uniform([THROW_L, THROW_R], KnowledgeTier::Mastered);
    let l_known = b_committed(THROW_L, Some(book.clone()));
    let obs = l_known.observe(SIDE_A);
    let cue = obs.enemy(B).unwrap().cue.as_ref().unwrap();
    assert_eq!(
        cue.break_key,
        Some(ThrowBreakKey::L),
        "studied opponents get their throws broken (spec §5.4)"
    );
}

// ── event-log privacy (spec §7.1): facts public, intent never ───────────────

#[test]
fn enemy_commitments_never_appear_in_observed_events() {
    let mut sim = duel(200);
    let script = Script::new([
        (0, A, Choice::Move { id: MID_POKE }),
        (0, B, Choice::Move { id: SIDESTEP_L }),
        (13, B, Choice::Move { id: MID_POKE }),
    ]);
    run(&mut sim, &script, 60);

    for (side, foe) in [(SIDE_A, B), (SIDE_B, A)] {
        let obs = sim.observe(side);
        assert!(
            !obs.events
                .iter()
                .any(|e| matches!(e, TraceEvent::Committed { actor, .. } if *actor == foe)),
            "enemy intent leaked into {side:?}'s event log"
        );
        // Resolved facts ARE there, exact and permanent.
        assert!(
            obs.events
                .iter()
                .any(|e| matches!(e, TraceEvent::Contact { .. }))
        );
    }
    // The full trace (the replay contract) still records everything.
    assert!(
        sim.trace()
            .iter()
            .any(|e| matches!(e, TraceEvent::Committed { actor, .. } if *actor == B))
    );
}

// ── the forecast (spec §7.4): exact about your move, silent about theirs ────

#[test]
fn forecast_matches_reality_when_the_world_holds_still() {
    let mut sim = duel(100);
    sim.advance();
    let obs = sim.observe(SIDE_A);
    let projected = engine::combat::forecast::forecast(&obs, A, Choice::Move { id: MID_POKE });
    assert_eq!(projected.contacts.len(), 1);
    let p = &projected.contacts[0];
    assert_eq!((p.t.0, p.victim, p.damage), (12, B, 60));

    // Reality, with B actually holding still: identical contact.
    let script = Script::new([
        (0, A, Choice::Move { id: MID_POKE }),
        (0, B, Choice::Wait { ticks: 30 }),
    ]);
    run(&mut sim, &script, 40);
    assert!(sim.trace().iter().any(|e| matches!(
        e,
        TraceEvent::Contact { t, victim, damage, .. }
            if t.0 == 12 && *victim == B && *damage == 60
    )));
}

#[test]
fn forecast_projects_the_visible_guard() {
    let mut sim = duel(100);
    let script = Script::new([
        (0, A, Choice::Wait { ticks: 4 }),
        (0, B, Choice::Move { id: STAND_GUARD }),
    ]);
    run(&mut sim, &script, 3);
    let obs = sim.observe(SIDE_A);
    let projected = engine::combat::forecast::forecast(&obs, A, Choice::Move { id: MID_POKE });
    assert_eq!(projected.contacts.len(), 1);
    assert!(
        matches!(
            projected.contacts[0].outcome,
            engine::combat::resolve::ContactOutcome::Blocked
        ),
        "a visibly guarding enemy projects as blocking"
    );
}

// ── the AI contract (spec §7.5): Observation-only fights complete ───────────

fn ai_fight(pa: ReadProfile, pb: ReadProfile, seed: u64) -> CombatSim {
    let mut sim = duel(180);
    let mut agent_a = Agent::new(pa, seed);
    let mut agent_b = Agent::new(pb, seed ^ 0x9E37_79B9);
    let mut guard = 0u32;
    loop {
        guard += 1;
        assert!(guard < 100_000, "AI fights must terminate");
        match sim.advance() {
            SimStatus::Over { .. } => return sim,
            SimStatus::AwaitingDecisions => {
                for p in sim.pending() {
                    let obs = sim.observe(p.side);
                    let agent = if p.side == SIDE_A {
                        &mut agent_a
                    } else {
                        &mut agent_b
                    };
                    let pick = agent.decide(&obs, &p);
                    if sim.commit_side(p.side, &[(p.actor, pick)]).is_err() {
                        sim.commit_side(p.side, &[(p.actor, fallback(p.kind))])
                            .expect("fallback is always legal");
                    }
                }
            }
        }
    }
}

#[test]
fn ai_vs_ai_fights_complete_for_every_profile_pairing() {
    let profiles = [
        ReadProfile::Aggressive,
        ReadProfile::Turtle,
        ReadProfile::Gambler,
        ReadProfile::StepHappy,
    ];
    for (i, &pa) in profiles.iter().enumerate() {
        for (j, &pb) in profiles.iter().enumerate() {
            let seed = 1000 + (i * 4 + j) as u64;
            let sim = ai_fight(pa, pb, seed);
            assert!(
                matches!(sim.trace().last(), Some(TraceEvent::SimEnded { .. })),
                "{pa:?} vs {pb:?} did not end"
            );
        }
    }
}

#[test]
fn ai_fights_are_deterministic_per_seed() {
    let one = ai_fight(ReadProfile::Aggressive, ReadProfile::Turtle, 42);
    let two = ai_fight(ReadProfile::Aggressive, ReadProfile::Turtle, 42);
    assert_eq!(
        serde_json::to_string(one.trace()).unwrap(),
        serde_json::to_string(two.trace()).unwrap()
    );
}
