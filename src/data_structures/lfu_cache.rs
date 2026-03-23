//! # LFU (Least Frequently Used) Cache Implementation
//!
//! An efficient `O(1)` LFU cache implementation.
//!
//! **Replaces Crates:** `lfu`, parts of `cached` and `moka`
//!
//! **Real-world Usage:**
//! - Intel's CPU cache policies to retain frequently accessed data over new sparse data.
//! - Caffeine JVM cache for high-performance memory caching.
//! - CDNs (Akamai, Cloudflare) retaining hot assets while evicting the long tail.
//!
//! **Why build it yourself?**
//! LFU is notoriously difficult to implement in true `O(1)` time complexity. While LRU just needs
//! a single queue, LFU requires managing multiple frequency buckets while keeping them ordered.
//! Implementing this teaches you how to compose multiple HashMaps and doubly-linked lists.

// =========================================================================================
// Architecture
// =========================================================================================
//
// We implement the classic O(1) algorithm by Ketan Shah et al.
// Data Structure:
// 1. `key_map`: HashMap<Key, NodeIndex> -> Locates the node in O(1).
// 2. `freq_map`: HashMap<Freq, DoublyLinkedList> -> Groups nodes by their access frequency.
// 3. `min_freq`: integer -> Tracks the lowest frequency to know which list to evict from in O(1).
// 4. Arena Array (`Vec<Node>`): Stores the actual nodes. We use indices instead of pointers
//    to satisfy Rust's borrow checker safely without `Rc<RefCell<>>`.
//
// Flow:
//   [Key] ──► `key_map` ──► Node { freq: 2, ... }
//                             │
//   [Freq: 1] ──► List(NodeA, NodeB)
//   [Freq: 2] ──► List(NodeC, Node) ◄── Node moved here on access
//
// Invariants:
// 1. `min_freq` always points to a valid list in `freq_map` (unless empty).
// 2. Every node in `key_map` is in exactly one list in `freq_map`.
// 3. The tail of a list in `freq_map` is the Least Recently Used element of that frequency.
//
// Complexity:
// ┌───────────────┬─────────────┬─────────────┐
// │ Operation     │ Time        │ Space       │
// ├───────────────┼─────────────┼─────────────┤
// │ get           │ O(1)        │ O(N)        │
// │ put           │ O(1)        │ O(N)        │
// └───────────────┴─────────────┴─────────────┘
//
// Design Decisions:
// - **Arena Allocation**: Using a `Vec<Node>` to back the doubly-linked lists.
//   - *Tradeoff*: Safe, highly performant `O(1)` indexing, zero pointer overhead.
//   - *Drawback*: Requires managing "null" indices and list heads/tails manually.

use std::collections::HashMap;

/// A node stored in our arena. Represents an entry in the LFU Cache.
#[derive(Clone, Copy, Debug)]
struct Node<K, V> {
    key: K,
    val: V,
    freq: usize,
    prev: Option<usize>,
    next: Option<usize>,
}

/// An intrusive doubly-linked list managing nodes of the same frequency.
/// We store the head and tail indices into the arena.
#[derive(Clone, Copy, Debug)]
struct List {
    head: Option<usize>,
    tail: Option<usize>,
    len: usize,
}

impl List {
    #[must_use]
    fn new() -> Self {
        Self {
            head: None,
            tail: None,
            len: 0,
        }
    }
}

/// Swappable strategy interface for cache eviction logic if we wanted to abstract it.
pub trait Cache<K, V> {
    fn get(&mut self, key: &K) -> Option<&V>;
    fn put(&mut self, key: K, value: V);
}

/// Least Frequently Used (LFU) Cache implementation using an Arena allocator.
pub struct LFUCache<K, V> {
    capacity: usize,
    min_freq: usize,
    key_map: HashMap<K, usize>,
    freq_map: HashMap<usize, List>,
    // The arena where all nodes live.
    nodes: Vec<Option<Node<K, V>>>,
    // Free list of indices in the arena to reuse memory.
    free_list: Vec<usize>,
}

