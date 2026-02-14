//! # Bloom Filter Implementation
//!
//! A space-efficient probabilistic data structure that tests whether an element is a member of a set.
//! False positive matches are possible, but false negatives are not – in other words, a query returns either "possibly in set" or "definitely not in set".
//!
//! **Replaces Crates:** `bloomfilter`, `fastbloom`
//!
//! **Real-world Usage:**
//! - **LSM-Tree Databases (RocksDB, Cassandra, LevelDB):** To avoid expensive disk lookups for keys that don't exist in an SSTable.
//! - **Web Browsers (Chrome):** Safe Browsing (malicious URL check) uses a local Bloom filter to screen URLs before checking the server.
//! - **CDNs (Akamai):** To prevent caching "one-hit-wonders" (items requested only once).
//!
//! **Why build it yourself?**
//! 1. Understand the math behind sizing: balancing space (`m` bits) vs. false positive rate (`p`).
//! 2. Learn how to simulate `k` independent hash functions using only 2 base hashes (Kirsch-Mitzenmacher optimization).
//! 3. Practice low-level bit manipulation (`Vec<u64>`) for performance.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::f64::consts::LN_2;

// =========================================================================================
// Architecture
// =========================================================================================
//
// Data Structure:
//      Bit Array: [0, 1, 0, 0, 1, ... ] (Size m)
//
//      Hash Functions (k):
//      h_i(x) = (h1(x) + i * h2(x)) % m
//
// Operations:
//      Insert(x):
//          Compute k positions.
//          Set bits to 1.
//
//      Contains(x):
//          Compute k positions.
//          If ALL bits are 1 -> Probably Present.
//          If ANY bit is 0 -> Definitely Not Present.
//
// Math:
//      m = - (n * ln(p)) / (ln(2)^2)
//      k = (m / n) * ln(2)
//
// Complexity:
// ┌─────────────┬──────────┬──────────┐
// │ Operation   │ Time     │ Space    │
// ├─────────────┼──────────┼──────────┤
// │ insert      │ O(k)     │ O(m)     │
// │ contains    │ O(k)     │ O(1)     │
// └─────────────┴──────────┴──────────┘
//
// Design Decisions:
// - **Bit Storage**: `Vec<u64>` instead of `Vec<bool>` or `BitVec` crate. `Vec<bool>` wastes 7 bits per boolean.
//   We implement manual bitwise operations.
// - **Hashing**: Double Hashing (Kirsch-Mitzenmacher). It's faster than computing k independent hashes and proven to be effectively as good.
// - **Hasher**: `DefaultHasher`. Note: standard caveats about stability apply (see `consistent_hashing.rs`).

/// A Bloom Filter.
#[derive(Debug, Clone)]
pub struct BloomFilter<T: ?Sized> {
    bits: Vec<u64>,
    num_bits: u64,
    num_hashes: u32,
    _phantom: PhantomData<T>,
}

impl<T: Hash + ?Sized> BloomFilter<T> {
    /// Creates a new Bloom Filter optimized for `expected_items` count and `false_positive_rate`.
    ///
    /// # Arguments
    /// * `expected_items` - The number of items you expect to insert (`n`).
    /// * `false_positive_rate` - The desired probability of a false positive (`p`). e.g., 0.01 for 1%.
    pub fn new(expected_items: usize, false_positive_rate: f64) -> Self {
        assert!(expected_items > 0, "Expected items must be > 0");
        assert!(
            false_positive_rate > 0.0 && false_positive_rate < 1.0,
            "False positive rate must be between 0 and 1"
        );

        // m = - (n * ln(p)) / (ln(2)^2)
        let ln_p = false_positive_rate.ln();
        let ln_2_sq = LN_2 * LN_2;
        let m_float = -1.0 * (expected_items as f64 * ln_p) / ln_2_sq;
        let num_bits = m_float.ceil() as u64;

        // k = (m / n) * ln(2)
        let k_float = (num_bits as f64 / expected_items as f64) * LN_2;
        let num_hashes = k_float.ceil() as u32;

        // Number of u64s needed = (num_bits + 63) / 64
        let vec_len = (num_bits + 63) / 64;
        let bits = vec![0; vec_len as usize];

        Self {
            bits,
            num_bits,
            num_hashes,
            _phantom: PhantomData,
        }
    }

    /// Inserts an item into the Bloom Filter.
    pub fn insert(&mut self, item: &T) {
        let (h1, h2) = self.get_hash_pair(item);

        for i in 0..self.num_hashes {
            let bit_index = self.get_bit_index(h1, h2, i);
            self.set_bit(bit_index);
        }
    }

