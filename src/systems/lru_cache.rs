//! # LRU Cache Implementation
//!
//! Implements a Least Recently Used (LRU) cache with O(1) time complexity for both `get` and `put` operations.
//!
//! **Replaces Crates:** `lru`, `cached` (partial)
//!
//! **Real-world Usage:**
//! - Database buffer pools (PostgreSQL, MySQL)
//! - CPU caches (L1/L2 eviction policies)
//! - Web browser resource caching
//! - CDN edge node content eviction
//!
//! **Why build it yourself?**
//! Implementing an O(1) LRU cache teaches you how to combine a hash map for fast lookup with a doubly-linked list for O(1) ordering updates.
//! It also forces you to confront Rust's ownership model—specifically why self-referential structures (a node owned by the map but pointed to by neighbors) are hard in safe Rust.
//! You'll learn how to use `NonNull` and `unsafe` responsibly to build high-performance data structures that safe Rust can't easily express without overhead.

use std::collections::HashMap;
use std::hash::Hash;
use std::ptr::NonNull;

// =========================================================================================
// Architecture
// =========================================================================================
//
// Data Structure:
//
//      HashMap (Key -> *Node)
//         │
//         ▼
//      ┌──────┐    ┌──────┐    ┌──────┐
//      │ Node │◄──►│ Node │◄──►│ Node │
//      └──────┘    └──────┘    └──────┘
//         ▲                       ▲
//         │                       │
//       Head                    Tail
//    (Most Recent)           (Least Recent)
//
// Invariants:
// 1. The HashMap contains exactly the nodes in the linked list.
// 2. `head` points to the most recently used node.
// 3. `tail` points to the least recently used node.
// 4. `len` matches the number of nodes in the map/list.
// 5. If `len > 0`, `head` and `tail` are non-null.
//
// Complexity:
// ┌───────────┬────────┬────────┐
// │ Operation │ Time   │ Space  │
// ├───────────┼────────┼────────┤
// │ get       │ O(1)   │ O(1)   │
// │ put       │ O(1)   │ O(1)   │
// └───────────┴────────┴────────┘
//
// Design Decisions:
// - **Backing Store**: `HashMap` for O(1) access to nodes by key.
// - **Ordering**: Doubly-linked list using `NonNull` pointers.
//   - *Alternative*: `Vec` with indices (generational arena). Safer, but O(1) deletions require swap-remove which breaks ordering, or a free-list which adds complexity.
//   - *Alternative*: `Rc<RefCell<Node>>`. Safe, but adds runtime overhead (ref counting, borrow checking) and isn't `Send`/`Sync`.
// - **Memory Management**: Manual `Box::from_raw` in `drop` and `remove` to avoid leaks.

/// A node in the doubly-linked list.
struct Node<K, V> {
    key: K,
    val: V,
    prev: Option<NonNull<Node<K, V>>>,
    next: Option<NonNull<Node<K, V>>>,
}

impl<K, V> Node<K, V> {
    fn new(key: K, val: V) -> Self {
        Self {
            key,
            val,
            prev: None,
            next: None,
        }
    }
}

/// A Least Recently Used (LRU) Cache.
pub struct LRUCache<K, V> {
    capacity: usize,
    // RUST INSIGHT: We use `NonNull` instead of `Box` or `&` because the nodes are
    // owned by the map but linked to each other. Multiple ownership in a cyclic graph
    // (doubly linked list) is the classic case where safe Rust struggles.
    // `NonNull` gives us a covariant pointer that we must manage manually.
    map: HashMap<K, NonNull<Node<K, V>>>,
    head: Option<NonNull<Node<K, V>>>,
    tail: Option<NonNull<Node<K, V>>>,
}

// UNSAFE JUSTIFICATION:
// We are implementing a doubly-linked list where nodes are also stored in a HashMap.
// - Pointers in `map`, `head`, `tail`, `prev`, `next` are valid as long as the node is in the map.
// - We ensure nodes are heap-allocated via `Box::into_raw`.
// - We ensure we re-construct the Box via `Box::from_raw` only when removing from the map/list to drop it.
// - We use `NonNull` to communicate that these pointers are never null (except where Option is used).
unsafe impl<K: Send, V: Send> Send for LRUCache<K, V> {}
unsafe impl<K: Sync, V: Sync> Sync for LRUCache<K, V> {}

