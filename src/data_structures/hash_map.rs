//! # Hash Map Implementation
//!
//! Implements a Hash Map from scratch using open addressing and linear probing with Robin Hood hashing.
//! This demonstrates how fundamental key-value stores manage collisions, load factors, and resizing without chaining.
//!
//! **Replaces Crates:** `std::collections::HashMap`, `hashbrown`
//!
//! **Real-world Usage:**
//! - Almost everywhere: caching, state management, symbol tables in compilers.
//! - Fast lookups in database query engines.
//! - Deduplication and grouping operations.
//!
//! **Why build it yourself?**
//! Everyone uses HashMaps, but few understand the mechanics of collision resolution,
//! load factor triggers, and the performance differences between chaining (linked lists)
//! and open addressing (arrays). Robin Hood hashing teaches you how to minimize the variance
//! of probe lengths, drastically reducing the worst-case lookup time.

use std::borrow::Borrow;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::mem;

// =========================================================================================
// Architecture
// =========================================================================================
//
// Data Structure:
//
//      Array of Entries (Buckets)
//      ┌───────┬───────┬───────┬───────┬───────┐
//      │ Empty │ K,V,D │ K,V,D │ Empty │ K,V,D │  (D = Distance from initial bucket)
//      └───────┴───────┴───────┴───────┴───────┘
//
// Robin Hood Hashing Logic:
// - When inserting, we probe forward if the bucket is occupied.
// - We track the "Distance from Initial Bucket" (DIB) for the element being inserted.
// - If we find an occupied bucket where the existing element has a *smaller* DIB than our
//   current element, we *swap* them! We steal from the "rich" (low DIB) and give to the
//   "poor" (high DIB). Then we continue inserting the displaced element.
//
// Invariants:
// 1. Capacity is always a power of 2 (allows fast modulo via bitwise AND).
// 2. Load factor never exceeds the maximum (typically 0.75 or 0.8) to guarantee O(1) performance.
// 3. Elements are contiguous in memory (Open Addressing), providing excellent cache locality.
//
// Complexity:
// ┌───────────┬──────────────┬────────┐
// │ Operation │ Time         │ Space  │
// ├───────────┼──────────────┼────────┤
// │ get       │ O(1) average │ O(1)   │
// │ insert    │ O(1) amort.  │ O(1)   │
// │ remove    │ O(1) average │ O(1)   │
// └───────────┴──────────────┴────────┘
//
// Design Decisions:
// - **Collision Resolution**: Open Addressing with Robin Hood Hashing.
//   - *Tradeoff*: Excellent cache locality and low variance in probe lengths, but deletions are tricky (requires backward shifting to prevent breaking probe chains).
//   - *Alternative*: Separate Chaining (Vec of LinkedLists) - easier to implement but terrible cache locality.
// - **Hashing**: `DefaultHasher` (SipHash).
//   - *Tradeoff*: `DefaultHasher` uses SipHash which is cryptographically resistant, but `DefaultHasher::new()` uses fixed keys internally.
//     Therefore, this implementation is NOT truly HashDoS-resistant by default. `std::collections::HashMap` uses `RandomState` to seed the hasher randomly per-instance.
// - **Capacity**: Power of 2. `hash & (capacity - 1)` is much faster than `hash % capacity`.

const INITIAL_CAPACITY: usize = 8;
const MAX_LOAD_FACTOR: f32 = 0.8;

#[derive(Clone, Debug)]
struct Entry<K, V> {
    key: K,
    value: V,
    dib: usize, // Distance from Initial Bucket
    hash: u64,  // Cached hash to speed up resizing and comparisons
}

/// A Hash Map using Robin Hood hashing.
pub struct HashMap<K, V> {
    buckets: Vec<Option<Entry<K, V>>>,
    len: usize,
}

impl<K: Hash + Eq, V> Default for HashMap<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K: Hash + Eq, V> HashMap<K, V> {
    /// Creates an empty HashMap.
    #[must_use]
    pub fn new() -> Self {
        let mut buckets = Vec::with_capacity(INITIAL_CAPACITY);
        buckets.resize_with(INITIAL_CAPACITY, || None);

        Self { buckets, len: 0 }
    }

    /// Returns the number of elements in the map.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Returns true if the map contains no elements.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Inserts a key-value pair into the map.
    /// If the map did not have this key present, `None` is returned.
    /// If the map did have this key present, the value is updated, and the old value is returned.
    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        if (self.len + 1) as f32 / self.buckets.len() as f32 > MAX_LOAD_FACTOR {
            self.resize();
        }