impl<K: std::hash::Hash + Eq + Clone, V> LFUCache<K, V> {
    /// Creates a new `LFUCache` with the specified capacity.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            min_freq: 0,
            key_map: HashMap::with_capacity(capacity),
            freq_map: HashMap::new(),
            nodes: Vec::with_capacity(capacity),
            free_list: Vec::with_capacity(capacity),
        }
    }

    // =========================================================================
    // List Operations (Internal)
    // =========================================================================

    /// Adds a node to the head of the doubly-linked list.
    fn push_head(&mut self, list_freq: usize, node_idx: usize) {
        let list = self.freq_map.entry(list_freq).or_insert_with(List::new);

        // Set the new node's pointers
        if let Some(node) = &mut self.nodes[node_idx] {
            node.next = list.head;
            node.prev = None;
        }

        // Update previous head's prev pointer
        if let Some(head_idx) = list.head
            && let Some(head_node) = &mut self.nodes[head_idx] {
                head_node.prev = Some(node_idx);
            }

        list.head = Some(node_idx);

        // If list was empty, this is also the tail
        if list.tail.is_none() {
            list.tail = Some(node_idx);
        }
        list.len += 1;
    }

    /// Removes a node from its current doubly-linked list.
    fn remove_node(&mut self, list_freq: usize, node_idx: usize) {
        let (prev_idx, next_idx) = if let Some(Some(node)) = self.nodes.get(node_idx) {
            (node.prev, node.next)
        } else {
            return;
        };

        let list = self.freq_map.get_mut(&list_freq).unwrap();

        if let Some(prev) = prev_idx {
            if let Some(prev_node) = &mut self.nodes[prev] {
                prev_node.next = next_idx;
            }
        } else {
            list.head = next_idx;
        }

        if let Some(next) = next_idx {
            if let Some(next_node) = &mut self.nodes[next] {
                next_node.prev = prev_idx;
            }
        } else {
            list.tail = prev_idx;
        }

        list.len -= 1;

        // PRODUCTION NOTE: In a long-running cache, if we don't remove empty lists from `freq_map`,
        // it will grow unbounded over time as frequencies increase, causing a slow memory leak.
        // We must remove the list if it's empty.
        if list.len == 0 {
            self.freq_map.remove(&list_freq);
        }

        // Clear the node's pointers to avoid dangling indices
        if let Some(node) = &mut self.nodes[node_idx] {
            node.prev = None;
            node.next = None;
        }
    }

    /// Pops the tail (LRU) node from the given frequency list.
    fn pop_tail(&mut self, list_freq: usize) -> Option<usize> {
        let list = self.freq_map.get_mut(&list_freq)?;
        let tail_idx = list.tail?;
        self.remove_node(list_freq, tail_idx);
        Some(tail_idx)
    }

    // =========================================================================
    // Core Logic
    // =========================================================================

    /// Updates a node's frequency and moves it to the appropriate list.
    fn update_frequency(&mut self, node_idx: usize) {
        let freq = if let Some(node) = &self.nodes[node_idx] {
            node.freq
        } else {
            return;
        };

        // Remove from current list
        self.remove_node(freq, node_idx);

        // Update min_freq if the current min_freq list is now empty
        // GOTCHA: We must check if the list was entirely removed from `freq_map` above.
        if self.min_freq == freq && !self.freq_map.contains_key(&freq) {
            self.min_freq += 1;
        }

        // Increment node's frequency
        let new_freq = freq + 1;
        if let Some(node) = &mut self.nodes[node_idx] {
            node.freq = new_freq;
        }

        // Add to the new list
        self.push_head(new_freq, node_idx);
    }
}

impl<K: std::hash::Hash + Eq + Clone, V> Cache<K, V> for LFUCache<K, V> {
    /// Returns a reference to the value corresponding to the key.
    fn get(&mut self, key: &K) -> Option<&V> {
        if self.capacity == 0 {
            return None;
        }

        if let Some(&node_idx) = self.key_map.get(key) {
            self.update_frequency(node_idx);
            // RUST INSIGHT: We must access `self.nodes` after `update_frequency` to satisfy
            // the borrow checker. By returning the reference here, the lifetime is tied to `&mut self`.
            if let Some(node) = &self.nodes[node_idx] {
                return Some(&node.val);
            }
        }
        None
    }

