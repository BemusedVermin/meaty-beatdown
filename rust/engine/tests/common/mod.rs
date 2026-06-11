//! Shared Phase 1 test kit: a minimal authored Form ("Test Form") plus a declarative
//! script driver. This is TEST content — the shipped content (and its audit) arrives in
//! Phase 6; authored magnitudes here exist to exercise every spec §5 interaction.
#![allow(dead_code)] // each test binary compiles this module; none uses all of it

use engine::combat::schedule::{Choice, DecisionKind};
use engine::combat::sim::{CombatSim, EntitySetup, SimConfig, SimStatus};
use engine::core::fx::{Fx, FxVec2, fx};
use engine::core::ids::{EntityId, SideId};
use engine::core::tick::Tick;
use engine::data::movedef::{
    HeightMask, InvulnCover, Move, MoveCategory, PhaseMotion, PropertyKind, PropertyWindow,
    ReachEnvelope, SelfMotion, StanceKind, StanceReq, StanceSpec, ThrowBreakKey, Timing, Tracking,
};
use engine::data::{
    ArenaDef, ChDefault, DefenseProfile, FormId, Height, HitEvent, MoveId, Reaction, Ruleset,
};
use std::collections::BTreeMap;

pub const A: EntityId = EntityId(1);
pub const B: EntityId = EntityId(2);
pub const SIDE_A: SideId = SideId(0);
pub const SIDE_B: SideId = SideId(1);

pub const JAB: MoveId = MoveId(1);
pub const MID_POKE: MoveId = MoveId(2);
pub const SWEEP: MoveId = MoveId(3);
pub const THROW_L: MoveId = MoveId(5);
pub const THROW_R: MoveId = MoveId(6);
pub const SIDESTEP_L: MoveId = MoveId(7);
pub const DASH_IN: MoveId = MoveId(8);
pub const BACKDASH: MoveId = MoveId(9);
pub const STAND_GUARD: MoveId = MoveId(10);
pub const CROUCH: MoveId = MoveId(11);
pub const CROUCH_GUARD: MoveId = MoveId(12);
pub const PARRY: MoveId = MoveId(13);
pub const WS_UPPERCUT: MoveId = MoveId(14);
pub const POWER_CRUSH: MoveId = MoveId(16);

/// Exact fraction helper (the engine bans float literals; ratios are exact in Q32.32).
#[must_use]
pub fn fxf(num: i32, den: i32) -> Fx {
    fx(num) / fx(den)
}

fn envelope(min_cm: i32, max_cm: i32, arc_cm: i32, track_cm: i32) -> ReachEnvelope {
    ReachEnvelope {
        min_range: fxf(min_cm, 100),
        max_range: fxf(max_cm, 100),
        arc_halfwidth: fxf(arc_cm, 100),
        track_halfwidth: fxf(track_cm, 100),
    }
}

fn strike(
    id: MoveId,
    name: &str,
    height: Height,
    timing: Timing,
    region: ReachEnvelope,
    hits: Vec<HitEvent>,
) -> Move {
    Move {
        id,
        name: name.into(),
        form: FormId(1),
        category: MoveCategory::Strike,
        height,
        blockable: true,
        tracking: Tracking::Linear,
        timing,
        hits,
        region,
        motion: SelfMotion::default(),
        properties: vec![],
        req_stance: None,
        break_key: None,
        stance_spec: None,
    }
}

fn hit(at: u32, damage: u32, chip: u32, blockstun: u32, reaction: Reaction) -> HitEvent {
    HitEvent {
        at,
        damage,
        chip_guard: chip,
        blockstun,
        block_push: fxf(30, 100),
        reaction,
        ch_reaction: None,
    }
}

