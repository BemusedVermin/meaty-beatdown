//! Phase 1 sim tests: one scenario per spec §5 interaction, plus the spec §14 worked
//! example's 1v1 beats (sidestep-whiff into CH whiff-punish; throw breaks both ways) —
//! the phase's exit criteria.

mod common;

use common::*;
use engine::combat::entity::{ActorState, Stance};
use engine::combat::resolve::ContactOutcome;
use engine::combat::schedule::Choice;
use engine::combat::sim::SimStatus;
use engine::data::movedef::ThrowBreakKey;
use engine::trace::{ThrowResolution, TraceEvent};

fn contacts(sim: &engine::combat::sim::CombatSim) -> Vec<(u64, u32, u32, ContactOutcome)> {
    sim.trace()
        .iter()
        .filter_map(|e| match e {
            TraceEvent::Contact {
                t,
                attacker,
                victim,
                outcome,
                ..
            } => Some((t.0, attacker.0, victim.0, *outcome)),
            _ => None,
        })
        .collect()
}

// ── heights & stances (spec §5.2) ───────────────────────────────────────────

#[test]
fn jab_hits_and_derives_advantage() {
    let mut sim = duel(100);
    let script = Script::new([(0, A, Choice::Move { id: JAB })]);
    run(&mut sim, &script, 40);
    // Commit T0, startup 6 -> contact T6.
    assert_eq!(
        contacts(&sim),
        vec![(6, 1, 2, ContactOutcome::Hit { counter: false })]
    );
    assert_eq!(sim.debug_entity(B).unwrap().hp, 970);
    // I-1, derived not stored: attacker free at T16 (6+2+8), victim stunned until T22.
    assert_eq!(sim.debug_entity(B).unwrap().state, ActorState::Free);
}

#[test]
fn high_whiffs_entirely_over_crouch() {
    let mut sim = duel(100);
    let script = Script::new([
        (0, A, Choice::Move { id: JAB }),
        (0, B, Choice::Move { id: CROUCH }),
    ]);
    run(&mut sim, &script, 30);
    // Not blocked — MISSED. No contact of any kind; the attacker ate full recovery.
    assert!(contacts(&sim).is_empty());
    assert_eq!(sim.debug_entity(B).unwrap().hp, 1000);
}

#[test]
fn mid_hits_the_croucher() {
    let mut sim = duel(100);
    let script = Script::new([
        (0, A, Choice::Move { id: MID_POKE }),
        (0, B, Choice::Move { id: CROUCH }),
    ]);
    run(&mut sim, &script, 40);
    assert_eq!(
        contacts(&sim),
        vec![(12, 1, 2, ContactOutcome::Hit { counter: false })]
    );
    assert_eq!(sim.debug_entity(B).unwrap().hp, 940);
}

// ── guard, chip, guard break (spec §5.3) ────────────────────────────────────

#[test]
fn standing_guard_blocks_high_and_chips_guard_not_hp() {
    let mut sim = duel(100);
    let script = Script::new([
        (0, A, Choice::Move { id: JAB }),
        (0, B, Choice::Move { id: STAND_GUARD }),
    ]);
    run(&mut sim, &script, 30);
    assert_eq!(contacts(&sim), vec![(6, 1, 2, ContactOutcome::Blocked)]);
    let b = sim.debug_entity(B).unwrap();
    assert_eq!(b.hp, 1000, "chip never touches HP (spec v2)");
    assert_eq!(b.guard, 50 - 12);
}

#[test]
fn low_goes_under_standing_guard() {
    let mut sim = duel(100);
    let script = Script::new([
        (0, A, Choice::Move { id: SWEEP }),
        (0, B, Choice::Move { id: STAND_GUARD }),
    ]);
    run(&mut sim, &script, 50);
    assert_eq!(
        contacts(&sim),
        vec![(18, 1, 2, ContactOutcome::Hit { counter: false })]
    );
    // Hard knockdown, then auto-rise.
    let b = sim.debug_entity(B).unwrap();
    assert_eq!(b.hp, 950);
}

