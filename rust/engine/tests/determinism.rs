//! The determinism gates (tech-plan §6), Phase 1 edition.
//!
//! - replay-twice: same script -> byte-identical traces.
//! - replay-from-trace: the `Committed` events ARE the decision log (C-DET) — a fight
//!   rebuilt from nothing but its own trace reproduces that trace byte for byte.
//! - cross-OS probe: the canonical trace matches a checked-in reference; the same assert
//!   passing on Ubuntu AND Windows CI is the cross-platform proof.

mod common;

use common::*;
use engine::combat::schedule::Choice;
use engine::data::movedef::ThrowBreakKey;
use engine::trace::TraceEvent;

/// The canonical Phase 1 scenario: a real exchange touching block, chip, sidestep-whiff,
/// CH whiff-punish, knockdown, and a throw break — every duel-core subsystem in ~90 ticks.
fn canonical_script() -> Script {
    Script::new([
        (0, A, Choice::Move { id: MID_POKE }),
        (0, B, Choice::Move { id: SIDESTEP_L }),
        (13, B, Choice::Move { id: MID_POKE }),
        // A rises from the CH knockdown at T65 and turtles; B chips then grabs.
        (65, A, Choice::Move { id: STAND_GUARD }),
        (43, B, Choice::Move { id: JAB }),
        (67, B, Choice::Move { id: JAB }),
        (83, B, Choice::Move { id: THROW_L }),
        (
            93,
            A,
            Choice::ThrowBreak {
                guess: Some(ThrowBreakKey::L),
            },
        ),
    ])
}

fn run_canonical() -> Vec<TraceEvent> {
    let mut sim = duel(200);
    run(&mut sim, &canonical_script(), 120);
    sim.trace().to_vec()
}

fn trace_json(trace: &[TraceEvent]) -> String {
    serde_json::to_string(trace).expect("trace serializes")
}

#[test]
fn replay_twice_byte_identical() {
    let first = trace_json(&run_canonical());
    let second = trace_json(&run_canonical());
    assert_eq!(first.into_bytes(), second.into_bytes());
}

/// Rebuild the script purely from the trace's Committed events and re-run: the trace is
/// a sufficient record of the fight (the golden-vector contract rests on this).
#[test]
fn replay_from_trace_byte_identical() {
    let original = run_canonical();
    let recovered = Script {
        at: original
            .iter()
            .filter_map(|e| match e {
                TraceEvent::Committed { t, actor, choice } => Some(((t.0, actor.0), *choice)),
                _ => None,
            })
            .collect(),
    };
    let mut sim = duel(200);
    run(&mut sim, &recovered, 120);
    assert_eq!(
        trace_json(sim.trace()).into_bytes(),
        trace_json(&original).into_bytes()
    );
}

/// Same-tick decisions resolve in entity-id order; the schedule interleaves by
/// ready_tick (spec §4.2) — asserted over the wait-only degenerate case.
#[test]
fn waits_interleave_in_stable_order() {
    let mut sim = duel(400);
    let script = Script::new([
        (0, A, Choice::Wait { ticks: 3 }),
        (0, B, Choice::Wait { ticks: 5 }),
        (3, A, Choice::Wait { ticks: 4 }),
        (5, B, Choice::Wait { ticks: 2 }),
        (7, A, Choice::Wait { ticks: 1 }),
        (7, B, Choice::Wait { ticks: 10 }),
    ]);
    run(&mut sim, &script, 7);
    let commits: Vec<(u64, u32)> = sim
        .trace()
        .iter()
        .filter_map(|e| match e {
            TraceEvent::Committed { t, actor, .. } => Some((t.0, actor.0)),
            _ => None,
        })
        .collect();
    assert_eq!(
        commits,
        vec![(0, 1), (0, 2), (3, 1), (5, 2), (7, 1), (7, 2)]
    );
}

/// The cross-OS probe (Phase 0 scaffolding, NOT golden vectors v2 — those freeze at
/// Phase 6). Regenerate deliberately when the schema moves:
/// `cargo test -p engine --test determinism -- --ignored regenerate_probe`
#[test]
fn cross_os_probe_matches_reference() {
    let reference = include_str!("data/probe_trace.json");
    assert_eq!(trace_json(&run_canonical()), reference.trim_end());
}

#[test]
#[ignore = "writes tests/data/probe_trace.json; run explicitly to regenerate"]
fn regenerate_probe() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/probe_trace.json");
    std::fs::write(path, trace_json(&run_canonical())).expect("probe written");
}
