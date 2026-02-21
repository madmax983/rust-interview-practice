//! # Count-Min Sketch Implementation
//!
//! A probabilistic data structure that serves as a frequency table of events in a stream of data.
//! It uses hash functions to map events to frequencies, but unlike a hash table, uses only sub-linear space, at the expense of overcounting some events due to collisions.
//!
//! **Replaces Crates:** `count-min-sketch`, `sketch`
//!
//! **Real-world Usage:**
//! - Heavy Hitters: Finding the most frequent items in a stream (e.g., top IP addresses in traffic).
//! - Query Optimization: Estimating the selectivity of database queries.
//! - Natural Language Processing: Storing n-gram frequencies.
//!
//! **Why build it yourself?**
//! Understanding Count-Min Sketch demystifies how "Big Data" systems process massive streams with tiny memory.
//! You learn about the trade-off between space (width * depth) and accuracy (epsilon, delta).

use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hash, Hasher};
use std::marker::PhantomData;

// =========================================================================================
// Architecture
// =========================================================================================
//
// Data Structure: 2D Array (Table)
// - `d` rows (depth): Number of hash functions.
// - `w` columns (width): Range of hash values.
//
// Operations:
// - Add(item, count):
//   For each row `i` from 0 to d-1:
//     `index = h_i(item) % w`
//     `table[i][index] += count`
//
// - Estimate(item):
//   For each row `i` from 0 to d-1:
//     `val = table[i][h_i(item) % w]`
//   Return `min(val)` across all rows.
//
// Invariants:
// 1. The estimate is always >= true count (never underestimates).
// 2. The error is bounded by `e * N` with probability `1 - delta`, where `e = 2/w` and `delta = 1/e^d`.
//
// Complexity:
// ┌───────────────┬─────────────┬─────────────┐
// │ Operation     │ Time        │ Space       │
// ├───────────────┼─────────────┼─────────────┤
// │ Add           │ O(d)        │ O(w * d)    │
// │ Estimate      │ O(d)        │ O(w * d)    │
// └───────────────┴─────────────┴─────────────┘

/// A Count-Min Sketch for estimating frequencies.
pub struct CountMinSketch<T: ?Sized> {
    table: Vec<Vec<u64>>,
    width: usize,
    depth: usize,
    total_count: u64,
    hasher: RandomState,
    _marker: PhantomData<T>,
}

impl<T: ?Sized + Hash> CountMinSketch<T> {
    /// Creates a new Count-Min Sketch.
    ///
    /// # Arguments
    /// * `epsilon` - Acceptable error rate (e.g., 0.01). Error is within `epsilon * N`.
    /// * `delta` - Probability of error exceeding the bound (e.g., 0.01).
    pub fn new(epsilon: f64, delta: f64) -> Self {
        let width = (2.0 / epsilon).ceil() as usize;
        // P(error) <= 1/2 per row. To get P(error) <= delta, we need (1/2)^d <= delta.
        // d >= log2(1/delta).
        let depth = (1.0 / delta).log2().ceil() as usize;

        Self {
            table: vec![vec![0; width]; depth],
            width,
            depth,
            total_count: 0,
            hasher: RandomState::new(),
            _marker: PhantomData,
        }
    }

    /// Adds an item to the sketch with a specific count.
    pub fn add(&mut self, item: &T, count: u64) {
        for i in 0..self.depth {
            let index = self.hash(item, i);
            self.table[i][index] = self.table[i][index].saturating_add(count);
        }
        self.total_count = self.total_count.saturating_add(count);
    }

    /// Estimates the frequency of an item.
    /// Guaranteed to be >= true count.
    pub fn estimate(&self, item: &T) -> u64 {
        let mut min_count = u64::MAX;
        for i in 0..self.depth {
            let index = self.hash(item, i);
            let val = self.table[i][index];
            if val < min_count {
                min_count = val;
            }
        }
        min_count
    }

    /// Returns the total number of items added (sum of all counts).
    pub fn total_count(&self) -> u64 {
        self.total_count
    }

    /// Computes the hash for the item at a specific row (depth).
    /// Uses double hashing to simulate `d` independent hash functions.
    fn hash(&self, item: &T, row: usize) -> usize {
        let (h1, h2) = self.get_hash_pair(item);
        // h_i(x) = (h1 + i * h2) % width
        let index = h1.wrapping_add((row as u64).wrapping_mul(h2)) as usize;
        index % self.width
    }

    fn get_hash_pair(&self, item: &T) -> (u64, u64) {
        // Use RandomState for h1 to prevent HashDoS.
        let mut hasher1 = self.hasher.build_hasher();
        item.hash(&mut hasher1);
        let h1 = hasher1.finish();

        // Use a simple custom hasher for the second hash to ensure independence.
        // FNV-1a style is fine here as secondary mixing.
        // We seed it with h1 to add randomness from RandomState.
        let mut hasher2 = Fnv1aHasher::new(0xcbf29ce484222325 ^ h1);
        item.hash(&mut hasher2);
        let h2 = hasher2.finish();

        (h1, h2)
    }
}

// Minimal FNV-1a Hasher
struct Fnv1aHasher {
    state: u64,
}

impl Fnv1aHasher {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }
}

impl Hasher for Fnv1aHasher {
    fn finish(&self) -> u64 {
        self.state
    }

    fn write(&mut self, bytes: &[u8]) {
        let prime = 1099511628211;
        for byte in bytes {
            self.state ^= *byte as u64;
            self.state = self.state.wrapping_mul(prime);
        }
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `count-min-sketch`: Standard implementation.
// - `sketch`: Collection of sketches including CMS.
//
// Missing vs. Production:
// - **Conservative Update**: A standard optimization where `add` only updates counters that are minimal.
//   This reduces overestimation significantly.
// - **Decay**: Halving counts periodically to favor recent data.
//
// Next Steps:
// 1. Implement "Conservative Update" strategy.
// 2. Add support for signed updates (Count-Mean-Min Sketch).

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cms_basic() {
        // epsilon=0.1 -> width=20, delta=0.1 -> depth=3
        let mut cms = CountMinSketch::new(0.1, 0.1);

        cms.add("apple", 1);
        cms.add("banana", 1);
        cms.add("apple", 2);

        assert!(cms.estimate("apple") >= 3);
        assert!(cms.estimate("banana") >= 1);
        // Cherry wasn't added, so count should be 0 (or small due to collision)
        // With these params, collisions are possible but unlikely for just 3 items.
        // We just ensure it runs.
    }

    #[test]
    fn test_overestimation_bound() {
        let n = 1000;
        let epsilon = 0.01;
        let delta = 0.01;
        let mut cms = CountMinSketch::new(epsilon, delta);

        for i in 0..n {
            cms.add(&i.to_string(), 1);
        }

        // Check estimate for a known item
        let est = cms.estimate(&"0".to_string());
        assert!(est >= 1);

        // Check error bound
        // Error <= epsilon * N with probability 1 - delta
        // epsilon * N = 0.01 * 1000 = 10.
        // So estimate should be <= 1 + 10 = 11.

        // This is probabilistic, but with high probability it holds.
        // If it fails, either we got unlucky or implementation is wrong.
        assert!(est <= 11, "Estimate {} too high for item with count 1", est);
    }

    #[test]
    fn test_total_count() {
        let mut cms = CountMinSketch::<str>::new(0.1, 0.1);
        cms.add("a", 10);
        cms.add("b", 20);
        assert_eq!(cms.total_count(), 30);
    }
}
