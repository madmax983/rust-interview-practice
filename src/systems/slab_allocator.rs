//! # Slab Allocator Implementation
//!
//! A memory management primitive that provides efficient storage for fixed-size objects with O(1) allocation
//! and deallocation, using a free list within a contiguous vector.
//!
//! **Replaces Crates:** `slab`, `sharded-slab`
//!
//! **Real-world Usage:**
//! - Entity Component Systems (ECS) in game engines (storing entities).
//! - Async runtimes (storing task state).
//! - Graph nodes (where indices are used as pointers).
//!
//! **Why build it yourself?**
//! Standard `Vec` has O(1) push but O(N) remove (unless `swap_remove`, which changes indices).
//! A Slab provides stable indices and O(1) removal by tracking a "free list" of vacant slots within the vector itself.
//! Adding "Generational Indices" solves the "ABA Problem" where a slot is reused but an old reference (handle) still points to it.

// =========================================================================================
// Architecture
// =========================================================================================
//
// Data Structure:
// - `entries`: Vec<Slot<T>>
// - `next_free`: Index of the next available slot (head of free list).
// - `len`: Count of occupied slots.
//
// Slot Struct:
// - `generation`: u64 counter to detect stale keys.
// - `data`: SlotData enum (Occupied(T) or Vacant(next_free)).
//
// Key (Handle):
// - `index`: The index in the vector.
// - `generation`: The generation ID expected at that index.
//
// Operations:
// - `insert(val)`:
//   - If free list not empty: Use slot at `next_free`. Update `next_free` to point to next vacant. Increment generation.
//   - If free list empty: Push new `Occupied` to Vec.
// - `remove(key)`:
//   - Check generation.
//   - Mark slot as `Vacant`.
//   - Point slot's `next_free` to current global `next_free`.
//   - Update global `next_free` to this slot.
//
// Complexity:
// ┌───────────────┬─────────────┬─────────────┐
// │ Operation     │ Time        │ Space       │
// ├───────────────┼─────────────┼─────────────┤
// │ Insert        │ O(1)        │ O(1)        │
// │ Remove        │ O(1)        │ O(1)        │
// │ Get           │ O(1)        │ O(1)        │
// └───────────────┴─────────────┴─────────────┘

/// A handle to an object in the Slab.
/// Includes a generation counter to detect stale references.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Key {
    index: usize,
    generation: u64,
}

// Internal storage unit
#[derive(Debug)]
struct Slot<T> {
    generation: u64,
    data: SlotData<T>,
}

#[derive(Debug)]
enum SlotData<T> {
    Occupied(T),
    Vacant(usize), // Next free index
}

pub struct Slab<T> {
    entries: Vec<Slot<T>>,
    next_free: usize,
    len: usize,
}

