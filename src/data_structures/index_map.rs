//! # IndexMap (Linked Hash Map) Implementation
//!
//! An ordered hash map that preserves insertion order while maintaining `O(1)` average
//! time complexity for lookups, insertions, and removals.
//!
//! **Replaces Crates:** `indexmap`, `linked-hash-map`
//!
//! **Real-world Usage:**
//! - JSON parsing (preserving key order of objects).
//! - Configuration files (TOML, YAML) where human-readable order matters.
//! - Caches that need predictable iteration (like LRU bases).
//!
//! **Why build it yourself?**
//! Building an `IndexMap` demystifies how to combine contiguous memory (`Vec`) for order/iteration
//! with a lookup table (`HashMap`) for indexing. It teaches you about index management during
//! deletions (`swap_remove` vs. `shift_remove`) and why traditional pointers (like in C++'s `std::list`)
//! are often slower than pure array indices due to cache locality.

use std::collections::HashMap;
use std::hash::Hash;

// =========================================================================================
// Architecture
// =========================================================================================
//
// Data Structure:
//
//      entries: Vec<(K, V)>        (Provides stable insertion order and cache-friendly iteration)
//      indices: HashMap<K, usize>  (Maps keys to their current index in the `entries` vector)
//
// Flow (Insert):
//      "apple" -> "red"
//      1. Push ("apple", "red") to `entries` -> gets index `i`
//      2. Insert "apple" -> `i` into `indices`
//
// Flow (Remove - swap_remove):
//      Remove index `i`.
//      1. Swap element at `i` with the last element in `entries`.
//      2. Pop the last element (which is the one we want to remove).
//      3. Update `indices` for the swapped element to point to `i`.
//
// Flow (Remove - shift_remove):
//      Remove index `i`.
//      1. Remove element at `i` from `entries` (shifts all subsequent elements down by 1).
//      2. Update `indices` for all elements from `i` to the end.
//
// Invariants:
// 1. `entries.len() == indices.len()`.
// 2. For every `(k, _)` at index `i` in `entries`, `indices.get(k) == Some(&i)`.
// 3. Keys must be unique; inserting an existing key overwrites the value but does NOT change its original order.
//
// Complexity:
// ┌───────────────┬─────────────┬─────────────┐
// │ Operation     │ Time        │ Space       │
// ├───────────────┼─────────────┼─────────────┤
// │ insert        │ O(1) avg    │ O(1)        │
// │ get           │ O(1) avg    │ O(1)        │
// │ swap_remove   │ O(1) avg    │ O(1)        │
// │ shift_remove  │ O(N)        │ O(1)        │
// └───────────────┴─────────────┴─────────────┘
//
// Design Decisions:
// - **Indices over Pointers**: Rust's borrow checker makes standard doubly-linked lists hard.
//   An arena/index-based approach is perfectly safe, requires zero `unsafe` code, and offers
//   better CPU cache locality since data is contiguous in memory.

/// A generic ordered map interface.
pub trait OrderedMap<K, V> {
    /// Inserts a key-value pair into the map.
    /// If the map did not have this key present, `None` is returned.
    /// If the map did have this key present, the value is updated, and the old value is returned.
    /// The key's original insertion order is preserved.
    fn insert(&mut self, key: K, value: V) -> Option<V>;

    /// Returns a reference to the value corresponding to the key.
    fn get(&self, key: &K) -> Option<&V>;

    /// Returns a mutable reference to the value corresponding to the key.
    fn get_mut(&mut self, key: &K) -> Option<&mut V>;

    /// Removes a key from the map, returning the value at the key if the key was previously in the map.
    /// This swaps the removed element with the last element, changing the order of the last element.
    fn swap_remove(&mut self, key: &K) -> Option<V>;

    /// Removes a key from the map, returning the value at the key if the key was previously in the map.
    /// This shifts all subsequent elements down, preserving the order of all remaining elements.
    fn shift_remove(&mut self, key: &K) -> Option<V>;

    /// Returns the number of elements in the map.
    fn len(&self) -> usize;

    /// Returns true if the map contains no elements.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// A hash map that preserves insertion order.
#[derive(Debug, Clone)]
pub struct IndexMap<K, V> {
    entries: Vec<(K, V)>,
    indices: HashMap<K, usize>,
}

impl<K, V> IndexMap<K, V>
where
    K: Eq + Hash + Clone,
{
    /// Creates an empty `IndexMap`.
    #[must_use]
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            indices: HashMap::new(),
        }
    }

    /// Creates an empty `IndexMap` with the specified capacity.
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            entries: Vec::with_capacity(capacity),
            indices: HashMap::with_capacity(capacity),
        }
    }

    /// Returns an iterator over the map's entries in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = (&K, &V)> {
        self.entries.iter().map(|(k, v)| (k, v))
    }
}

