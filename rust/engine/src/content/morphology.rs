//! Morphology (the body plan → a 3D stick figure) + the modifiers that depend on it.
//!
//! A modifier is **not orthogonal** to morphology: it requires a base part to attach to (a claw needs
//! an arm), but a base part doesn't imply the modifier (an arm isn't necessarily clawed). A modifier
//! *adds* a part ([`crate::fighting::BodyPart::Fangs`] / `Claws`), so the engine's morphology gate
//! then handles the rest — only a body that has `Fangs` can bite.

use crate::fighting::{Body, BodyPart, Box3, Vec3};

/// The base body plan — defines the stick-figure parts (hurtboxes + attack sources).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Morphology {
    /// Upright: head, torso, legs, two arms (`Fist`), feet (`Foot`).
    Biped,
    /// On all fours: head, torso, legs, paws (`Foot`) — no arms.
    Quadruped,
}

impl Morphology {
    /// The base parts this plan has (used to gate which modifiers/moves are possible).
    pub fn parts(self) -> &'static [BodyPart] {
        match self {
            Morphology::Biped => {
                &[BodyPart::Head, BodyPart::Torso, BodyPart::Legs, BodyPart::Fist, BodyPart::Foot]
            }
            Morphology::Quadruped => {
                &[BodyPart::Head, BodyPart::Torso, BodyPart::Legs, BodyPart::Foot]
            }
        }
    }

    /// The stick-figure body: each base part as a simple box.
    pub fn body(self) -> Body {
        let parts = match self {
            Morphology::Biped => vec![
                (BodyPart::Torso, Box3::new(Vec3::new(0.0, 1.0, 0.0), Vec3::new(0.3, 0.5, 0.2))),
                (BodyPart::Head, Box3::new(Vec3::new(0.0, 1.7, 0.0), Vec3::new(0.2, 0.2, 0.2))),
                (BodyPart::Legs, Box3::new(Vec3::new(0.0, 0.4, 0.0), Vec3::new(0.25, 0.4, 0.2))),
                (BodyPart::Fist, Box3::new(Vec3::new(0.45, 1.1, 0.0), Vec3::new(0.15, 0.15, 0.15))),
                (BodyPart::Foot, Box3::new(Vec3::new(0.2, 0.1, 0.0), Vec3::new(0.15, 0.12, 0.15))),
            ],
            Morphology::Quadruped => vec![
                (BodyPart::Torso, Box3::new(Vec3::new(0.0, 0.6, 0.0), Vec3::new(0.5, 0.3, 0.25))),
                (BodyPart::Head, Box3::new(Vec3::new(0.6, 0.7, 0.0), Vec3::new(0.25, 0.22, 0.2))),
                (BodyPart::Legs, Box3::new(Vec3::new(0.0, 0.25, 0.0), Vec3::new(0.45, 0.25, 0.22))),
                (BodyPart::Foot, Box3::new(Vec3::new(0.3, 0.08, 0.0), Vec3::new(0.18, 0.1, 0.18))),
            ],
        };
        Body { parts }
    }
}

/// A trait layered on a morphology. Each requires a base part and adds a new attack-source part.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Modifier {
    /// Fangs → a bite. Requires a `Head`; adds `Fangs`.
    Fanged,
    /// Claws → a claw swipe. Requires a `Fist` (an arm); adds `Claws`.
    Clawed,
}

impl Modifier {
    /// The base part this modifier must attach to.
    pub fn requires(self) -> BodyPart {
        match self {
            Modifier::Fanged => BodyPart::Head,
            Modifier::Clawed => BodyPart::Fist,
        }
    }
    /// The part it adds (a new attack source).
    pub fn adds(self) -> BodyPart {
        match self {
            Modifier::Fanged => BodyPart::Fangs,
            Modifier::Clawed => BodyPart::Claws,
        }
    }
    /// The added part's box, derived from the box of the part it attaches to (a smaller box just in
    /// front of it). Morphology-agnostic — fangs sit at whichever head the body has.
    pub fn added_box(self, base: Box3) -> Box3 {
        Box3::new(
            Vec3::new(base.center.x + base.half.x, base.center.y, base.center.z),
            base.half * 0.7,
        )
    }
    /// Can this modifier attach to a body with these base parts?
    pub fn compatible_with(self, parts: &[BodyPart]) -> bool {
        parts.contains(&self.requires())
    }
}
