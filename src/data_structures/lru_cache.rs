//! # LRU Cache Implementation
//!
//! Implements a Least Recently Used (LRU) Cache, solving LeetCode #146.
//!
//! **Replaces Crates:** `lru`, `cached`
//!
//! **Real-world Usage:**
//! - CPU Cache Replacement Policies (e.g., Intel's L1/L2 caches often use variations of LRU/pseudo-LRU).
//! - Database page caches (e.g., PostgreSQL shared buffers).
//! - Web caching (e.g., Redis, Memcached, CDN edge nodes).
//!
//! **Why build it yourself?**
//! Building an LRU cache in Rust forces you to confront the Borrow Checker. A classic LRU requires
//! a Doubly-Linked List and a Hash Map pointing to the nodes. In C++ or Java, you'd use raw pointers
//! or object references. In Rust, you can't easily have a `HashMap` and a `LinkedList` both mutably
//! referencing the same nodes without using `Rc<RefCell<T>>` or `unsafe`.
//! Building an arena-backed linked list using array indices teaches a foundational pattern for
//! building graph-like structures in safe Rust.

use std::collections::HashMap;
use std::hash::Hash;

// =========================================================================================
// Architecture
// =========================================================================================
//
// Data Structure (ArenaLruCache):
//
//      HashMap<K, Index>
//           │
//           ▼
//      ┌─────────┐    next    ┌─────────┐
//      │ Node 0  │ ─────────► │ Node 1  │
//      │(Head)   │ ◄───────── │(Tail)   │
//      └─────────┘    prev    └─────────┘
//
// Nodes are stored in a flat `Vec<Node>`. "Pointers" are just `usize` indices into this `Vec`.
//
// Invariants:
// 1. The capacity is fixed; when full, the least recently used item is evicted.
// 2. Head of the list represents the Most Recently Used (MRU).
// 3. Tail of the list represents the Least Recently Used (LRU).
// 4. `HashMap` always accurately reflects the keys currently in the `Vec`.
//
// Complexity:
// ┌─────────────┬─────────────┬─────────────┐
// │ Operation   │ Time (Naive)│ Time (Arena)│
// ├─────────────┼─────────────┼─────────────┤
// │ get         │ O(N)        │ O(1)        │
// │ put         │ O(N)        │ O(1)        │
// └─────────────┴─────────────┴─────────────┘
// Space Complexity: O(N) for both, where N is capacity.
//
// Design Decisions:
// - **Arena vs Pointers**: We use an arena (`Vec<Node>`) and indices.
//   - *Alternative*: `std::collections::LinkedList` + `HashMap`. Doesn't work because `LinkedList` doesn't expose iterators that allow removing specific nodes in O(1).
//   - *Alternative*: `unsafe` raw pointers. This is what the `lru` crate does for max performance, but we avoid it to demonstrate safe Rust patterns.

/// A generic cache trait allowing for swappable implementations.
pub trait Cache<K, V> {
    /// Gets a value from the cache, updating its MRU status.
    fn get(&mut self, key: &K) -> Option<&V>;

    /// Inserts a key-value pair into the cache. If the cache is full,
    /// the least recently used item is evicted.
    fn put(&mut self, key: K, value: V);
}

// =========================================================================================
// Naive Approach (O(N))
// =========================================================================================

/// A naive LRU cache using a single `Vec`.
///
/// Time: O(N) for `get` and `put` because we must linearly search the vector.
/// Space: O(N) where N is the capacity.
pub struct NaiveLruCache<K, V> {
    capacity: usize,
    entries: Vec<(K, V)>,
}

impl<K: Eq + Clone, V> NaiveLruCache<K, V> {
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "Capacity must be greater than 0");
        Self {
            capacity,
            entries: Vec::with_capacity(capacity),
        }
    }
}

