//! # Bloom Filter
//!
//! A probabilistic data structure that tests whether an element is a member of a set.
//! False positive matches are possible, but false negatives are not.
//!
//! Replaces: `bloomfilter`, `fastbloom`
//! Used in: BigTable (Google), Cassandra, Postgres, Chromium (Safe Browsing)
//! Why build it: To understand the trade-offs between space efficiency and false positive rates,
//! and how double hashing simulates $k$ hash functions.
//!
//! ## Architecture
//!
//! The Bloom Filter uses a bit array of size $m$ and $k$ independent hash functions.
//!
//! ```text
//! [ 0, 1, 0, 0, 1, 0, ... ]  <-- Bit Array (m bits)
//!      ^        ^
//!      |        |
//!    h1(x)    h2(x) ... hk(x)
//! ```
//!
//! ### Invariants
//! 1. `bits` size matches `num_bits` (rounded up to nearest u64).
//! 2. `num_hashes` is at least 1.
//!
//! ### Complexity
//!
//! | Operation | Time | Space |
//! |-----------|------|-------|
//! | Insert    | O(k) | O(m)  |
//! | Check     | O(k) | O(1)  |
//!
//! Where $k$ is the number of hash functions and $m$ is the bit array size.
//!
//! ### Design Decisions
//!
//! - **Double Hashing**: Instead of computing $k$ distinct hashes, we verify two hashes $h_1(x)$ and $h_2(x)$
//!   and simulate the rest using $g_i(x) = h_1(x) + i \cdot h_2(x)$. This is shown to be as effective as
//!   $k$ independent hashes for reasonable $m$ and $k$ (Kirsch and Mitzenmacher).
//! - **BitVec**: We use `Vec<u64>` for manual bit manipulation to avoid external dependencies like `bitvec`.
//! - **DefaultHasher**: We use Rust's `DefaultHasher`. Note that this is not cryptographically secure nor
//!   guaranteed to be consistent across Rust versions/processes (SipHash with random keys). For persistent
//!   filters, a stable hasher (like Murmur3 or FNV) would be required.

use std::collections::hash_map::DefaultHasher;
use std::f64::consts::LN_2;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;

/// A space-efficient probabilistic data structure.
#[derive(Debug, Clone)]
pub struct BloomFilter<T: ?Sized> {
    /// The bit array stored as a vector of u64 blocks.
    bits: Vec<u64>,
    /// Total number of bits (m).
    num_bits: u64,
    /// Number of hash functions (k).
    num_hashes: u32,
    /// Ghost marker to hold the type T.
    _marker: PhantomData<T>,
}

impl<T: ?Sized + Hash> BloomFilter<T> {
    /// Creates a new `BloomFilter` optimized for `expected_items` and `false_positive_rate`.
    ///
    /// # Panics
    /// Panics if `expected_items` is 0 or `false_positive_rate` is not between 0 and 1.
    pub fn new(expected_items: usize, false_positive_rate: f64) -> Self {
        assert!(expected_items > 0, "Expected items must be greater than 0");
        assert!(
            false_positive_rate > 0.0 && false_positive_rate < 1.0,
            "False positive rate must be between 0 and 1"
        );

        let num_bits = Self::optimal_m(expected_items, false_positive_rate);
        let num_hashes = Self::optimal_k(expected_items, num_bits);

        // Round up to nearest u64 (64 bits)
        let num_u64s = (num_bits + 63) / 64;
        let bits = vec![0; num_u64s as usize];

        Self {
            bits,
            num_bits,
            num_hashes,
            _marker: PhantomData,
        }
    }

    /// Adds an item to the Bloom filter.
    pub fn insert(&mut self, item: &T) {
        let (h1, h2) = self.get_hash_pair(item);

        for i in 0..self.num_hashes {
            // RUST INSIGHT: Wrapping arithmetic avoids panic on overflow, which is
            // desirable here as we just want a pseudo-random sequence.
            let idx = h1.wrapping_add((u64::from(i)).wrapping_mul(h2)) % self.num_bits;
            self.set_bit(idx);
        }
    }

    /// Checks if an item might be in the Bloom filter.
    ///
    /// Returns `true` if the item might be present, `false` if it is definitely not.
    pub fn contains(&self, item: &T) -> bool {
        let (h1, h2) = self.get_hash_pair(item);

        for i in 0..self.num_hashes {
            let idx = h1.wrapping_add((u64::from(i)).wrapping_mul(h2)) % self.num_bits;
            if !self.get_bit(idx) {
                return false;
            }
        }
        true
    }