impl<K: Hash + Eq + Clone, V> LRUCache<K, V> {
    /// Creates a new LRU Cache with the given capacity.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        // GOTCHA: A capacity of 0 is effectively useless but valid in some interpretations.
        // However, for an LRU, it implies we can't store anything.
        assert!(capacity > 0, "Capacity must be greater than 0");

        Self {
            capacity,
            map: HashMap::with_capacity(capacity),
            head: None,
            tail: None,
        }
    }

    /// Gets the value associated with the key.
    /// Moves the accessed item to the head of the list (most recently used).
    pub fn get(&mut self, key: &K) -> Option<&V> {
        if let Some(&node_ptr) = self.map.get(key) {
            // UNSAFE JUSTIFICATION:
            // The pointer comes from our internal map, so it must point to a valid node
            // that we allocated. We have `&mut self`, so no other access can happen.
            unsafe {
                self.move_to_head(node_ptr);
                Some(&(*node_ptr.as_ptr()).val)
            }
        } else {
            None
        }
    }

    /// Inserts a key-value pair into the cache.
    /// If the key already exists, updates the value and moves it to the head.
    /// If the cache is full, evicts the least recently used item (tail).
    pub fn put(&mut self, key: K, val: V) {
        if let Some(&node_ptr) = self.map.get(&key) {
            // Update existing
            unsafe {
                // Update value
                (*node_ptr.as_ptr()).val = val;
                self.move_to_head(node_ptr);
            }
        } else {
            // Insert new
            // Check capacity
            if self.map.len() >= self.capacity {
                self.evict();
            }

            // Create new node
            let node = Box::new(Node::new(key.clone(), val));
            let node_ptr = NonNull::new(Box::into_raw(node)).unwrap();

            // Insert into map
            self.map.insert(key, node_ptr);

            // Add to head
            unsafe {
                self.add_to_head(node_ptr);
            }
        }
    }

    /// Moves an existing node to the head of the list.
    ///
    /// # Safety
    /// `node` must be a valid pointer to a node currently in the list.
    unsafe fn move_to_head(&mut self, node: NonNull<Node<K, V>>) {
        if self.head == Some(node) {
            return; // Already at head
        }

        // Unlink from current position
        // SAFETY: The caller ensures `node` is valid and in the list.
        unsafe { self.unlink(node) };

        // Link to head
        // SAFETY: We just unlinked it, so it's not in the list. It's valid.
        unsafe { self.add_to_head(node) };
    }

    /// Adds a node to the head of the list.
    ///
    /// # Safety
    /// `node` must be a valid pointer to a node that is *not* currently in the list
    /// (or has been unlinked).
    unsafe fn add_to_head(&mut self, mut node: NonNull<Node<K, V>>) {
        // SAFETY: Caller guarantees node is valid.
        let node_ref = unsafe { node.as_mut() };

        node_ref.next = self.head;
        node_ref.prev = None;

        if let Some(mut old_head) = self.head {
            // SAFETY: old_head is valid as per invariant.
            unsafe { old_head.as_mut().prev = Some(node) };
        }

        self.head = Some(node);

        if self.tail.is_none() {
            self.tail = Some(node);
        }
    }

    /// Unlinks a node from the list.
    ///
    /// # Safety
    /// `node` must be a valid pointer to a node currently in the list.
    unsafe fn unlink(&mut self, mut node: NonNull<Node<K, V>>) {
        // SAFETY: Caller guarantees node is valid.
        let node_ref = unsafe { node.as_mut() };

        let prev = node_ref.prev;
        let next = node_ref.next;

        if let Some(mut p) = prev {
            // SAFETY: p is a valid neighbor.
            unsafe { p.as_mut().next = next };
        } else {
            // Node was head
            self.head = next;
        }

        if let Some(mut n) = next {
            // SAFETY: n is a valid neighbor.
            unsafe { n.as_mut().prev = prev };
        } else {
            // Node was tail
            self.tail = prev;
        }

        // Clear links for safety (though not strictly required if we overwrite them soon)
        node_ref.prev = None;
        node_ref.next = None;
    }

    /// Evicts the least recently used item (tail).
    fn evict(&mut self) {
        if let Some(tail_ptr) = self.tail {
            // Remove from list
            unsafe {
                self.unlink(tail_ptr);

                // PRODUCTION NOTE: A production LRU would reuse the allocation instead of dropping it
                // and allocating a new one, if the types match. This is an optimization.

                // Reconstruct Box to drop
                let node = Box::from_raw(tail_ptr.as_ptr());

                // Remove from map
                self.map.remove(&node.key);

                // Node is dropped here
            }
        }
    }
}

