//! # LFU Cache Implementation
//!
//! Implements a Least Frequently Used (LFU) cache with O(1) time complexity for `get` and `put` operations.
//! When multiple items have the same frequency, the Least Recently Used (LRU) item among them is evicted first.
//!
//! **Replaces Crates:** `cached` (partial), `lru` (partial - though primarily LRU)
//!
//! **Real-world Usage:**
//! - HTTP Caching (Squid, Varnish) where frequency matters more than recency.
//! - Database buffer pools where scan resistance is needed (though ARC is often preferred).
//! - CPU Cache replacement policies (some levels).
//!
//! **Why build it yourself?**
//! Implementing O(1) LFU is significantly harder than LRU. While LRU just needs one list, LFU needs a list *per frequency*,
//! and you must manage the `min_frequency` pointer dynamically.
//! It teaches you about 2D data structures (Map of Lists) and managing complex invariants in Unsafe Rust.

use std::collections::HashMap;
use std::hash::Hash;
use std::ptr::NonNull;

// =========================================================================================
// Architecture
// =========================================================================================
//
// Data Structure:
//
//      key_map: HashMap (Key -> *Node)
//         │
//         ▼
//      ┌──────┐
//      │ Node │ (stores Key, Value, Frequency)
//      └──────┘
//         ▲
//         │ (prev/next pointers link nodes with SAME frequency)
//         ▼
//      freq_map: HashMap (Frequency -> FrequencyList)
//         │
//         ▼
//      ┌───────────────┐
//      │ FrequencyList │ ◄──► [Node] ◄──► [Node] ◄──► ...
//      └───────────────┘      (Head)      (Tail)
//
// Invariants:
// 1. `key_map` contains exactly the nodes in all `FrequencyList`s combined.
// 2. `freq_map` contains an entry for every frequency that has at least one node.
// 3. `min_freq` always points to the lowest frequency present in `freq_map`.
// 4. Nodes in a `FrequencyList` are ordered by recency (Head = MRU, Tail = LRU).
//
// Complexity:
// ┌───────────┬────────┬────────┐
// │ Operation │ Time   │ Space  │
// ├───────────┼────────┼────────┤
// │ get       │ O(1)   │ O(N)   │
// │ put       │ O(1)   │ O(N)   │
// └───────────┴────────┴────────┘
//
// Design Decisions:
// - **Backing Store**: `HashMap` for O(1) access to nodes by key.
// - **Frequency Lists**: Another `HashMap` mapping frequency -> linked list.
//   - *Alternative*: Array of lists? No, frequency can grow arbitrarily large.
// - **Node Structure**: Stores `freq` to know which bucket to remove from.
// - **Memory Management**: Manual `Box` management with `NonNull`.

/// A node in the LFU Cache.
/// Links to other nodes with the *same* frequency.
struct Node<K, V> {
    key: K,
    val: V,
    freq: usize,
    prev: Option<NonNull<Self>>,
    next: Option<NonNull<Self>>,
}

impl<K, V> Node<K, V> {
    const fn new(key: K, val: V) -> Self {
        Self {
            key,
            val,
            freq: 1,
            prev: None,
            next: None,
        }
    }
}

/// A doubly-linked list for a specific frequency.
struct FrequencyList<K, V> {
    head: Option<NonNull<Node<K, V>>>,
    tail: Option<NonNull<Node<K, V>>>,
}

impl<K, V> FrequencyList<K, V> {
    const fn new() -> Self {
        Self {
            head: None,
            tail: None,
        }
    }

    /// Adds a node to the head (MRU position for this frequency).
    /// Safety: Node must be valid and not currently linked.
    const unsafe fn push_front(&mut self, mut node: NonNull<Node<K, V>>) {
        // SAFETY: Caller guarantees node is valid.
        unsafe {
            let node_ref = node.as_mut();
            node_ref.prev = None;
            node_ref.next = self.head;

            if let Some(mut head) = self.head {
                head.as_mut().prev = Some(node);
            }

            self.head = Some(node);
            if self.tail.is_none() {
                self.tail = Some(node);
            }
        }
    }

    /// Removes a specific node from the list.
    /// Safety: Node must be in this list.
    const unsafe fn remove(&mut self, mut node: NonNull<Node<K, V>>) {
        // SAFETY: Caller guarantees node is valid and in this list.
        unsafe {
            let node_ref = node.as_mut();
            let prev = node_ref.prev;
            let next = node_ref.next;

            if let Some(mut p) = prev {
                p.as_mut().next = next;
            } else {
                self.head = next;
            }

            if let Some(mut n) = next {
                n.as_mut().prev = prev;
            } else {
                self.tail = prev;
            }

            node_ref.prev = None;
            node_ref.next = None;
        }
    }