/// The authored test kit. Reference frame data (1 unit = 1 m):
///
/// | move        | type           | s/a/r    | notes                                   |
/// |-------------|----------------|----------|-----------------------------------------|
/// | JAB         | strike HIGH    | 6/2/8    | 30 dmg, hitstun 16, chip 12             |
/// | MID_POKE    | strike MID     | 12/2/16  | 60 dmg, hitstun 20, CH: hard knockdown  |
/// | SWEEP       | strike LOW     | 18/2/22  | 50 dmg, hard knockdown                  |
/// | THROW_L/R   | throw, break L/R | 10/4/20 | grab; slam at +8 (70 dmg, knockdown)   |
/// | SIDESTEP_L  | motion         | 3/6/4    | 1.2 lateral (left) during active        |
/// | DASH_IN     | motion         | 2/4/3    | 1.5 forward                             |
/// | BACKDASH    | motion         | 2/4/6    | -1.2 forward, strike-invuln ticks 0-4   |
/// | STAND_GUARD | stance         | 2/0/4    | guards HIGH+MID                         |
/// | CROUCH      | stance         | 1/0/2    | crouching, no guard                     |
/// | CROUCH_GUARD| stance         | 2/0/4    | crouching, guards LOW                   |
/// | PARRY       | utility        | 2/6/14   | guard-point H+M ticks 2-7, freeze 20    |
/// | WS_UPPERCUT | strike MID     | 8/2/12   | requires crouch, 45 dmg hitstun 18      |
/// | POWER_CRUSH | strike MID     | 14/2/18  | armor H+M ticks 2-13 (1 hit, 50% dmg)   |
#[must_use]
pub fn kit() -> Vec<Move> {
    let mut moves = vec![
        strike(
            JAB,
            "jab",
            Height::High,
            Timing {
                startup: 6,
                active: 2,
                recovery: 8,
            },
            envelope(0, 150, 50, 150),
            vec![hit(0, 30, 12, 12, Reaction::Hitstun { ticks: 16 })],
        ),
        {
            let mut m = strike(
                MID_POKE,
                "mid poke",
                Height::Mid,
                Timing {
                    startup: 12,
                    active: 2,
                    recovery: 16,
                },
                envelope(0, 250, 60, 180),
                vec![hit(0, 60, 10, 14, Reaction::Hitstun { ticks: 20 })],
            );
            m.hits[0].ch_reaction = Some(Reaction::Knockdown {
                hard: true,
                down_ticks: 40,
            });
            m
        },
        strike(
            SWEEP,
            "sweep",
            Height::Low,
            Timing {
                startup: 18,
                active: 2,
                recovery: 22,
            },
            envelope(0, 220, 80, 220),
            vec![hit(
                0,
                50,
                8,
                18,
                Reaction::Knockdown {
                    hard: true,
                    down_ticks: 45,
                },
            )],
        ),
        throw(THROW_L, "shoulder toss", ThrowBreakKey::L),
        throw(THROW_R, "hip toss", ThrowBreakKey::R),
        motion_move(
            SIDESTEP_L,
            "sidestep L",
            Timing {
                startup: 3,
                active: 6,
                recovery: 4,
            },
            SelfMotion {
                active: PhaseMotion {
                    forward: fx(0),
                    lateral: fxf(120, 100),
                },
                ..SelfMotion::default()
            },
        ),
        motion_move(
            DASH_IN,
            "dash in",
            Timing {
                startup: 2,
                active: 4,
                recovery: 3,
            },
            SelfMotion {
                active: PhaseMotion {
                    forward: fxf(150, 100),
                    lateral: fx(0),
                },
                ..SelfMotion::default()
            },
        ),
        {
            let mut m = motion_move(
                BACKDASH,
                "backdash",
                Timing {
                    startup: 2,
                    active: 4,
                    recovery: 6,
                },
                SelfMotion {
                    active: PhaseMotion {
                        forward: fxf(-120, 100),
                        lateral: fx(0),
                    },
                    ..SelfMotion::default()
                },
            );
            m.properties = vec![PropertyWindow {
                from: 0,
                to: 4,
                kind: PropertyKind::Invuln {
                    covers: InvulnCover::Strike,
                },
            }];
            m
        },
        stance_move(
            STAND_GUARD,
            "guard",
            Timing {
                startup: 2,
                active: 0,
                recovery: 4,
            },
            StanceSpec {
                stance: StanceKind::Standing,
                guard: Some(HeightMask::STANDING_GUARD),
            },
        ),
        stance_move(
            CROUCH,
            "crouch",
            Timing {
                startup: 1,
                active: 0,
                recovery: 2,
            },
            StanceSpec {
                stance: StanceKind::Crouching,
                guard: None,
            },
        ),
        stance_move(
            CROUCH_GUARD,
            "crouch guard",
            Timing {
                startup: 2,
                active: 0,
                recovery: 4,
            },
            StanceSpec {
                stance: StanceKind::Crouching,
                guard: Some(HeightMask::CROUCHING_GUARD),
            },
        ),
        {
            let mut m = strike(
                PARRY,
                "leaf parry",
                Height::None,
                Timing {
                    startup: 2,
                    active: 6,
                    recovery: 14,
                },
                envelope(0, 0, 0, 0),
                vec![],
            );
            m.category = MoveCategory::Utility;
            m.properties = vec![PropertyWindow {
                from: 2,
                to: 7,
                kind: PropertyKind::GuardPoint {
                    covers: HeightMask {
                        high: true,
                        mid: true,
                        low: false,
                    },
                    freeze_attacker: 20,
                    parry_recovery: 6,
                },
            }];
            m
        },
        {
            let mut m = strike(
                WS_UPPERCUT,
                "rising palm",
                Height::Mid,
                Timing {
                    startup: 8,
                    active: 2,
                    recovery: 12,
                },
                envelope(0, 180, 60, 180),
                vec![hit(0, 45, 8, 12, Reaction::Hitstun { ticks: 18 })],
            );
            m.req_stance = Some(StanceReq::Crouching);
            m
        },
        {
            let mut m = strike(
                POWER_CRUSH,
                "iron shoulder",
                Height::Mid,
                Timing {
                    startup: 14,
                    active: 2,
                    recovery: 18,
                },
                envelope(0, 200, 70, 200),
                vec![hit(0, 70, 12, 16, Reaction::Hitstun { ticks: 20 })],
            );
            m.properties = vec![PropertyWindow {
                from: 2,
                to: 13,
                kind: PropertyKind::Armor {
                    hits: 1,
                    dmg_mult: fxf(1, 2),
                    covers: HeightMask {
                        high: true,
                        mid: true,
                        low: false,
                    },
                },
            }];
            m
        },
    ];
    moves.sort_by_key(|m| m.id);
    moves
}