#[test]
fn crouch_guard_blocks_the_low() {
    let mut sim = duel(100);
    let script = Script::new([
        (0, A, Choice::Move { id: SWEEP }),
        (0, B, Choice::Move { id: CROUCH_GUARD }),
    ]);
    run(&mut sim, &script, 50);
    assert_eq!(contacts(&sim), vec![(18, 1, 2, ContactOutcome::Blocked)]);
    assert_eq!(sim.debug_entity(B).unwrap().guard, 50 - 8);
}

#[test]
fn guard_break_is_the_anti_turtle_terminus() {
    let mut sim = duel(100);
    // Five blocked pokes drain 50 guard (the pushback walks B from 1.0 out to 2.2,
    // still inside the poke's 2.5 reach); the fifth breaks; the punish lands clean
    // inside the long, fully punishable stun.
    // A's decision points land exactly at the poke's 30-tick cycle.
    let script = Script::new([
        (0, B, Choice::Move { id: STAND_GUARD }),
        (0, A, Choice::Move { id: MID_POKE }),
        (30, A, Choice::Move { id: MID_POKE }),
        (60, A, Choice::Move { id: MID_POKE }),
        (90, A, Choice::Move { id: MID_POKE }),
        (120, A, Choice::Move { id: MID_POKE }),
        (150, A, Choice::Move { id: MID_POKE }),
    ]);
    run(&mut sim, &script, 220);
    assert!(
        sim.trace()
            .iter()
            .any(|e| matches!(e, TraceEvent::GuardBroken { actor, .. } if *actor == B)),
        "fifth blocked chip must shatter the guard"
    );
    // The sixth poke meets the broken guard (T132 break, stun until T172): clean hit.
    let last = contacts(&sim).last().copied().unwrap();
    assert_eq!(last, (162, 1, 2, ContactOutcome::Hit { counter: false }));
    assert_eq!(sim.debug_entity(B).unwrap().hp, 940);
}

// ── the §14 beats: sidestep-whiff into CH whiff-punish ──────────────────────

#[test]
fn sidestep_whiffs_the_linear_mid_and_ch_punishes() {
    let mut sim = duel(200);
    let script = Script::new([
        // Side-blind same-tick commits: A swings, B steps off the lane.
        (0, A, Choice::Move { id: MID_POKE }),
        (0, B, Choice::Move { id: SIDESTEP_L }),
        // B is free at T13; A recovers until T30. The whiff punish meets recovery: CH.
        (13, B, Choice::Move { id: MID_POKE }),
    ]);
    run(&mut sim, &script, 80);
    // A's poke never contacts (B vacated the LINEAR band): the only contact is B's
    // punish, a counter-hit, which fires the authored ch_reaction (hard knockdown).
    assert_eq!(
        contacts(&sim),
        vec![(25, 2, 1, ContactOutcome::Hit { counter: true })]
    );
    assert_eq!(
        sim.debug_entity(A).unwrap().hp,
        940,
        "CH override keeps authored damage"
    );
}

#[test]
fn frame_trap_counter_hit_uses_ruleset_default() {
    let mut sim = duel(100);
    let script = Script::new([
        (0, A, Choice::Wait { ticks: 2 }),
        (2, A, Choice::Move { id: JAB }),
        (0, B, Choice::Move { id: MID_POKE }),
    ]);
    run(&mut sim, &script, 40);
    // A's jab (active T8) catches B's startup (T0..T11): CH, no authored override ->
    // Ruleset default: 30 * 1.25 = 37, stun 16+6.
    assert_eq!(
        contacts(&sim),
        vec![(8, 1, 2, ContactOutcome::Hit { counter: true })]
    );
    assert_eq!(sim.debug_entity(B).unwrap().hp, 1000 - 37);
    // B's move was interrupted: its poke never lands.
    assert_eq!(contacts(&sim).len(), 1);
}

// ── throws & the directional break (spec §5.4) ──────────────────────────────

