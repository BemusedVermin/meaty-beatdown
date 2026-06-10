//! Fixed-point math — `Fx` and `FxVec2`.
//!
//! **Behavioral contract (tech-plan §2.1):** `Fx` is `fixed::types::I32F32` — Q32.32, a
//! 64-bit signed fixed-point number with 32 integer and 32 fractional bits. The format is
//! part of the determinism contract: changing it changes every trace and re-freezes the
//! golden vectors. It is documented here once and never changed casually.
//!
//! **Thin glue only** (library policy, tech-plan §1.1): arithmetic delegates to `fixed`,
//! sqrt delegates to `cordic`. No numeric algorithms are implemented in this module — adding
//! one is a review-blocking smell. The unit tests pin exact bit patterns for representative
//! inputs; those pins are the cordic determinism gate (implementation-plan Phase 0) and CI
//! runs them on Linux and Windows, so a pass on both is a cross-platform bit-identity proof.

use serde::{Deserialize, Serialize};
use std::ops::{Add, AddAssign, Mul, Neg, Sub, SubAssign};

/// The engine scalar. See the module docs: Q32.32, contract-frozen.
pub type Fx = fixed::types::I32F32;

/// Shorthand constructor for integer-valued `Fx` (test and content ergonomics).
#[must_use]
pub fn fx(value: i32) -> Fx {
    Fx::from_num(value)
}

/// A position / direction on the ground plane (spec §3.1). Plain data; all behavior
/// delegates to `fixed` and `cordic`.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FxVec2 {
    pub x: Fx,
    pub y: Fx,
}

impl FxVec2 {
    pub const ZERO: Self = Self {
        x: Fx::ZERO,
        y: Fx::ZERO,
    };

    #[must_use]
    pub const fn new(x: Fx, y: Fx) -> Self {
        Self { x, y }
    }

    #[must_use]
    pub fn dot(self, rhs: Self) -> Fx {
        self.x * rhs.x + self.y * rhs.y
    }

    #[must_use]
    pub fn length_sq(self) -> Fx {
        self.dot(self)
    }

    /// Euclidean length, via `cordic::sqrt`.
    #[must_use]
    pub fn length(self) -> Fx {
        cordic::sqrt(self.length_sq())
    }

    #[must_use]
    pub fn distance(self, rhs: Self) -> Fx {
        (rhs - self).length()
    }

    /// Unit vector in this direction, or `ZERO` for the zero vector (facing math must not
    /// panic when two actors share a position).
    #[must_use]
    pub fn normalize_or_zero(self) -> Self {
        let len = self.length();
        if len == Fx::ZERO {
            Self::ZERO
        } else {
            Self {
                x: self.x / len,
                y: self.y / len,
            }
        }
    }

    /// The counter-clockwise perpendicular `(-y, x)` — the lateral (sidestep) axis of a
    /// lane (spec §3.2).
    #[must_use]
    pub fn perp(self) -> Self {
        Self {
            x: -self.y,
            y: self.x,
        }
    }
}

impl Add for FxVec2 {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        Self {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
        }
    }
}

impl AddAssign for FxVec2 {
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

impl Sub for FxVec2 {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self {
        Self {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
        }
    }
}

impl SubAssign for FxVec2 {
    fn sub_assign(&mut self, rhs: Self) {
        *self = *self - rhs;
    }
}

impl Neg for FxVec2 {
    type Output = Self;

    fn neg(self) -> Self {
        Self {
            x: -self.x,
            y: -self.y,
        }
    }
}

impl Mul<Fx> for FxVec2 {
    type Output = Self;

    fn mul(self, rhs: Fx) -> Self {
        Self {
            x: self.x * rhs,
            y: self.y * rhs,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// |a - b| within `2^-24` — the accuracy bar for the cordic evaluation. (Q32.32 has
    /// 2^-32 resolution; cordic converges to well under this bar for our magnitude range.)
    fn assert_close(a: Fx, b: Fx) {
        let eps = Fx::from_bits(1 << 8); // 2^-24
        assert!((a - b).abs() <= eps, "{a} !~ {b}");
    }

    #[test]
    fn arithmetic_delegates_to_fixed() {
        let a = FxVec2::new(fx(3), fx(4));
        let b = FxVec2::new(fx(-1), fx(2));
        assert_eq!(a + b, FxVec2::new(fx(2), fx(6)));
        assert_eq!(a - b, FxVec2::new(fx(4), fx(2)));
        assert_eq!(-a, FxVec2::new(fx(-3), fx(-4)));
        assert_eq!(a * fx(2), FxVec2::new(fx(6), fx(8)));
        assert_eq!(a.dot(b), fx(5));
        assert_eq!(a.length_sq(), fx(25));
        assert_eq!(a.perp(), FxVec2::new(fx(-4), fx(3)));
    }

    #[test]
    fn sqrt_accuracy_reference_values() {
        // The cordic evaluation gate: known roots, within the accuracy bar.
        assert_close(FxVec2::new(fx(3), fx(4)).length(), fx(5));
        assert_close(FxVec2::new(fx(1), fx(0)).length(), fx(1));
        assert_close(cordic::sqrt(fx(2)), Fx::from_bits(0x16A09E667)); // sqrt(2), true rounded
        assert_close(cordic::sqrt(fx(9)), fx(3));
        assert_eq!(FxVec2::ZERO.length(), Fx::ZERO);
    }

    #[test]
    fn normalize_handles_zero_and_unit() {
        assert_eq!(FxVec2::ZERO.normalize_or_zero(), FxVec2::ZERO);
        let n = FxVec2::new(fx(10), fx(0)).normalize_or_zero();
        assert_eq!(n.x, fx(1));
        assert_eq!(n.y, fx(0));
        let d = FxVec2::new(fx(1), fx(1)).normalize_or_zero();
        assert_close(d.length(), fx(1));
    }

    /// THE DETERMINISM PINS. Exact output bits of every delegated operation for
    /// representative inputs, captured at the Phase 0 evaluation (cordic 0.1, fixed 1.x).
    /// CI runs this on Linux AND Windows: passing on both is the cross-platform bit-identity
    /// proof for the math layer. If a deliberate dependency upgrade shifts these, that is a
    /// behavioral change: re-pin with a changelog entry (standing rule 4).
    #[test]
    fn determinism_bit_pins() {
        // Captured 2026-06-10 (cordic 0.1.5, fixed 1.x, rustc 1.96.0). Notably exact:
        // sqrt(2) and sqrt(5) are the true round-to-nearest Q32.32 values; length(3,4) is 5.0.
        let pins: &[(Fx, i64)] = &[
            (cordic::sqrt(fx(2)), 0x1_6A09_E668),
            (cordic::sqrt(fx(5)), 0x2_3C6E_F373),
            (cordic::sqrt(Fx::from_bits(0x1_8000_0000)), 0x1_3988_E140), // sqrt(1.5)
            (FxVec2::new(fx(3), fx(4)).length(), 0x5_0000_0000),
            (FxVec2::new(fx(1), fx(1)).normalize_or_zero().x, 0xB504_F333),
            (FxVec2::new(fx(-7), fx(2)).length(), 0x7_47B5_481D),
        ];
        for (i, (actual, expected_bits)) in pins.iter().enumerate() {
            assert_eq!(
                actual.to_bits(),
                *expected_bits,
                "pin {i}: got bits {:#x}",
                actual.to_bits()
            );
        }
    }
}