    /// Inserts a key-value pair into the cache.
    fn put(&mut self, key: K, value: V) {
        if self.capacity == 0 {
            return;
        }

        // If key exists, update value and frequency
        if let Some(&node_idx) = self.key_map.get(&key) {
            if let Some(node) = &mut self.nodes[node_idx] {
                node.val = value;
            }
            self.update_frequency(node_idx);
            return;
        }

        // Evict if at capacity
        if self.key_map.len() == self.capacity
            && let Some(lru_idx) = self.pop_tail(self.min_freq) {
                if let Some(node) = self.nodes[lru_idx].take() {
                    self.key_map.remove(&node.key);
                }
                // Add the index to the free list to be reused
                self.free_list.push(lru_idx);
            }

        // Create new node
        let new_node = Node {
            key: key.clone(),
            val: value,
            freq: 1,
            prev: None,
            next: None,
        };

        // Allocate index from free list or push to nodes
        let node_idx = if let Some(free_idx) = self.free_list.pop() {
            self.nodes[free_idx] = Some(new_node);
            free_idx
        } else {
            let idx = self.nodes.len();
            self.nodes.push(Some(new_node));
            idx
        };

        self.key_map.insert(key, node_idx);
        self.min_freq = 1;
        self.push_head(1, node_idx);
    }
}

// =========================================================================
// Footer
// =========================================================================
//
// Comparison to Canonical Crates:
// - `moka`: High-performance concurrent caching library using a combination of LRU and TinyLFU (W-TinyLFU).
//   It features probabilistic admission control to prevent scan-pollution and handles expiry.
// - `cached`: More of a macro-based caching wrapper around simple HashMaps or LRU caches.
//
// What's missing vs. production:
// - **Concurrency**: This implementation is single-threaded. True concurrent LFU uses sharding or
//   message-passing queues to update frequencies asynchronously.
// - **W-TinyLFU / Count-Min Sketch**: Standard LFU suffers from "cache pollution" (old, highly frequent
//   items stick around forever even if they are never accessed again). Production crates use probabilistic
//   structures like Count-Min Sketch to track frequencies with aging.
//
// Next steps:
// 1. Implement W-TinyLFU using a Count-Min Sketch.
// 2. Add expiration (TTL).
//
// Benchmarking Note:
// Use `criterion` to benchmark this against `lru_cache.rs`.
// 1. Test zipfian distributions: LFU should drastically outperform LRU hit rates.
// 2. Test scan resistance: A large scan of unique items will flush the LRU cache entirely,
//    while LFU might retain highly frequent items (though basic LFU struggles with stale hot items).
// Use `std::hint::black_box` around `cache.get()` in benchmarks to avoid optimizer elision.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lfu_basic() {
        let mut cache = LFUCache::new(2);

        cache.put(1, 1);
        cache.put(2, 2);
        assert_eq!(cache.get(&1), Some(&1)); // freq(1)=2, freq(2)=1

        cache.put(3, 3); // evicts 2
        assert_eq!(cache.get(&2), None);
        assert_eq!(cache.get(&3), Some(&3)); // freq(3)=2

        cache.put(4, 4); // evicts 1
        assert_eq!(cache.get(&1), None);
        assert_eq!(cache.get(&3), Some(&3)); // freq(3)=3
        assert_eq!(cache.get(&4), Some(&4)); // freq(4)=2
    }

    #[test]
    fn test_capacity_zero() {
        let mut cache = LFUCache::new(0);
        cache.put(1, 1);
        assert_eq!(cache.get(&1), None);
    }

    #[test]
    fn test_update_existing_value() {
        let mut cache = LFUCache::new(2);
        cache.put(1, 10);
        cache.put(1, 20); // Should update value to 20 and freq to 2

        assert_eq!(cache.get(&1), Some(&20));
    }

    #[test]
    fn test_eviction_lru_tiebreaker() {
        let mut cache = LFUCache::new(3);
        cache.put(1, 1);
        cache.put(2, 2);
        cache.put(3, 3);

        // All frequencies are 1. The oldest is 1.
        cache.put(4, 4); // Evicts 1

        assert_eq!(cache.get(&1), None);
        assert_eq!(cache.get(&2), Some(&2));
        assert_eq!(cache.get(&3), Some(&3));
        assert_eq!(cache.get(&4), Some(&4));
    }
}
