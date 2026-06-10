//! Seeded randomness — the only randomness the engine is allowed (C-DET: no ambient RNG,
//! no wall clock). Combat resolution uses none of this (no dice in combat); worldgen and
//! loot consume seeds carried in sim state.
//!
//! Sequence stability is guaranteed by the pinned `fastrand` version in `Cargo.lock` plus
//! the determinism suite; a deliberate `fastrand` upgrade that changes sequences must be
//! treated like any other behavioral change (re-freeze, changelog).

/// A seeded generator stored in sim state. Thin wrapper over `fastrand` (library policy).
#[derive(Debug, Clone)]
pub struct SeededRng(fastrand::Rng);

impl SeededRng {
    #[must_use]
    pub fn new(seed: u64) -> Self {
        Self(fastrand::Rng::with_seed(seed))
    }

    /// Uniform `u64` in `range`.
    pub fn u64(&mut self, range: std::ops::Range<u64>) -> u64 {
        self.0.u64(range)
    }

    /// Uniform `usize` in `range` (slice indexing).
    pub fn usize(&mut self, range: std::ops::Range<usize>) -> usize {
        self.0.usize(range)
    }

    /// Fork an independent child generator (e.g. one stream per hex / per loot roll), so
    /// consumption order in one subsystem can't perturb another.
    pub fn fork(&mut self) -> Self {
        Self(fastrand::Rng::with_seed(self.0.u64(..)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_same_sequence() {
        let mut a = SeededRng::new(0xDEAD_BEEF);
        let mut b = SeededRng::new(0xDEAD_BEEF);
        for _ in 0..64 {
            assert_eq!(a.u64(0..u64::MAX), b.u64(0..u64::MAX));
        }
    }

    #[test]
    fn forks_are_independent_streams() {
        let mut root_a = SeededRng::new(7);
        let mut root_b = SeededRng::new(7);
        let mut fork_a = root_a.fork();
        // Consuming from the fork must not perturb the root's stream.
        let _ = fork_a.u64(0..100);
        let _ = fork_a.u64(0..100);
        let _ = root_b.fork();
        assert_eq!(root_a.u64(0..u64::MAX), root_b.u64(0..u64::MAX));
    }
}
