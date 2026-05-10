//! # Bloom Filter Implementation
//!
//! A space-efficient probabilistic data structure that is used to test whether an element is a member of a set.
//!
//! **Replaces Crates:** `bloomfilter`, `fastbloom`, `probabilistic-collections`
//!
//! **Real-world Usage:**
//! - **Databases & Storage Engines (LSM Trees):** Used in RocksDB, Cassandra, and Bigtable to avoid disk lookups for non-existent keys (SSTables).
//! - **Web Browsers:** Chrome used it to check for malicious URLs (Safe Browsing).
//! - **CDNs & Caches:** Akamai uses it to prevent "one-hit wonders" from polluting web caches.
//! - **Cryptocurrency:** Bitcoin clients (SPV) use it to request relevant transactions without revealing exact addresses.
//!
//! **Why build it yourself?**
//! Understanding Bloom Filters is essential for system design involving large-scale data. Building one teaches you about bit manipulation, hash functions, the mathematics behind false positive probabilities, and the Kirsch-Mitzenmacher optimization (simulating multiple hash functions from a single hash).

// =========================================================================================
// Architecture
// =========================================================================================
//
// Array of Bits (m bits):
// [ 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 ]
//
// Insert("apple"):
// 1. Hash("apple") -> h1 = 2, h2 = 5, h3 = 10
// 2. Set bits at 2, 5, 10
// [ 0 | 0 | 1 | 0 | 0 | 1 | 0 | 0 | 0 | 0 | 1 | 0 ]
//
// Check("apple"):
// 1. Hash("apple") -> 2, 5, 10
// 2. All are 1? Yes -> "Probably present"
//
// Check("banana"):
// 1. Hash("banana") -> 1, 5, 8
// 2. Bit 1 is 0? -> "Definitely not present"
//
// Check("orange"): (False Positive Scenario)
// 1. Hash("orange") -> 2, 5, 10 (Hash collision with "apple")
// 2. All are 1? Yes -> "Probably present" (Even though it wasn't inserted)
//
// Mathematical Optimization (Kirsch-Mitzenmacher):
// To avoid running `k` separate expensive hash functions, we run one 64-bit hash (or two 32-bit hashes)
// and simulate `k` hashes:
// `h_i = (h1 + i * h2) % m`
//
// Invariants:
// 1. No false negatives: If an item was inserted, `contains` will always return `true`.
// 2. False positives are possible: `contains` might return `true` for an item that wasn't inserted.
// 3. Bits are never cleared (standard Bloom filter does not support delete).
//
// Time/Space Complexity:
// - `insert`: O(k) time, O(1) space (aside from the pre-allocated bit array).
// - `contains`: O(k) time, O(1) space.
// - Space complexity: O(m) where `m = - (n * ln(p)) / (ln(2)^2)`.
//   (n = expected items, p = target false positive rate).
//
// =========================================================================================

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;

/// Defines the behavior of a probabilistic set.
pub trait ProbabilisticSet<T: ?Sized> {
    /// Inserts an element into the set.
    fn insert(&mut self, item: &T);

    /// Checks if an element might be in the set.
    /// Returns `true` if the element might be present (false positive possible).
    /// Returns `false` if the element is definitely not present.
    fn contains(&self, item: &T) -> bool;

    /// Clears all elements from the set.
    fn clear(&mut self);
}

/// A standard Bloom Filter.
pub struct BloomFilter<T: ?Sized> {
    /// The bit array, stored as a vector of `u64` blocks for density.
    bits: Vec<u64>,
    /// The total number of bits (`m`).
    m: usize,
    /// The number of hash functions to simulate (`k`).
    k: u32,
    /// Phantom data to bind the type `T` to the struct without storing it.
    _marker: PhantomData<T>,
}

impl<T: Hash + ?Sized> BloomFilter<T> {
    /// Creates a Bloom filter with a specific number of bits (`m`) and hash functions (`k`).
    ///
    /// Note: It is usually easier to use `with_rate` to calculate these optimally.
    pub fn new(m: usize, k: u32) -> Self {
        // Calculate the number of u64 blocks needed.
        // We use (m + 63) / 64 to round up.
        let num_blocks = (m + 63) / 64;

        // RUST INSIGHT:
        // By pre-allocating the vector with `vec![0; num_blocks]`, we ensure continuous memory
        // and eliminate runtime allocations during inserts.
        Self {
            bits: vec![0; num_blocks],
            m,
            k,
            _marker: PhantomData,
        }
    }

    /// Creates an optimally sized Bloom filter given the expected number of items (`n`)
    /// and the desired false positive probability (`p`).
    pub fn with_rate(expected_items: usize, false_positive_rate: f64) -> Self {
        assert!(
            false_positive_rate > 0.0 && false_positive_rate < 1.0,
            "False positive rate must be between 0.0 and 1.0"
        );

        let n = expected_items as f64;
        let p = false_positive_rate;

        // m = - (n * ln(p)) / (ln(2)^2)
        let m_f64 = -(n * p.ln()) / (2.0f64.ln().powi(2));
        let m = m_f64.ceil() as usize;

        // k = (m / n) * ln(2)
        let k_f64 = (m as f64 / n) * 2.0f64.ln();
        let k = k_f64.ceil() as u32;

        // Ensure k is at least 1
        let k = if k == 0 { 1 } else { k };

        Self::new(m, k)
    }

