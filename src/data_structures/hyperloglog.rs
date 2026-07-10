//! # `HyperLogLog` Implementation
//!
//! # Header
//!
//! *   **Problem Name**: `HyperLogLog` (HLL) Cardinality Estimator
//! *   **Difficulty**: Medium (Probabilistic Data Structures)
//! *   **Link**: <http://algo.inria.fr/flajolet/Publications/FlajoletFusyGandouetMeunier07.pdf>
//! *   **Replaces Crates**: `hyperloglog`, `amadeus-streaming` (partially), custom Redis implementations.
//! *   **Real-world Usage**:
//!     *   **Redis**: `PFADD`, `PFCOUNT` commands use HLL for distinct counts.
//!     *   **Big Data**: Google `BigQuery`, Amazon Redshift for `COUNT(DISTINCT)`.
//!     *   **Networking**: Counting unique IP addresses in traffic streams.
//! *   **Why build it yourself?**
//!     Implementing HLL demystifies how databases count billions of unique items with only ~12KB of memory.
//!     You learn about stochastic averaging, harmonic means to reduce outlier impact, and bit-level hashing tricks.
//!
//! **Note**: This implementation fills a gap in the "Probabilistic Data Structures" category, as the prioritized list (Bloom/Cuckoo) was already implemented.
//!
//! # Architecture
//!
//! `HyperLogLog` uses randomized hashing to estimate the cardinality (number of unique elements) of a set.
//! It relies on the observation that the cardinality of a set of uniformly distributed random numbers can be estimated
//! by calculating the maximum number of leading zeros in the binary representation of each number.
//!
//! **Structure:**
//!
//! 1.  **Registers (Buckets)**: An array $M$ of $m = 2^p$ small counters (typically 6 bits, we use `u8`).
//! 2.  **Hash Function**: Maps input $x$ to a 64-bit hash using **FNV-1a** (stable).
//! 3.  **Add(x)**:
//!     -   Split hash into two parts:
//!         -   **Index**: First $p$ bits determine the register index $j$.
//!         -   **Rank**: Remaining $w-p$ bits determine the run-length of zeros $\rho(w) + 1$.
//!     -   Update: $M[j] = \max(M[j], \text{Rank})$.
//! 4.  **`Count()`**:
//!     -   Compute the harmonic mean of $2^{M[j]}$.
//!     -   Apply bias correction (`alpha_m`).
//!     -   Apply "Linear Counting" for small ranges (many empty registers).
//!
//! **Complexity:**
//!
//! | Operation | Time | Space |
//! | :--- | :--- | :--- |
//! | Add | O(1) | O(1) |
//! | Count | O(m) | O(1) |
//! | Merge | O(m) | O(m) |
//! | Space | - | $O(2^p)$ bits |
//!
//! **Example (p=14, m=16384):**
//! -   Space: $16384 \times 6$ bits $\approx 12$ KB.
//! -   Error: $1.04 / \sqrt{m} \approx 0.81\%$.
//!
//! # Rust Insight
//!
//! *   **`u8` Registers**: We use `Vec<u8>` for simplicity. Production implementations (like Redis) might pack 6-bit registers tightly to save 25% memory.
//! *   **Stable Hashing**: We implement a minimal **FNV-1a** hasher inline. This ensures consistency across Rust versions and platforms, unlike `std::collections::hash_map::DefaultHasher`.
//!
//! # Gotchas
//!
//! *   **Small Cardinality Bias**: The raw HLL formula has a large bias for small cardinalities. We switch to "Linear Counting" when $E \le \frac{5}{2}m$.
//! *   **Hash Collisions**: With 64-bit hashes, collisions are negligible for cardinalities $< 2^{60}$.
//! *   **Register Size**: $2^p$ grows exponentially. $p=16$ needs 64KB (if u8). $p=4$ needs 16 bytes.

// Intentional index/byte/word manipulation and statistical estimation casts.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)]

use std::hash::{Hash, Hasher};

/// A simple FNV-1a (64-bit) Hasher for stability.
struct Fnv1aHasher {
    state: u64,
}

impl Fnv1aHasher {
    const fn new() -> Self {
        Self {
            state: 0xcbf2_9ce4_8422_2325,
        }
    }
}

