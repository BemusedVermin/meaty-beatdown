//! Baseline AI read-profiles (spec §7.5). The contract: agents receive the same
//! Observation a player would and NEVER read hidden commitments — the signature
//! enforces it (an `Agent` has no access to `CombatSim`). Their differing strength is
//! authored disposition, not x-ray vision; bosses' "smart reads" (better priors) join
//! with encounter authoring in later phases.

use crate::core::fx::Fx;
use crate::core::rng::SeededRng;
use crate::data::ThrowBreakKey;
use crate::data::movedef::{Move, MoveCategory, StanceReq};
use serde::{Deserialize, Serialize};

use super::entity::Entity;
use super::observe::{CuePhase, EnemyView, Observation, StateClass};
use super::schedule::{Choice, DecisionKind, PendingDecision};

/// The authored dispositions (implementation-plan Phase 3).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReadProfile {
    /// Pressure: close distance, swing on any opening, take every cancel.
    Aggressive,
    /// Patience: guard by default, punish visible recovery.
    Turtle,
    /// Variance: random reads, throws, and big swings.
    Gambler,
    /// Lateral: sidestep wind-ups, poke from the new lane.
    StepHappy,
}

/// A seeded, Observation-only decision maker. Deterministic per (profile, seed).
#[derive(Debug, Clone)]
pub struct Agent {
    pub profile: ReadProfile,
    rng: SeededRng,
}

impl Agent {
    #[must_use]
    pub fn new(profile: ReadProfile, seed: u64) -> Self {
        Self {
            profile,
            rng: SeededRng::new(seed),
        }
    }

    /// Choose for one prompt. May return a choice the sim rejects (e.g. a cancel whose
    /// window just closed); drivers fall back to the safe default — agents stay simple
    /// and the fog boundary stays type-enforced.
    #[must_use]
    pub fn decide(&mut self, obs: &Observation, prompt: &PendingDecision) -> Choice {
        let Some(me) = obs.ally(prompt.actor) else {
            return fallback(prompt.kind);
        };
        let foe = obs.enemy(me.target).or_else(|| obs.enemies.first());
        match prompt.kind {
            DecisionKind::Ready => self.ready(me, foe),
            DecisionKind::StanceReevaluate => self.reevaluate(foe),
            DecisionKind::ThrowBreak { .. } => self.break_read(me, foe),
            DecisionKind::Cancel => self.cancel(me),
            DecisionKind::WakeUp => self.wake(me),
            DecisionKind::Burst => self.burst(me),
        }
    }

    fn ready(&mut self, me: &Entity, foe: Option<&EnemyView>) -> Choice {
        let Some(foe) = foe else {
            return Choice::Wait { ticks: 4 };
        };
        if foe.state_class == StateClass::Ko {
            return Choice::Wait { ticks: 8 };
        }
        let dist = me.pos.distance(foe.pos);
        let committed_windup = foe
            .cue
            .as_ref()
            .is_some_and(|c| c.phase == CuePhase::WindUp);
        let recovering = foe
            .cue
            .as_ref()
            .is_some_and(|c| c.phase == CuePhase::Recovering);
        let vulnerable =
            recovering || matches!(foe.state_class, StateClass::Reeling | StateClass::Down);

        match self.profile {
            ReadProfile::Aggressive => {
                if let Some(m) = self.pick_strike(me, dist, vulnerable) {
                    return Choice::Move { id: m };
                }
                self.approach(me, dist)
            }
            ReadProfile::Turtle => {
                if vulnerable && let Some(m) = self.pick_strike(me, dist, true) {
                    return Choice::Move { id: m };
                }
                if let Some(g) = guard_move(me) {
                    return Choice::Move { id: g };
                }
                Choice::Wait { ticks: 6 }
            }
            ReadProfile::Gambler => {
                let roll = self.rng.u64(0..4);
                if roll == 0
                    && dist < Fx::ONE
                    && let Some(t) = throw_move(me)
                {
                    return Choice::Move { id: t };
                }
                let coin = self.rng.u64(0..2) == 0;
                if roll <= 2
                    && let Some(m) = self.pick_strike(me, dist, coin)
                {
                    return Choice::Move { id: m };
                }
                self.approach(me, dist)
            }
            ReadProfile::StepHappy => {
                if committed_windup && let Some(s) = lateral_move(me) {
                    return Choice::Move { id: s };
                }
                if let Some(m) = self.pick_strike(me, dist, vulnerable) {
                    return Choice::Move { id: m };
                }
                self.approach(me, dist)
            }
        }
    }

    /// Fastest affordable strike that reaches; the punisher takes the biggest instead.
    fn pick_strike(&mut self, me: &Entity, dist: Fx, punish: bool) -> Option<crate::data::MoveId> {
        let mut options: Vec<&Move> = me
            .moves
            .iter()
            .filter(|m| {
                m.category == MoveCategory::Strike
                    && !m.req_down
                    && !m.flags.rescue
                    && !m.flags.burst
                    && m.req_stance != Some(StanceReq::Crouching)
                    && affordable(me, m)
                    && m.region.max_range >= dist
                    && !m.hits.is_empty()
            })
            .collect();
        if options.is_empty() {
            return None;
        }
        if punish {
            options.sort_by_key(|m| std::cmp::Reverse(m.hits[0].damage));
        } else {
            options.sort_by_key(|m| m.timing.startup);
        }
        Some(options[0].id)
    }

