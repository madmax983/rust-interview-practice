//! Skip List Implementation
//!
//! # What this implements
//! A probabilistic data structure that provides `O(log n)` search, insertion, and deletion complexity,
//! similar to balanced trees but easier to implement and lock-free friendly.
//!
//! # Replaces
//! `skiplist`, `crossbeam-skiplist` (simplified version)
//!
//! # Real-world usage
//! - **Redis** uses Skip Lists for Sorted Sets (ZSET).
//! - **LevelDB** and **RocksDB** use Skip Lists for their MemTables.
//!
//! # Why build it yourself?
//! To understand how probabilistic balancing works and why it's a compelling alternative to
//! complex rebalancing algorithms in Red-Black or AVL trees, especially in concurrent scenarios.
//! It also demonstrates manual memory management with `NonNull` to avoid the `Option<Box<>>`
//! overhead and borrowing complexity.

use std::cmp::Ordering;
use std::marker::PhantomData;
use std::ptr::NonNull;

/// Maximum height of the skip list.
/// With P = 0.5, 32 levels can hold 2^32 elements.
const MAX_LEVEL: usize = 32;

// RUST INSIGHT:
// We use `NonNull` for pointers to avoid `Option` overhead and to allow
// fine-grained control over aliasing, which is difficult with safe `Box` references
// in a self-referential structure like a Skip List.
struct Node<T> {
    value: T,
    // Vector of forward pointers. forward[i] is the next node at level i.
    forward: Vec<Option<NonNull<Node<T>>>>,
}

impl<T> Node<T> {
    fn new(value: T, level: usize) -> Self {
        Self {
            value,
            forward: vec![None; level + 1],
        }
    }
}

/// A probabilistic Skip List.
///
/// # Architecture
///
/// ```text
/// Level 3: [Head] -------------------------------------> [9] -> NULL
/// Level 2: [Head] -------------> [5] ------------------> [9] -> NULL
/// Level 1: [Head] -> [2] ------> [5] -> [7] -----------> [9] -> NULL
/// Level 0: [Head] -> [2] -> [3] -> [5] -> [7] -> [8] -> [9] -> NULL
/// ```
///
/// - **Invariants**:
///   - Level 0 contains all elements.
///   - If a node exists at level `i`, it must exist at level `i-1`.
///   - The list is always sorted.
///
/// # Complexity
///
/// | Operation | Average | Worst Case |
/// |-----------|---------|------------|
/// | Search    | O(log n)| O(n)       |
/// | Insert    | O(log n)| O(n)       |
/// | Delete    | O(log n)| O(n)       |
/// | Space     | O(n)    | O(n log n) |
pub struct SkipList<T> {
    // The head is a dummy node or just an array of pointers.
    // We use a dummy node logic here: `head` vector stores pointers to the first real node.
    // Effectively, `head[i]` is the start of level `i`.
    head: Vec<Option<NonNull<Node<T>>>>,
    length: usize,
    // Helper for random number generation
    rng: XorShift,
    _marker: PhantomData<T>,
}

impl<T: Ord> SkipList<T> {
    /// Creates a new empty Skip List.
    #[must_use]
    pub fn new() -> Self {
        Self {
            head: vec![None; MAX_LEVEL],
            length: 0,
            rng: XorShift::new(),
            _marker: PhantomData,
        }
    }

    /// Returns the number of elements in the skip list.
    #[must_use]
    pub fn len(&self) -> usize {
        self.length
    }