    /// Checks if the item might be in the set.
    ///
    /// Returns `true` if the item might be present (false positive possible).
    /// Returns `false` if the item is definitely not present.
    pub fn contains(&self, item: &T) -> bool {
        let (h1, h2) = self.get_hash_pair(item);

        for i in 0..self.num_hashes {
            let bit_index = self.get_bit_index(h1, h2, i);
            if !self.check_bit(bit_index) {
                return false;
            }
        }
        true
    }

    /// Returns the number of bits in the filter (`m`).
    pub fn bit_count(&self) -> u64 {
        self.num_bits
    }

    /// Returns the number of hash functions (`k`).
    pub fn hash_count(&self) -> u32 {
        self.num_hashes
    }

    // --- Internal Helpers ---

    fn get_hash_pair(&self, item: &T) -> (u64, u64) {
        // RUST INSIGHT: We simulate two hash functions by hashing the item twice with different "salts" (implied state).
        // Since DefaultHasher doesn't accept a seed, we can hash a tuple (item, 0) and (item, 1).
        // This is a common trick.

        let mut hasher1 = DefaultHasher::new();
        item.hash(&mut hasher1);
        0u8.hash(&mut hasher1); // Salt 1
        let h1 = hasher1.finish();

        let mut hasher2 = DefaultHasher::new();
        item.hash(&mut hasher2);
        1u8.hash(&mut hasher2); // Salt 2
        let h2 = hasher2.finish();

        (h1, h2)
    }

    fn get_bit_index(&self, h1: u64, h2: u64, i: u32) -> u64 {
        // Double hashing formula: (h1 + i * h2) % m
        // We use wrapping_add to handle overflow gracefully.
        let offset = h2.wrapping_mul(i as u64);
        let hash = h1.wrapping_add(offset);
        hash % self.num_bits
    }

    fn set_bit(&mut self, index: u64) {
        let (vec_idx, bit_offset) = self.get_vec_coords(index);
        // Turn on the bit
        self.bits[vec_idx] |= 1u64 << bit_offset;
    }

    fn check_bit(&self, index: u64) -> bool {
        let (vec_idx, bit_offset) = self.get_vec_coords(index);
        (self.bits[vec_idx] & (1u64 << bit_offset)) != 0
    }

    fn get_vec_coords(&self, index: u64) -> (usize, u64) {
        let vec_idx = (index / 64) as usize;
        let bit_offset = index % 64;
        (vec_idx, bit_offset)
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `bloomfilter`: Provides similar functionality.
// - `fastbloom`: Optimized for speed, SIMD support.
//
// Missing vs. Production:
// - **SIMD**: Production implementations often use SIMD to set/check multiple bits or hashes in parallel.
// - **Deletions**: Standard Bloom filters don't support deletion (requires Counting Bloom Filter).
// - **Serialization**: No Serde support implemented here.
//
// Next Steps:
// 1. Implement `CountingBloomFilter` to support deletion (using counters instead of bits).
// 2. Add support for merging two Bloom Filters (bitwise OR).

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bloom_filter_basic() {
        let mut bf = BloomFilter::new(100, 0.01);

        // Initially empty
        assert!(!bf.contains("hello"));

        bf.insert("hello");
        assert!(bf.contains("hello"));

        // Probably not present (though small chance of FP, with "world" it's very low)
        assert!(!bf.contains("world"));
    }

    #[test]
    fn test_false_positive_rate() {
        // We can't strictly test FP rate deterministically without fixing the seed,
        // but we can sanity check that it's low.
        let n = 1000;
        let p = 0.05; // 5%
        let mut bf = BloomFilter::new(n, p);

        // Insert n items
        for i in 0..n {
            bf.insert(&i);
        }

        // Check inserted items (should all be true - no false negatives)
        for i in 0..n {
            assert!(bf.contains(&i), "False negative detected! Should not happen.");
        }

        // Check non-inserted items
        let mut fp_count = 0;
        let test_count = 1000;
        for i in n..(n + test_count) {
            if bf.contains(&i) {
                fp_count += 1;
            }
        }

        let actual_rate = fp_count as f64 / test_count as f64;
        println!("Expected FP rate: {}, Actual: {}", p, actual_rate);

        // Allow some variance, but it shouldn't be wildly off (e.g., > 15% for 5% target)
        assert!(actual_rate < 0.15, "FP rate too high");
    }

    #[test]
    fn test_small_filter() {
        let mut bf = BloomFilter::new(1, 0.5);
        bf.insert("a");
        assert!(bf.contains("a"));
    }

    #[test]
    #[should_panic]
    fn test_invalid_params() {
        BloomFilter::<i32>::new(0, 0.1);
    }
}