impl<T> Default for Slab<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Slab<T> {
    /// Creates a new empty Slab.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            entries: Vec::new(),
            next_free: usize::MAX, // Sentinel for "no free slots"
            len: 0,
        }
    }

    /// Creates a new Slab with specified capacity.
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            entries: Vec::with_capacity(capacity),
            next_free: usize::MAX,
            len: 0,
        }
    }

    /// Inserts a value into the slab and returns its Key.
    pub fn insert(&mut self, val: T) -> Key {
        if self.next_free == usize::MAX {
            // No free slots, push new one
            let index = self.entries.len();
            self.entries.push(Slot {
                generation: 0,
                data: SlotData::Occupied(val),
            });
            self.len += 1;
            Key {
                index,
                generation: 0,
            }
        } else {
            // Reuse a free slot
            let index = self.next_free;
            let slot = &mut self.entries[index];

            // Extract next free pointer from current vacant slot
            let next_free = match slot.data {
                SlotData::Vacant(next) => next,
                SlotData::Occupied(_) => unreachable!("Corrupted free list"),
            };

            // Update global next_free
            self.next_free = next_free;

            // Update slot to Occupied
            // Increment generation on reuse to invalidate old keys
            slot.generation += 1;
            slot.data = SlotData::Occupied(val);
            self.len += 1;

            Key {
                index,
                generation: slot.generation,
            }
        }
    }

    /// Removes a value associated with the key.
    /// Returns the value if the key is valid, None otherwise.
    pub fn remove(&mut self, key: Key) -> Option<T> {
        if key.index >= self.entries.len() {
            return None;
        }

        let slot = &mut self.entries[key.index];

        // Check generation
        if slot.generation != key.generation {
            return None; // Stale key
        }

        // Check if actually occupied (redundant with generation check usually, but safe)
        if matches!(slot.data, SlotData::Vacant(_)) {
            return None;
        }

        // Take value and replace with Vacant
        let old_data = std::mem::replace(&mut slot.data, SlotData::Vacant(self.next_free));

        // Link this slot into free list
        self.next_free = key.index;
        self.len -= 1;

        match old_data {
            SlotData::Occupied(val) => Some(val),
            SlotData::Vacant(_) => unreachable!(),
        }
    }

    /// Returns a reference to the value associated with the key.
    #[must_use]
    pub fn get(&self, key: Key) -> Option<&T> {
        if key.index >= self.entries.len() {
            return None;
        }

        let slot = &self.entries[key.index];

        if slot.generation != key.generation {
            return None;
        }

        match &slot.data {
            SlotData::Occupied(val) => Some(val),
            SlotData::Vacant(_) => None,
        }
    }

    /// Returns a mutable reference to the value associated with the key.
    pub fn get_mut(&mut self, key: Key) -> Option<&mut T> {
        if key.index >= self.entries.len() {
            return None;
        }

        let slot = &mut self.entries[key.index];

        if slot.generation != key.generation {
            return None;
        }

        match &mut slot.data {
            SlotData::Occupied(val) => Some(val),
            SlotData::Vacant(_) => None,
        }
    }

    /// Returns the number of elements in the slab.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.len
    }

    /// Returns true if the slab is empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `slab`: The standard crate. Does NOT use generational indices by default (returns just `usize`).
//   Users must handle stale index reuse themselves or ensure logic doesn't allow it.
// - `sharded-slab`: Concurrent, sharded version.
// - `slotmap`: Implements generational indices (Versioning) similar to this.
//
// Missing vs. Production:
// - **Iterators**: We haven't implemented `iter()` or `iter_mut()`, which requires skipping Vacant slots.
// - **Shrinking**: No `compact` or `shrink_to_fit` to release memory.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_insert_get_remove() {
        let mut slab = Slab::new();
        let k1 = slab.insert(10);
        let k2 = slab.insert(20);

        assert_eq!(slab.len(), 2);
        assert_eq!(slab.get(k1), Some(&10));
        assert_eq!(slab.get(k2), Some(&20));

        assert_eq!(slab.remove(k1), Some(10));
        assert_eq!(slab.get(k1), None);
        assert_eq!(slab.len(), 1);

        assert_eq!(slab.get(k2), Some(&20));
    }

    #[test]
    fn test_reuse_and_generation() {
        let mut slab = Slab::new();
        let k1 = slab.insert("A"); // Index 0, Gen 0
        slab.remove(k1);

        let k2 = slab.insert("B"); // Should reuse Index 0, Gen 1

        assert_eq!(k1.index, k2.index);
        assert_ne!(k1.generation, k2.generation);

        // Old key should fail
        assert_eq!(slab.get(k1), None);
        // New key should work
        assert_eq!(slab.get(k2), Some(&"B"));
    }

    #[test]
    fn test_stale_key_access() {
        let mut slab = Slab::new();
        let k1 = slab.insert(100);
        slab.remove(k1);

        // k1 is now stale. The slot is Vacant.
        // Accessing via k1 should return None (Vacant check or Gen check if reused).
        assert_eq!(slab.get(k1), None);

        slab.insert(200); // Reused
        // Now slot is Occupied, but generation is higher.
        assert_eq!(slab.get(k1), None);
    }

    #[test]
    fn test_capacity_expansion() {
        let mut slab = Slab::with_capacity(1);
        slab.insert(1);
        slab.insert(2); // Should trigger vec resize
        assert_eq!(slab.len(), 2);
    }
}
