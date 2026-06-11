//! Target-lane spatial math (spec §3): the lane is the segment from actor to target; all
//! spacing runs along it (`forward`), lateral evasion perpendicular to it (`lateral`,
//! positive = the attacker's left). `does_hit` stays one predicate — the whole spatial
//! model hides behind it.

use crate::core::fx::{Fx, FxVec2};
use crate::data::movedef::{Height, Move, MoveCategory, Tracking};
use crate::data::{ArenaDef, WallSpec};

use super::entity::{Entity, Stance};

/// A victim's position decomposed onto an attacker's facing axis.
#[derive(Copy, Clone, Debug)]
pub struct LaneOffsets {
    /// Distance along the attacker's facing (range axis).
    pub forward: Fx,
    /// Perpendicular deviation (positive = the attacker's left).
    pub lateral: Fx,
}

#[must_use]
pub fn lane_offsets(
    attacker_pos: FxVec2,
    attacker_facing: FxVec2,
    victim_pos: FxVec2,
) -> LaneOffsets {
    let delta = victim_pos - attacker_pos;
    LaneOffsets {
        forward: delta.dot(attacker_facing),
        lateral: delta.dot(attacker_facing.perp()),
    }
}

/// The spatial + phase-independent part of `does_hit` (spec §3.3): range, arc (with
/// tracking realignment, §3.5), and the height/stance clause (§5.2). The (phase) clause
/// is checked by the caller (it owns the tick bookkeeping) and the (type) clause —
/// invulnerability — by the resolver's priority table (spec §5.1 step 1).
#[must_use]
pub fn does_hit_spatially(attacker: &Entity, mv: &Move, victim: &Entity) -> bool {
    // Downed bodies are below every Phase 1 envelope (okizeme arrives with the combo
    // system in Phase 2).
    if victim.stance == Stance::Down {
        return false;
    }

    // (height) — the Tekken triangle: a HIGH whiffs entirely over a crouching victim.
    if mv.height == Height::High && victim.stance == Stance::Crouching {
        return false;
    }

    let off = lane_offsets(attacker.pos, attacker.facing, victim.pos);

    // (range) along the facing axis (the move's authored advance has already moved the
    // attacker by the time its active window opens).
    if off.forward < mv.region.min_range || off.forward > mv.region.max_range {
        return false;
    }

    // (arc) — the lateral band, widened on tracked sides. A sidestep off a LINEAR band
    // routes through the same whiff path as a baited whiff (spec §3.3).
    let base = mv.region.arc_halfwidth;
    let track = mv.region.track_halfwidth;
    let (left_cover, right_cover) = match mv.tracking {
        Tracking::Linear => (base, base),
        Tracking::TrackL => (track, base),
        Tracking::TrackR => (base, track),
        Tracking::Homing => (track, track),
    };
    // lateral > 0 = victim toward the attacker's left.
    if off.lateral > left_cover || off.lateral < -right_cover {
        return false;
    }

    // Throws realign on auto-facing but still require a standing, grabbable victim —
    // checked in the resolver; spatially they behave like very short strikes.
    debug_assert!(
        mv.category != MoveCategory::Stance,
        "stances have no envelope"
    );
    true
}

/// Clamp a position to the arena boundary. Returns the clamped position plus the wall
/// that did the clamping, if any (the splat check's input, spec §3.7). A corner clamp
/// reports the x-axis wall (stable, arbitrary).
#[must_use]
pub fn clamp_to_arena(arena: &ArenaDef, pos: FxVec2) -> (FxVec2, Option<WallSpec>) {
    let clamped = FxVec2::new(
        pos.x.clamp(-arena.half_extents.x, arena.half_extents.x),
        pos.y.clamp(-arena.half_extents.y, arena.half_extents.y),
    );
    let wall = if pos.x > arena.half_extents.x {
        Some(arena.walls.east)
    } else if pos.x < -arena.half_extents.x {
        Some(arena.walls.west)
    } else if pos.y > arena.half_extents.y {
        Some(arena.walls.north)
    } else if pos.y < -arena.half_extents.y {
        Some(arena.walls.south)
    } else {
        None
    };
    (clamped, wall)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::fx::fx;

    #[test]
    fn lane_offsets_decompose() {
        // Attacker at origin facing +x; victim 3 forward, 2 to the left (+y).
        let off = lane_offsets(
            FxVec2::ZERO,
            FxVec2::new(fx(1), fx(0)),
            FxVec2::new(fx(3), fx(2)),
        );
        assert_eq!(off.forward, fx(3));
        assert_eq!(off.lateral, fx(2));
        // Facing -x flips both axes.
        let off = lane_offsets(
            FxVec2::ZERO,
            FxVec2::new(fx(-1), fx(0)),
            FxVec2::new(fx(3), fx(2)),
        );
        assert_eq!(off.forward, fx(-3));
        assert_eq!(off.lateral, fx(-2));
    }

    #[test]
    fn arena_clamps_and_reports_the_wall() {
        let arena = ArenaDef {
            half_extents: FxVec2::new(fx(10), fx(6)),
            walls: crate::data::Walls {
                east: WallSpec { splattable: true },
                ..Default::default()
            },
        };
        let (p, wall) = clamp_to_arena(&arena, FxVec2::new(fx(14), fx(-9)));
        assert_eq!(p, FxVec2::new(fx(10), fx(-6)));
        assert_eq!(
            wall,
            Some(WallSpec { splattable: true }),
            "x clamp wins at corners"
        );
        let (_, none) = clamp_to_arena(&arena, FxVec2::new(fx(3), fx(2)));
        assert_eq!(none, None);
    }
}
