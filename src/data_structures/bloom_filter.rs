//! # Bloom Filter Implementation
//!
//! Implements a space-efficient probabilistic data structure that tests whether an element is a member of a set.
//! False positive matches are possible, but false negatives are not.
//!
//! **Replaces Crates:** `bloomfilter`, `fastbloom`
//!
//! **Real-world Usage:**
//! - Databases (Cassandra, Postgres) to avoid disk lookups for non-existent rows.
//! - Web browsers to check against safe-browsing blacklists.
//! - CDNs to track cached items.
//!
//! **Why build it yourself?**
//! - Understanding the math behind optimal sizing (`m` bits, `k` hashes).
//! - Implementing "double hashing" to simulate `k` hash functions.
//! - Bitwise manipulation for efficient storage (`Vec<u64>`).

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;

// =========================================================================================
// Architecture
// =========================================================================================
//
// Structure:
//   [ 0 | 0 | 1 | 0 | ... | 1 ]  <-- Bit Array (m bits)
//
// Insertion(item):
//   1. Compute k hash values: h1, h2, ..., hk.
//   2. Set bits at indices h1 % m, h2 % m, ..., hk % m to 1.
//
// Query(item):
//   1. Compute k hash values.
//   2. Check if ALL bits at corresponding indices are 1.
//   3. If any is 0 -> Definitely not in set.
//   4. If all are 1 -> Probably in set (False Positive rate `p`).
//
// Invariants:
// 1. Once a bit is set to 1, it is never flipped back to 0 (unless we implement Counting Bloom Filter).
// 2. False negatives are impossible.
//
// Complexity:
// ┌───────────────┬─────────────┬─────────────┐
// │ Operation     │ Time        │ Space       │
// ├───────────────┼─────────────┼─────────────┤
// │ Insert        │ O(k)        │ O(m)        │
// ├───────────────┼─────────────┼─────────────┤
// │ Contains      │ O(k)        │ O(m)        │
// └───────────────┴─────────────┴─────────────┘
//
// Design Decisions:
// - **Double Hashing**: Instead of computing k independent hashes (slow), we compute two hashes `h1` and `h2`
//   and simulate the rest: `g_i(x) = (h1(x) + i * h2(x)) % m`.
//   - *Tradeoff*: Slightly higher collision rate than true independent hashes, but much faster.
// - **Storage**: `Vec<u64>` instead of `Vec<bool>`.
//   - *Benefit*: 64x space efficiency vs `Vec<bool>` (which uses 1 byte per bool).
//   - *Implementation*: Manual bitwise ops (`|`, `&`, `<<`).

pub struct BloomFilter<T: ?Sized> {
    bits: Vec<u64>,
    num_bits: u64,   // m
    num_hashes: u32, // k
    _phantom: PhantomData<T>,
}

impl<T: ?Sized + Hash> BloomFilter<T> {
    /// Creates a new Bloom Filter with optimal parameters for the expected item count and false positive rate.
    ///
    /// # Arguments
    ///
    /// * `expected_items` - The number of items you expect to insert.
    /// * `false_positive_rate` - The desired false positive probability (e.g., 0.01 for 1%).
    ///
    /// # Panics
    ///
    /// Panics if `expected_items` is 0 or `false_positive_rate` is not between 0 and 1.
    pub fn new(expected_items: usize, false_positive_rate: f64) -> Self {
        assert!(expected_items > 0, "expected_items must be > 0");
        assert!(false_positive_rate > 0.0 && false_positive_rate < 1.0, "false_positive_rate must be between 0 and 1");

        // Optimal m = -(n * ln(p)) / (ln(2)^2)
        let ln2 = std::f64::consts::LN_2;
        let n = expected_items as f64;
        let p = false_positive_rate;

        let m = -(n * p.ln()) / (ln2 * ln2);
        let num_bits = m.ceil() as u64;
        // Ensure at least 1 bit to avoid division by zero in indexing
        let num_bits = num_bits.max(1);

        // Optimal k = (m / n) * ln(2)
        let k = (m / n) * ln2;
        let num_hashes = k.ceil() as u32;
        // Ensure at least 1 hash
        let num_hashes = num_hashes.max(1);

        // We store bits in u64 blocks.
        // Number of u64s needed = (num_bits + 63) / 64
        let vec_size = ((num_bits + 63) / 64) as usize;
        let bits = vec![0; vec_size];

        BloomFilter {
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
            let bit_index = self.get_index(h1, h2, i);
            self.set_bit(bit_index);
        }
    }