// RUST INSIGHT: Implementing Drop is crucial when using raw pointers / NonNull.
// If we don't, the Boxed nodes will leak because the HashMap only owns the pointer (usize/u64 equivalent),
// not the actual memory allocation logic for the Node struct.
impl<K, V> Drop for LRUCache<K, V> {
    fn drop(&mut self) {
        // Iterate through the list and drop all nodes
        // Alternatively, we can just iterate the values in the HashMap and construct Boxes.
        // But iterating the map consumes the map, so we'd need `drain`.
        // However, `self.map` stores `NonNull`, so dropping the map just drops pointers.

        for (_, node_ptr) in self.map.drain() {
            unsafe {
                // Re-capture ownership to drop
                let _ = Box::from_raw(node_ptr.as_ptr());
            }
        }
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `lru` crate: Uses a similar approach but is heavily optimized and supports multiple implementations.
// - `cached` crate: Provides macros for memoization, often wrapping a simpler cache.
//
// Missing vs. Production:
// - **Thread Safety**: This implementation is not thread-safe. A production version would use `Mutex` or `RwLock`.
// - **Concurrency**: For high concurrency, a sharded lock approach (like `dashmap` or `moka`) is preferred to reduce contention.
// - **Weight-based Eviction**: Production caches often consider item size (weight), not just count.
// - **TTL**: No expiration support.
//
// Next Steps:
// 1. Make it thread-safe with `Arc<Mutex<LRUCache>>`.
// 2. Implement a `ConcurrentLRU` using sharding.
// 3. Add TTL support.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lru_cache_happy_path() {
        let mut cache = LRUCache::new(2);

        cache.put(1, 10);
        cache.put(2, 20);

        assert_eq!(cache.get(&1), Some(&10));
        assert_eq!(cache.get(&2), Some(&20));

        // Evict 1
        cache.put(3, 30);
        assert_eq!(cache.get(&1), None);
        assert_eq!(cache.get(&2), Some(&20));
        assert_eq!(cache.get(&3), Some(&30));
    }

    #[test]
    fn test_lru_cache_update_moves_to_head() {
        let mut cache = LRUCache::new(2);

        cache.put(1, 10);
        cache.put(2, 20);

        // Access 1, making it MRU, 2 becomes LRU
        cache.get(&1);

        cache.put(3, 30); // Should evict 2
        assert_eq!(cache.get(&1), Some(&10));
        assert_eq!(cache.get(&2), None);
        assert_eq!(cache.get(&3), Some(&30));
    }

    #[test]
    fn test_lru_cache_put_update_moves_to_head() {
        let mut cache = LRUCache::new(2);

        cache.put(1, 10);
        cache.put(2, 20);

        // Update 1, making it MRU
        cache.put(1, 100);

        cache.put(3, 30); // Should evict 2
        assert_eq!(cache.get(&1), Some(&100));
        assert_eq!(cache.get(&2), None);
        assert_eq!(cache.get(&3), Some(&30));
    }

    #[test]
    fn test_lru_cache_capacity_1() {
        let mut cache = LRUCache::new(1);
        cache.put(1, 10);
        assert_eq!(cache.get(&1), Some(&10));

        cache.put(2, 20);
        assert_eq!(cache.get(&1), None);
        assert_eq!(cache.get(&2), Some(&20));
    }

    #[test]
    #[should_panic(expected = "Capacity must be greater than 0")]
    fn test_lru_cache_zero_capacity() {
        let _ = LRUCache::<i32, i32>::new(0);
    }

    #[test]
    fn test_lru_complex_sequence() {
        let mut cache = LRUCache::new(3);
        // State: []

        cache.put(1, 1);
        cache.put(2, 2);
        cache.put(3, 3);
        // State: [3, 2, 1] (MRU -> LRU)

        cache.put(4, 4);
        // State: [4, 3, 2] - 1 evicted
        assert_eq!(cache.get(&1), None);
        assert_eq!(cache.get(&2), Some(&2));
        assert_eq!(cache.get(&3), Some(&3));
        assert_eq!(cache.get(&4), Some(&4));

        // Access 2 -> [2, 4, 3]
        cache.get(&2);

        cache.put(5, 5);
        // State: [5, 2, 4] - 3 evicted
        assert_eq!(cache.get(&3), None);
        assert_eq!(cache.get(&2), Some(&2));
        assert_eq!(cache.get(&4), Some(&4));
        assert_eq!(cache.get(&5), Some(&5));
    }
}
