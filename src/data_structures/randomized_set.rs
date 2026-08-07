//! # 380. Insert Delete `GetRandom` O(1)
//! Link: <https://leetcode.com/problems/insert-delete-getrandom-o1/>
//!
//! Implement the `RandomizedSet` class:
//! - `RandomizedSet()` Initializes the `RandomizedSet` object.
//! - `bool insert(int val)` Inserts an item `val` into the set if not present.
//! - `bool remove(int val)` Removes an item `val` from the set if present.
//! - `int getRandom()` Returns a random element from the current set of elements.
//!
//! ## Why this matters in Rust
//! This problem perfectly demonstrates the interplay between `Vec` (for fast `O(1)` random access by index)
//! and `HashMap` (for fast `O(1)` lookups by value). It highlights how Rust's collections can be composed
//! to satisfy complex time complexity constraints, and introduces safe random number generation in Rust.

// Expected `i32` for values/random output by Leetcode, intentionally casting indices to/from `usize`/`u64` for random selection.
// Added backticks around GetRandom to satisfy clippy::doc_markdown
#![allow(clippy::cast_possible_truncation)]

use crate::cryptography::rand::{Prng, RngCore};
use std::collections::HashMap;

// =========================================================================================
// Approach: HashMap + Vec (Optimal O(1))
// =========================================================================================

/// A set that supports O(1) inserts, removals, and random sampling.
///
/// We use a `Vec` to store the elements to allow `O(1)` random access via index.
/// We use a `HashMap` to store the `value -> index` mapping to allow `O(1)` lookups and removals.
///
/// Time: O(1) average for all operations.
/// Space: O(N) where N is the number of elements.
pub struct RandomizedSet {
    /// Stores the values for O(1) random access.
    values: Vec<i32>,
    /// Maps a value to its current index in `values`.
    indices: HashMap<i32, usize>,
    /// Pseudo-random number generator for `getRandom`.
    rng: Prng,
}

impl Default for RandomizedSet {
    fn default() -> Self {
        Self::new()
    }
}

impl RandomizedSet {
    /// Initializes the `RandomizedSet` object.
    #[must_use]
    pub fn new() -> Self {
        Self {
            values: Vec::new(),
            indices: HashMap::new(),
            // Seed PRNG (using a fixed seed for deterministic testing, but normally one might use time/OS entropy)
            rng: Prng::new(42),
        }
    }

    /// Inserts an item `val` into the set if not present. Returns `true` if the item was not present, `false` otherwise.
    pub fn insert(&mut self, val: i32) -> bool {
        // RUST INSIGHT: We use `contains_key` because `insert` returns an `Option`.
        // Using `entry` API would also be idiomatic here, but simple check is clean.
        if self.indices.contains_key(&val) {
            return false;
        }

        let idx = self.values.len();
        self.values.push(val);
        self.indices.insert(val, idx);
        true
    }

    /// Removes an item `val` from the set if present. Returns `true` if the item was present, `false` otherwise.
    pub fn remove(&mut self, val: i32) -> bool {
        // Find the index of the element to remove
        if let Some(idx) = self.indices.remove(&val) {
            // RUST INSIGHT: To remove from a Vec in O(1) time without shifting all elements (which is O(N)),
            // we use `swap_remove`. We swap the element to remove with the last element in the Vec,
            // then pop the last element. We then must update the index of the swapped element in the HashMap.

            let last_idx = self.values.len() - 1;

            // If the element to remove is not the last element, we must update the swapped element's index.
            if idx != last_idx {
                let last_val = self.values[last_idx];
                self.values[idx] = last_val;
                // Update the moved element's index in the map
                self.indices.insert(last_val, idx);
            }

            // Now safely pop the last element (which was the one we wanted to remove)
            self.values.pop();
            return true;
        }

        false
    }

    /// Returns a random element from the current set of elements.
    ///
    /// # Panics
    /// Panics if the set is empty.
    pub fn get_random(&mut self) -> i32 {
        let len = self.values.len();
        assert!(len > 0, "Cannot get random element from empty set");

        // Generate a random index in [0, len)
        // RUST INSIGHT: Our PRNG guarantees a uniform distribution using `gen_range`.
        let rand_idx = self.rng.gen_range(len as u64) as usize;
        self.values[rand_idx]
    }
}

// =========================================================================================
// Alternative Approaches
// =========================================================================================
// 1. `std::collections::HashSet` with `.iter().choose(&mut rng)` - This is `O(N)` for random selection
//    because `HashSet` does not provide index-based access, requiring a linear scan to find the Nth element.
//    Therefore, combining `Vec` and `HashMap` is mandatory for true `O(1)` bounds.

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn test_happy_path() {
        let mut set = RandomizedSet::new();
        assert!(set.insert(1));
        assert!(!set.remove(2));
        assert!(set.insert(2));

        let random_val = set.get_random();
        assert!(random_val == 1 || random_val == 2);

        assert!(set.remove(1));
        assert!(!set.insert(2));
        assert_eq!(set.get_random(), 2);
    }

    #[test]
    fn test_edge_case_empty() {
        let mut set = RandomizedSet::new();
        assert!(!set.remove(1));
        assert!(set.insert(1));
        assert!(set.remove(1));
    }

    #[test]
    #[should_panic(expected = "Cannot get random element from empty set")]
    fn test_panic_on_empty_get_random() {
        let mut set = RandomizedSet::new();
        set.get_random();
    }

    #[test]
    fn test_stress_test() {
        let mut set = RandomizedSet::new();
        // Insert a bunch of elements
        for i in 0..100 {
            assert!(set.insert(i));
        }

        // Ensure random distribution covers multiple values (statistically highly likely)
        let mut seen = HashSet::new();
        for _ in 0..1000 {
            seen.insert(set.get_random());
        }

        // We should have seen at least most of the numbers
        assert!(seen.len() > 80);

        // Remove half
        for i in 0..50 {
            assert!(set.remove(i));
        }

        // Insert some back
        for i in 25..75 {
            set.insert(i);
        }

        // Validate internal state consistency
        assert_eq!(set.values.len(), 75); // (100 - 50) + (75 - 25 - overlap(25..50)) -> wait, 0..50 removed, so 50..100 left (50 elements). Adding 25..75: 25..50 are new (25 elements), 50..75 are already there. Total = 75.
        assert_eq!(set.indices.len(), 75);

        for val in &set.values {
            assert!(set.indices.contains_key(val));
        }
    }
}