        let hash = Self::hash_key(&key);
        let mut entry = Entry {
            key,
            value,
            dib: 0,
            hash,
        };

        let cap = self.buckets.len();
        let mut idx = (hash as usize) & (cap - 1);

        loop {
            match self.buckets[idx].as_mut() {
                Some(existing) => {
                    if existing.hash == entry.hash && existing.key == entry.key {
                        // Key exists, update value and return old
                        return Some(mem::replace(&mut existing.value, entry.value));
                    }

                    // Robin Hood swap: if the current entry has probed further than the existing one, swap them.
                    if entry.dib > existing.dib {
                        mem::swap(existing, &mut entry);
                    }

                    // Continue probing for the displaced (or original) entry
                    entry.dib += 1;
                    idx = (idx + 1) & (cap - 1);
                }
                None => {
                    // Empty bucket found, insert here
                    self.buckets[idx] = Some(entry);
                    self.len += 1;
                    return None;
                }
            }
        }
    }

    /// Returns a reference to the value corresponding to the key.
    pub fn get<Q>(&self, key: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        let hash = Self::hash_key(key);
        let cap = self.buckets.len();
        let mut idx = (hash as usize) & (cap - 1);
        let mut dib = 0;

        loop {
            match &self.buckets[idx] {
                Some(entry) => {
                    if entry.hash == hash && entry.key.borrow() == key {
                        return Some(&entry.value);
                    }
                    if dib > entry.dib {
                        // The element we are looking for would have been inserted before this one
                        // or would have displaced it. Therefore, it's not in the map.
                        return None;
                    }
                    dib += 1;
                    idx = (idx + 1) & (cap - 1);
                }
                None => return None,
            }
        }
    }

    /// Returns a mutable reference to the value corresponding to the key.
    pub fn get_mut<Q>(&mut self, key: &Q) -> Option<&mut V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        let hash = Self::hash_key(key);
        let cap = self.buckets.len();
        let mut idx = (hash as usize) & (cap - 1);
        let mut dib = 0;

        loop {
            // RUST INSIGHT: To avoid NLL borrow checking issues, we first extract
            // whether the current bucket matches or whether we should stop searching.
            let (is_match, stop) = match &self.buckets[idx] {
                Some(entry) => (
                    entry.hash == hash && entry.key.borrow() == key,
                    dib > entry.dib,
                ),
                None => return None,
            };

            if is_match {
                // Now we can safely take a mutable reference because the immutable borrow above has ended.
                return self.buckets[idx].as_mut().map(|e| &mut e.value);
            }

            if stop {
                return None;
            }

            dib += 1;
            idx = (idx + 1) & (cap - 1);
        }
    }

    /// Removes a key from the map, returning the value at the key if the key was previously in the map.
    pub fn remove<Q>(&mut self, key: &Q) -> Option<V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        let hash = Self::hash_key(key);
        let cap = self.buckets.len();
        let mut idx = (hash as usize) & (cap - 1);
        let mut dib = 0;

        loop {
            match &self.buckets[idx] {
                Some(entry) => {
                    if entry.hash == hash && entry.key.borrow() == key {
                        // Found it. Remove it and do backward shifting to fill the gap.
                        let old_val = self.buckets[idx].take().unwrap().value;
                        self.len -= 1;
                        self.backward_shift(idx);
                        return Some(old_val);
                    }
                    if dib > entry.dib {
                        return None;
                    }
                    dib += 1;
                    idx = (idx + 1) & (cap - 1);
                }
                None => return None,
            }
        }
    }

    /// Checks if the map contains the specific key.
    pub fn contains_key<Q>(&self, key: &Q) -> bool
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.get(key).is_some()
    }

    // --- Private Helpers ---

    fn hash_key<Q: Hash + ?Sized>(key: &Q) -> u64 {
        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        hasher.finish()
    }

    fn resize(&mut self) {
        let new_cap = self.buckets.len() * 2;
        let mut new_buckets = Vec::with_capacity(new_cap);
        new_buckets.resize_with(new_cap, || None);

        let old_buckets = mem::replace(&mut self.buckets, new_buckets);
        self.len = 0; // Will be incremented during insert

        for mut entry in old_buckets.into_iter().flatten() {
            // Re-insert using the cached hash directly to avoid recalculating
            entry.dib = 0; // Reset DIB for the new array

            let cap = self.buckets.len();
            let mut idx = (entry.hash as usize) & (cap - 1);

            loop {
                match self.buckets[idx].as_mut() {
                    Some(existing) => {
                        if entry.dib > existing.dib {
                            mem::swap(existing, &mut entry);
                        }
                        entry.dib += 1;
                        idx = (idx + 1) & (cap - 1);
                    }
                    None => {
                        self.buckets[idx] = Some(entry);
                        self.len += 1;
                        break;
                    }
                }
            }
        }
    }

    /// Shifts elements backward to fill the hole left by a removed element.
    /// This prevents breaking the probe chains.
    fn backward_shift(&mut self, mut hole_idx: usize) {
        let cap = self.buckets.len();
        loop {
            let next_idx = (hole_idx + 1) & (cap - 1);
            match self.buckets[next_idx].take() {
                Some(mut next_entry) => {
                    if next_entry.dib == 0 {
                        // This element is exactly where it belongs, stop shifting.
                        self.buckets[next_idx] = Some(next_entry);
                        break;
                    }
                    // Move it back to the hole, decrementing its DIB
                    next_entry.dib -= 1;
                    self.buckets[hole_idx] = Some(next_entry);
                    hole_idx = next_idx;
                }
                None => {
                    // Reached an empty bucket, chain ends here.
                    break;
                }
            }
        }
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `hashbrown` (which is standard `std::collections::HashMap` since Rust 1.36):
//   Uses SwissTable (Google's Abseil `flat_hash_map`). It splits the map into a control bytes array (1 byte per bucket)
//   and a dense data array. It uses SIMD to probe 16 buckets at once. It's significantly more complex and faster
//   than pure Robin Hood hashing.
//
// Missing vs. Production:
// - **Iterators**: We don't implement `.keys()`, `.values()`, `.iter()`, or `.into_iter()`.
// - **Entry API**: We don't have the `map.entry(key).or_insert(val)` API which prevents double lookups.
// - **SIMD Probing**: Modern maps use SIMD (like SwissTable).
// - **DOS Resistance**: `DefaultHasher::new()` uses fixed keys and is NOT DOS-resistant. We need `RandomState` to randomly seed the hasher per-instance.
//
// Next Steps:
// 1. Implement the Entry API (`OccupiedEntry`, `VacantEntry`).
// 2. Add iterators (Requires scanning the sparse `buckets` array).
// 3. Allow generic `S: BuildHasher`.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert_and_get() {
        let mut map = HashMap::new();
        assert_eq!(map.insert("a", 1), None);
        assert_eq!(map.insert("b", 2), None);

        assert_eq!(map.get("a"), Some(&1));
        assert_eq!(map.get("b"), Some(&2));
        assert_eq!(map.get("c"), None);
        assert_eq!(map.len(), 2);
    }

    #[test]
    fn test_update() {
        let mut map = HashMap::new();
        map.insert("a", 1);
        assert_eq!(map.insert("a", 2), Some(1));
        assert_eq!(map.get("a"), Some(&2));
        assert_eq!(map.len(), 1);
    }

    #[test]
    fn test_remove() {
        let mut map = HashMap::new();
        map.insert("a", 1);
        map.insert("b", 2);

        assert_eq!(map.remove("a"), Some(1));
        assert_eq!(map.get("a"), None);
        assert_eq!(map.len(), 1);

        assert_eq!(map.remove("c"), None);
    }

    #[test]
    fn test_resize() {
        let mut map = HashMap::new();
        let num_elements = 100;

        for i in 0..num_elements {
            map.insert(i, i * 10);
        }

        assert_eq!(map.len(), num_elements);
        assert!(map.buckets.len() > INITIAL_CAPACITY);

        for i in 0..num_elements {
            assert_eq!(map.get(&i), Some(&(i * 10)));
        }
    }

    #[test]
    fn test_backward_shift() {
        // We craft a scenario where elements collide to force DIB > 0
        let mut map = HashMap::new();

        // Insert many elements to cause collisions
        for i in 0..20 {
            map.insert(i, i);
        }

        // Remove an element from the middle of a probe chain
        map.remove(&10);

        // Ensure all other elements are still reachable
        for i in 0..20 {
            if i != 10 {
                assert_eq!(map.get(&i), Some(&i));
            } else {
                assert_eq!(map.get(&i), None);
            }
        }
    }
}
