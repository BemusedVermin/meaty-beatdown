//! The tick: 1 tick = 1 frame at 60 Hz (spec §2.1). A single global counter `T` is shared by
//! all actors; the wall clock is irrelevant — the engine advances `T` only when no actor
//! needs to decide.

use serde::{Deserialize, Serialize};
use std::ops::Add;

/// A point on the shared tick timeline.
#[derive(
    Copy, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub struct Tick(pub u64);

impl Tick {
    pub const ZERO: Self = Self(0);

    /// Advance by one tick (the bottom of the advance loop, spec §4.3).
    pub fn advance(&mut self) {
        self.0 += 1;
    }
}

impl Add<u64> for Tick {
    type Output = Self;

    fn add(self, rhs: u64) -> Self {
        Self(self.0 + rhs)
    }
}

impl std::fmt::Display for Tick {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "T{}", self.0)
    }
}
