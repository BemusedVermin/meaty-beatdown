//! Shared test kit: a minimal authored Form ("Test Form") plus a declarative script
//! driver. This is TEST content — the shipped content (and its audit gate in CI) arrives
//! in Phase 6; authored magnitudes here exist to exercise every spec §5–§6 interaction.
#![allow(dead_code)] // each test binary compiles this module; none uses all of it

use engine::combat::schedule::{Choice, DecisionKind};
use engine::combat::sim::{CombatSim, EntitySetup, SimConfig, SimStatus};
use engine::core::fx::{Fx, FxVec2, fx};
use engine::core::ids::{EntityId, SideId};
use engine::core::tick::Tick;
use engine::data::movedef::{
    CancelGate, CancelWindow, CueClass, GainGate, GainResource, HeightMask, InvulnCover, Move,
    MoveCategory, MoveCost, MoveFlags, PhaseMotion, PropertyKind, PropertyWindow, ReachEnvelope,
    ResourceGain, SelfMotion, StanceKind, StanceReq, StanceSpec, ThrowBreakKey, Timing, Tracking,
};
use engine::data::{
    ArenaDef, ChDefault, DefenseProfile, ExtenderLatches, FocusGains, FormId, Height, HitEvent,
    KnowledgeBook, MeterVisibility, MoveId, Reaction, Ruleset, WallSpec, Walls,
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
// ── the combo string (Phase 2) ──
pub const LAUNCHER: MoveId = MoveId(20);
pub const JUGGLE_PALM: MoveId = MoveId(21);
pub const SCREW_KICK: MoveId = MoveId(22);
pub const BOUND_SLAM: MoveId = MoveId(23);
pub const ENDER: MoveId = MoveId(24);
pub const WAKE_REVERSAL: MoveId = MoveId(26);
pub const BURST: MoveId = MoveId(30);
pub const RESCUE_STRIKE: MoveId = MoveId(31);
pub const REVIVE: MoveId = MoveId(32);
pub const WIDE_CLEAVE: MoveId = MoveId(33);

// ── the cue vocabulary (spec §7.2): shared cues ARE the feints ──
/// JAB: the quick high flick.
pub const CUE_QUICK: CueClass = CueClass(1);
/// MID_POKE + SWEEP: the §14 "low coil" — thrust feint or sweep? A guess.
pub const CUE_LOW_COIL: CueClass = CueClass(2);
/// THROW_L + THROW_R + DASH_IN: the lunging shape — grab (which break?) or approach?
pub const CUE_LUNGE: CueClass = CueClass(3);
/// LAUNCHER + WS_UPPERCUT + WAKE_REVERSAL: the rising shapes.
pub const CUE_RISING: CueClass = CueClass(4);
/// SIDESTEP_L + BACKDASH: weight shifts.
pub const CUE_STEP: CueClass = CueClass(5);
/// Guards, crouch, and the parry (a sabaki LOOKS like guarding — the built-in lie).
pub const CUE_GUARD: CueClass = CueClass(6);
/// The juggle string + POWER_CRUSH: the flurry.
pub const CUE_FLURRY: CueClass = CueClass(7);

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

#[expect(
    clippy::too_many_arguments,
    reason = "an authoring shorthand, not an API"
)]
fn strike(
    id: MoveId,
    name: &str,
    height: Height,
    timing: Timing,
    region: ReachEnvelope,
    hits: Vec<HitEvent>,
    ap: u32,
    cue: CueClass,
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
        cost: MoveCost {
            breath: 1,
            ap,
            focus: 0,
        },
        gains: vec![],
        cancels: vec![],
        startup_cancelable: false,
        cue,
        req_stance: None,
        req_down: false,
        break_key: None,
        stance_spec: None,
        flags: MoveFlags::default(),
    }
}

fn hit(at: u32, damage: u32, chip: u32, blockstun: u32, reaction: Reaction) -> HitEvent {
    HitEvent {
        at,
        damage,
        chip_guard: chip,
        blockstun,
        block_push: fxf(30, 100),
        juggle_carry: fxf(50, 100),
        reaction,
        ch_reaction: None,
    }
}

fn chain(from: u32, to: u32, gate: CancelGate, into: MoveId, ap: u32, focus: u32) -> CancelWindow {
    CancelWindow {
        from,
        to,
        gate,
        into,
        ap_cost: ap,
        focus_cost: focus,
    }
}

