//! # Skip List Implementation
//!
//! Implements a Skip List, a probabilistic data structure that allows O(log n) average time complexity for insertion, deletion, and search.
//! It serves as an alternative to balanced trees (like AVL or Red-Black trees) but is simpler to implement and often faster in practice due to better cache locality and lock-free potential.
//!
//! **Replaces Crates:** `skiplist`, `crossbeam-skiplist` (concurrent)
//!
//! **Real-world Usage:**
//! - Redis (Sorted Sets / ZSET) - uses Skip Lists for ranking and range queries.
//! - `LevelDB` / `RocksDB` (`MemTable`) - uses Skip Lists for in-memory write buffers.
//!
//! **Why build it yourself?**
//! Implementing a Skip List teaches you about probabilistic balancing. Unlike trees which require strict rebalancing (rotations),
//! Skip Lists rely on randomness to maintain balance. You'll also learn about "pointer chasing" and multi-level linked structures.
//! It's a perfect playground for `NonNull` and manual memory management in Rust.

use std::cmp::Ordering;
use std::marker::PhantomData;
use std::ptr::NonNull;

// =========================================================================================
// Architecture
// =========================================================================================
//
// Structure:
//
//      Level 3:  [Head] -------------------------------------> [Node 50] -> NULL
//      Level 2:  [Head] -------------> [Node 20] ------------> [Node 50] -> NULL
//      Level 1:  [Head] -> [Node 10] -> [Node 20] -> [Node 30] -> [Node 50] -> NULL
//
//      Each Node has a value and a variable-height array of forward pointers.
//
// Invariants:
// 1. The list is always sorted by value.
// 2. Level 0 contains all elements.
// 3. If a node exists at level `i`, it must exist at level `i-1`.
// 4. `head` is a sentinel node with max height.
//
// Complexity:
// ┌───────────┬─────────────┬─────────────┐
// │ Operation │ Average     │ Worst Case  │
// ├───────────┼─────────────┼─────────────┤
// │ Search    │ O(log n)    │ O(n)        │
// │ Insert    │ O(log n)    │ O(n)        │
// │ Delete    │ O(log n)    │ O(n)        │
// │ Space     │ O(n)        │ O(n log n)  │
// └───────────┴─────────────┴─────────────┘
//
// Design Decisions:
// - **Max Level**: 16 (supports ~2^16 elements efficiently, though typically 32 is used for large DBs).
// - **Probability P**: 0.5 (coin flip).
// - **Memory**: Uses `Vec` for forward pointers inside nodes. This adds slight overhead vs a fixed array or custom allocator, but simplifies implementation.
//   - *Optimization*: Production implementations (Redis) often use a "flexible array member" approach (allocating Node + array in one contiguous block) to reduce indirection.
// - **Ownership**: `SkipList` owns the nodes via `NonNull`. We manually `drop` them.

const MAX_LEVEL: usize = 16;
// const P: f64 = 0.5; // Probability is implied by XorShift logic (50%)

/// A node in the skip list.
struct Node<T> {
    val: Option<T>, // None for head sentinel
    forward: Vec<Option<NonNull<Self>>>,
}

impl<T> Node<T> {
    fn new(val: Option<T>, level: usize) -> Self {
        Self {
            val,
            forward: vec![None; level + 1],
        }
    }
}

/// A probabilistic Skip List.
pub struct SkipList<T> {
    head: NonNull<Node<T>>,
    level: usize, // Current max level in the list
    length: usize,
    rng: XorShift,
    _marker: PhantomData<Box<Node<T>>>,
}

// UNSAFE JUSTIFICATION:
// We manage memory manually using NonNull.
// T must be Send for SkipList to be Send.
// T must be Sync for SkipList to be Sync (if we were thread-safe, which we aren't, but let's implement Send).
unsafe impl<T: Send> Send for SkipList<T> {}
unsafe impl<T: Sync> Sync for SkipList<T> {}

impl<T: Ord> Default for SkipList<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Ord> SkipList<T> {
    /// Creates a new empty `SkipList`.
    #[must_use]
    pub fn new() -> Self {
        // Create head node with max level
        let head = Box::new(Node::new(None, MAX_LEVEL));
        let head_ptr = unsafe { NonNull::new_unchecked(Box::into_raw(head)) };

        Self {
            head: head_ptr,
            level: 0,
            length: 0,
            rng: XorShift::new(12345), // Fixed seed for reproducibility
            _marker: PhantomData,
        }
    }