#[test]
fn throw_breaks_on_the_correct_directional_read() {
    let mut sim = duel(80);
    let script = Script::new([
        (0, A, Choice::Move { id: THROW_L }),
        (
            10,
            B,
            Choice::ThrowBreak {
                guess: Some(ThrowBreakKey::L),
            },
        ),
    ]);
    run(&mut sim, &script, 40);
    assert!(sim.trace().iter().any(|e| matches!(
        e,
        TraceEvent::ThrowResolved {
            resolution: ThrowResolution::Teched,
            ..
        }
    )));
    assert_eq!(
        sim.debug_entity(B).unwrap().hp,
        1000,
        "teched throws deal nothing"
    );
    // Both reset with separation: the gap grew past the throw's reach.
    let gap = sim
        .debug_entity(A)
        .unwrap()
        .pos
        .distance(sim.debug_entity(B).unwrap().pos);
    assert!(gap > fxf(90, 100));
}

#[test]
fn wrong_break_guess_eats_the_throw() {
    let mut sim = duel(80);
    let script = Script::new([
        (0, A, Choice::Move { id: THROW_L }),
        (
            10,
            B,
            Choice::ThrowBreak {
                guess: Some(ThrowBreakKey::R),
            },
        ),
    ]);
    run(&mut sim, &script, 90);
    assert!(sim.trace().iter().any(|e| matches!(
        e,
        TraceEvent::ThrowResolved {
            resolution: ThrowResolution::Thrown,
            ..
        }
    )));
    // Connect at T10, then the slam at connect+8 with the authored knockdown.
    assert_eq!(
        contacts(&sim),
        vec![
            (10, 1, 2, ContactOutcome::GrabConnected),
            (18, 1, 2, ContactOutcome::Hit { counter: false })
        ]
    );
    assert_eq!(sim.debug_entity(B).unwrap().hp, 930);
}

#[test]
fn declining_the_break_also_eats_the_throw() {
    let mut sim = duel(80);
    // No scripted break: the driver default declines.
    let script = Script::new([(0, A, Choice::Move { id: THROW_L })]);
    run(&mut sim, &script, 90);
    assert!(sim.trace().iter().any(|e| matches!(
        e,
        TraceEvent::ThrowResolved {
            resolution: ThrowResolution::Thrown,
            ..
        }
    )));
    assert_eq!(sim.debug_entity(B).unwrap().hp, 930);
}

#[test]
fn throws_whiff_on_crouchers() {
    let mut sim = duel(80);
    let script = Script::new([
        (0, A, Choice::Move { id: THROW_L }),
        (0, B, Choice::Move { id: CROUCH }),
    ]);
    run(&mut sim, &script, 40);
    // Standing grab vs croucher: resolver whiff, no prompt, no resolution.
    assert!(
        !sim.trace()
            .iter()
            .any(|e| matches!(e, TraceEvent::ThrowResolved { .. }))
    );
    assert_eq!(sim.debug_entity(B).unwrap().hp, 1000);
}

#[test]
fn mutual_throws_tech_automatically() {
    let mut sim = duel(80);
    let script = Script::new([
        (0, A, Choice::Move { id: THROW_L }),
        (0, B, Choice::Move { id: THROW_R }),
    ]);
    run(&mut sim, &script, 40);
    let techs: Vec<_> = contacts(&sim)
        .into_iter()
        .filter(|c| c.3 == ContactOutcome::ThrowTech)
        .collect();
    assert_eq!(techs.len(), 1, "a mutual clash resolves exactly once");
    assert!(
        !sim.trace()
            .iter()
            .any(|e| matches!(e, TraceEvent::ThrowResolved { .. }))
    );
}

// ── parry & armor (spec §5.5, §2.5) ─────────────────────────────────────────

#[test]
fn guard_point_freezes_the_attacker() {
    let mut sim = duel(100);
    let script = Script::new([
        (0, A, Choice::Move { id: JAB }),
        (4, B, Choice::Move { id: PARRY }),
        (12, B, Choice::Move { id: MID_POKE }),
    ]);
    run(&mut sim, &script, 60);
    let cs = contacts(&sim);
    assert!(matches!(cs[0], (6, 1, 2, ContactOutcome::Parried { .. })));
    // The parrier recovers fast (T12) and punishes the authored freeze (until T26).
    assert_eq!(cs[1], (24, 2, 1, ContactOutcome::Hit { counter: false }));
    assert_eq!(sim.debug_entity(A).unwrap().hp, 940);
}

