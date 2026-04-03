//! # 146. LRU Cache
//!
//! Difficulty: Medium
//! Link: https://leetcode.com/problems/lru-cache/
//!
//! Design a data structure that follows the constraints of a Least Recently Used (LRU) cache.
//!
//! Implement the `LRUCache` class:
//! - `LRUCache(int capacity)` Initialize the LRU cache with positive size `capacity`.
//! - `int get(int key)` Return the value of the `key` if the key exists, otherwise return `-1`.
//! - `void put(int key, int value)` Update the value of the `key` if the `key` exists. Otherwise, add the `key-value` pair to the cache. If the number of keys exceeds the `capacity` from this operation, evict the least recently used key.
//!
//! The functions `get` and `put` must each run in `O(1)` average time complexity.
//!
//! This problem is a natural fit for demonstrating how to build a doubly-linked list without fighting
//! the borrow checker, avoiding `Rc<RefCell<T>>` or `unsafe` entirely by using an array-based approach.
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::data_structures::lru_cache::LRUCache;
//!
//! let mut lru = LRUCache::new(2);
//! lru.put(1, 1); // cache is {1=1}
//! lru.put(2, 2); // cache is {1=1, 2=2}
//! assert_eq!(lru.get(1), 1);    // return 1
//! lru.put(3, 3); // evicts key 2, cache is {1=1, 3=3}
//! assert_eq!(lru.get(2), -1);   // returns -1 (not found)
//! lru.put(4, 4); // evicts key 1, cache is {4=4, 3=3}
//! assert_eq!(lru.get(1), -1);   // return -1 (not found)
//! assert_eq!(lru.get(3), 3);    // return 3
//! assert_eq!(lru.get(4), 4);    // return 4
//! ```
//!
//! ## Constraints
//!
//! - `1 <= capacity <= 3000`
//! - `0 <= key <= 10^4`
//! - `0 <= value <= 10^5`
//! - At most `2 * 10^5` calls will be made to `get` and `put`.

use std::collections::HashMap;

// ============================================================================
// Brute Force / Naive Approach
// ============================================================================

/// Brute force approach: HashMap + Vec
///
/// Uses a `HashMap` for `O(1)` lookups and a `Vec` to track the order of recently used keys.
/// Updating the LRU order requires finding the key in the `Vec` and moving it to the back,
/// which takes `O(N)` time.
///
/// Time: `get` `O(N)`, `put` `O(N)` due to searching/removing from the `Vec`.
/// Space: `O(capacity)`
pub struct LRUCacheNaive {
    capacity: usize,
    map: HashMap<i32, i32>,
    order: Vec<i32>, // Back is most recent, front is least recent
}

impl LRUCacheNaive {
    #[must_use]
    pub fn new(capacity: i32) -> Self {
        let cap = capacity as usize;
        Self {
            capacity: cap,
            // BOLT OPTIMIZATION: Pre-allocate capacity to eliminate dynamic heap reallocations.
            // Since the cache has a fixed maximum size, pre-allocating the underlying HashMap
            // and Vec exactly to this capacity prevents reallocations during inserts.
            map: HashMap::with_capacity(cap),
            order: Vec::with_capacity(cap),
        }
    }

    #[must_use]
    pub fn get(&mut self, key: i32) -> i32 {
        if let Some(&val) = self.map.get(&key) {
            // RUST INSIGHT: `position` takes a closure to find the index. We must borrow `key`.
            // After finding the index, we remove it and push it to the back to mark as most recent.
            let idx = self.order.iter().position(|&k| k == key).unwrap();
            self.order.remove(idx);
            self.order.push(key);
            val
        } else {
            -1
        }
    }

    pub fn put(&mut self, key: i32, value: i32) {
        if self.map.contains_key(&key) {
            // Update existing value and mark as most recent
            self.map.insert(key, value);
            let idx = self.order.iter().position(|&k| k == key).unwrap();
            self.order.remove(idx);
            self.order.push(key);
        } else {
            // If at capacity, evict the least recently used (front of Vec)
            if self.map.len() == self.capacity {
                let lru_key = self.order.remove(0);
                self.map.remove(&lru_key);
            }
            // Insert new key-value pair
            self.map.insert(key, value);
            self.order.push(key);
        }
    }
}

// ============================================================================
// Optimal Approach
// ============================================================================

/// Represents a node in the doubly-linked list.
#[derive(Clone, Copy)]
struct Node {
    key: i32,
    val: i32,
    prev: usize,
    next: usize,
}

/// Optimal approach: HashMap + Array-based Doubly-Linked List
///
/// Instead of fighting the borrow checker with `Rc<RefCell<Node>>` or risking
/// UB with `unsafe` pointers, we use a `Vec<Node>` to act as an arena allocator.
/// "Pointers" are simply `usize` indices into this `Vec`.
///
/// Time: `get` `O(1)`, `put` `O(1)` average case.
/// Space: `O(capacity)`
///
/// # RUST INSIGHT: The Arena Pattern
/// Creating cyclic data structures (like a doubly-linked list) in Rust using references
/// is notoriously difficult due to ownership rules. The "Arena Pattern" uses indices
/// into a `Vec` to represent links. This sidesteps lifetime and borrowing issues
/// entirely, offering a safe, highly performant `O(1)` implementation.
pub struct LRUCacheOptimal {
    capacity: usize,
    map: HashMap<i32, usize>, // Maps Key -> Index in the nodes array
    nodes: Vec<Node>,
    head: usize,
    tail: usize,
}