    /// Removes and returns the tail node (LRU position for this frequency).
    const unsafe fn pop_back(&mut self) -> Option<NonNull<Node<K, V>>> {
        if let Some(tail) = self.tail {
            // SAFETY: tail is valid as it comes from self.tail
            unsafe {
                self.remove(tail);
            }
            Some(tail)
        } else {
            None
        }
    }

    const fn is_empty(&self) -> bool {
        self.head.is_none()
    }
}

pub struct LFUCache<K, V> {
    capacity: usize,
    min_freq: usize,
    key_map: HashMap<K, NonNull<Node<K, V>>>,
    freq_map: HashMap<usize, FrequencyList<K, V>>,
}

// UNSAFE JUSTIFICATION:
// Same as LRU, we own the nodes via the `key_map`. The `freq_map` just organizes them.
// We implement Send/Sync manually because `NonNull` is !Send/!Sync.
// Send is upheld manually (see SAFETY note above); the raw NonNull fields are owned exclusively.
#[allow(clippy::non_send_fields_in_send_ty)]
unsafe impl<K: Send, V: Send> Send for LFUCache<K, V> {}
unsafe impl<K: Sync, V: Sync> Sync for LFUCache<K, V> {}

impl<K: Hash + Eq + Clone, V> LFUCache<K, V> {
    /// Creates a new LFU Cache with the given capacity.
    ///
    /// # Panics
    /// Panics if `capacity` is 0.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "Capacity must be greater than 0");
        Self {
            capacity,
            min_freq: 0,
            key_map: HashMap::new(),
            freq_map: HashMap::new(),
        }
    }

    /// Gets the value associated with the key, increasing its frequency.
    pub fn get(&mut self, key: &K) -> Option<&V> {
        if let Some(&node_ptr) = self.key_map.get(key) {
            unsafe {
                self.update_freq(node_ptr);
                Some(&(*node_ptr.as_ptr()).val)
            }
        } else {
            None
        }
    }

    /// Inserts a key-value pair into the cache.
    pub fn put(&mut self, key: K, val: V) {
        if self.capacity == 0 {
            return;
        }

        if let Some(&node_ptr) = self.key_map.get(&key) {
            unsafe {
                (*node_ptr.as_ptr()).val = val;
                self.update_freq(node_ptr);
            }
        } else {
            if self.key_map.len() >= self.capacity {
                self.evict();
            }

            // Create new node
            let node = Box::new(Node::new(key.clone(), val));
            // SAFETY: `Box::into_raw` never returns a null pointer, so this cannot be null.
            let node_ptr = unsafe { NonNull::new_unchecked(Box::into_raw(node)) };

            self.key_map.insert(key, node_ptr);

            // New nodes have freq 1
            self.min_freq = 1;
            // SAFETY: node_ptr is valid and we just created it.
            unsafe {
                self.freq_map
                    .entry(1)
                    .or_insert_with(FrequencyList::new)
                    .push_front(node_ptr);
            }
        }
    }

    /// Updates the frequency of a node.
    /// Moves it from `freq_list[freq]` to `freq_list[freq + 1]`.
    unsafe fn update_freq(&mut self, mut node: NonNull<Node<K, V>>) {
        // SAFETY: Caller guarantees node is valid.
        unsafe {
            let freq = node.as_ref().freq;

            // Remove from current frequency list
            if let Some(list) = self.freq_map.get_mut(&freq) {
                list.remove(node);
                // If this list is empty and it was the min_freq, increment min_freq
                if list.is_empty() {
                    self.freq_map.remove(&freq); // Cleanup empty list
                    if self.min_freq == freq {
                        self.min_freq += 1;
                    }
                }
            }

            // Increment freq
            node.as_mut().freq += 1;
            let new_freq = freq + 1;

            // Add to new frequency list
            self.freq_map
                .entry(new_freq)
                .or_insert_with(FrequencyList::new)
                .push_front(node);
        }
    }

    /// Evicts the least frequently used item.
    fn evict(&mut self) {
        if let Some(list) = self.freq_map.get_mut(&self.min_freq) {
            unsafe {
                if let Some(node_ptr) = list.pop_back() {
                    // Reconstruct Box to drop
                    let node = Box::from_raw(node_ptr.as_ptr());

                    // Remove from key_map
                    self.key_map.remove(&node.key);

                    // If list is now empty, remove it
                    if list.is_empty() {
                        self.freq_map.remove(&self.min_freq);
                        // We don't need to update min_freq here because we are about to insert a new item
                        // which will set min_freq to 1 (if put is calling this).
                        // However, strictly speaking, the cache is now empty at min_freq.
                    }
                }
            }
        }
    }

    /// Returns the current size of the cache.
    #[must_use] 
    pub fn len(&self) -> usize {
        self.key_map.len()
    }

    /// Returns true if the cache is empty.
    #[must_use] 
    pub fn is_empty(&self) -> bool {
        self.key_map.is_empty()
    }
}

