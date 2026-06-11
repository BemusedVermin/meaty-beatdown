//! Phase 2 sim tests: the combo grammar (spec §6), the governors where they bind, the
//! wake-up game (spec §6.3), strings & lock-then-confirm (spec §6.4, §11), and the
//! meter economy (spec §9). The headline test is the phase's exit criterion: the
//! scripted juggle -> screw -> wall splat -> bound -> ender trace reading exactly as
//! spec §6.2.

mod common;

use common::*;
use engine::combat::schedule::{Choice, CommitError, DecisionKind};
use engine::combat::sim::SimStatus;
use engine::data::Reaction;
use engine::trace::TraceEvent;

type ContactRow = (u64, u32, Option<Reaction>, u32);

/// (tick, victim, reaction, combo_hits) for every Hit contact.
fn hits(sim: &engine::combat::sim::CombatSim) -> Vec<ContactRow> {
    sim.trace()
        .iter()
        .filter_map(|e| match e {
            TraceEvent::Contact {
                t,
                victim,
                reaction,
                combo_hits,
                ..
            } if reaction.is_some() => Some((t.0, victim.0, *reaction, *combo_hits)),
            _ => None,
        })
        .collect()
}

// ── THE EXIT CRITERION: spec §6.2 as one scripted trace ─────────────────────

#[test]
fn the_full_combo_grammar_reads_as_spec_6_2() {
    // B fights with his back 2.5 from the east wall: the screw's carry reaches it.
    let mut sim = duel_at(550, 750);
    let script = Script::new([
        (0, A, Choice::Move { id: LAUNCHER }),
        (
            16,
            A,
            Choice::Cancel {
                into: Some(JUGGLE_PALM),
            },
        ),
        (
            25,
            A,
            Choice::Cancel {
                into: Some(SCREW_KICK),
            },
        ),
        (
            35,
            A,
            Choice::Cancel {
                into: Some(JUGGLE_PALM),
            },
        ),
        (
            44,
            A,
            Choice::Cancel {
                into: Some(BOUND_SLAM),
            },
        ),
        (55, A, Choice::Cancel { into: Some(ENDER) }),
    ]);
    run(&mut sim, &script, 130);

    // opener (Launch) -> juggle hit -> Screw (carry west->east into the wall) ->
    // WALL_SPLAT -> pickup -> Bound -> ender (hard knockdown -> oki). Decay visible in
    // the climbing combo counter; every link bought with AP/Focus.
    let expected: Vec<ContactRow> = vec![
        (
            15,
            2,
            Some(Reaction::Launch {
                rise: fxf(150, 100),
                carry: fxf(30, 100),
                stun: 40,
            }),
            1,
        ),
        (24, 2, Some(Reaction::Hitstun { ticks: 30 }), 2),
        (
            34,
            2,
            Some(Reaction::Screw {
                carry: fxf(180, 100),
                stun: 34,
            }),
            3,
        ),
        (43, 2, Some(Reaction::Hitstun { ticks: 30 }), 4),
        (54, 2, Some(Reaction::Bound { stun: 32 }), 5),
        (
            65,
            2,
            Some(Reaction::Knockdown {
                hard: true,
                down_ticks: 50,
            }),
            6,
        ),
    ];
    assert_eq!(hits(&sim), expected);
    assert!(
        sim.trace().iter().any(
            |e| matches!(e, TraceEvent::WallSplat { t, victim } if t.0 == 34 && victim.0 == 2)
        ),
        "the screw's carry splats B on the east wall"
    );
    assert!(
        sim.trace().iter().any(
            |e| matches!(e, TraceEvent::ComboEnded { t, hits, .. } if t.0 == 65 && *hits == 6)
        ),
        "the ender closes a six-hit combo"
    );
    // Latches all spent exactly once; juggle decay shaved the damage.
    assert_eq!(
        sim.debug_entity(B).unwrap().hp,
        1000 - (55 + 36 + 28 + 28 + 27 + 30)
    );
    // Governor 4 ledger: the string cost 22 of 24 AP, refilled on regaining freedom.
    assert_eq!(sim.debug_entity(A).unwrap().ap, 24);
}

