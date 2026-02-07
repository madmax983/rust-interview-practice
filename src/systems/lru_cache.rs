//! # LRU Cache
//!
//! Implements a Least Recently Used (LRU) cache with O(1) time complexity for both `get` and `put` operations.
//!
//! Replaces crates like `lru` (which uses a similar approach but often backed by a `Vec` arena) or `cached`.
//!
//! ## Real-world usage
//! LRU caches are ubiquitous in computing:
//! - CPU caches (L1/L2/L3) often approximate LRU.
//! - Database buffer pools (PostgreSQL, MySQL).
//! - Web browser caches.
//! - CDN edge nodes.
//!
//! ## Why build it yourself?
//! Implementing an LRU cache from scratch teaches you about:
//! - Combining data structures (Hash Map + Doubly Linked List) to achieve strict performance bounds.
//! - Managing raw pointers and memory safety in Rust (when bypassing the borrow checker for self-referential structures).
//! - Implementing `Drop` correctly to avoid memory leaks in manual memory management scenarios.
//!
//! ## Architecture
//!
//! ```text
//! HashMap<Key, *Node>
//!      |
//!      v
//! [Node] <-> [Node] <-> [Node]
//!   ^                     ^
//!   |                     |
//!  Head                  Tail
//! (MRU)                 (LRU)
//! ```
//!
//! **Invariants:**
//! 1. The `map` contains exactly the same nodes as the linked list.
//! 2. `map.len() == list.len()`.
//! 3. `head` points to the Most Recently Used item.
//! 4. `tail` points to the Least Recently Used item.
//! 5. `capacity > 0`.
//!
//! **Complexity:**
//!
//! | Operation | Time | Space |
//! |-----------|------|-------|
//! | `get`     | O(1) | O(1)  |
//! | `put`     | O(1) | O(1)  |
//!
//! **Design Decisions:**
//! - **Doubly Linked List:** Necessary for O(1) deletion of any node (which happens during `get` to move to head, or eviction).
//! - **HashMap:** Necessary for O(1) lookup.
//! - **`NonNull` pointers:** used instead of `Rc<RefCell<Node>>` or `Option<Box<Node>>` because removing a node from the middle of the list requires mutable access to its neighbors, which is hard to prove safe to the borrow checker when the HashMap also holds references. `NonNull` gives us pointer stability and interior mutability without runtime overhead, at the cost of `unsafe`.
//!
//! ## Implementation

use std::collections::HashMap;
use std::hash::Hash;
use std::ptr::NonNull;
use std::marker::PhantomData;

// RUST INSIGHT:
// We use a struct to wrap the key and value along with the prev/next pointers.
// Since we are managing memory manually, we need to be careful about ownership.
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

pub struct LruCache<K, V> {
    capacity: usize,
    // PRODUCTION NOTE:
    // In a real production system, you might use a high-performance hashing algorithm like AHash or FxHash.
    // The standard `RandomState` is secure but slower.
    map: HashMap<K, NonNull<Node<K, V>>>,
    head: Option<NonNull<Node<K, V>>>,
    tail: Option<NonNull<Node<K, V>>>,
    // RUST INSIGHT:
    // Since we hold raw pointers to `Node<K, V>`, the compiler doesn't know we "own" K and V.
    // `PhantomData` tells the drop checker and auto traits (Send/Sync) that we effectively own K and V.
    _marker: PhantomData<Box<Node<K, V>>>,
}

// UNSAFE JUSTIFICATION:
// We are implementing Send and Sync manually.
// Since we use `NonNull` (raw pointers), the compiler doesn't implement these automatically.
// The `LruCache` owns the data (K, V) logically. If K and V are Send, the Cache can be moved to another thread.
unsafe impl<K: Send, V: Send> Send for LruCache<K, V> {}
unsafe impl<K: Sync, V: Sync> Sync for LruCache<K, V> {}

impl<K, V> LruCache<K, V> {
    /// Removes the node from the linked list structure (but doesn't deallocate).
    fn detach(&mut self, mut node: NonNull<Node<K, V>>) {
        unsafe {
            let prev = node.as_ref().prev;
            let next = node.as_ref().next;

            match prev {
                Some(mut p) => p.as_mut().next = next,
                None => self.head = next, // Node was head
            }

            match next {
                Some(mut n) => n.as_mut().prev = prev,
                None => self.tail = prev, // Node was tail
            }

            // Clear pointers of the detached node for safety
            node.as_mut().prev = None;
            node.as_mut().next = None;
        }
    }

    /// Attaches the node to the head of the list.
    fn attach_head(&mut self, mut node: NonNull<Node<K, V>>) {
        unsafe {
            node.as_mut().next = self.head;
            node.as_mut().prev = None;

            if let Some(mut head) = self.head {
                head.as_mut().prev = Some(node);
            }

            self.head = Some(node);

            if self.tail.is_none() {
                self.tail = Some(node);
            }
        }
    }
}