impl LRUCacheOptimal {
    /// Initializes the LRU cache with the given capacity.
    #[must_use]
    pub fn new(capacity: i32) -> Self {
        let capacity = capacity as usize;
        let mut nodes = Vec::with_capacity(capacity + 2);

        // Push dummy head (index 0) and dummy tail (index 1)
        nodes.push(Node {
            key: 0,
            val: 0,
            prev: 0,
            next: 1,
        });
        nodes.push(Node {
            key: 0,
            val: 0,
            prev: 0,
            next: 1,
        });

        Self {
            capacity,
            map: HashMap::with_capacity(capacity),
            nodes,
            head: 0,
            tail: 1,
        }
    }

    /// Removes a node from its current position in the linked list.
    fn remove_node(&mut self, idx: usize) {
        let prev = self.nodes[idx].prev;
        let next = self.nodes[idx].next;

        self.nodes[prev].next = next;
        self.nodes[next].prev = prev;
    }

    /// Adds a node to the front of the list (right after the dummy head).
    fn add_node(&mut self, idx: usize) {
        let next = self.nodes[self.head].next;

        self.nodes[idx].prev = self.head;
        self.nodes[idx].next = next;

        self.nodes[self.head].next = idx;
        self.nodes[next].prev = idx;
    }

    /// Moves an existing node to the front of the list.
    fn move_to_front(&mut self, idx: usize) {
        self.remove_node(idx);
        self.add_node(idx);
    }

    /// Gets the value for the given key, returning -1 if not found.
    #[must_use]
    pub fn get(&mut self, key: i32) -> i32 {
        if let Some(&idx) = self.map.get(&key) {
            self.move_to_front(idx);
            self.nodes[idx].val
        } else {
            -1
        }
    }

    /// Puts a key-value pair into the cache.
    pub fn put(&mut self, key: i32, value: i32) {
        if let Some(&idx) = self.map.get(&key) {
            // Update existing node
            self.nodes[idx].val = value;
            self.move_to_front(idx);
        } else if self.map.len() == self.capacity {
            // Evict LRU node (node just before dummy tail)
            let lru_idx = self.nodes[self.tail].prev;
            self.remove_node(lru_idx);
            self.map.remove(&self.nodes[lru_idx].key);

            // GOTCHA: Instead of creating a new node, we reuse the evicted node's slot!
            // This prevents the `Vec` from growing indefinitely.
            self.nodes[lru_idx].key = key;
            self.nodes[lru_idx].val = value;
            self.add_node(lru_idx);
            self.map.insert(key, lru_idx);
        } else {
            // Create new node
            let idx = self.nodes.len();
            self.nodes.push(Node {
                key,
                val: value,
                prev: 0,
                next: 0,
            });
            self.add_node(idx);
            self.map.insert(key, idx);
        }
    }
}

// ============================================================================
// Main Entry Point
// ============================================================================

/// We export the optimal implementation as `LRUCache` to match the problem description.
pub type LRUCache = LRUCacheOptimal;

// ============================================================================
// Alternative Approaches
// ============================================================================
// 1. `std::collections::LinkedHashMap` - In production, you'd just use the `linked-hash-map` crate
//    or `indexmap`, which internally implement very similar structures.
// 2. Unsafe pointers (`*mut Node`) - The standard library's `LinkedList` uses unsafe raw pointers.
//    This allows slightly faster `O(1)` node allocation/deallocation without maintaining a `Vec`,
//    but is complex and prone to subtle UB if not implemented perfectly. The arena pattern is heavily preferred in safe Rust.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_naive_lru_cache() {
        let mut lru = LRUCacheNaive::new(2);
        lru.put(1, 1);
        lru.put(2, 2);
        assert_eq!(lru.get(1), 1);
        lru.put(3, 3); // evicts 2
        assert_eq!(lru.get(2), -1);
        lru.put(4, 4); // evicts 1
        assert_eq!(lru.get(1), -1);
        assert_eq!(lru.get(3), 3);
        assert_eq!(lru.get(4), 4);
    }

    #[test]
    fn test_optimal_lru_cache() {
        let mut lru = LRUCacheOptimal::new(2);
        lru.put(1, 1);
        lru.put(2, 2);
        assert_eq!(lru.get(1), 1);
        lru.put(3, 3); // evicts 2
        assert_eq!(lru.get(2), -1);
        lru.put(4, 4); // evicts 1
        assert_eq!(lru.get(1), -1);
        assert_eq!(lru.get(3), 3);
        assert_eq!(lru.get(4), 4);
    }

    #[test]
    fn test_capacity_one() {
        let mut lru = LRUCache::new(1);
        lru.put(2, 1);
        assert_eq!(lru.get(2), 1);
        lru.put(3, 2);
        assert_eq!(lru.get(2), -1);
        assert_eq!(lru.get(3), 2);
    }

    #[test]
    fn test_update_existing_key() {
        let mut lru = LRUCache::new(2);
        lru.put(2, 1);
        lru.put(2, 2);
        assert_eq!(lru.get(2), 2);
        lru.put(1, 1);
        lru.put(4, 1);
        assert_eq!(lru.get(2), -1); // 2 should have been evicted, 1 was updated most recently
    }
}