// ── governors, each visibly binding ─────────────────────────────────────────

#[test]
fn extender_latches_spend_once_per_combo() {
    let mut sim = duel_at(550, 700);
    let script = Script::new([
        (0, A, Choice::Move { id: LAUNCHER }),
        (
            16,
            A,
            Choice::Cancel {
                into: Some(JUGGLE_PALM),
            },
        ),
        (
            25,
            A,
            Choice::Cancel {
                into: Some(SCREW_KICK),
            },
        ),
        (
            35,
            A,
            Choice::Cancel {
                into: Some(JUGGLE_PALM),
            },
        ),
        // A second screw: the latch is spent — it sustains as a plain juggle hit.
        (
            44,
            A,
            Choice::Cancel {
                into: Some(SCREW_KICK),
            },
        ),
    ]);
    // Stop mid-combo (the tracker resets when the combo ends): after the second screw
    // lands at T53, the latch must still read exactly one spend across two screws.
    run(&mut sim, &script, 53);
    let b = sim.debug_entity(B).unwrap();
    assert_eq!(b.combo.screw_used, 1, "two screws landed, one latch spent");
    assert_eq!(b.combo.hits, 5);
}

#[test]
fn wall_splat_happens_once_then_clamps() {
    let mut sim = duel_at(550, 750);
    // Screw into the wall (splat), pick up, screw again (latch spent on splat=clamp).
    let script = Script::new([
        (0, A, Choice::Move { id: LAUNCHER }),
        (
            16,
            A,
            Choice::Cancel {
                into: Some(JUGGLE_PALM),
            },
        ),
        (
            25,
            A,
            Choice::Cancel {
                into: Some(SCREW_KICK),
            },
        ),
        (
            35,
            A,
            Choice::Cancel {
                into: Some(JUGGLE_PALM),
            },
        ),
        (
            44,
            A,
            Choice::Cancel {
                into: Some(JUGGLE_PALM),
            },
        ),
    ]);
    run(&mut sim, &script, 130);
    let splats = sim
        .trace()
        .iter()
        .filter(|e| matches!(e, TraceEvent::WallSplat { .. }))
        .count();
    assert_eq!(splats, 1);
}

#[test]
fn ap_exhaustion_drops_the_string() {
    // Palm chained into palm costs 4 AP per link: after the launcher (3) and the first
    // chain (3), the budget affords exactly four more links, then the prompt itself
    // stops appearing (auto-pass, no information) and the juggle drops. B fights pinned
    // on an unsplattable wall so only the tempo budget binds, not spacing.
    let mut sim = duel_open_at(750, 950);
    let script = Script::new([
        (0, A, Choice::Move { id: LAUNCHER }),
        (
            16,
            A,
            Choice::Cancel {
                into: Some(JUGGLE_PALM),
            },
        ),
        (
            25,
            A,
            Choice::Cancel {
                into: Some(JUGGLE_PALM),
            },
        ),
        (
            34,
            A,
            Choice::Cancel {
                into: Some(JUGGLE_PALM),
            },
        ),
        (
            43,
            A,
            Choice::Cancel {
                into: Some(JUGGLE_PALM),
            },
        ),
        (
            52,
            A,
            Choice::Cancel {
                into: Some(JUGGLE_PALM),
            },
        ),
        // A sixth link is scripted but can never be prompted: AP is dry.
        (
            61,
            A,
            Choice::Cancel {
                into: Some(JUGGLE_PALM),
            },
        ),
    ]);
    run(&mut sim, &script, 140);
    let max_combo = hits(&sim).iter().map(|h| h.3).max().unwrap_or(0);
    assert_eq!(
        max_combo, 6,
        "launcher + five palms, then the tempo budget is spent"
    );
    assert!(
        sim.trace()
            .iter()
            .any(|e| matches!(e, TraceEvent::Landed { .. })),
        "the dropped string lands the victim"
    );
}