    /// Returns the number of elements in the list.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.length
    }

    /// Returns true if the list is empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.length == 0
    }

    /// Inserts a value into the list.
    /// Returns true if the value was inserted, false if it already existed (no duplicates).
    pub fn insert(&mut self, val: T) -> bool {
        let mut update = [None; MAX_LEVEL + 1];
        let mut curr = self.head;

        // 1. Traverse to find position
        unsafe {
            for i in (0..=self.level).rev() {
                while let Some(next_ptr) = curr.as_ref().forward[i] {
                    let next_node = next_ptr.as_ref();
                    if let Some(ref next_val) = next_node.val {
                        if next_val < &val {
                            curr = next_ptr;
                            continue;
                        } else if next_val == &val {
                            return false; // Duplicate
                        }
                    }
                    break;
                }
                update[i] = Some(curr);
            }
        }

        // 2. Generate random level
        let new_level = self.random_level();
        if new_level > self.level {
            update[(self.level + 1)..=new_level].fill(Some(self.head));
            self.level = new_level;
        }

        // 3. Create new node
        let new_node = Box::new(Node::new(Some(val), new_level));
        let new_node_ptr = unsafe { NonNull::new_unchecked(Box::into_raw(new_node)) };

        // 4. Update pointers
        unsafe {
            // Index by level `i`: we splice `new_node` into the parallel
            // forward-pointer arrays of both the predecessor and the new node.
            #[allow(clippy::needless_range_loop)]
            for i in 0..=new_level {
                // `update[i]` is always `Some` here (filled during traversal).
                if let Some(mut prev_ptr) = update[i] {
                    let prev_node = prev_ptr.as_mut();

                    let next_ptr = prev_node.forward[i];
                    // Explicitly dereference and borrow mutably to avoid implicit autoref of raw pointer
                    (&mut (*new_node_ptr.as_ptr()).forward)[i] = next_ptr;
                    prev_node.forward[i] = Some(new_node_ptr);
                }
            }
        }

        self.length += 1;
        true
    }

    /// Returns true if the list contains the value.
    pub fn contains(&self, val: &T) -> bool {
        let mut curr = self.head;

        unsafe {
            for i in (0..=self.level).rev() {
                while let Some(next_ptr) = curr.as_ref().forward[i] {
                    let next_node = next_ptr.as_ref();
                    if let Some(ref next_val) = next_node.val {
                        match next_val.cmp(val) {
                            Ordering::Less => {
                                curr = next_ptr;
                            }
                            Ordering::Equal => return true,
                            Ordering::Greater => break,
                        }
                    }
                }
            }
        }
        false
    }

    /// Removes a value from the list.
    /// Returns true if the value was found and removed.
    pub fn remove(&mut self, val: &T) -> bool {
        let mut update = [None; MAX_LEVEL + 1];
        let mut curr = self.head;

        unsafe {
            // Find predecessor
            for i in (0..=self.level).rev() {
                while let Some(next_ptr) = curr.as_ref().forward[i] {
                    let next_node = next_ptr.as_ref();
                    if let Some(ref next_val) = next_node.val
                        && next_val < val
                    {
                        curr = next_ptr;
                        continue;
                    }
                    break;
                }
                update[i] = Some(curr);
            }

            // Check if found
            // At level 0, curr is predecessor. curr.forward[0] should be target.
            let target_ptr_opt = curr.as_ref().forward[0];
            if let Some(target_ptr) = target_ptr_opt {
                let target_node = target_ptr.as_ref();
                if let Some(ref target_val) = target_node.val
                    && target_val == val
                {
                    // Found. Remove it.
                    // We must unlink at all levels where it exists.
                    // Index by level `i` to unlink from each parallel forward array.
                    #[allow(clippy::needless_range_loop)]
                    for i in 0..=self.level {
                        // `update[i]` is always `Some` here (filled during traversal).
                        let Some(mut prev_ptr) = update[i] else { break };
                        let prev_node = prev_ptr.as_mut();

                        if prev_node.forward[i] != Some(target_ptr) {
                            break; // Target doesn't extend this high
                        }
                        prev_node.forward[i] = target_node.forward[i];
                    }

                    // Drop node
                    let _ = Box::from_raw(target_ptr.as_ptr());
                    self.length -= 1;

                    // Lower level if needed
                    while self.level > 0 && self.head.as_ref().forward[self.level].is_none() {
                        self.level -= 1;
                    }
                    return true;
                }
            }
        }

        false
    }

    // Helper: Generate random level
    const fn random_level(&mut self) -> usize {
        let mut lvl = 0;
        while self.rng.next_bool() && lvl < MAX_LEVEL {
            lvl += 1;
        }
        lvl
    }

    /// Returns an iterator over the values.
    #[must_use]
    pub fn iter(&self) -> Iter<'_, T> {
        unsafe {
            Iter {
                curr: self.head.as_ref().forward[0],
                _marker: PhantomData,
            }
        }
    }
}