impl<K: Eq + Clone, V> Cache<K, V> for NaiveLruCache<K, V> {
    fn get(&mut self, key: &K) -> Option<&V> {
        let idx = self.entries.iter().position(|(k, _)| k == key)?;
        // RUST INSIGHT: We remove the element and push it to the end (MRU position).
        // This is O(N) due to shifting elements.
        let entry = self.entries.remove(idx);
        self.entries.push(entry);
        self.entries.last().map(|(_, v)| v)
    }

    fn put(&mut self, key: K, value: V) {
        if let Some(idx) = self.entries.iter().position(|(k, _)| k == &key) {
            // Update existing key and move to MRU
            self.entries.remove(idx);
            self.entries.push((key, value));
            return;
        }

        if self.entries.len() == self.capacity {
            // Evict LRU (index 0)
            self.entries.remove(0);
        }
        self.entries.push((key, value));
    }
}

// =========================================================================================
// Optimal Approach (O(1)) - Arena Based
// =========================================================================================

/// Represents a pointer to a node, which is just an index in the `Vec`.
type Link = Option<usize>;

struct Node<K, V> {
    key: K,
    value: V,
    prev: Link,
    next: Link,
}

/// An optimal LRU cache using an Arena-backed Doubly-Linked List and a HashMap.
///
/// Time: O(1) for `get` and `put`.
/// Space: O(N) where N is the capacity.
pub struct ArenaLruCache<K, V> {
    capacity: usize,
    map: HashMap<K, usize>,
    nodes: Vec<Node<K, V>>,
    head: Link,
    tail: Link,
}

impl<K: Hash + Eq + Clone, V> ArenaLruCache<K, V> {
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "Capacity must be greater than 0");
        Self {
            capacity,
            map: HashMap::with_capacity(capacity),
            nodes: Vec::with_capacity(capacity),
            head: None,
            tail: None,
        }
    }

    /// Removes a node from its current position in the linked list.
    fn detach(&mut self, node_idx: usize) {
        let prev = self.nodes[node_idx].prev;
        let next = self.nodes[node_idx].next;

        if let Some(p) = prev {
            self.nodes[p].next = next;
        } else {
            // Node was head
            self.head = next;
        }

        if let Some(n) = next {
            self.nodes[n].prev = prev;
        } else {
            // Node was tail
            self.tail = prev;
        }

        self.nodes[node_idx].prev = None;
        self.nodes[node_idx].next = None;
    }

    /// Inserts a node at the head (MRU) position.
    fn attach_to_head(&mut self, node_idx: usize) {
        if let Some(h) = self.head {
            self.nodes[h].prev = Some(node_idx);
            self.nodes[node_idx].next = Some(h);
            self.nodes[node_idx].prev = None;
            self.head = Some(node_idx);
        } else {
            // List was empty
            self.nodes[node_idx].prev = None;
            self.nodes[node_idx].next = None;
            self.head = Some(node_idx);
            self.tail = Some(node_idx);
        }
    }
}

impl<K: Hash + Eq + Clone, V> Cache<K, V> for ArenaLruCache<K, V> {
    fn get(&mut self, key: &K) -> Option<&V> {
        let node_idx = *self.map.get(key)?;

        // Move to MRU (head)
        self.detach(node_idx);
        self.attach_to_head(node_idx);

        Some(&self.nodes[node_idx].value)
    }