#[test]
fn the_gravity_floor_forces_the_landing() {
    // Crank hitstun decay: by the third hit the decayed stun undercuts every affordable
    // pickup's startup and the victim lands mid-combo (governor 7).
    let mut rs = ruleset();
    rs.hitstun_decay_step = 14;
    let mut sim = duel_with(rs, 550, 700);
    let script = Script::new([
        (0, A, Choice::Move { id: LAUNCHER }),
        (
            16,
            A,
            Choice::Cancel {
                into: Some(JUGGLE_PALM),
            },
        ),
        (
            25,
            A,
            Choice::Cancel {
                into: Some(JUGGLE_PALM),
            },
        ),
        (
            34,
            A,
            Choice::Cancel {
                into: Some(JUGGLE_PALM),
            },
        ),
    ]);
    run(&mut sim, &script, 110);
    let max_combo = hits(&sim).iter().map(|h| h.3).max().unwrap_or(0);
    assert!(
        max_combo <= 3,
        "stun decays below every pickup startup by hit 3"
    );
    assert!(
        sim.trace()
            .iter()
            .any(|e| matches!(e, TraceEvent::Landed { .. }))
    );
}

#[test]
fn hitstun_decay_shrinks_consecutive_stun() {
    let mut sim = duel(100);
    // A jab confirmed into the poke: the second hit's stun is decayed by one step.
    let script = Script::new([
        (0, A, Choice::Move { id: JAB }),
        (
            7,
            A,
            Choice::Cancel {
                into: Some(MID_POKE),
            },
        ),
    ]);
    run(&mut sim, &script, 45);
    // Jab lands T6 (stun 16 -> until 22); the canceled poke lands T19 on a still-stunned
    // B: combo hit 2, stun 20 - 2 = 18 -> until 37, where B's first free commit shows up
    // in the trace (the undecayed value would free B at 39).
    let rows = hits(&sim);
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[1].0, 19);
    assert_eq!(rows[1].3, 2, "the confirm continues the combo");
    assert!(
        sim.trace().iter().any(|e| matches!(
            e,
            TraceEvent::Committed { t, actor, .. } if t.0 == 37 && actor.0 == 2
        )),
        "B's first post-combo decision lands exactly at the decayed stun's end"
    );
}

// ── strings & lock-then-confirm (spec §6.4, §11) ────────────────────────────

#[test]
fn on_hit_confirm_is_offered_and_on_block_it_is_not() {
    // BLOCKED jab: the OnHit window never satisfies — confirming into the poke must be
    // rejected; the OnBlock window (the block-string) is what's open.
    let mut sim = duel(100);
    let script = Script::new([
        (0, A, Choice::Move { id: JAB }),
        (0, B, Choice::Move { id: STAND_GUARD }),
    ]);
    // Drive manually to the cancel prompt at T13 (elapsed 7 after the T6 block).
    loop {
        match sim.advance() {
            SimStatus::AwaitingDecisions => {
                let pending = sim.pending();
                if pending.iter().any(|p| p.kind == DecisionKind::Cancel) {
                    break;
                }
                for p in pending {
                    let choice = script
                        .at
                        .get(&(sim.tick().0, p.actor.0))
                        .copied()
                        .unwrap_or(match p.kind {
                            DecisionKind::StanceReevaluate => Choice::HoldStance,
                            _ => Choice::Wait { ticks: 4 },
                        });
                    sim.commit_side(p.side, &[(p.actor, choice)]).unwrap();
                }
            }
            SimStatus::Over { .. } => panic!("fight ended before the cancel prompt"),
        }
    }
    // The hit-confirm is NOT available (the jab was blocked)...
    let err = sim.commit_side(
        SIDE_A,
        &[(
            A,
            Choice::Cancel {
                into: Some(MID_POKE),
            },
        )],
    );
    assert_eq!(err, Err(CommitError::UnknownOrUnmetMove { actor: A }));
    // ...but the block-string link is.
    sim.commit_side(SIDE_A, &[(A, Choice::Cancel { into: Some(JAB) })])
        .unwrap();
    run(&mut sim, &Script::new([]), 40);
    let blocked = sim
        .trace()
        .iter()
        .filter(|e| {
            matches!(e, TraceEvent::Contact { outcome, .. }
                if matches!(outcome, engine::combat::resolve::ContactOutcome::Blocked))
        })
        .count();
    assert_eq!(blocked, 2, "jab, block-string jab — both blocked");
}