    /// Computes the two 32-bit halves of a 64-bit hash.
    ///
    /// PRODUCTION NOTE:
    /// In a production system like RocksDB, `DefaultHasher` (which uses SipHash to prevent DOS attacks)
    /// might be considered too slow. A non-cryptographic, high-performance hash like `xxHash` or `MurmurHash3`
    /// is strongly preferred for Bloom filters since collision resistance against attackers isn't usually the
    /// primary concern for internal database structures.
    #[inline]
    fn get_hash_halves(&self, item: &T) -> (u32, u32) {
        let mut hasher = DefaultHasher::new();
        item.hash(&mut hasher);
        let hash64 = hasher.finish();

        // Split the 64-bit hash into two 32-bit hashes.
        let h1 = (hash64 & 0xFFFF_FFFF) as u32;
        let h2 = (hash64 >> 32) as u32;

        (h1, h2)
    }

    /// Sets the bit at the given index.
    #[inline]
    fn set_bit(&mut self, index: usize) {
        let block_idx = index / 64;
        let bit_idx = index % 64;

        // GOTCHA:
        // Always use `1 << bit_idx`, not `bit_idx << 1`.
        self.bits[block_idx] |= 1 << bit_idx;
    }

    /// Gets the bit at the given index.
    #[inline]
    fn get_bit(&self, index: usize) -> bool {
        let block_idx = index / 64;
        let bit_idx = index % 64;

        (self.bits[block_idx] & (1 << bit_idx)) != 0
    }
}

impl<T: Hash + ?Sized> ProbabilisticSet<T> for BloomFilter<T> {
    fn insert(&mut self, item: &T) {
        let (h1, h2) = self.get_hash_halves(item);

        for i in 0..self.k {
            // Kirsch-Mitzenmacher optimization: h_i = h1 + i * h2
            // We use wrapping addition to safely allow overflow.
            let h_i = h1.wrapping_add(i.wrapping_mul(h2));

            // Map the hash to a valid bit index.
            let bit_index = (h_i as usize) % self.m;
            self.set_bit(bit_index);
        }
    }

    fn contains(&self, item: &T) -> bool {
        let (h1, h2) = self.get_hash_halves(item);

        for i in 0..self.k {
            let h_i = h1.wrapping_add(i.wrapping_mul(h2));
            let bit_index = (h_i as usize) % self.m;

            if !self.get_bit(bit_index) {
                return false; // Definitely not present
            }
        }

        true // Probably present
    }

    fn clear(&mut self) {
        self.bits.fill(0);
    }
}

// =========================================================================================
// Alternative Implementations / Canonical comparisons
// =========================================================================================
//
// 1. `fastbloom`: Uses block-level caching or SIMD for extreme performance. Our implementation
//    does a full % m on each hash, which can cause cache misses if m is very large. Real ones
//    sometimes restrict standard queries to a single CPU cache line (Block Bloom Filters).
//
// 2. Missing Features vs Production:
//    - Counting Bloom Filter: To support deletions, you'd need a 4-bit or 8-bit counter per "bit"
//      instead of just 1 bit, rolling back the counter on delete.
//    - Serialization/Deserialization (e.g. `serde`): Essential for saving to disk (SSTables).
//    - Configurable Hashers: E.g. passing an `ahash::AHasher` builder.
//
// Next Steps:
// - Implement a `CountingBloomFilter`.
// - Implement a `CuckooFilter` which has better space efficiency and supports deletions natively.
// =========================================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::hint::black_box;
    use std::time::Instant;

    #[test]
    fn test_basic_inserts_and_contains() {
        let mut filter = BloomFilter::<str>::new(100, 3);

        assert!(!filter.contains("apple"));
        assert!(!filter.contains("banana"));

        filter.insert("apple");
        filter.insert("banana");

        assert!(filter.contains("apple"));
        assert!(filter.contains("banana"));
        assert!(!filter.contains("orange"));
    }

    #[test]
    fn test_with_rate() {
        // We expect 10,000 items with a 1% false positive rate.
        let mut filter = BloomFilter::<str>::with_rate(10000, 0.01);

        // m should be roughly 95,850 bits, and k should be 7
        // (10000 * 9.58 bits/item)
        assert!(filter.m > 90000 && filter.m < 100000);
        assert_eq!(filter.k, 7);

        filter.insert("test1");
        assert!(filter.contains("test1"));
        assert!(!filter.contains("test2")); // With a 1% rate, very likely to be false.
    }

    #[test]
    fn test_false_positive_rate() {
        let expected_items = 1000;
        let target_fpr = 0.05;

        let mut filter = BloomFilter::<usize>::with_rate(expected_items, target_fpr);

        // Insert expected items
        for i in 0..expected_items {
            filter.insert(&i);
        }

        // Check false positives on a disjoint set
        let mut false_positives = 0;
        let test_size = 10000;
        for i in expected_items..(expected_items + test_size) {
            if filter.contains(&i) {
                false_positives += 1;
            }
        }

        let actual_fpr = false_positives as f64 / test_size as f64;

        // The actual FPR should be close to the target FPR.
        // We allow some variance due to randomness of hashes.
        assert!(
            actual_fpr < target_fpr * 1.5,
            "FPR was {} which is significantly higher than target {}",
            actual_fpr,
            target_fpr
        );
    }

    #[test]
    fn test_clear() {
        let mut filter = BloomFilter::<str>::new(100, 3);
        filter.insert("hello");
        assert!(filter.contains("hello"));

        filter.clear();
        assert!(!filter.contains("hello"));
    }

    #[test]
    fn benchmark_bloom_filter() {
        let start = Instant::now();
        let mut filter = BloomFilter::<usize>::with_rate(100_000, 0.01);

        for i in 0..10_000 {
            filter.insert(black_box(&i));
        }

        for i in 0..10_000 {
            let _ = black_box(filter.contains(black_box(&i)));
        }

        let duration = start.elapsed();
        // Benchmark note: To actually measure performance, run `cargo test -- --nocapture`
        // or use `criterion` crate in a dedicated bench/.
        println!("Benchmark completed in {:?}", duration);
    }
}