impl<K: Hash + Eq + Clone, V> LruCache<K, V> {
    /// Creates a new LRU Cache with the given capacity.
    ///
    /// # Panics
    /// Panics if capacity is 0.
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "Capacity must be greater than 0");
        Self {
            capacity,
            map: HashMap::with_capacity(capacity),
            head: None,
            tail: None,
            _marker: PhantomData,
        }
    }

    /// Gets a reference to the value associated with the key.
    /// Updates the item to be the most recently used.
    pub fn get(&mut self, key: &K) -> Option<&V> {
        // GOTCHA:
        // We need to use a raw pointer from the map to modify the list structure.
        // If we just took a reference, we couldn't mutate `self.head` and `self.tail` easily while holding it.
        if let Some(&node_ptr) = self.map.get(key) {
            self.detach(node_ptr);
            self.attach_head(node_ptr);
            // UNSAFE JUSTIFICATION:
            // The pointer is valid because it's in the map, and we haven't removed it.
            // We just moved it to the head.
            unsafe { Some(&node_ptr.as_ref().val) }
        } else {
            None
        }
    }

    /// Puts a key-value pair into the cache.
    /// If the key already exists, updates the value and moves it to the head.
    /// If the cache is full, evicts the least recently used item.
    pub fn put(&mut self, key: K, val: V) {
        if let Some(&node_ptr) = self.map.get(&key) {
            // Key exists, update value and move to head
            // UNSAFE JUSTIFICATION:
            // Pointer is valid as it comes from the map.
            unsafe {
                (*node_ptr.as_ptr()).val = val;
            }
            self.detach(node_ptr);
            self.attach_head(node_ptr);
        } else {
            // Key doesn't exist
            if self.map.len() >= self.capacity {
                self.evict();
            }

            // Create new node
            // RUST INSIGHT:
            // We use `Box::into_raw` to transfer ownership of the memory to us (manual management).
            // We must remember to `Box::from_raw` later to drop it.
            let node = Box::new(Node::new(key.clone(), val));
            let node_ptr = unsafe { NonNull::new_unchecked(Box::into_raw(node)) };

            self.attach_head(node_ptr);
            self.map.insert(key, node_ptr);
        }
    }

    /// Evicts the least recently used item (tail).
    fn evict(&mut self) {
        if let Some(tail_ptr) = self.tail {
            self.detach(tail_ptr);
            // Remove from map and deallocate
            // UNSAFE JUSTIFICATION:
            // We are reconstructing the Box to drop it.
            // We must make sure we don't use `tail_ptr` after this.
            unsafe {
                let node = Box::from_raw(tail_ptr.as_ptr());
                self.map.remove(&node.key);
                // `node` is dropped here, freeing memory
            }
        }
    }
}

impl<K, V> Drop for LruCache<K, V> {
    fn drop(&mut self) {
        // PRODUCTION NOTE:
        // Since we used `Box::into_raw`, Rust won't automatically drop the nodes.
        // We need to iterate and drop them manually to avoid memory leaks.
        while let Some(ptr) = self.head {
            // UNSAFE JUSTIFICATION:
            // We are iterating through the list and dropping nodes.
            // `detach` updates `self.head`.
            self.detach(ptr);
            unsafe {
                let _ = Box::from_raw(ptr.as_ptr());
            }
        }
    }
}

// Footer

// Comparison:
// This implementation is very similar to `lru` crate's internals but uses explicit `NonNull`
// for educational purposes, whereas `lru` uses a `Vec` and indices (safe code wrapper) in some versions,
// or similar pointer logic.
//
// Missing:
// - `get_mut` (mutable reference to value)
// - `peek` (look without updating LRU)
// - `resize` (dynamic capacity adjustment)
// - Iterators (Iter, IterMut, IntoIter)

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_put_get() {
        let mut lru = LruCache::new(2);
        lru.put("a", 1);
        lru.put("b", 2);

        assert_eq!(lru.get(&"a"), Some(&1));
        assert_eq!(lru.get(&"b"), Some(&2));
        assert_eq!(lru.get(&"c"), None);
    }

    #[test]
    fn test_eviction() {
        let mut lru = LruCache::new(2);
        lru.put("a", 1);
        lru.put("b", 2);

        // Access "a" to make it MRU
        lru.get(&"a");

        // "b" is now LRU
        lru.put("c", 3);

        assert_eq!(lru.get(&"a"), Some(&1));
        assert_eq!(lru.get(&"b"), None); // evicted
        assert_eq!(lru.get(&"c"), Some(&3));
    }

    #[test]
    fn test_update_existing() {
        let mut lru = LruCache::new(2);
        lru.put("a", 1);
        lru.put("b", 2);

        // Update "a", it becomes MRU
        lru.put("a", 10);

        // Add "c", "b" should be evicted (LRU)
        lru.put("c", 3);

        assert_eq!(lru.get(&"a"), Some(&10));
        assert_eq!(lru.get(&"b"), None);
        assert_eq!(lru.get(&"c"), Some(&3));
    }

    #[test]
    fn test_capacity_1() {
         let mut lru = LruCache::new(1);
         lru.put("a", 1);
         assert_eq!(lru.get(&"a"), Some(&1));

         lru.put("b", 2);
         assert_eq!(lru.get(&"a"), None);
         assert_eq!(lru.get(&"b"), Some(&2));
    }

    #[test]
    #[should_panic(expected = "Capacity must be greater than 0")]
    fn test_zero_capacity() {
        let _ = LruCache::<i32, i32>::new(0);
    }

    #[test]
    fn test_drop_cleanup() {
        // This test is mainly for Miri or Valgrind to check for leaks.
        // But we can check logic.
        let mut lru = LruCache::new(10);
        for i in 0..100 {
            lru.put(i, i);
        }
        // Should have 10 items.
        // Dropping `lru` should free all nodes.
    }
}