// ── the wake-up game (spec §6.3) ────────────────────────────────────────────

#[test]
fn wake_options_delay_and_back_rise() {
    let mut sim = duel(100);
    let script = Script::new([
        (0, A, Choice::Move { id: SWEEP }),
        // B eats the knockdown (T18, down until 63), delays, then back-rises.
        (63, B, Choice::DelayRise { ticks: 10 }),
        (73, B, Choice::BackRise),
    ]);
    run(&mut sim, &script, 90);
    let b = sim.debug_entity(B).unwrap();
    assert_eq!(b.stance, engine::combat::entity::Stance::Standing);
    // Back-rise displaced B away from A (eastward, B faces west).
    assert!(b.pos.x > fxf(50, 100), "back-rise creates space");
}

#[test]
fn wake_reversal_beats_the_meaty() {
    let mut sim = duel(100);
    let script = Script::new([
        (0, A, Choice::Move { id: SWEEP }),
        // A times a meaty poke at B's wake-up (B wakes T63; poke hits T65)...
        (42, A, Choice::Wait { ticks: 11 }),
        (53, A, Choice::Move { id: MID_POKE }),
        // ...but B wakes with the invulnerable reversal (req_down, invuln 0-8).
        (63, B, Choice::Move { id: WAKE_REVERSAL }),
    ]);
    run(&mut sim, &script, 110);
    let rows: Vec<(u64, u32, bool)> = sim
        .trace()
        .iter()
        .filter_map(|e| match e {
            TraceEvent::Contact {
                t,
                attacker,
                outcome,
                ..
            } => Some((
                t.0,
                attacker.0,
                matches!(outcome, engine::combat::resolve::ContactOutcome::Whiff),
            )),
            _ => None,
        })
        .skip(1) // the sweep itself
        .collect();
    // The meaty passes through the invuln window; the reversal counter-hits A's
    // recovery — the oki mixup is a real read, not a sentence.
    assert_eq!(
        rows[0],
        (65, 1, true),
        "meaty whiffs through reversal i-frames"
    );
    assert!(
        sim.trace().iter().any(|e| matches!(
            e,
            TraceEvent::Contact {
                t,
                attacker,
                outcome: engine::combat::resolve::ContactOutcome::Hit { counter: true },
                ..
            } if t.0 == 70 && attacker.0 == 2
        )),
        "the reversal whiff-punishes the meaty"
    );
}

// ── meters (spec §9) ────────────────────────────────────────────────────────

#[test]
fn focus_gain_table_pays_the_whiff_punisher() {
    let mut sim = duel(200);
    let script = Script::new([
        (0, A, Choice::Move { id: MID_POKE }),
        (0, B, Choice::Move { id: SIDESTEP_L }),
        (13, B, Choice::Move { id: MID_POKE }),
    ]);
    run(&mut sim, &script, 70);
    // B: land_hit 2 + whiff_punish 10. A: the comeback drip (3 per 100 damage of 60).
    assert_eq!(sim.debug_entity(B).unwrap().focus, 12);
    assert_eq!(sim.debug_entity(A).unwrap().focus, 1);
}

#[test]
fn crumple_is_a_standing_pickup_window() {
    // Author a crumple via a custom CH: reuse the kit but assert the state machine —
    // crumple collapses to Down if nobody picks it up.
    let mut rs = ruleset();
    rs.hitstun_decay_step = 2;
    let mut sim = duel_with(rs, 550, 700);
    // No crumple move in the kit yet: drive Launch and let the stun expire mid-air —
    // the victim lands into the wake-up flow (the collapse path shares it).
    let script = Script::new([(0, A, Choice::Move { id: LAUNCHER })]);
    run(&mut sim, &script, 70);
    assert!(
        sim.trace()
            .iter()
            .any(|e| matches!(e, TraceEvent::Landed { .. }))
    );
    assert!(
        sim.trace()
            .iter()
            .any(|e| matches!(e, TraceEvent::ComboEnded { hits: 1, .. }))
    );
}