impl Hasher for Fnv1aHasher {
    fn write(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.state ^= u64::from(byte);
            self.state = self.state.wrapping_mul(0x0100_0000_01b3);
        }
    }

    fn finish(&self) -> u64 {
        let mut x = self.state;
        // MurmurHash3 64-bit finalizer to improve avalanche
        x ^= x >> 33;
        x = x.wrapping_mul(0xff51_afd7_ed55_8ccd);
        x ^= x >> 33;
        x = x.wrapping_mul(0xc4ce_b9fe_1a85_ec53);
        x ^= x >> 33;
        x
    }
}

/// A `HyperLogLog` probabilistic counter.
#[derive(Debug, Clone)]
pub struct HyperLogLog {
    p: u8,    // Precision parameter (4..16)
    m: usize, // Number of registers (2^p)
    registers: Vec<u8>,
}

impl HyperLogLog {
    /// Creates a new `HyperLogLog` with precision `p`.
    ///
    /// # Arguments
    ///
    /// * `p` - The number of bits used for indexing registers. Must be in [4, 16].
    ///   - Higher `p` means lower error but more memory.
    ///   - Error $\approx 1.04 / \sqrt{2^p}$.
    ///   - Memory $\approx 2^p$ bytes (using u8 registers).
    ///
    /// # Panics
    ///
    /// Panics if `p` is not in the range [4, 16].
    #[must_use]
    pub fn new(p: u8) -> Self {
        assert!(
            (4..=16).contains(&p),
            "Precision p must be between 4 and 16"
        );
        let m = 1 << p;
        Self {
            p,
            m,
            registers: vec![0; m],
        }
    }

    /// Adds an item to the `HyperLogLog`.
    pub fn add<T: Hash + ?Sized>(&mut self, item: &T) {
        let mut hasher = Fnv1aHasher::new();
        item.hash(&mut hasher);
        let hash = hasher.finish();

        // 1. Calculate register index (first p bits)
        // We shift right by (64 - p) to get the top p bits.
        let j = (hash >> (64 - self.p)) as usize;

        // 2. Calculate rank (remaining bits)
        // We mask out the top p bits to isolate the remaining (64-p) bits.
        // Actually, we can just shift left by p to clear the top p bits
        // and put the "remaining" bits at the top for `leading_zeros`.
        // Plus 1 because rank is 1-based (0 zeros -> rank 1).
        let w = hash << self.p;

        // Count leading zeros of the remaining part.
        // Cap at 64 - p + 1.
        // If w is 0 (all remaining bits are 0), leading_zeros is 64.
        // We saturate it to the maximum possible rank for this precision.
        let mut rank = (w.leading_zeros() as u8) + 1;
        let max_rank = (64 - self.p) + 1;
        if rank > max_rank {
            rank = max_rank;
        }

        // 3. Update register
        if rank > self.registers[j] {
            self.registers[j] = rank;
        }
    }

    /// Estimates the cardinality of the set.
    #[must_use]
    pub fn count(&self) -> u64 {
        let m = self.m as f64;
        let alpha = self.get_alpha();

        // Harmonic mean of 2^register
        let mut sum_inv_pow = 0.0;
        let mut zero_count = 0;

        for &val in &self.registers {
            sum_inv_pow += 2.0_f64.powi(-i32::from(val));
            if val == 0 {
                zero_count += 1;
            }
        }

        // Raw Estimate E
        let raw_estimate = alpha * m * m / sum_inv_pow;

        // Corrections
        if raw_estimate <= 2.5 * m {
            // Small Range Correction (Linear Counting)
            if zero_count > 0 {
                (m * (m / f64::from(zero_count)).ln()) as u64
            } else {
                raw_estimate as u64
            }
        } else if raw_estimate > (1.0 / 30.0) * 4_294_967_296.0 {
            // Large Range Correction (only strictly needed for 32-bit hashes)
            // For 64-bit, this threshold is huge, but we include the logic for completeness/educational value
            // if we were treating the hash space as 32-bit.
            // However, with 64-bit hashes, this is rarely hit.
            // We'll stick to the raw estimate for large values as standard HLL implementations often do for 64-bit.
            raw_estimate as u64
        } else {
            raw_estimate as u64
        }
    }

