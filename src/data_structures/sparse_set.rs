//! # Sparse Set
//!
//! A Sparse Set is a high-performance data structure widely used in Entity-Component Systems (ECS)
//! to store and manage components associated with entities.
//!
//! **Replaces Crates:** `sparsey`, `hecs` (internal component storage), C++'s `entt`
//! **Real-world systems:** Game engines (Bevy, Amethyst, `EnTT`) use sparse sets to pack component data
//! contiguously in memory, enabling extremely fast, cache-friendly iteration while maintaining `O(1)` random access.
//!
//! **Why build it yourself?**
//! Understanding sparse sets demystifies how modern game engines achieve such high performance. It also
//! showcases advanced array manipulation and safe abstractions over what is traditionally very pointer-heavy C++ code.
//!
//! ## Architecture
//!
//! A Sparse Set consists of two arrays:
//! 1. `sparse`: An array indexed by the external `id` (e.g., Entity ID). It stores the index into the `dense` array.
//!    Since external IDs can be large and disjoint, this array has "holes" (hence, sparse).
//! 2. `dense`: A tightly packed array that stores the actual values, alongside the original `id` they belong to.
//!
//! ```text
//! External ID: 4
//!
//! sparse array (indexed by ID):
//! [0] -> None
//! [1] -> None
//! [2] -> Some(1)
//! [3] -> None
//! [4] -> Some(0)  <-- ID 4 is at dense index 0
//!
//! dense array (packed contiguously):
//! [0] -> (ID: 4, Value: "Health(100)")
//! [1] -> (ID: 2, Value: "Health(50)")
//! ```
//!
//! **Invariants:**
//! - If an `id` is present, `sparse[id]` contains `dense_index`.
//! - `dense[dense_index]` contains `(id, value)`.
//! - `dense` has no holes. When an element is removed, the last element in `dense` is swapped into its place
//!   (`swap_remove`), and the `sparse` array is updated to point to the new location.
//!
//! **Complexity:**
//! - `insert(id)`: O(1) amortized
//! - `remove(id)`: O(1) (using `swap_remove`)
//! - `get(id)`: O(1)
//! - `iterate()`: O(N) over contiguous memory (extremely cache-friendly)

/// The API for a Sparse Set.
pub trait SparseSetApi<V> {
    /// Inserts a value for the given ID.
    fn insert(&mut self, id: usize, value: V);

    /// Retrieves a reference to the value for the given ID.
    fn get(&self, id: usize) -> Option<&V>;

    /// Retrieves a mutable reference to the value for the given ID.
    fn get_mut(&mut self, id: usize) -> Option<&mut V>;

    /// Removes and returns the value for the given ID.
    fn remove(&mut self, id: usize) -> Option<V>;

    /// Returns true if the set contains the given ID.
    fn contains(&self, id: usize) -> bool;

    /// Clears all elements from the set.
    fn clear(&mut self);

    /// Returns the number of elements in the set.
    fn len(&self) -> usize;

    /// Returns true if the set is empty.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// A packed entry in the dense array.
#[derive(Debug, Clone)]
pub struct Entry<V> {
    /// The original sparse ID. We need this to update the sparse array during a `swap_remove`.
    pub id: usize,
    /// The actual component data.
    pub value: V,
}

/// A Sparse Set implementation.
#[derive(Debug, Clone)]
pub struct SparseSet<V> {
    /// Maps a sparse ID to an index in the `dense` array.
    /// Uses `Option<usize>` to represent empty slots.
    sparse: Vec<Option<usize>>,

    /// Tightly packed entries containing both the ID and the Value.
    dense: Vec<Entry<V>>,
}

impl<V> SparseSet<V> {
    /// Creates a new, empty Sparse Set.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            sparse: Vec::new(),
            dense: Vec::new(),
        }
    }

    /// Creates a new, empty Sparse Set with the given capacities.
    #[must_use]
    pub fn with_capacity(sparse_cap: usize, dense_cap: usize) -> Self {
        Self {
            sparse: Vec::with_capacity(sparse_cap),
            dense: Vec::with_capacity(dense_cap),
        }
    }

    /// Returns a slice of all tightly packed entries.
    /// This is the killer feature: O(N) cache-friendly iteration.
    #[must_use]
    pub fn entries(&self) -> &[Entry<V>] {
        &self.dense
    }

    /// Returns a mutable slice of all tightly packed entries.
    pub fn entries_mut(&mut self) -> &mut [Entry<V>] {
        &mut self.dense
    }
}

impl<V> Default for SparseSet<V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<V> SparseSetApi<V> for SparseSet<V> {
    fn insert(&mut self, id: usize, value: V) {
        // Expand the sparse array if the ID is out of bounds.
        // GOTCHA: This is where sparse sets can waste memory. If you insert ID `1_000_000`
        // into an empty set, the sparse array will allocate 1M elements.
        // In real ECS, IDs are typically contiguous (0, 1, 2...) or generational indices.
        if id >= self.sparse.len() {
            self.sparse.resize_with(id + 1, || None);
        }

        if let Some(dense_idx) = self.sparse[id] {
            // RUST INSIGHT: Safe mutable access via indices.
            // If the ID already exists, overwrite the value.
            self.dense[dense_idx].value = value;
        } else {
            // New entry: push to the dense array and map it in the sparse array.
            let dense_idx = self.dense.len();
            self.sparse[id] = Some(dense_idx);
            self.dense.push(Entry { id, value });
        }
    }

