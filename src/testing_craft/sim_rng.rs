//! # Seeded Determinism
//!
//! Every randomized test in this category draws from a [`SimRng`] built from
//! an explicit seed. When a test fails it prints the seed, and re-running with
//! that seed replays the exact same inputs, interleaving choices, and crash
//! decisions. "Random" without a seed is just "flaky".
//!
//! The generator is `SplitMix64`: tiny, fast, statistically good enough for
//! test input generation, and trivially reproducible across platforms.
//!
//! ```
//! use rust_interview_practice::testing_craft::sim_rng::SimRng;
//!
//! let mut a = SimRng::new(42);
//! let mut b = SimRng::new(42);
//! assert_eq!(a.next_u64(), b.next_u64()); // same seed, same stream
//! assert!(a.below(10) < 10);
//! ```

/// Deterministic pseudo-random generator for tests (`SplitMix64`).
#[derive(Debug, Clone)]
pub struct SimRng {
    state: u64,
}

impl SimRng {
    /// Creates a generator that will always produce the same stream for `seed`.
    #[must_use]
    pub const fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    /// Returns the next 64 random bits.
    pub const fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Returns a value in `0..bound`. Returns 0 when `bound == 0`.
    ///
    /// Uses Lemire's multiply-shift reduction: unbiased enough for testing and
    /// branch-free.
    #[allow(clippy::cast_possible_truncation)] // high 64 bits of a u128 product fit in u64
    pub const fn below(&mut self, bound: usize) -> usize {
        if bound == 0 {
            return 0;
        }
        let wide = (self.next_u64() as u128) * (bound as u128);
        (wide >> 64) as usize
    }

    /// Returns `true` with probability `numerator / denominator`.
    pub const fn chance(&mut self, numerator: usize, denominator: usize) -> bool {
        self.below(denominator) < numerator
    }

    /// Returns a random byte.
    #[allow(clippy::cast_possible_truncation)] // deliberately keep the low 8 bits
    pub const fn byte(&mut self) -> u8 {
        self.next_u64() as u8
    }

    /// Returns `len` random bytes.
    pub fn bytes(&mut self, len: usize) -> Vec<u8> {
        (0..len).map(|_| self.byte()).collect()
    }

    /// Picks a random element of `items`, or `None` when it is empty.
    pub fn pick<'a, T>(&mut self, items: &'a [T]) -> Option<&'a T> {
        if items.is_empty() {
            None
        } else {
            Some(&items[self.below(items.len())])
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_same_seed_same_stream() {
        let mut a = SimRng::new(7);
        let mut b = SimRng::new(7);
        for _ in 0..100 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn test_different_seeds_diverge() {
        let mut a = SimRng::new(1);
        let mut b = SimRng::new(2);
        assert_ne!(a.next_u64(), b.next_u64());
    }

    #[test]
    fn test_below_respects_bound() {
        let mut rng = SimRng::new(3);
        for bound in 1..50 {
            for _ in 0..50 {
                assert!(rng.below(bound) < bound);
            }
        }
        assert_eq!(rng.below(0), 0);
    }

    #[test]
    fn test_below_hits_every_bucket() {
        let mut rng = SimRng::new(11);
        let mut seen = [false; 8];
        for _ in 0..500 {
            seen[rng.below(8)] = true;
        }
        assert!(seen.iter().all(|&s| s));
    }

    #[test]
    fn test_chance_extremes() {
        let mut rng = SimRng::new(5);
        assert!((0..100).all(|_| rng.chance(1, 1)));
        assert!((0..100).all(|_| !rng.chance(0, 1)));
    }

    #[test]
    fn test_bytes_and_pick() {
        let mut rng = SimRng::new(9);
        assert_eq!(rng.bytes(17).len(), 17);
        let empty: [u8; 0] = [];
        assert_eq!(rng.pick(&empty), None);
        let items = [10, 20, 30];
        let picked = *rng.pick(&items).expect("non-empty");
        assert!(items.contains(&picked));
    }
}
