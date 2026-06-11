//! The anti-infinite property suite (C-FIN; tech-plan §6): proptest-driven fuzz agents
//! pick random affordable actions for both sides; no fight may ever exceed the audit's
//! combo bound, deadlock, or corrupt a meter. Shrinking gives minimal repro seeds.

mod common;

use common::*;
use engine::combat::schedule::{Choice, DecisionKind};
use engine::combat::sim::{CombatSim, SimStatus};
use engine::core::ids::EntityId;
use engine::core::rng::SeededRng;
use engine::data::movedef::ThrowBreakKey;
use engine::trace::TraceEvent;
use proptest::prelude::*;

/// A fuzz agent: tries random plausible choices through the public commit API; anything
/// rejected falls back to the always-legal default. Never reads hidden state.
fn random_choice(
    rng: &mut SeededRng,
    sim: &CombatSim,
    actor: EntityId,
    kind: DecisionKind,
) -> Vec<Choice> {
    let movelist = sim
        .debug_entity(actor)
        .map(|e| e.moves.clone())
        .unwrap_or_default();
    let mut candidates: Vec<Choice> = Vec::new();
    match kind {
        DecisionKind::Ready
        | DecisionKind::StanceReevaluate
        | DecisionKind::WakeUp
        | DecisionKind::Burst => {
            for _ in 0..3 {
                let idx = rng.usize(0..movelist.len());
                candidates.push(Choice::Move {
                    id: movelist[idx].id,
                });
            }
            match kind {
                DecisionKind::Ready => candidates.push(Choice::Wait {
                    ticks: u32::try_from(rng.u64(1..12)).expect("small"),
                }),
                DecisionKind::StanceReevaluate => {
                    candidates.push(if rng.u64(0..2) == 0 {
                        Choice::Release
                    } else {
                        Choice::HoldStance
                    });
                }
                DecisionKind::WakeUp => {
                    candidates.push(match rng.u64(0..3) {
                        0 => Choice::Rise,
                        1 => Choice::BackRise,
                        _ => Choice::DelayRise {
                            ticks: u32::try_from(rng.u64(1..20)).expect("small"),
                        },
                    });
                }
                DecisionKind::Burst => candidates.push(Choice::Wait { ticks: 1 }),
                _ => unreachable!(),
            }
        }
        DecisionKind::ThrowBreak { .. } => candidates.push(Choice::ThrowBreak {
            guess: match rng.u64(0..3) {
                0 => Some(ThrowBreakKey::L),
                1 => Some(ThrowBreakKey::R),
                _ => None,
            },
        }),
        DecisionKind::Cancel => {
            for _ in 0..2 {
                let idx = rng.usize(0..movelist.len());
                candidates.push(Choice::Cancel {
                    into: Some(movelist[idx].id),
                });
            }
            candidates.push(Choice::Cancel { into: None });
        }
    }
    // The guaranteed-legal fallback per kind.
    candidates.push(match kind {
        DecisionKind::Ready => Choice::Wait { ticks: 4 },
        DecisionKind::StanceReevaluate => Choice::HoldStance,
        DecisionKind::ThrowBreak { .. } => Choice::ThrowBreak { guess: None },
        DecisionKind::Cancel => Choice::Cancel { into: None },
        DecisionKind::WakeUp => Choice::Rise,
        DecisionKind::Burst => Choice::Wait { ticks: 1 },
    });
    candidates
}

fn fuzz_fight(seed: u64) -> CombatSim {
    let mut rng = SeededRng::new(seed);
    let mut sim = duel(150);
    let mut guard = 0u32;
    loop {
        guard += 1;
        assert!(
            guard < 100_000,
            "no deadlock: a decision or the end is always reachable"
        );
        match sim.advance() {
            SimStatus::Over { .. } => return sim,
            SimStatus::AwaitingDecisions => {
                for p in sim.pending() {
                    for candidate in random_choice(&mut rng, &sim, p.actor, p.kind) {
                        if sim.commit_side(p.side, &[(p.actor, candidate)]).is_ok() {
                            break;
                        }
                    }
                }
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(48))]

    /// C-FIN: no fuzzed fight may exceed the audit's combo bound (governors 1-5 & 7,
    /// each independently sufficient, here all live at once).
    #[test]
    fn no_combo_exceeds_the_audit_bound(seed in any::<u64>()) {
        let bound = engine::content::audit::audit(&kit(), &ruleset()).combo_bound;
        let sim = fuzz_fight(seed);
        let max_combo = sim
            .trace()
            .iter()
            .filter_map(|e| match e {
                TraceEvent::Contact { combo_hits, .. } => Some(*combo_hits),
                _ => None,
            })
            .max()
            .unwrap_or(0);
        prop_assert!(max_combo <= bound, "combo of {max_combo} exceeds bound {bound}");
    }

    /// Meters never escape their compiled bounds, and the fight always reaches an end
    /// state on the trace.
    #[test]
    fn meters_bounded_and_fights_end(seed in any::<u64>()) {
        let sim = fuzz_fight(seed);
        for id in [A, B] {
            let e = sim.debug_entity(id).unwrap();
            prop_assert!(e.hp <= e.defense.hp_max);
            prop_assert!(e.guard <= e.defense.guard_max);
            prop_assert!(e.breath <= e.defense.breath_max);
            prop_assert!(e.ap <= e.defense.ap_max);
            prop_assert!(e.focus <= e.defense.focus_max);
        }
        let ended = matches!(sim.trace().last(), Some(TraceEvent::SimEnded { .. }));
        prop_assert!(ended);
    }

    /// C-DET under fuzz: any recorded fight replays byte-identically from its own
    /// Committed events (kind-aware: an actor can commit twice on one tick — a
    /// tick-start choice and a mid-tick throw break).
    #[test]
    fn fuzzed_fights_replay_from_trace(seed in any::<u64>()) {
        let original = fuzz_fight(seed);
        let mut script = replay_script(original.trace());
        let mut replay = duel(150);
        run_replay(&mut replay, &mut script);
        let a = serde_json::to_string(original.trace()).expect("serializes");
        let b = serde_json::to_string(replay.trace()).expect("serializes");
        prop_assert_eq!(a, b);
    }
}