#[test]
fn armor_absorbs_scaled_and_the_move_continues() {
    let mut sim = duel(100);
    let script = Script::new([
        (0, B, Choice::Move { id: POWER_CRUSH }),
        (0, A, Choice::Wait { ticks: 2 }),
        (2, A, Choice::Move { id: JAB }),
    ]);
    run(&mut sim, &script, 50);
    let cs = contacts(&sim);
    // Jab meets the armor window: absorbed at half damage, no stun...
    assert_eq!(cs[0], (8, 1, 2, ContactOutcome::Armored));
    assert_eq!(sim.debug_entity(B).unwrap().hp, 1000 - 15);
    // ...and the crush plows on through A's recovery: a counter-hit.
    assert_eq!(cs[1], (14, 2, 1, ContactOutcome::Hit { counter: true }));
}

#[test]
fn backdash_iframes_pass_the_strike_through() {
    let mut sim = duel(100);
    let script = Script::new([
        (0, A, Choice::Move { id: JAB }),
        (4, B, Choice::Move { id: BACKDASH }),
    ]);
    run(&mut sim, &script, 30);
    assert_eq!(contacts(&sim), vec![(6, 1, 2, ContactOutcome::Whiff)]);
    assert_eq!(sim.debug_entity(B).unwrap().hp, 1000);
}

// ── stances as moves (spec §5.2–5.3) ────────────────────────────────────────

#[test]
fn while_crouching_moves_commit_from_the_held_stance() {
    let mut sim = duel(150);
    let script = Script::new([
        (0, B, Choice::Move { id: CROUCH }),
        // First stance reevaluate (T1 hold + 30): attack straight out of the crouch.
        (31, B, Choice::Move { id: WS_UPPERCUT }),
    ]);
    run(&mut sim, &script, 60);
    assert_eq!(
        contacts(&sim),
        vec![(39, 2, 1, ContactOutcome::Hit { counter: false })]
    );
    assert_eq!(sim.debug_entity(A).unwrap().hp, 955);
}

#[test]
fn release_pays_the_authored_recovery() {
    let mut sim = duel(300);
    let script = Script::new([
        (0, B, Choice::Move { id: STAND_GUARD }),
        (32, B, Choice::Release),
        // A's Wait{1} cadence gives the driver a pause at T33, mid-release.
        (32, A, Choice::Wait { ticks: 1 }),
    ]);
    let status = run(&mut sim, &script, 32);
    assert_eq!(status, SimStatus::AwaitingDecisions);
    // Released at T32: locked through the stance's release recovery (4 ticks, free T36).
    assert_eq!(sim.debug_entity(B).unwrap().state, ActorState::Acting);
    run(&mut sim, &script, 45);
    assert_eq!(sim.debug_entity(B).unwrap().state, ActorState::Free);
    assert_eq!(sim.debug_entity(B).unwrap().stance, Stance::Standing);
}

// ── outcome (spec §8.6, the 1v1 special case) ───────────────────────────────

#[test]
fn ko_ends_the_fight_with_a_winner() {
    let mut sim = duel(100);
    // 17 mid pokes (60 damage) overcome 1000 HP; a 34-tick cadence (30-tick move + 4)
    // keeps the whole beatdown inside the tick cap.
    let entries: Vec<(u64, engine::core::ids::EntityId, Choice)> = (0..17)
        .map(|i| (i * 34, A, Choice::Move { id: MID_POKE }))
        .collect();
    let script = Script {
        at: entries.iter().map(|&(t, id, c)| ((t, id.0), c)).collect(),
    };
    let status = run(&mut sim, &script, 1000);
    assert_eq!(
        status,
        SimStatus::Over {
            winner: Some(SIDE_A)
        }
    );
    assert!(
        sim.trace()
            .iter()
            .any(|e| matches!(e, TraceEvent::Ko { actor, .. } if *actor == B))
    );
    assert!(
        matches!(sim.trace().last(), Some(TraceEvent::SimEnded { winner: Some(s), .. }) if *s == SIDE_A)
    );
}