impl<'a, T: Ord> IntoIterator for &'a SkipList<T> {
    type Item = &'a T;
    type IntoIter = Iter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<T> Drop for SkipList<T> {
    fn drop(&mut self) {
        unsafe {
            // Reconstruct the Box for the head node to take ownership
            let head_ptr = self.head;
            let head_box = Box::from_raw(head_ptr.as_ptr());

            // Get the first real node
            let mut next_ptr_opt = head_box.forward[0];

            // head_box is dropped here

            while let Some(ptr) = next_ptr_opt {
                // Reconstruct Box for the current node
                let node_box = Box::from_raw(ptr.as_ptr());

                // Save the next pointer before the node is dropped
                next_ptr_opt = node_box.forward[0];

                // node_box is dropped here
            }
        }
    }
}

// =========================================================================================
// Iterator
// =========================================================================================

pub struct Iter<'a, T> {
    curr: Option<NonNull<Node<T>>>,
    _marker: PhantomData<&'a T>,
}

impl<'a, T> Iterator for Iter<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(node_ptr) = self.curr {
            unsafe {
                let node = node_ptr.as_ref();
                self.curr = node.forward[0];
                node.val.as_ref()
            }
        } else {
            None
        }
    }
}

// =========================================================================================
// Helper: PRNG
// =========================================================================================

struct XorShift {
    state: u32,
}

impl XorShift {
    const fn new(seed: u32) -> Self {
        Self {
            state: if seed == 0 { 12345 } else { seed },
        }
    }

    const fn next_u32(&mut self) -> u32 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.state = x;
        x
    }

    const fn next_bool(&mut self) -> bool {
        self.next_u32().is_multiple_of(2)
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `skiplist`: Generic implementation.
// - `crossbeam-skiplist`: Lock-free implementation for concurrent access.
//
// Missing vs. Production:
// - **Concurrency**: This is single-threaded. Redis uses single-threaded access so this is close to Redis logic.
// - **Optimized Memory**: `Vec<Option<NonNull>>` in each node is memory heavy (24 bytes per level per node).
//   Redis uses a "Level" struct array trailing the node allocation.
// - **Backwards Pointers**: Redis ZSET nodes have a backward pointer at level 0 for reverse iteration (`ZREVRANGE`).
//
// Next Steps:
// 1. Implement `DoubleEndedIterator` by adding backward pointers at level 0.
// 2. Add `rank` operations (find index of element) by storing span/width in forward pointers (like Redis).

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_skip_list_basic() {
        let mut list = SkipList::new();
        assert!(list.is_empty());

        assert!(list.insert(10));
        assert!(list.insert(20));
        assert!(list.insert(5));

        assert_eq!(list.len(), 3);

        assert!(list.contains(&10));
        assert!(list.contains(&20));
        assert!(list.contains(&5));
        assert!(!list.contains(&15));

        // Iteration sorted
        let collected: Vec<&i32> = list.iter().collect();
        assert_eq!(collected, vec![&5, &10, &20]);
    }

    #[test]
    fn test_skip_list_duplicates() {
        let mut list = SkipList::new();
        assert!(list.insert(10));
        assert!(!list.insert(10)); // Duplicate
        assert_eq!(list.len(), 1);
    }

    #[test]
    fn test_skip_list_remove() {
        let mut list = SkipList::new();
        list.insert(10);
        list.insert(20);
        list.insert(30);

        assert!(list.remove(&20));
        assert!(!list.contains(&20));
        assert_eq!(list.len(), 2);

        assert!(!list.remove(&99));

        let collected: Vec<&i32> = list.iter().collect();
        assert_eq!(collected, vec![&10, &30]);
    }

    #[test]
    fn test_skip_list_stress() {
        let mut list = SkipList::new();
        for i in 0..100 {
            list.insert(i);
        }

        assert_eq!(list.len(), 100);

        for i in 0..100 {
            assert!(list.contains(&i));
        }

        for i in 0..50 {
            assert!(list.remove(&i));
        }

        assert_eq!(list.len(), 50);
        assert!(!list.contains(&0));
        assert!(list.contains(&50));
    }
}