    fn put(&mut self, key: K, value: V) {
        if let Some(&node_idx) = self.map.get(&key) {
            // Key exists, update value and move to MRU
            self.nodes[node_idx].value = value;
            self.detach(node_idx);
            self.attach_to_head(node_idx);
            return;
        }

        if self.map.len() == self.capacity {
            // Evict LRU (tail)
            // GOTCHA: We must use `unwrap` here because if len == capacity > 0, tail must exist.
            let tail_idx = self.tail.unwrap();
            self.detach(tail_idx);

            self.map.remove(&self.nodes[tail_idx].key);

            // Re-use the evicted node's slot
            self.nodes[tail_idx].key = key.clone();
            self.nodes[tail_idx].value = value;
            self.attach_to_head(tail_idx);
            self.map.insert(key, tail_idx);
        } else {
            // Allocate new node
            let idx = self.nodes.len();
            self.nodes.push(Node {
                key: key.clone(),
                value,
                prev: None,
                next: None,
            });

            self.attach_to_head(idx);
            self.map.insert(key, idx);
        }
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `lru`: The definitive production LRU crate. It uses `NonNull` raw pointers instead of
//   an arena of indices. This avoids the bounds-checking overhead of `Vec` indexing and
//   the extra memory required for `Vec` capacity tracking.
// - `cached`: A macro-based caching layer that often wraps `lru` or standard `HashMap`.
//
// Missing vs. Production:
// - **Unsafe Optimizations**: We use safe Rust with bounds checks (`self.nodes[idx]`).
//   A production cache might use raw pointers to squeeze out every nanosecond.
// - **TTL (Time To Live)**: Production caches often support expiring keys after a certain time.
// - **Concurrent Access**: This is not thread-safe. A concurrent LRU is much more complex
//   due to lock contention on the linked list.
//
// Next Steps:
// 1. Implement `Remove` functionality to explicitly delete a key (you would need to manage a `free_list` of indices to reuse holes in the `Vec`).
// 2. Add an iterator over the keys from MRU to LRU.
// 3. Try implementing with `unsafe` and `NonNull` pointers.
//
// Benchmarking Note:
// To benchmark, create a large workload (e.g. 10,000 puts and gets). Use `std::time::Instant::now()`
// before and after the workload. Wrap the resulting values from `.get()` in `std::hint::black_box()`
// to prevent the compiler from optimizing away the reads. You should see `ArenaLruCache` vastly
// outperform `NaiveLruCache` as N grows.

#[cfg(test)]
mod tests {
    use super::*;

    fn test_cache_logic<C: Cache<i32, String>>(mut cache: C) {
        cache.put(1, "one".to_string());
        cache.put(2, "two".to_string());

        assert_eq!(cache.get(&1), Some(&"one".to_string()));

        cache.put(3, "three".to_string()); // Evicts 2

        assert_eq!(cache.get(&2), None);
        assert_eq!(cache.get(&1), Some(&"one".to_string()));
        assert_eq!(cache.get(&3), Some(&"three".to_string()));

        cache.put(4, "four".to_string()); // Evicts 1
        assert_eq!(cache.get(&1), None);
        assert_eq!(cache.get(&3), Some(&"three".to_string()));
        assert_eq!(cache.get(&4), Some(&"four".to_string()));
    }

    #[test]
    fn test_naive_lru() {
        let cache = NaiveLruCache::new(2);
        test_cache_logic(cache);
    }

    #[test]
    fn test_arena_lru() {
        let cache = ArenaLruCache::new(2);
        test_cache_logic(cache);
    }

    #[test]
    fn test_update_existing_key() {
        let mut cache = ArenaLruCache::new(2);
        cache.put(1, "one".to_string());
        cache.put(2, "two".to_string());
        cache.put(1, "ONE".to_string()); // Updates 1, makes it MRU
        cache.put(3, "three".to_string()); // Evicts 2

        assert_eq!(cache.get(&2), None);
        assert_eq!(cache.get(&1), Some(&"ONE".to_string()));
        assert_eq!(cache.get(&3), Some(&"three".to_string()));
    }

    #[test]
    fn test_capacity_one() {
        let mut cache = ArenaLruCache::new(1);
        cache.put(1, "one".to_string());
        assert_eq!(cache.get(&1), Some(&"one".to_string()));

        cache.put(2, "two".to_string()); // Evicts 1
        assert_eq!(cache.get(&1), None);
        assert_eq!(cache.get(&2), Some(&"two".to_string()));
    }
}