    /// Returns `true` if the skip list is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.length == 0
    }

    /// Generates a random level for a new node.
    /// Returns a value between 0 and MAX_LEVEL - 1.
    fn random_level(&mut self) -> usize {
        let mut level = 0;
        // P = 0.5: 50% chance to increase level
        while level < MAX_LEVEL - 1 && self.rng.next() % 2 == 0 {
            level += 1;
        }
        level
    }

    /// Inserts a value into the skip list.
    /// Returns `true` if the value was inserted, `false` if it already existed.
    pub fn insert(&mut self, value: T) -> bool {
        let mut update: Vec<Option<NonNull<Node<T>>>> = vec![None; MAX_LEVEL];
        let mut cursor: Option<NonNull<Node<T>>> = None;

        for i in (0..MAX_LEVEL).rev() {
            loop {
                let next_ptr = if let Some(c) = cursor {
                    unsafe { c.as_ref().forward[i] }
                } else {
                    self.head[i]
                };

                match next_ptr {
                    Some(next_node) => {
                        let next_ref = unsafe { next_node.as_ref() };
                        if next_ref.value < value {
                            cursor = Some(next_node);
                        } else {
                            break;
                        }
                    }
                    None => break,
                }
            }
            update[i] = cursor;
        }

        // Check if element already exists
        let candidate = if let Some(c) = cursor {
            unsafe { c.as_ref().forward[0] }
        } else {
            self.head[0]
        };

        if let Some(node) = candidate {
            let node_ref = unsafe { node.as_ref() };
            if node_ref.value == value {
                return false; // Already exists
            }
        }

        // Insert new node
        let new_level = self.random_level();
        let mut new_node = Box::new(Node::new(value, new_level));
        let mut new_node_ptr = unsafe { NonNull::new_unchecked(Box::into_raw(new_node)) };

        for i in 0..=new_level {
            let prev_ptr = update[i];

            if let Some(mut prev) = prev_ptr {
                unsafe {
                    let prev_ref = prev.as_mut();
                    new_node_ptr.as_mut().forward[i] = prev_ref.forward[i];
                    prev_ref.forward[i] = Some(new_node_ptr);
                }
            } else {
                unsafe {
                    new_node_ptr.as_mut().forward[i] = self.head[i];
                }
                self.head[i] = Some(new_node_ptr);
            }
        }

        self.length += 1;
        true
    }

    /// Returns `true` if the value is found in the skip list.
    pub fn contains(&self, value: &T) -> bool {
        let mut cursor: Option<NonNull<Node<T>>> = None;

        for i in (0..MAX_LEVEL).rev() {
            loop {
                let next_ptr = if let Some(c) = cursor {
                    unsafe { c.as_ref().forward[i] }
                } else {
                    self.head[i]
                };

                match next_ptr {
                    Some(next_node) => {
                        let next_ref = unsafe { next_node.as_ref() };
                        match next_ref.value.cmp(value) {
                            Ordering::Less => cursor = Some(next_node),
                            Ordering::Equal => return true,
                            Ordering::Greater => break,
                        }
                    }
                    None => break,
                }
            }
        }

        false
    }

    /// Removes a value from the skip list.
    /// Returns `true` if the value was removed.
    pub fn remove(&mut self, value: &T) -> bool {
        let mut update: Vec<Option<NonNull<Node<T>>>> = vec![None; MAX_LEVEL];
        let mut cursor: Option<NonNull<Node<T>>> = None;

        for i in (0..MAX_LEVEL).rev() {
            loop {
                let next_ptr = if let Some(c) = cursor {
                    unsafe { c.as_ref().forward[i] }
                } else {
                    self.head[i]
                };

                match next_ptr {
                    Some(next_node) => {
                        let next_ref = unsafe { next_node.as_ref() };
                        if next_ref.value < *value {
                            cursor = Some(next_node);
                        } else {
                            break;
                        }
                    }
                    None => break,
                }
            }
            update[i] = cursor;
        }

        let candidate = if let Some(c) = cursor {
            unsafe { c.as_ref().forward[0] }
        } else {
            self.head[0]
        };

        if let Some(target_node) = candidate {
            let target_ref = unsafe { target_node.as_ref() };
            if target_ref.value != *value {
                return false;
            }

            // Found the node to remove.
            let levels = target_ref.forward.len();

            for i in 0..levels {
                let prev_ptr = update[i];
                if let Some(mut prev) = prev_ptr {
                    unsafe {
                        prev.as_mut().forward[i] = target_ref.forward[i];
                    }
                } else {
                    // Removing from head
                    self.head[i] = target_ref.forward[i];
                }
            }

            // Drop the node
            unsafe {
                let _ = Box::from_raw(target_node.as_ptr());
            }
            self.length -= 1;
            true
        } else {
            false
        }
    }
}

impl<T: Ord> Default for SkipList<T> {
    fn default() -> Self {
        Self::new()
    }
}

// UNSAFE JUSTIFICATION:
// We manually implement `Drop` to iterate through the bottom level (level 0)
// and deallocate all nodes using `Box::from_raw`.
// This prevents memory leaks.
impl<T> Drop for SkipList<T> {
    fn drop(&mut self) {
        let mut cursor = self.head[0];
        while let Some(node_ptr) = cursor {
            unsafe {
                let node = Box::from_raw(node_ptr.as_ptr());
                cursor = node.forward[0];
                // node is dropped here
            }
        }
    }
}

/// Simple Xorshift RNG to avoid external dependencies.
struct XorShift {
    state: u64,
}

impl XorShift {
    fn new() -> Self {
        Self {
            state: 0x1234_5678_9ABC_DEF0,
        }
    }

    fn next(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_operations() {
        let mut list = SkipList::new();
        assert!(list.is_empty());

        assert!(list.insert(10));
        assert!(list.insert(20));
        assert!(list.insert(30));
        assert_eq!(list.len(), 3);

        assert!(list.contains(&10));
        assert!(list.contains(&20));
        assert!(list.contains(&30));
        assert!(!list.contains(&40));

        assert!(!list.insert(20)); // Duplicate
        assert_eq!(list.len(), 3);

        assert!(list.remove(&20));
        assert!(!list.contains(&20));
        assert_eq!(list.len(), 2);
    }

    #[test]
    fn test_large_volume() {
        let mut list = SkipList::new();
        let n = 1000;
        for i in 0..n {
            list.insert(i);
        }
        assert_eq!(list.len(), n);

        for i in 0..n {
            assert!(list.contains(&i));
        }

        for i in 0..n {
            assert!(list.remove(&i));
        }
        assert!(list.is_empty());
    }

    #[test]
    fn test_strings() {
        let mut list = SkipList::new();
        list.insert("apple".to_string());
        list.insert("banana".to_string());
        list.insert("cherry".to_string());

        assert!(list.contains(&"apple".to_string()));
        assert!(!list.contains(&"date".to_string()));
    }
}