/// The authored test kit. Reference frame data (1 unit = 1 m):
///
/// | move        | type             | s/a/r    | notes                                  |
/// |-------------|------------------|----------|----------------------------------------|
/// | JAB         | strike HIGH      | 6/2/8    | 30 dmg, hitstun 16; OnHit -> MID_POKE, |
/// |             |                  |          | OnBlock -> JAB (the block string)      |
/// | MID_POKE    | strike MID       | 12/2/16  | 60 dmg, hitstun 20, CH: hard knockdown |
/// | SWEEP       | strike LOW       | 18/2/22  | 50 dmg, hard knockdown                 |
/// | THROW_L/R   | throw, break L/R | 10/4/20  | grab; slam at +8 (70 dmg, knockdown)   |
/// | LAUNCHER    | strike MID       | 15/2/20  | 55 dmg, Launch(1.5, 0.3, 40)           |
/// | JUGGLE_PALM | strike MID       | 8/2/10   | 40 dmg, hitstun 30, carry 0.5; chains  |
/// | SCREW_KICK  | strike MID       | 9/2/14   | 35 dmg, Screw(+1.8 carry, 34)          |
/// | BOUND_SLAM  | strike MID       | 10/2/16  | 45 dmg, Bound(32); its windows cost    |
/// |             |                  |          | Focus (the "special cancel" price)     |
/// | ENDER       | strike MID       | 10/2/18  | 60 dmg, hard knockdown (the oki ender) |
/// | WAKE_REVERSAL | strike MID     | 7/2/20   | req_down, invuln 0-8: the wake reversal|
#[must_use]
pub fn kit() -> Vec<Move> {
    let mut moves = vec![
        {
            let mut m = strike(
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
                1,
                CUE_QUICK,
            );
            m.cancels = vec![
                chain(7, 12, CancelGate::OnHit, MID_POKE, 2, 0),
                chain(7, 12, CancelGate::OnBlock, JAB, 3, 0),
            ];
            m
        },
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
                2,
                CUE_LOW_COIL,
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
            2,
            CUE_LOW_COIL,
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
                1,
                CUE_GUARD,
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
            m.gains = vec![ResourceGain {
                resource: GainResource::Focus,
                amount: 5,
                gate: GainGate::OnParry,
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
                2,
                CUE_RISING,
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
                3,
                CUE_FLURRY,
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
        // ── the combo string ────────────────────────────────────────────────
        {
            let mut m = strike(
                LAUNCHER,
                "rising dragon",
                Height::Mid,
                Timing {
                    startup: 15,
                    active: 2,
                    recovery: 20,
                },
                envelope(0, 200, 60, 180),
                vec![hit(
                    0,
                    55,
                    10,
                    14,
                    Reaction::Launch {
                        rise: fxf(150, 100),
                        carry: fxf(30, 100),
                        stun: 40,
                    },
                )],
                3,
                CUE_RISING,
            );
            m.cancels = vec![chain(16, 24, CancelGate::OnHit, JUGGLE_PALM, 2, 0)];
            // Juggle moves advance: the string chases its own carry (🔬 Tekken).
            m.motion.startup = PhaseMotion {
                forward: fxf(40, 100),
                lateral: fx(0),
            };
            m
        },
        {
            let mut m = strike(
                JUGGLE_PALM,
                "drifting palm",
                Height::Mid,
                Timing {
                    startup: 8,
                    active: 2,
                    recovery: 10,
                },
                envelope(0, 200, 70, 200),
                vec![hit(0, 40, 6, 10, Reaction::Hitstun { ticks: 30 })],
                1,
                CUE_FLURRY,
            );
            m.cancels = vec![
                chain(9, 14, CancelGate::OnHit, JUGGLE_PALM, 3, 0),
                chain(9, 14, CancelGate::OnHit, SCREW_KICK, 2, 0),
                chain(9, 14, CancelGate::OnHit, BOUND_SLAM, 2, 4),
                chain(9, 14, CancelGate::OnHit, ENDER, 2, 0),
            ];
            m.motion.startup = PhaseMotion {
                forward: fxf(80, 100),
                lateral: fx(0),
            };
            m
        },
        {
            let mut m = strike(
                SCREW_KICK,
                "spiral heel",
                Height::Mid,
                Timing {
                    startup: 9,
                    active: 2,
                    recovery: 14,
                },
                envelope(0, 200, 70, 200),
                vec![hit(
                    0,
                    35,
                    6,
                    10,
                    Reaction::Screw {
                        carry: fxf(180, 100),
                        stun: 34,
                    },
                )],
                2,
                CUE_FLURRY,
            );
            m.cancels = vec![
                chain(10, 16, CancelGate::OnHit, JUGGLE_PALM, 2, 0),
                chain(10, 16, CancelGate::OnHit, ENDER, 2, 0),
            ];
            m.motion.startup = PhaseMotion {
                forward: fx(1),
                lateral: fx(0),
            };
            m
        },
        {
            let mut m = strike(
                BOUND_SLAM,
                "meteor slam",
                Height::Mid,
                Timing {
                    startup: 10,
                    active: 2,
                    recovery: 16,
                },
                envelope(0, 200, 70, 200),
                vec![hit(0, 45, 6, 10, Reaction::Bound { stun: 32 })],
                3,
                CUE_FLURRY,
            );
            m.cancels = vec![
                chain(11, 17, CancelGate::OnHit, JUGGLE_PALM, 2, 0),
                chain(11, 17, CancelGate::OnHit, ENDER, 2, 0),
            ];
            m.motion.startup = PhaseMotion {
                forward: fxf(80, 100),
                lateral: fx(0),
            };
            m
        },
        {
            let mut m = strike(
                ENDER,
                "falling star",
                Height::Mid,
                Timing {
                    startup: 10,
                    active: 2,
                    recovery: 18,
                },
                envelope(0, 200, 70, 200),
                vec![hit(
                    0,
                    60,
                    8,
                    12,
                    Reaction::Knockdown {
                        hard: true,
                        down_ticks: 50,
                    },
                )],
                2,
                CUE_FLURRY,
            );
            m.motion.startup = PhaseMotion {
                forward: fxf(60, 100),
                lateral: fx(0),
            };
            m
        },
        {
            let mut m = strike(
                WAKE_REVERSAL,
                "rising tempest",
                Height::Mid,
                Timing {
                    startup: 7,
                    active: 2,
                    recovery: 20,
                },
                envelope(0, 180, 70, 200),
                vec![hit(0, 50, 8, 12, Reaction::Hitstun { ticks: 20 })],
                2,
                CUE_RISING,
            );
            m.req_down = true;
            m.properties = vec![PropertyWindow {
                from: 0,
                to: 8,
                kind: PropertyKind::Invuln {
                    covers: InvulnCover::All,
                },
            }];
            m
        },
        {
            let mut m = strike(
                BURST,
                "burst",
                Height::Mid,
                Timing {
                    startup: 0,
                    active: 1,
                    recovery: 1,
                },
                envelope(0, 180, 180, 180),
                vec![hit(0, 0, 0, 0, Reaction::Push { dist: fx(1) })],
                0,
                CUE_FLURRY,
            );
            m.flags.burst = true;
            m.properties = vec![PropertyWindow {
                from: 0,
                to: 1,
                kind: PropertyKind::Invuln {
                    covers: InvulnCover::All,
                },
            }];
            m
        },
        {
            let mut m = strike(
                RESCUE_STRIKE,
                "rescue shoulder",
                Height::Mid,
                Timing {
                    startup: 6,
                    active: 2,
                    recovery: 16,
                },
                envelope(0, 600, 180, 260),
                vec![hit(0, 55, 8, 12, Reaction::Hitstun { ticks: 20 })],
                2,
                CUE_FLURRY,
            );
            m.flags.rescue = true;
            m.motion.startup = PhaseMotion {
                forward: fxf(120, 100),
                lateral: fx(0),
            };
            m.properties = vec![PropertyWindow {
                from: 0,
                to: 5,
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
        {
            let mut m = strike(
                REVIVE,
                "revive",
                Height::None,
                Timing {
                    startup: 8,
                    active: 1,
                    recovery: 18,
                },
                envelope(0, 100, 50, 50),
                vec![],
                4,
                CUE_GUARD,
            );
            m.category = MoveCategory::Utility;
            m.flags.revive_hp = 250;
            m
        },
        {
            let mut m = strike(
                WIDE_CLEAVE,
                "wide cleave",
                Height::Mid,
                Timing {
                    startup: 10,
                    active: 2,
                    recovery: 20,
                },
                envelope(0, 280, 180, 220),
                vec![hit(0, 65, 10, 14, Reaction::Hitstun { ticks: 18 })],
                3,
                CUE_FLURRY,
            );
            m.tracking = Tracking::Homing;
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
            juggle_carry: fx(0),
            reaction: Reaction::Knockdown {
                hard: true,
                down_ticks: 50,
            },
            ch_reaction: None,
        }],
        region: envelope(0, 90, 0, 0),
        motion: SelfMotion::default(),
        properties: vec![],
        cost: MoveCost {
            breath: 2,
            ap: 2,
            focus: 0,
        },
        gains: vec![],
        cancels: vec![],
        startup_cancelable: false,
        cue: CUE_LUNGE,
        req_stance: None,
        req_down: false,
        break_key: Some(key),
        stance_spec: None,
        flags: MoveFlags::default(),
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
        cost: MoveCost {
            breath: 1,
            ap: 1,
            focus: 0,
        },
        gains: vec![],
        cancels: vec![],
        startup_cancelable: false,
        // The dash shares the grab's lunging shape (the approach ambiguity); steps and
        // backdashes read as weight shifts.
        cue: if id == DASH_IN { CUE_LUNGE } else { CUE_STEP },
        req_stance: None,
        req_down: false,
        break_key: None,
        stance_spec: None,
        flags: MoveFlags::default(),
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
        cost: MoveCost::default(),
        gains: vec![],
        cancels: vec![],
        startup_cancelable: false,
        cue: CUE_GUARD,
        req_stance: None,
        req_down: false,
        break_key: None,
        stance_spec: Some(spec),
        flags: MoveFlags::default(),
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
        hitstun_decay_step: 2,
        juggle_decay_step: fxf(1, 10),
        extender_latches: ExtenderLatches {
            screw: 1,
            bound: 1,
            wall_splat: 1,
        },
        forced_landing: true,
        splat_duration: 25,
        landing_down_ticks: 35,
        wake_rise_ticks: 8,
        wake_back_rise_push: fxf(80, 100),
        wake_back_rise_ticks: 12,
        wake_delay_max: 30,
        focus_gains: FocusGains {
            land_hit: 2,
            hit_blocked: 1,
            take_damage_per_100: 3,
            parry: 8,
            counter_hit: 6,
            whiff_punish: 10,
        },
    }
}

#[must_use]
pub fn defense() -> DefenseProfile {
    DefenseProfile {
        hp_max: 1000,
        guard_max: 50,
        guard_regen_interval: 20,
        weight: Fx::ONE,
        breath_max: 100,
        breath_regen_interval: 1,
        ap_max: 24,
        focus_max: 50,
        visibility: MeterVisibility::default(),
    }
}

fn arena() -> ArenaDef {
    ArenaDef {
        half_extents: FxVec2::new(fx(10), fx(6)),
        walls: Walls {
            east: WallSpec { splattable: true },
            west: WallSpec { splattable: true },
            north: WallSpec { splattable: true },
            south: WallSpec { splattable: true },
        },
    }
}

/// A 1v1 with both fighters at the given x positions (in cm), facing each other.
#[must_use]
pub fn duel_at(ax_cm: i32, bx_cm: i32) -> CombatSim {
    duel_with(ruleset(), ax_cm, bx_cm)
}

/// As `duel_at`, but with no splat-able walls (pure clamp — geometry-neutral tests).
#[must_use]
pub fn duel_open_at(ax_cm: i32, bx_cm: i32) -> CombatSim {
    let mut sim_arena = arena();
    sim_arena.walls = Walls::default();
    duel_full(ruleset(), sim_arena, ax_cm, bx_cm)
}

/// As `duel_at`, but under a custom Ruleset (governor-focused tests retune the curves).
#[must_use]
pub fn duel_with(rs: Ruleset, ax_cm: i32, bx_cm: i32) -> CombatSim {
    duel_full(rs, arena(), ax_cm, bx_cm)
}

/// As `duel_at`, plus per-side knowledge books (the fog-gradient tests, spec §7.3).
#[must_use]
pub fn duel_knowing(
    ax_cm: i32,
    bx_cm: i32,
    knowledge: BTreeMap<SideId, KnowledgeBook>,
) -> CombatSim {
    let mut config = duel_config(ruleset(), arena(), ax_cm, bx_cm);
    config.knowledge = knowledge;
    CombatSim::new(config)
}

fn duel_full(rs: Ruleset, sim_arena: ArenaDef, ax_cm: i32, bx_cm: i32) -> CombatSim {
    CombatSim::new(duel_config(rs, sim_arena, ax_cm, bx_cm))
}

fn duel_config(rs: Ruleset, sim_arena: ArenaDef, ax_cm: i32, bx_cm: i32) -> SimConfig {
    SimConfig {
        arena: sim_arena,
        ruleset: rs,
        entities: vec![
            EntitySetup {
                id: A,
                side: SIDE_A,
                pos: FxVec2::new(fxf(ax_cm, 100), fx(0)),
                target: B,
                ready_at: Tick::ZERO,
                defense: defense(),
                moves: kit(),
            },
            EntitySetup {
                id: B,
                side: SIDE_B,
                pos: FxVec2::new(fxf(bx_cm, 100), fx(0)),
                target: A,
                ready_at: Tick::ZERO,
                defense: defense(),
                moves: kit(),
            },
        ],
        max_ticks: 600,
        knowledge: BTreeMap::new(),
    }
}

/// A 1v1 centered on the origin, `gap_cm` apart.
#[must_use]
pub fn duel(gap_cm: i32) -> CombatSim {
    duel_at(-gap_cm / 2, gap_cm / 2)
}

/// Declarative script driver: choices keyed by (tick, actor). Unscripted prompts get
/// defaults (Wait 4 / HoldStance / decline breaks and cancels / rise) so scenarios stay
/// terse.
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
        self.at
            .get(&(t.0, actor.0))
            .copied()
            .filter(|c| fits(*c, kind))
            .unwrap_or(default_choice(kind))
    }
}

/// Does a choice fit a prompt kind? (An actor can face two prompts on one tick — a
/// tick-start decision and a mid-tick throw break — so replays match by kind.)
#[must_use]
pub fn fits(choice: Choice, kind: DecisionKind) -> bool {
    matches!(
        (kind, choice),
        (
            DecisionKind::Ready,
            Choice::Wait { .. }
                | Choice::Move { .. }
                | Choice::MoveAt { .. }
                | Choice::SwitchFocus { .. }
        ) | (
            DecisionKind::StanceReevaluate,
            Choice::HoldStance
                | Choice::Release
                | Choice::Move { .. }
                | Choice::MoveAt { .. }
                | Choice::SwitchFocus { .. }
        ) | (DecisionKind::ThrowBreak { .. }, Choice::ThrowBreak { .. })
            | (DecisionKind::Cancel, Choice::Cancel { .. })
            | (
                DecisionKind::WakeUp,
                Choice::Rise
                    | Choice::BackRise
                    | Choice::DelayRise { .. }
                    | Choice::Move { .. }
                    | Choice::MoveAt { .. }
            )
            | (
                DecisionKind::Burst,
                Choice::Wait { .. } | Choice::Move { .. } | Choice::MoveAt { .. }
            )
    )
}

#[must_use]
pub fn default_choice(kind: DecisionKind) -> Choice {
    match kind {
        DecisionKind::Ready => Choice::Wait { ticks: 4 },
        DecisionKind::StanceReevaluate => Choice::HoldStance,
        DecisionKind::ThrowBreak { .. } => Choice::ThrowBreak { guess: None },
        DecisionKind::Cancel => Choice::Cancel { into: None },
        DecisionKind::WakeUp => Choice::Rise,
        DecisionKind::Burst => Choice::Wait { ticks: 1 },
    }
}

/// Rebuild a replayable script from a trace's Committed events: a multimap consumed in
/// trace order, matched to each prompt by kind (C-DET: the trace IS the decision log).
#[must_use]
pub fn replay_script(trace: &[engine::trace::TraceEvent]) -> ReplayScript {
    let mut at: BTreeMap<(u64, u32), Vec<Choice>> = BTreeMap::new();
    for e in trace {
        if let engine::trace::TraceEvent::Committed { t, actor, choice } = e {
            at.entry((t.0, actor.0)).or_default().push(*choice);
        }
    }
    ReplayScript { at }
}

pub struct ReplayScript {
    at: BTreeMap<(u64, u32), Vec<Choice>>,
}

/// Drive a sim from a recorded script until it ends.
pub fn run_replay(sim: &mut CombatSim, script: &mut ReplayScript) -> SimStatus {
    let mut guard = 0u32;
    loop {
        guard += 1;
        assert!(guard < 1_000_000, "replay terminates");
        match sim.advance() {
            SimStatus::Over { winner } => return SimStatus::Over { winner },
            SimStatus::AwaitingDecisions => {
                let pending = sim.pending();
                if pending
                    .iter()
                    .all(|p| !script.at.contains_key(&(sim.tick().0, p.actor.0)))
                    && sim.tick().0 > script.at.keys().map(|k| k.0).max().unwrap_or(0)
                {
                    // Past the recording's horizon: the original stopped here.
                    return SimStatus::AwaitingDecisions;
                }
                for p in pending {
                    let key = (sim.tick().0, p.actor.0);
                    let choice = script
                        .at
                        .get_mut(&key)
                        .and_then(|v| {
                            let idx = v.iter().position(|c| fits(*c, p.kind))?;
                            Some(v.remove(idx))
                        })
                        .unwrap_or(default_choice(p.kind));
                    sim.commit_side(p.side, &[(p.actor, choice)])
                        .expect("recorded choice replays");
                }
            }
        }
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
                    if let Err(e) = sim.commit_side(side, &choices) {
                        panic!(
                            "scripted choice is illegal at {}: {e:?}\npending: {pending:?}\nchoices: {choices:?}",
                            sim.tick()
                        );
                    }
                }
            }
        }
    }
}