    /// Helper to get two independent hashes using double hashing technique.
    fn get_hash_pair(&self, item: &T) -> (u64, u64) {
        let mut hasher1 = DefaultHasher::new();
        item.hash(&mut hasher1);
        let h1 = hasher1.finish();

        let mut hasher2 = DefaultHasher::new();
        item.hash(&mut hasher2);
        // GOTCHA: We must modify the second hash state to ensure independence.
        // Simply rehashing the same item with the same hasher state would yield h1 == h2.
        // We salt it by hashing a constant.
        hasher2.write_u8(0xFF);
        let h2 = hasher2.finish();

        (h1, h2)
    }

    /// Sets the bit at the given index.
    fn set_bit(&mut self, idx: u64) {
        let block_idx = (idx / 64) as usize;
        let bit_idx = (idx % 64) as usize;
        // UNSAFE JUSTIFICATION: Standard vector access is safe.
        // Using `get_unchecked_mut` could be an optimization here but not necessary for this educational impl.
        if let Some(block) = self.bits.get_mut(block_idx) {
            *block |= 1 << bit_idx;
        }
    }

    /// Gets the bit at the given index.
    fn get_bit(&self, idx: u64) -> bool {
        let block_idx = (idx / 64) as usize;
        let bit_idx = (idx % 64) as usize;
        if let Some(block) = self.bits.get(block_idx) {
            (block & (1 << bit_idx)) != 0
        } else {
            false
        }
    }

    /// Calculates optimal bit array size (m).
    /// Formula: m = - (n * ln(p)) / (ln(2)^2)
    pub fn optimal_m(n: usize, p: f64) -> u64 {
        let n = n as f64;
        let numerator = -1.0 * n * p.ln();
        let denominator = LN_2.powi(2);
        (numerator / denominator).ceil() as u64
    }

    /// Calculates optimal number of hash functions (k).
    /// Formula: k = (m / n) * ln(2)
    pub fn optimal_k(n: usize, m: u64) -> u32 {
        let n = n as f64;
        let m = m as f64;
        let k = (m / n) * LN_2;
        (k.ceil() as u32).max(1)
    }

    /// Returns the number of bits in the filter.
    pub fn num_bits(&self) -> u64 {
        self.num_bits
    }

    /// Returns the number of hash functions used.
    pub fn num_hashes(&self) -> u32 {
        self.num_hashes
    }
}

// Footer:
// Comparison to canonical crates:
// - `bloomfilter`: Uses a similar bit-vec approach but offers more hashing algorithms (Murmur3, etc.).
// - `fastbloom`: Highly optimized, supports counting bloom filters, SIMD.
//
// Missing vs Production:
// - No support for custom hashers (generic BuildHasher).
// - Not serializable (DefaultHasher is not stable).
// - No resize capability (Scalable Bloom Filter).
// - No deletion (needs Counting Bloom Filter).

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bloom_filter_basic() {
        let mut bf = BloomFilter::new(100, 0.01);
        bf.insert("hello");
        bf.insert("world");

        assert!(bf.contains("hello"));
        assert!(bf.contains("world"));
        assert!(!bf.contains("foo"));
        assert!(!bf.contains("bar"));
    }

    #[test]
    fn test_false_positive_rate() {
        // Create a filter with 1% false positive rate for 1000 items
        let n = 1000;
        let p = 0.01;
        let mut bf = BloomFilter::new(n, p);

        // Insert n items
        for i in 0..n {
            bf.insert(&i.to_string());
        }

        // Check false positives
        let mut false_positives = 0;
        let trials = 10000;
        for i in n..(n + trials) {
            if bf.contains(&i.to_string()) {
                false_positives += 1;
            }
        }

        let actual_rate = false_positives as f64 / trials as f64;
        // The actual rate should be close to p (0.01)
        // Probabilistic, so we give it some leeway
        assert!(actual_rate < p * 2.0, "Actual FP rate {} too high", actual_rate);
    }

    #[test]
    fn test_optimal_params() {
        let n = 100;
        let p = 0.01;
        let m = BloomFilter::<()>::optimal_m(n, p);
        let k = BloomFilter::<()>::optimal_k(n, m);

        // m should be around 958 bits for n=100, p=0.01
        // - (100 * ln(0.01)) / (ln(2)^2) ≈ 958.5
        assert!(m > 900 && m < 1000);

        // k should be around 7
        // (959 / 100) * ln(2) ≈ 6.64 -> 7
        assert_eq!(k, 7);
    }
}
