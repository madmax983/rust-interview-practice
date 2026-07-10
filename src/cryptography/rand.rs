//! # Pseudo-Random Number Generator Implementation
//!
//! Implements a fast, non-cryptographically secure Pseudo-Random Number Generator (PRNG).
//! Specifically, this implements the **Xoroshiro128+** (XOR/rotate/shift/rotate) algorithm.
//!
//! **Replaces Crates:** `rand` (specifically `rand_pcg` or `rand_xoshiro` components)
//!
//! **Real-world Usage:**
//! - Game development for deterministic procedural generation (e.g., Minecraft chunk generation).
//! - Monte Carlo simulations in data science.
//! - Randomized algorithms (like quicksort pivot selection or Skip List level generation).
//!
//! **Why build it yourself?**
//! Understanding PRNGs shows how mathematical bitwise operations can create statistically
//! robust randomness. It teaches you the importance of seeding, state management, and
//! the difference between non-cryptographic (fast, predictable) and cryptographic (slow, unpredictable)
//! random number generators.

// =========================================================================================
// Architecture
// =========================================================================================
//
// Xoroshiro128+ State:
// Consists of two 64-bit integers (`s0`, `s1`).
//
// Next Value Generation (xoshiro128+):
// 1. Result = s0 + s1
// 2. Update state using bitwise XOR, shifts, and rotations.
//
// Invariants:
// 1. The state (`s0`, `s1`) must never be entirely zero (0, 0). If it is, the generator
//    will output only zeros forever.
//
// Complexity:
// ┌───────────────┬─────────────┬─────────────┐
// │ Operation     │ Time        │ Space       │
// ├───────────────┼─────────────┼─────────────┤
// │ next_u64      │ O(1)        │ O(1)        │
// │ next_f64      │ O(1)        │ O(1)        │
// └───────────────┴─────────────┴─────────────┘
//
// Design Decisions:
// - `Xoroshiro128Plus`: Chosen over PCG32 because it's widely used in Rust (e.g., small_rng)
//   for its speed and excellent statistical properties.
// - **Seed initialization**: A SplitMix64 generator is used internally to initialize
//   the 128-bit state from a single 64-bit seed. This prevents the "too many zeros"
//   bad initial state problem.
//
// Benchmarking Note:
// Use `criterion` to benchmark the throughput of `next_u64()`.
// Xoroshiro128+ typically hits multiple GB/s. Wrap `rng.next_u64()` in a black_box to prevent optimization.

/// Trait defining the core interface for a random number generator.
pub trait RngCore {
    /// Generates the next random `u64`.
    fn next_u64(&mut self) -> u64;

    /// Generates the next random `u32`.
    fn next_u32(&mut self) -> u32 {
        (self.next_u64() >> 32) as u32
    }

    /// Generates a random `f64` in the range `[0.0, 1.0)`.
    // Both operands hold at most 53 significant bits, so the f64 casts are exact by construction.
    #[allow(clippy::cast_precision_loss)]
    fn next_f64(&mut self) -> f64 {
        let float_size = 1.0 / (1u64 << 53) as f64;
        (self.next_u64() >> 11) as f64 * float_size
    }

    /// Generates a random `u64` in the range `[0, bound)`.
    fn gen_range(&mut self, bound: u64) -> u64 {
        if bound == 0 {
            return 0;
        }

        if bound.is_power_of_two() {
            return self.next_u64() & (bound - 1);
        }

        let mut val = self.next_u64();
        let max_acceptable = u64::MAX - (u64::MAX % bound);
        while val >= max_acceptable {
            val = self.next_u64();
        }
        val % bound
    }
}

/// A Pseudo-Random Number Generator using the Xoroshiro128+ algorithm.
pub struct Prng {
    s0: u64,
    s1: u64,
}

impl Prng {
    /// Creates a new PRNG seeded with the given 64-bit seed.
    ///
    /// Uses `SplitMix64` to initialize the 128-bit state.
    #[must_use]
    pub const fn new(seed: u64) -> Self {
        let mut sm64 = SplitMix64 { state: seed };
        let s0 = sm64.next();
        let s1 = sm64.next();

        // GOTCHA: If the state is exactly (0, 0), the Xoroshiro generator will only output 0 forever.
        // We must prevent this invalid state.
        let (s0, s1) = if s0 == 0 && s1 == 0 { (1, 0) } else { (s0, s1) };

        Self { s0, s1 }
    }
}

impl RngCore for Prng {
    fn next_u64(&mut self) -> u64 {
        let s0 = self.s0;
        let mut s1 = self.s1;

        // RUST INSIGHT: Wrapping add prevents panic on overflow in debug mode.
        let result = s0.wrapping_add(s1);

        s1 ^= s0;

        self.s0 = s0.rotate_left(24) ^ s1 ^ (s1 << 16); // a, b
        self.s1 = s1.rotate_left(37); // c

        result
    }

    // Default implementations are used for next_u32, next_f64, gen_range
}

// SplitMix64 is used to initialize the state of Xoroshiro128+
struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    const fn next(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `rand`: The `rand` crate provides traits (`Rng`, `SeedableRng`) and multiple generator implementations.
//   It uses OS entropy (`getrandom`) by default.
//
// Missing vs. Production:
// - **Traits**: We don't implement the `RngCore` trait from the `rand` crate ecosystem, meaning this
//   can't be plugged into existing Rust libraries that expect `Rng`.
// - **Cryptographic Security**: Xoroshiro128+ is NOT cryptographically secure. For passwords or keys,
//   you MUST use an OS-level CSPRNG (like `rand_core::OsRng` or `ChaCha20`).
// - **Distributions**: We only support basic uniform distributions, not Normal/Gaussian or others.

#[cfg(test)]
mod tests {
    // test-code: bounds are small constants, so the u64->usize index casts cannot truncate.
    #![allow(clippy::cast_possible_truncation)]

    use super::*;

    #[test]
    fn test_prng_deterministic() {
        let mut rng1 = Prng::new(42);
        let mut rng2 = Prng::new(42);

        assert_eq!(rng1.next_u64(), rng2.next_u64());
        assert_eq!(rng1.next_u64(), rng2.next_u64());
        assert_eq!(rng1.next_u64(), rng2.next_u64());
    }

    #[test]
    fn test_prng_different_seeds() {
        let mut rng1 = Prng::new(42);
        let mut rng2 = Prng::new(43);

        assert_ne!(rng1.next_u64(), rng2.next_u64());
    }

    #[test]
    fn test_prng_f64_range() {
        let mut rng = Prng::new(12345);
        for _ in 0..1000 {
            let val = rng.next_f64();
            assert!((0.0..1.0).contains(&val));
        }
    }

    #[test]
    fn test_prng_gen_range() {
        let mut rng = Prng::new(999);
        let bound = 10;
        let mut counts = vec![0; bound as usize];

        // Generate many numbers to check distribution loosely
        for _ in 0..10000 {
            let val = rng.gen_range(bound);
            assert!(val < bound);
            counts[val as usize] += 1;
        }

        // All buckets should have *some* hits
        for count in counts {
            assert!(count > 0);
        }
    }

    #[test]
    fn test_prng_gen_range_power_of_two() {
        let mut rng = Prng::new(555);
        let bound = 8;
        for _ in 0..100 {
            let val = rng.gen_range(bound);
            assert!(val < bound);
        }
    }
}
