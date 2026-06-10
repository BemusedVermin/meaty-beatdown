//! The determinism bedrock tests (implementation-plan Phase 0, tech-plan §6).
//!
//! Replay-twice and the cross-OS probe are CI gates from day one: every later phase builds
//! on the guarantee proven here — same inputs, byte-identical trace, on every platform.

use engine::combat::sim::{CombatSim, Decision, DecisionLog, EntityInit, SimConfig};
use engine::core::ids::{EntityId, SideId};
use engine::core::tick::Tick;
use engine::trace::TraceEvent;

/// The canonical Phase 0 scenario: two entities, interleaved WAITs, a shared decision tick
/// (T7, exercising stable entity-id commit order), ending on a dry log at T8.
fn canonical_scenario() -> (SimConfig, DecisionLog) {
    let config = SimConfig {
        entities: vec![
            EntityInit {
                id: EntityId(1),
                side: SideId(0),
                ready_at: Tick::ZERO,
            },
            EntityInit {
                id: EntityId(2),
                side: SideId(1),
                ready_at: Tick::ZERO,
            },
        ],
        max_ticks: 64,
    };
    let log = DecisionLog::new([
        Decision::Wait { ticks: 3 },  // T0 e1 -> ready T3
        Decision::Wait { ticks: 5 },  // T0 e2 -> ready T5
        Decision::Wait { ticks: 4 },  // T3 e1 -> ready T7
        Decision::Wait { ticks: 2 },  // T5 e2 -> ready T7
        Decision::Wait { ticks: 1 },  // T7 e1 (id order first) -> ready T8
        Decision::Wait { ticks: 10 }, // T7 e2 -> ready T17
    ]);
    (config, log)
}

fn run_canonical() -> Vec<TraceEvent> {
    let (config, log) = canonical_scenario();
    let mut sim = CombatSim::new(config);
    sim.run(log);
    sim.trace().to_vec()
}

fn trace_json(trace: &[TraceEvent]) -> String {
    serde_json::to_string(trace).expect("trace serializes")
}

#[test]
fn schedule_interleaves_by_ready_tick() {
    let trace = run_canonical();
    let commits: Vec<(u64, u32)> = trace
        .iter()
        .filter_map(|e| match e {
            TraceEvent::Committed { t, actor, .. } => Some((t.0, actor.0)),
            _ => None,
        })
        .collect();
    // (tick, actor): interleaved by ready_tick; same-tick T7 resolves in entity-id order.
    assert_eq!(
        commits,
        vec![(0, 1), (0, 2), (3, 1), (5, 2), (7, 1), (7, 2)]
    );
    // Dry log at e1's T8 decision point ends the sim there.
    assert_eq!(trace.last(), Some(&TraceEvent::SimEnded { t: Tick(8) }));
}

/// C-DET: same (initial state, decision log) twice -> byte-identical serialized traces.
#[test]
fn replay_twice_byte_identical() {
    let first = trace_json(&run_canonical());
    let second = trace_json(&run_canonical());
    assert_eq!(first.into_bytes(), second.into_bytes());
}

/// The cross-OS probe: the canonical trace must match the checked-in reference byte for
/// byte. The same assertion passing on Linux AND Windows CI is the cross-platform
/// determinism proof. This is Phase 0 scaffolding, NOT golden vectors v2 (those freeze at
/// Phase 6); regenerate deliberately via the ignored test below when the schema moves.
#[test]
fn cross_os_probe_matches_reference() {
    let reference = include_str!("data/probe_trace.json");
    assert_eq!(trace_json(&run_canonical()), reference.trim_end());
}

/// Regeneration path (deliberate, with a changelog mention in the commit):
/// `cargo test -p engine --test determinism -- --ignored regenerate_probe`
#[test]
#[ignore = "writes tests/data/probe_trace.json; run explicitly to regenerate"]
fn regenerate_probe() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/probe_trace.json");
    std::fs::write(path, trace_json(&run_canonical())).expect("probe written");
}