fn throw(id: MoveId, name: &str, key: ThrowBreakKey) -> Move {
    Move {
        id,
        name: name.into(),
        form: FormId(1),
        category: MoveCategory::Throw,
        height: Height::None,
        blockable: false,
        tracking: Tracking::Homing,
        timing: Timing {
            startup: 10,
            active: 4,
            recovery: 20,
        },
        hits: vec![HitEvent {
            at: 8,
            damage: 70,
            chip_guard: 0,
            blockstun: 0,
            block_push: fx(0),
            reaction: Reaction::Knockdown {
                hard: true,
                down_ticks: 50,
            },
            ch_reaction: None,
        }],
        region: envelope(0, 90, 0, 0),
        motion: SelfMotion::default(),
        properties: vec![],
        req_stance: None,
        break_key: Some(key),
        stance_spec: None,
    }
}

fn motion_move(id: MoveId, name: &str, timing: Timing, motion: SelfMotion) -> Move {
    Move {
        id,
        name: name.into(),
        form: FormId(1),
        category: MoveCategory::Motion,
        height: Height::None,
        blockable: false,
        tracking: Tracking::Linear,
        timing,
        hits: vec![],
        region: envelope(0, 0, 0, 0),
        motion,
        properties: vec![],
        req_stance: None,
        break_key: None,
        stance_spec: None,
    }
}