    /// Merges another `HyperLogLog` into this one.
    ///
    /// # Panics
    ///
    /// Panics if the precision `p` of the two HLLs does not match.
    pub fn merge(&mut self, other: &Self) {
        assert_eq!(
            self.p, other.p,
            "Cannot merge HLLs with different precision"
        );

        for i in 0..self.m {
            if other.registers[i] > self.registers[i] {
                self.registers[i] = other.registers[i];
            }
        }
    }

    /// Clears the `HyperLogLog`.
    pub fn clear(&mut self) {
        for x in &mut self.registers {
            *x = 0;
        }
    }

    /// Returns the precision parameter `p`.
    #[must_use]
    pub const fn p(&self) -> u8 {
        self.p
    }

    // Helper to get alpha constant based on m
    fn get_alpha(&self) -> f64 {
        match self.m {
            16 => 0.673,
            32 => 0.697,
            64 => 0.709,
            _ => 0.7213 / (1.0 + 1.079 / (self.m as f64)),
        }
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `hyperloglog`: The standard crate. Optimized for performance.
// - `redis`: Uses a dense/sparse representation hybrid to save space for small sets.
//
// Missing vs. Production:
// - **Sparse Representation**: We always allocate `2^p` bytes. Production HLLs use a sparse list
//   (store index-value pairs) until the set is large enough to warrant a dense array.
// - **6-bit packing**: We use `u8` (8 bits) for registers. Packing 6-bit values saves 25% space.
// - **Hash Function**: We use a custom FNV-1a. Production might use Murmur3 or CityHash.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hll_basic() {
        let mut hll = HyperLogLog::new(12); // p=12 -> 4096 registers, error ~1.6%
        hll.add("apple");
        hll.add("banana");
        hll.add("cherry");

        let count = hll.count();
        // For very small N, Linear Counting should be exact if no collisions
        assert!(count == 3, "Count should be 3, got {}", count);
    }

    #[test]
    fn test_hll_idempotence() {
        let mut hll = HyperLogLog::new(12);
        hll.add("apple");
        hll.add("apple");
        hll.add("apple");

        assert_eq!(hll.count(), 1);
    }

    #[test]
    fn test_hll_accuracy() {
        // p=14 -> Error ~0.81%
        let mut hll = HyperLogLog::new(14);
        let n = 10_000;

        for i in 0..n {
            hll.add(&i.to_string());
        }

        let count = hll.count();
        let error = (count as i64 - n as i64).abs() as f64 / n as f64;

        println!(
            "Expected: {}, Got: {}, Error: {:.4}%",
            n,
            count,
            error * 100.0
        );

        // Allow slightly generous margin for stochastic tests (2% is > 2 * std_err)
        assert!(error < 0.02, "Error too high: {:.4}%", error * 100.0);
    }

    #[test]
    fn test_hll_merge() {
        let mut hll1 = HyperLogLog::new(12);
        let mut hll2 = HyperLogLog::new(12);

        for i in 0..1000 {
            hll1.add(&i);
        }
        for i in 500..1500 {
            hll2.add(&i);
        }

        hll1.merge(&hll2);

        // Union should be 0..1500 -> 1500 items
        let count = hll1.count();
        let error = (count as i64 - 1500).abs() as f64 / 1500.0;

        assert!(error < 0.05, "Merge error too high: {:.4}%", error * 100.0);
    }

    #[test]
    fn test_hll_small_range() {
        // Test Linear Counting trigger
        let mut hll = HyperLogLog::new(10); // m=1024
        // Add 10 items. 10 <= 2.5 * 1024.
        for i in 0..10 {
            hll.add(&i);
        }

        // Due to probabilistic collisions (birthday paradox), 10 items in 1024 buckets
        // has ~5% chance of collision. Allow +/- 1.
        let count = hll.count();
        assert!(
            (9..=11).contains(&count),
            "Expected 10 (+/- 1), got {}",
            count
        );
    }

    #[test]
    #[should_panic(expected = "Precision p must be between 4 and 16")]
    fn test_invalid_p_low() {
        HyperLogLog::new(3);
    }

    #[test]
    #[should_panic(expected = "Precision p must be between 4 and 16")]
    fn test_invalid_p_high() {
        HyperLogLog::new(17);
    }
}