    fn get(&self, id: usize) -> Option<&V> {
        // Double indirection: Sparse -> Dense -> Value
        self.sparse
            .get(id)
            .copied()
            .flatten()
            .map(|dense_idx| &self.dense[dense_idx].value)
    }

    fn get_mut(&mut self, id: usize) -> Option<&mut V> {
        self.sparse
            .get(id)
            .copied()
            .flatten()
            .map(|dense_idx| &mut self.dense[dense_idx].value)
    }

    fn remove(&mut self, id: usize) -> Option<V> {
        // Find the dense index from the sparse array.
        let dense_idx = self.sparse.get(id).copied().flatten()?;

        // RUST INSIGHT: `swap_remove` is O(1). It replaces the removed element
        // with the last element in the vector to maintain contiguous memory.
        let removed_entry = self.dense.swap_remove(dense_idx);

        // Clear the sparse entry for the removed ID.
        self.sparse[id] = None;

        // PRODUCTION NOTE: If `dense_idx` was *not* the last element, we must update
        // the sparse index of the element that was swapped into its place.
        if dense_idx < self.dense.len() {
            let swapped_id = self.dense[dense_idx].id;
            self.sparse[swapped_id] = Some(dense_idx);
        }

        Some(removed_entry.value)
    }

    fn contains(&self, id: usize) -> bool {
        self.sparse.get(id).copied().flatten().is_some()
    }

    fn clear(&mut self) {
        // O(N) where N is dense.len(), keeps capacity.
        // We could just clear the sparse array entirely, or only clear the mapped indices.
        for entry in &self.dense {
            self.sparse[entry.id] = None;
        }
        self.dense.clear();
    }

    fn len(&self) -> usize {
        self.dense.len()
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `sparsey` or `hecs`: Provide fully-fledged ECS storage including generational indices
//   (e.g. `Entity(id: u32, generation: u32)`) to prevent ID reuse bugs.
// - This implementation uses raw `usize` for simplicity, but in production, you'd use a tuple
//   of `(index, generation)` for the ID.
//
// Missing vs. Production:
// - **Generational Indices:** To handle deleted entities safely without ABA problems.
// - **Iterators:** We expose a slice `entries()`, but real ECS provides zipped iterators to
//   join multiple Sparse Sets together (e.g. `for (pos, vel) in join(&positions, &velocities)`).
// - **Paging:** Instead of one giant `sparse` Vec, production ECS uses paged arrays (e.g., arrays
//   of arrays) so you don't waste memory if you insert ID `0` and `1,000,000`.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert_and_get() {
        let mut set = SparseSet::new();
        assert!(set.is_empty());

        set.insert(4, "Entity 4");
        set.insert(2, "Entity 2");
        set.insert(10, "Entity 10");

        assert_eq!(set.len(), 3);
        assert_eq!(set.get(4), Some(&"Entity 4"));
        assert_eq!(set.get(2), Some(&"Entity 2"));
        assert_eq!(set.get(10), Some(&"Entity 10"));
        assert_eq!(set.get(0), None);
        assert_eq!(set.get(5), None);
    }

    #[test]
    fn test_overwrite() {
        let mut set = SparseSet::new();
        set.insert(1, "A");
        assert_eq!(set.get(1), Some(&"A"));

        set.insert(1, "B");
        assert_eq!(set.get(1), Some(&"B"));
        assert_eq!(set.len(), 1); // Should not increase length
    }

    #[test]
    fn test_remove() {
        let mut set = SparseSet::new();
        set.insert(1, "A");
        set.insert(2, "B");
        set.insert(3, "C");

        // Remove from the middle to test `swap_remove`
        assert_eq!(set.remove(2), Some("B"));
        assert_eq!(set.len(), 2);
        assert_eq!(set.get(2), None);

        // Verify the other elements are still intact
        assert_eq!(set.get(1), Some(&"A"));
        assert_eq!(set.get(3), Some(&"C"));

        // Verify `dense` is contiguous (C should have swapped to index 1)
        assert_eq!(set.entries().len(), 2);
        assert_eq!(set.entries()[0].value, "A");
        assert_eq!(set.entries()[1].value, "C");

        // Remove non-existent
        assert_eq!(set.remove(5), None);
    }

    #[test]
    fn test_clear() {
        let mut set = SparseSet::new();
        set.insert(1, "A");
        set.insert(2, "B");

        set.clear();
        assert!(set.is_empty());
        assert_eq!(set.get(1), None);
        assert_eq!(set.get(2), None);

        // Verify capacity is preserved (dense array)
        assert!(set.dense.capacity() >= 2);
    }

    #[test]
    fn test_large_gap_allocation() {
        let mut set = SparseSet::new();

        // This causes the sparse array to allocate 10_001 elements.
        set.insert(10_000, "Far away");

        assert_eq!(set.len(), 1);
        assert_eq!(set.get(10_000), Some(&"Far away"));
        assert_eq!(set.get(0), None);
    }

    #[test]
    fn test_iteration() {
        let mut set = SparseSet::new();
        set.insert(5, 50);
        set.insert(1, 10);
        set.insert(9, 90);

        // Iteration order depends on insertion and `swap_remove` history.
        // Initially, it matches insertion order.
        let values: Vec<i32> = set.entries().iter().map(|e| e.value).collect();
        assert_eq!(values, vec![50, 10, 90]);

        set.remove(5); // Removes 5 (index 0), swaps 9 (index 2) to index 0.

        let values2: Vec<i32> = set.entries().iter().map(|e| e.value).collect();
        assert_eq!(values2, vec![90, 10]);
    }
}