impl<K, V> OrderedMap<K, V> for IndexMap<K, V>
where
    K: Eq + Hash + Clone,
{
    fn insert(&mut self, key: K, value: V) -> Option<V> {
        if let Some(&index) = self.indices.get(&key) {
            // RUST INSIGHT: Overwriting existing keys doesn't change their order.
            // We directly mutate the `entries` vector using the known index.
            let old_val = std::mem::replace(&mut self.entries[index].1, value);
            Some(old_val)
        } else {
            let index = self.entries.len();
            self.indices.insert(key.clone(), index);
            self.entries.push((key, value));
            None
        }
    }

    fn get(&self, key: &K) -> Option<&V> {
        let index = self.indices.get(key)?;
        Some(&self.entries[*index].1)
    }

    fn get_mut(&mut self, key: &K) -> Option<&mut V> {
        let index = self.indices.get(key)?;
        Some(&mut self.entries[*index].1)
    }

    fn swap_remove(&mut self, key: &K) -> Option<V> {
        // Find the index of the element to remove.
        let index = self.indices.remove(key)?;

        // RUST INSIGHT: `Vec::swap_remove` replaces the element at `index` with the last element
        // in the vector, then pops the end. This is O(1).
        let removed_entry = self.entries.swap_remove(index);

        // GOTCHA: After swapping, the element that was at the end of `entries` is now at `index`.
        // We MUST update its index in the `indices` map, UNLESS the removed element was already the last one.
        if index < self.entries.len() {
            let swapped_key = &self.entries[index].0;
            self.indices.insert(swapped_key.clone(), index);
        }

        Some(removed_entry.1)
    }

    fn shift_remove(&mut self, key: &K) -> Option<V> {
        // Find the index of the element to remove.
        let index = self.indices.remove(key)?;

        // `Vec::remove` shifts all subsequent elements to the left. This is O(N).
        let removed_entry = self.entries.remove(index);

        // We must update the indices of all elements that were shifted.
        // Starting from `index`, every element's index decreased by 1.
        for i in index..self.entries.len() {
            let shifted_key = &self.entries[i].0;
            if let Some(pos) = self.indices.get_mut(shifted_key) {
                *pos = i;
            }
        }

        Some(removed_entry.1)
    }

    fn len(&self) -> usize {
        self.entries.len()
    }
}

impl<K, V> Default for IndexMap<K, V>
where
    K: Eq + Hash + Clone,
{
    fn default() -> Self {
        Self::new()
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `indexmap`: The standard crate uses `hashbrown::raw::RawTable` directly to avoid storing
//   hashes or keys twice (once in the Vec, once in the Map). Instead, it stores the hash
//   and an index into the `entries` Vec inside the `RawTable`. This drastically reduces memory overhead.
//   Our implementation stores the Key in both `entries` and `indices`, doubling the key storage cost.
//
// Missing vs. Production:
// - **Memory Efficiency**: As noted, we duplicate keys. A production index map uses raw hash tables
//   to map hashes directly to vector indices.
// - **Entry API**: We lack the `Entry` API (`Vacant`/`Occupied`) which is standard for Rust maps.
// - **Sorting**: Production implementations allow sorting the underlying vector `sort_keys()` because
//   the contiguous storage makes sorting trivial compared to a standard `HashMap`.
//
// Next Steps:
// 1. Implement the `Entry` API.
// 2. Implement a `sort_keys()` method to allow ordering the map post-insertion.
//
// Benchmarking Note:
// To benchmark, compare `shift_remove` (O(N)) against `swap_remove` (O(1)) on maps of 100k+ elements.
// You will see a massive performance cliff for `shift_remove`.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert_and_get() {
        let mut map = IndexMap::new();
        assert_eq!(map.insert("a", 1), None);
        assert_eq!(map.insert("b", 2), None);

        assert_eq!(map.get(&"a"), Some(&1));
        assert_eq!(map.get(&"b"), Some(&2));
        assert_eq!(map.get(&"c"), None);
        assert_eq!(map.len(), 2);
    }

    #[test]
    fn test_update_existing_key() {
        let mut map = IndexMap::new();
        map.insert("a", 1);
        map.insert("b", 2);

        // Update "a"
        assert_eq!(map.insert("a", 10), Some(1));
        assert_eq!(map.get(&"a"), Some(&10));

        // Order should remain unchanged
        let iter: Vec<_> = map.iter().collect();
        assert_eq!(iter, vec![(&"a", &10), (&"b", &2)]);
    }

    #[test]
    fn test_swap_remove() {
        let mut map = IndexMap::new();
        map.insert("a", 1);
        map.insert("b", 2);
        map.insert("c", 3);
        map.insert("d", 4);

        // Remove "b"
        assert_eq!(map.swap_remove(&"b"), Some(2));
        assert_eq!(map.len(), 3);

        // "d" should have swapped into "b"'s spot. Order is now a, d, c
        let iter: Vec<_> = map.iter().collect();
        assert_eq!(iter, vec![(&"a", &1), (&"d", &4), (&"c", &3)]);

        // Ensure "d" is still accessible
        assert_eq!(map.get(&"d"), Some(&4));
    }

    #[test]
    fn test_swap_remove_last_element() {
        let mut map = IndexMap::new();
        map.insert("a", 1);
        map.insert("b", 2);

        assert_eq!(map.swap_remove(&"b"), Some(2));
        let iter: Vec<_> = map.iter().collect();
        assert_eq!(iter, vec![(&"a", &1)]);
    }

    #[test]
    fn test_shift_remove() {
        let mut map = IndexMap::new();
        map.insert("a", 1);
        map.insert("b", 2);
        map.insert("c", 3);
        map.insert("d", 4);

        // Remove "b"
        assert_eq!(map.shift_remove(&"b"), Some(2));
        assert_eq!(map.len(), 3);

        // Order is preserved, elements shift down. Order is now a, c, d
        let iter: Vec<_> = map.iter().collect();
        assert_eq!(iter, vec![(&"a", &1), (&"c", &3), (&"d", &4)]);

        // Ensure indices are updated correctly
        assert_eq!(map.get(&"c"), Some(&3));
        assert_eq!(map.get(&"d"), Some(&4));
    }
}