impl<K, V> Drop for LFUCache<K, V> {
    fn drop(&mut self) {
        for (_, node_ptr) in self.key_map.drain() {
            unsafe {
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
// - `cached`: Uses a different strategy, often not true O(1) LFU or relies on approximations.
// - `lru`: Optimized for LRU, doesn't support LFU.
//
// Missing vs. Production:
// - **Concurrency**: Not thread-safe. Use `Arc<Mutex<LFUCache>>` or a sharded implementation.
// - **Adaptive**: Real LFU often suffers from "cache pollution" where one-time scans fill the cache with freq=1 items
//   that stick around too long if the cache is large. Window-LFU or TinyLFU (using Sketching) is preferred in production (e.g., Caffeine).
//
// Next Steps:
// 1. Implement W-TinyLFU (Window Tiny LFU) which uses a Bloom Filter/CountMinSketch to admit items.
// 2. Add concurrency support.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lfu_basic_put_get() {
        let mut cache = LFUCache::new(2);
        cache.put(1, 10);
        cache.put(2, 20);

        assert_eq!(cache.get(&1), Some(&10));
        assert_eq!(cache.get(&2), Some(&20));
    }

    #[test]
    fn test_lfu_eviction() {
        let mut cache = LFUCache::new(2);
        cache.put(1, 10); // freq: 1
        cache.put(2, 20); // freq: 1

        assert_eq!(cache.get(&1), Some(&10)); // 1 freq: 2, 2 freq: 1

        cache.put(3, 30); // Should evict 2 (freq 1 < freq 2)

        assert_eq!(cache.get(&1), Some(&10));
        assert_eq!(cache.get(&2), None);
        assert_eq!(cache.get(&3), Some(&30));
    }

    #[test]
    fn test_lfu_tie_breaking_lru() {
        let mut cache = LFUCache::new(3);
        cache.put(1, 10); // freq 1
        cache.put(2, 20); // freq 1
        cache.put(3, 30); // freq 1

        // Access 1 and 2 to bump freq to 2
        cache.get(&1); // 1 freq: 2
        cache.get(&2); // 2 freq: 2

        // 3 is freq 1.

        cache.put(4, 40); // Should evict 3 (min freq 1)

        assert_eq!(cache.get(&3), None);
        assert_eq!(cache.get(&1), Some(&10));
        assert_eq!(cache.get(&2), Some(&20));
        assert_eq!(cache.get(&4), Some(&40));

        // Now 1, 2 have freq 2. 4 has freq 1.
        // Access 4 to make it freq 2.
        cache.get(&4); // 4 freq: 2.

        // Now 1, 2, 4 have freq 2.
        // Order of becoming freq 2: 1, 2, 4.
        // LRU within freq 2 should be 1?
        // Wait. `get` moves to head of the new frequency list.
        // 1 moved to head of freq 2 list first.
        // 2 moved to head of freq 2 list second. So 2 is MRU, 1 is LRU.
        // 4 moved to head of freq 2 list third. So 4 is MRU, 2 is middle, 1 is LRU.

        cache.put(5, 50); // Should evict 1.

        assert_eq!(cache.get(&1), None);
        assert_eq!(cache.get(&2), Some(&20));
        assert_eq!(cache.get(&4), Some(&40));
        assert_eq!(cache.get(&5), Some(&50));
    }

    #[test]
    fn test_lfu_update_value() {
        let mut cache = LFUCache::new(2);
        cache.put(1, 10);
        cache.put(1, 11); // Update value, freq should increase?

        // "If the key already exists, updates the value and moves it to the head."
        // Logic says `update_freq` is called. So yes, freq increases.

        assert_eq!(cache.get(&1), Some(&11)); // freq becomes 3 now (1 insert, 1 update, 1 get)

        cache.put(2, 20); // freq 1

        // 1 has high freq. 2 has low freq.
        cache.put(3, 30); // Evicts 2.

        assert_eq!(cache.get(&1), Some(&11));
        assert_eq!(cache.get(&2), None);
        assert_eq!(cache.get(&3), Some(&30));
    }
}