    fn approach(&mut self, me: &Entity, dist: Fx) -> Choice {
        if dist > Fx::ONE
            && let Some(d) = advance_move(me)
        {
            return Choice::Move { id: d };
        }
        Choice::Wait { ticks: 4 }
    }

    fn reevaluate(&mut self, foe: Option<&EnemyView>) -> Choice {
        let threatening =
            foe.is_some_and(|f| f.cue.is_some() || matches!(f.state_class, StateClass::Committed));
        match self.profile {
            ReadProfile::Turtle => Choice::HoldStance,
            _ if threatening => Choice::HoldStance,
            _ => Choice::Release,
        }
    }

    /// The directional read (spec §5.4) — a T3 reveal on the cue decides it outright;
    /// otherwise disposition guesses.
    fn break_read(&mut self, _me: &Entity, foe: Option<&EnemyView>) -> Choice {
        if let Some(key) = foe.and_then(|f| f.cue.as_ref()).and_then(|c| c.break_key) {
            return Choice::ThrowBreak { guess: Some(key) };
        }
        let guess = match self.profile {
            ReadProfile::Turtle => None, // eats it rather than guessing wrong
            _ => Some(if self.rng.u64(0..2) == 0 {
                ThrowBreakKey::L
            } else {
                ThrowBreakKey::R
            }),
        };
        Choice::ThrowBreak { guess }
    }

    fn cancel(&mut self, me: &Entity) -> Choice {
        if self.profile == ReadProfile::Turtle {
            return Choice::Cancel { into: None };
        }
        // Own frame data is own information: chase the first listed continuation.
        let into = me
            .current_move()
            .and_then(|m| m.cancels.first())
            .map(|w| w.into);
        Choice::Cancel { into }
    }

    fn wake(&mut self, me: &Entity) -> Choice {
        match self.profile {
            ReadProfile::Aggressive => Choice::Rise,
            ReadProfile::Turtle => Choice::BackRise,
            ReadProfile::Gambler => {
                let reversal = me
                    .moves
                    .iter()
                    .find(|m| m.req_down && affordable(me, m))
                    .map(|m| m.id);
                match reversal {
                    Some(id) if self.rng.u64(0..2) == 0 => Choice::Move { id },
                    _ => Choice::Rise,
                }
            }
            ReadProfile::StepHappy => Choice::BackRise,
        }
    }

    fn burst(&mut self, me: &Entity) -> Choice {
        me.moves
            .iter()
            .find(|m| m.flags.burst && affordable(me, m))
            .map_or(Choice::Wait { ticks: 1 }, |m| Choice::Move { id: m.id })
    }
}

/// The always-legal default per prompt kind (drivers use this when an agent's pick is
/// rejected).
#[must_use]
pub fn fallback(kind: DecisionKind) -> Choice {
    match kind {
        DecisionKind::Ready => Choice::Wait { ticks: 4 },
        DecisionKind::StanceReevaluate => Choice::HoldStance,
        DecisionKind::ThrowBreak { .. } => Choice::ThrowBreak { guess: None },
        DecisionKind::Cancel => Choice::Cancel { into: None },
        DecisionKind::WakeUp => Choice::Rise,
        DecisionKind::Burst => Choice::Wait { ticks: 1 },
    }
}

fn affordable(e: &Entity, m: &Move) -> bool {
    e.breath >= m.cost.breath && e.ap >= m.cost.ap && e.focus >= m.cost.focus
}

fn guard_move(me: &Entity) -> Option<crate::data::MoveId> {
    me.moves
        .iter()
        .find(|m| {
            m.category == MoveCategory::Stance
                && m.stance_spec
                    .is_some_and(|s| s.guard.is_some_and(|g| g.high && g.mid))
        })
        .map(|m| m.id)
}

fn throw_move(me: &Entity) -> Option<crate::data::MoveId> {
    me.moves
        .iter()
        .find(|m| m.category == MoveCategory::Throw && affordable(me, m))
        .map(|m| m.id)
}

fn lateral_move(me: &Entity) -> Option<crate::data::MoveId> {
    me.moves
        .iter()
        .find(|m| {
            m.category == MoveCategory::Motion
                && m.motion.active.lateral != Fx::ZERO
                && affordable(me, m)
        })
        .map(|m| m.id)
}

fn advance_move(me: &Entity) -> Option<crate::data::MoveId> {
    me.moves
        .iter()
        .find(|m| {
            m.category == MoveCategory::Motion
                && m.motion.active.forward > Fx::ZERO
                && affordable(me, m)
        })
        .map(|m| m.id)
}