fn stance_move(id: MoveId, name: &str, timing: Timing, spec: StanceSpec) -> Move {
    Move {
        id,
        name: name.into(),
        form: FormId(1),
        category: MoveCategory::Stance,
        height: Height::None,
        blockable: false,
        tracking: Tracking::Linear,
        timing,
        hits: vec![],
        region: envelope(0, 0, 0, 0),
        motion: SelfMotion::default(),
        properties: vec![],
        req_stance: None,
        break_key: None,
        stance_spec: Some(spec),
    }
}

#[must_use]
pub fn ruleset() -> Ruleset {
    Ruleset {
        ch_default: ChDefault {
            damage_mult: fxf(5, 4),
            stun_bonus: 6,
        },
        guard_break_stun: 40,
        throw_tech_recovery: 12,
        throw_tech_push: fx(1),
        block_reevaluate_every: 30,
    }
}

#[must_use]
pub fn defense() -> DefenseProfile {
    DefenseProfile {
        hp_max: 1000,
        guard_max: 50,
        guard_regen_interval: 20,
    }
}

/// A 1v1 with both fighters on the x-axis, facing each other, `gap_cm` apart.
#[must_use]
pub fn duel(gap_cm: i32) -> CombatSim {
    let half = fxf(gap_cm, 200);
    CombatSim::new(SimConfig {
        arena: ArenaDef {
            half_extents: FxVec2::new(fx(10), fx(6)),
        },
        ruleset: ruleset(),
        entities: vec![
            EntitySetup {
                id: A,
                side: SIDE_A,
                pos: FxVec2::new(-half, fx(0)),
                target: B,
                ready_at: Tick::ZERO,
                defense: defense(),
                moves: kit(),
            },
            EntitySetup {
                id: B,
                side: SIDE_B,
                pos: FxVec2::new(half, fx(0)),
                target: A,
                ready_at: Tick::ZERO,
                defense: defense(),
                moves: kit(),
            },
        ],
        max_ticks: 600,
    })
}

/// Declarative script driver: choices keyed by (tick, actor). Unscripted prompts get
/// defaults (Wait 4 / HoldStance / decline the break) so scenarios stay terse.
pub struct Script {
    pub at: BTreeMap<(u64, u32), Choice>,
}

impl Script {
    #[must_use]
    pub fn new<const N: usize>(entries: [(u64, EntityId, Choice); N]) -> Self {
        Self {
            at: entries.iter().map(|&(t, id, c)| ((t, id.0), c)).collect(),
        }
    }

    fn choice_for(&self, t: Tick, actor: EntityId, kind: DecisionKind) -> Choice {
        self.at.get(&(t.0, actor.0)).copied().unwrap_or(match kind {
            DecisionKind::Ready => Choice::Wait { ticks: 4 },
            DecisionKind::StanceReevaluate => Choice::HoldStance,
            DecisionKind::ThrowBreak { .. } => Choice::ThrowBreak { guess: None },
        })
    }
}

/// Drive the sim against a script until the fight ends or `until` ticks pass.
pub fn run(sim: &mut CombatSim, script: &Script, until: u64) -> SimStatus {
    loop {
        match sim.advance() {
            SimStatus::Over { winner } => return SimStatus::Over { winner },
            SimStatus::AwaitingDecisions => {
                if sim.tick().0 > until {
                    return SimStatus::AwaitingDecisions;
                }
                let pending = sim.pending();
                let mut sides: Vec<SideId> = pending.iter().map(|p| p.side).collect();
                sides.sort_unstable();
                sides.dedup();
                for side in sides {
                    let choices: Vec<(EntityId, Choice)> = pending
                        .iter()
                        .filter(|p| p.side == side)
                        .map(|p| (p.actor, script.choice_for(sim.tick(), p.actor, p.kind)))
                        .collect();
                    sim.commit_side(side, &choices)
                        .expect("scripted choice is legal");
                }
            }
        }
    }
}