    /// Checks if an item is possibly in the Bloom Filter.
    ///
    /// Returns `true` if the item is *probably* present.
    /// Returns `false` if the item is *definitely* not present.
    pub fn contains(&self, item: &T) -> bool {
        let (h1, h2) = self.get_hash_pair(item);

        for i in 0..self.num_hashes {
            let bit_index = self.get_index(h1, h2, i);
            if !self.get_bit(bit_index) {
                return false;
            }
        }

        true
    }

    // Helpers

    fn get_hash_pair(&self, item: &T) -> (u64, u64) {
        let mut hasher1 = DefaultHasher::new();
        item.hash(&mut hasher1);
        let h1 = hasher1.finish();

        let mut hasher2 = DefaultHasher::new();
        // RUST INSIGHT: Salting
        // We need a second independent hash. Since `DefaultHasher` doesn't accept a seed,
        // we write a salt first, then the item.
        hasher2.write_u8(1);
        item.hash(&mut hasher2);
        let h2 = hasher2.finish();

        (h1, h2)
    }

    fn get_index(&self, h1: u64, h2: u64, i: u32) -> u64 {
        // g_i(x) = h1 + i * h2
        // We use wrapping_add to handle overflow gracefully
        let i = i as u64;
        let combined_hash = h1.wrapping_add(i.wrapping_mul(h2));
        combined_hash % self.num_bits
    }

    fn set_bit(&mut self, index: u64) {
        let block_index = (index / 64) as usize;
        let bit_offset = (index % 64) as usize;

        self.bits[block_index] |= 1 << bit_offset;
    }

    fn get_bit(&self, index: u64) -> bool {
        let block_index = (index / 64) as usize;
        let bit_offset = (index % 64) as usize;

        (self.bits[block_index] & (1 << bit_offset)) != 0
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `bloomfilter`: Similar implementation.
// - `fastbloom`: Optimized with SIMD and cache-friendly blocked Bloom filters.
//
// Missing vs. Production:
// - **Different Hashers**: Production filters might use Murmur3 or xxHash for speed. `DefaultHasher` (SipHash) is slower but DoS-resistant.
// - **Counting Bloom Filter**: This implementation cannot delete items. A counting variant uses counters instead of bits.
// - **Serialization**: No Serde support.
//
// Next Steps:
// 1. Implement `CountingBloomFilter` to support deletion.
// 2. Add `clear()` method.
// 3. Benchmark against `std::collections::HashSet` for memory usage.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert_contains() {
        let mut bf = BloomFilter::new(100, 0.01);

        bf.insert("hello");
        bf.insert("world");

        assert!(bf.contains("hello"));
        assert!(bf.contains("world"));
        assert!(!bf.contains("foo")); // Probabilistic, but should be false
    }

    #[test]
    fn test_false_positive_rate() {
        // Parameters: n=1000, p=0.05
        let n = 1000;
        let p = 0.05;
        let mut bf = BloomFilter::new(n, p);

        // Insert 0..n
        for i in 0..n {
            bf.insert(&i.to_string());
        }

        // Check 0..n (should all be true)
        for i in 0..n {
            assert!(bf.contains(&i.to_string()));
        }

        // Check n..2n (should be mostly false)
        let mut fp_count = 0;
        let trials = 1000;
        for i in n..(n + trials) {
            if bf.contains(&i.to_string()) {
                fp_count += 1;
            }
        }

        let actual_p = fp_count as f64 / trials as f64;
        println!("Expected p: {}, Actual p: {}", p, actual_p);

        // Allow some variance, but it should be close.
        // With 1000 trials, standard deviation is sqrt(p(1-p)/N) approx sqrt(0.05*0.95/1000) ~= 0.007
        // So 0.05 +/- 0.03 is a safe bet.
        assert!(actual_p < p + 0.05);
    }

    #[test]
    fn test_stress() {
        let mut bf = BloomFilter::new(10_000, 0.01);
        for i in 0..10_000 {
            bf.insert(&i);
        }

        for i in 0..10_000 {
            assert!(bf.contains(&i));
        }
    }
}
