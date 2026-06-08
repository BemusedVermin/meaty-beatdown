//! 3D hit geometry. A contact is an **AABB overlap** in a 3-axis space:
//! **X = lane** (forward/back distance — the primary spacing axis), **Y = vertical** (highs / lows /
//! anti-air), **Z = lateral** (the Tekken sidestep axis).
//!
//! Boxes are authored as center + half-extents ([`Box3`]); the actual overlap test uses
//! **`bevy_math`**'s [`Aabb3d`] (`IntersectsVolume`) — we do not hand-roll collision. (This is the
//! one place the engine leans on `bevy_math`/glam; it's *math only* — no ECS, no App, no real-time.)
//!
//! A fighter's hittable volume and its attack-box sources are the **same body parts**, so hitboxes
//! "fall out" of the body + move: a punch uses the `Fist` box, a kick the `Foot` box, each sized to
//! that fighter's body. [`super::frame::HitboxSource::Custom`] covers anything bespoke.

pub use bevy_math::Vec3;
use bevy_math::bounding::{Aabb3d, IntersectsVolume};

/// An axis-aligned box: center + half-extents, on the 3 axes (X lane, Y vertical, Z lateral).
#[derive(Clone, Copy, Debug)]
pub struct Box3 {
    pub center: Vec3,
    pub half: Vec3,
}
impl Box3 {
    pub const fn new(center: Vec3, half: Vec3) -> Self {
        Self { center, half }
    }
    /// A degenerate empty box (fallback for a missing body part).
    pub const ZERO: Box3 = Box3 { center: Vec3::ZERO, half: Vec3::ZERO };
    /// Convert to the library bounding type for the overlap test.
    pub fn aabb(&self) -> Aabb3d {
        Aabb3d::new(self.center, self.half)
    }
}

/// A named body region — serves as a hurtbox *and* as a potential attack-box source.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum BodyPart {
    Head,
    Torso,
    Legs,
    Fist,
    Foot,
    Knee,
    Elbow,
    /// Added by the `Fanged` modifier (attaches to a `Head`) — the bite source.
    Fangs,
    /// Added by the `Clawed` modifier (attaches to a `Fist`) — the claw source.
    Claws,
}

/// Place a body-local box into world space: mirror X by facing, then translate by `pos`.
pub fn place(local: Box3, pos: Vec3, facing: i8) -> Box3 {
    let f = facing as f32;
    Box3 {
        center: Vec3::new(
            pos.x + f * local.center.x,
            pos.y + local.center.y,
            pos.z + local.center.z,
        ),
        half: local.half,
    }
}

/// True iff the hitbox overlaps any hurtbox — the 3D AABB intersection, via `bevy_math`.
pub fn overlaps(hit: &Box3, hurts: &[Box3]) -> bool {
    let h = hit.aabb();
    hurts.iter().any(|b| h.intersects(&b.aabb()))
}
