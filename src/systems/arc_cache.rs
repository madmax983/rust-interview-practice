//! # ARC Cache Implementation
//!
//! Implements an Adaptive Replacement Cache (ARC) with O(1) time complexity.
//! ARC dynamically tunes the balance between recency (LRU) and frequency (LFU) based on workload patterns,
//! offering better hit rates than LRU for many real-world traces.
//!
//! **Replaces Crates:** `lru` (advanced usage), `cached` (partial), specialized ARC crates.
//!
//! **Real-world Usage:**
//! - ZFS (ZFS Adaptive Replacement Cache) - likely the most famous use case.
//! - Database buffer pools (PostgreSQL experimented with it, though often use simpler approximations like 2Q due to patent/complexity).
//! - Storage systems (IBM DS8000).
//!
//! **Why build it yourself?**
//! ARC is the next step after LRU. It introduces the concept of "ghost lists" (history of evicted items)
//! to detect whether the cache should favor recent items or frequent items.
//! Implementing it teaches you about adaptive algorithms, maintaining complex invariants across 4 linked lists,
//! and handling "phantom hits" (hits on evicted metadata).

use std::collections::HashMap;
use std::fmt;
use std::hash::Hash;
use std::mem;
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
//      ┌──────┐
//      │ Node │ (Key, Option<Value>, ListType)
//      └──────┘
//
//      4 Doubly-Linked Lists:
//      1. T1: Recent items (in cache).
//      2. T2: Frequent items (in cache).
//      3. B1: Ghost list for T1 (evicted keys, no value).
//      4. B2: Ghost list for T2 (evicted keys, no value).
//
//      Parameter `p`: Target size of T1. 0 <= p <= c.
//
// Logic:
// - Hits in T1/T2 -> Move to MRU of T2.
// - Hits in B1 (phantom) -> Increase p (favor recency). Move to T2.
// - Hits in B2 (phantom) -> Decrease p (favor frequency). Move to T2.
// - Miss -> Insert to T1. Evict from T1/T2/B1/B2 as needed to maintain:
//     |T1| + |T2| <= c
//     |T1| + |B1| <= c
//     |T1| + |B1| + |T2| + |B2| <= 2c
//
// Complexity:
// ┌───────────┬────────┬────────┐
// │ Operation │ Time   │ Space  │
// ├───────────┼────────┼────────┤
// │ get       │ O(1)   │ O(1)   │
// │ put       │ O(1)   │ O(1)   │
// └───────────┴────────┴────────┘

/// A trait for cache implementations.
/// Allows swapping strategies (LRU, LFU, ARC) without changing consumer code.
pub trait Cache<K, V> {
    /// Gets the value associated with the key.
    fn get(&mut self, key: &K) -> Option<&V>;

    /// Inserts a key-value pair into the cache.
    fn put(&mut self, key: K, val: V);
}

#[derive(PartialEq, Clone, Copy, Debug)]
enum ListType {
    T1,
    T2,
    B1,
    B2,
}

/// A node in the ARC Cache.
struct Node<K, V> {
    key: K,
    val: Option<V>, // None if in B1 or B2 (Ghost)
    list_type: ListType,
    prev: Option<NonNull<Node<K, V>>>,
    next: Option<NonNull<Node<K, V>>>,
}

impl<K, V> Node<K, V> {
    fn new(key: K, val: Option<V>, list_type: ListType) -> Self {
        Self {
            key,
            val,
            list_type,
            prev: None,
            next: None,
        }
    }
}

/// Helper struct to manage a doubly-linked list of Nodes.
struct LinkedList<K, V> {
    head: Option<NonNull<Node<K, V>>>,
    tail: Option<NonNull<Node<K, V>>>,
    len: usize,
}

impl<K, V> LinkedList<K, V> {
    fn new() -> Self {
        Self {
            head: None,
            tail: None,
            len: 0,
        }
    }

    /// Adds a node to the head of the list.
    /// Safety: Node must not be in any list.
    unsafe fn push_front(&mut self, mut node: NonNull<Node<K, V>>) {
        // SAFETY: Caller guarantees node is valid.
        let node_ref = unsafe { node.as_mut() };
        node_ref.next = self.head;
        node_ref.prev = None;

        if let Some(mut head) = self.head {
            // SAFETY: head is valid.
            unsafe { head.as_mut().prev = Some(node) };
        }

        self.head = Some(node);
        if self.tail.is_none() {
            self.tail = Some(node);
        }
        self.len += 1;
    }

    /// Removes a specific node from the list.
    /// Safety: Node must be in this list.
    unsafe fn remove(&mut self, mut node: NonNull<Node<K, V>>) {
        // SAFETY: Caller guarantees node is valid.
        let node_ref = unsafe { node.as_mut() };
        let prev = node_ref.prev;
        let next = node_ref.next;

        if let Some(mut p) = prev {
            // SAFETY: p is valid neighbor.
            unsafe { p.as_mut().next = next };
        } else {
            self.head = next;
        }

        if let Some(mut n) = next {
            // SAFETY: n is valid neighbor.
            unsafe { n.as_mut().prev = prev };
        } else {
            self.tail = prev;
        }

        node_ref.prev = None;
        node_ref.next = None;
        self.len -= 1;
    }

    /// Removes and returns the tail node (LRU).
    unsafe fn pop_back(&mut self) -> Option<NonNull<Node<K, V>>> {
        if let Some(tail) = self.tail {
            // SAFETY: tail is valid.
            unsafe { self.remove(tail) };
            Some(tail)
        } else {
            None
        }
    }
}

pub struct ARCCache<K, V> {
    capacity: usize, // c
    p: usize,        // Target size for T1
    map: HashMap<K, NonNull<Node<K, V>>>,
    t1: LinkedList<K, V>,
    t2: LinkedList<K, V>,
    b1: LinkedList<K, V>,
    b2: LinkedList<K, V>,
}

// UNSAFE JUSTIFICATION:
// We use NonNull pointers to manage the linked lists manually for O(1) splicing and moving.
// The `map` owns the nodes (conceptually), or rather the `ARCCache` owns the allocated Boxes.
// We implement Send/Sync manually as NonNull is !Send/!Sync, but our usage is safe because
// we never expose raw pointers and the struct owns all data it points to.
unsafe impl<K: Send, V: Send> Send for ARCCache<K, V> {}
unsafe impl<K: Sync, V: Sync> Sync for ARCCache<K, V> {}

impl<K: Hash + Eq + Clone + fmt::Debug, V> ARCCache<K, V> {
    /// Creates a new ARC Cache with the given capacity.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "Capacity must be greater than 0");
        Self {
            capacity,
            p: 0,
            map: HashMap::with_capacity(capacity),
            t1: LinkedList::new(),
            t2: LinkedList::new(),
            b1: LinkedList::new(),
            b2: LinkedList::new(),
        }
    }

    /// The generic REPLACE procedure from ARC.
    /// adjust_p_for_b2: true if the triggering miss was in B2 (used for condition check).
    fn replace(&mut self, adjust_p_for_b2: bool) {
        let t1_len = self.t1.len;
        let p = self.p;

        // Condition: (T1 is not empty) AND ((T1 > p) OR (hit in B2 and T1 == p))
        if t1_len > 0 && (t1_len > p || (adjust_p_for_b2 && t1_len == p)) {
            // Move LRU(T1) to MRU(B1)
            unsafe {
                if let Some(lru) = self.t1.pop_back() {
                    (*lru.as_ptr()).val = None; // Drop value (becomes ghost)
                    self.b1.push_front(lru);
                    (*lru.as_ptr()).list_type = ListType::B1;
                }
            }
        } else {
            // Move LRU(T2) to MRU(B2)
            unsafe {
                if let Some(lru) = self.t2.pop_back() {
                    (*lru.as_ptr()).val = None; // Drop value (becomes ghost)
                    self.b2.push_front(lru);
                    (*lru.as_ptr()).list_type = ListType::B2;
                }
            }
        }
    }

    /// Helper: Detaches a node from its current list.
    unsafe fn detach(&mut self, node: NonNull<Node<K, V>>) {
        // SAFETY: Caller guarantees node is valid.
        match unsafe { node.as_ref().list_type } {
            ListType::T1 => unsafe { self.t1.remove(node) },
            ListType::T2 => unsafe { self.t2.remove(node) },
            ListType::B1 => unsafe { self.b1.remove(node) },
            ListType::B2 => unsafe { self.b2.remove(node) },
        }
    }

    /// Helper: Attaches a node to the head of a specific list.
    unsafe fn attach(&mut self, node: NonNull<Node<K, V>>, list_type: ListType) {
        // SAFETY: Caller guarantees node is valid.
        unsafe { (*node.as_ptr()).list_type = list_type };
        match list_type {
            ListType::T1 => unsafe { self.t1.push_front(node) },
            ListType::T2 => unsafe { self.t2.push_front(node) },
            ListType::B1 => unsafe { self.b1.push_front(node) },
            ListType::B2 => unsafe { self.b2.push_front(node) },
        }
    }

    /// Helper: Removes the LRU node from a specific list and drops it entirely.
    fn delete_lru(&mut self, list_type: ListType) {
        unsafe {
            let node_opt = match list_type {
                ListType::T1 => self.t1.pop_back(),
                ListType::T2 => self.t2.pop_back(),
                ListType::B1 => self.b1.pop_back(),
                ListType::B2 => self.b2.pop_back(),
            };

            if let Some(node_ptr) = node_opt {
                // Remove from map
                let node = Box::from_raw(node_ptr.as_ptr());
                self.map.remove(&node.key);
                // Node dropped here
            }
        }
    }
}

impl<K: Hash + Eq + Clone + fmt::Debug, V> Cache<K, V> for ARCCache<K, V> {
    /// Gets the value associated with the key.
    ///
    /// - If key is in T1 or T2 (Cache Hit): Moves item to MRU of T2 and returns value.
    /// - If key is in B1 or B2 (Ghost Hit): Returns None (value not cached).
    ///   NOTE: Does NOT automatically promote/adapt. Use `put` to promote ghost hits.
    ///   This design choice allows the caller to decide if they want to fetch and insert.
    /// - If key is missing: Returns None.
    fn get(&mut self, key: &K) -> Option<&V> {
        if let Some(&node_ptr) = self.map.get(key) {
            unsafe {
                let node_ref = node_ptr.as_ref();
                if node_ref.val.is_some() {
                    // Cache Hit (T1 or T2)
                    // Move to T2 MRU
                    self.detach(node_ptr);
                    self.attach(node_ptr, ListType::T2);
                    return node_ptr.as_ref().val.as_ref();
                } else {
                    // Ghost Hit (B1 or B2)
                    // Return None, user must fetch and put.
                    return None;
                }
            }
        }
        None
    }

    /// Inserts a key-value pair into the cache.
    /// Handles all ARC adaptation logic (p adjustment, ghost hits).
    fn put(&mut self, key: K, val: V) {
        if let Some(&node_ptr) = self.map.get(&key) {
            // Key exists (T1, T2, B1, or B2)
            unsafe {
                let node_ref = node_ptr.as_ref();
                match node_ref.list_type {
                    ListType::T1 | ListType::T2 => {
                        // Update value and move to T2 MRU
                        (*node_ptr.as_ptr()).val = Some(val);
                        self.detach(node_ptr);
                        self.attach(node_ptr, ListType::T2);
                    }
                    ListType::B1 => {
                        // Ghost Hit in B1 (Recency win)
                        // Adaptation: Increase p
                        let delta = if self.b1.len >= self.b2.len {
                            1
                        } else {
                            self.b2.len / self.b1.len
                        };
                        self.p = (self.p + delta).min(self.capacity);

                        // Replace and Move
                        self.replace(false); // false because hit was in B1 (not B2)

                        // Move from B1 to T2
                        (*node_ptr.as_ptr()).val = Some(val);
                        self.detach(node_ptr);
                        self.attach(node_ptr, ListType::T2);
                    }
                    ListType::B2 => {
                        // Ghost Hit in B2 (Frequency win)
                        // Adaptation: Decrease p
                        let delta = if self.b2.len >= self.b1.len {
                            1
                        } else {
                            self.b1.len / self.b2.len
                        };
                        if delta > self.p {
                            self.p = 0;
                        } else {
                            self.p -= delta;
                        }

                        // Replace and Move
                        self.replace(true); // true because hit was in B2

                        // Move from B2 to T2
                        (*node_ptr.as_ptr()).val = Some(val);
                        self.detach(node_ptr);
                        self.attach(node_ptr, ListType::T2);
                    }
                }
            }
        } else {
            // Cache Miss
            // Case A: L1 (T1 + B1) has c items
            if self.t1.len + self.b1.len == self.capacity {
                if self.t1.len < self.capacity {
                    // Delete LRU of B1
                    self.delete_lru(ListType::B1);
                    self.replace(false);
                } else {
                    // Delete LRU of T1
                    self.delete_lru(ListType::T1);
                }
            }
            // Case B: L1 < c AND L1 + L2 >= c
            else if (self.t1.len + self.b1.len < self.capacity)
                && (self.t1.len + self.b1.len + self.t2.len + self.b2.len >= self.capacity)
            {
                if self.t1.len + self.b1.len + self.t2.len + self.b2.len == 2 * self.capacity {
                    // Delete LRU of B2
                    self.delete_lru(ListType::B2);
                }
                self.replace(false);
            }

            // Insert into T1
            let node = Box::new(Node::new(key.clone(), Some(val), ListType::T1));
            let node_ptr = NonNull::new(Box::into_raw(node)).unwrap();
            self.map.insert(key, node_ptr);
            unsafe {
                self.t1.push_front(node_ptr);
            }
        }
    }
}

impl<K, V> Drop for ARCCache<K, V> {
    fn drop(&mut self) {
        for (_, node_ptr) in self.map.drain() {
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
// - `lru`: Standard LRU is simpler and faster but hit rate is lower for mixed workloads.
// - `cached`: Mostly provides memoization, not advanced eviction policies like ARC.
//
// Missing vs. Production:
// - **Concurrency**: Not thread-safe. Use `Arc<Mutex<ARCCache>>`.
// - **Scan Resistance**: ARC inherently has scan resistance (T1 filters scans), unlike LRU.
// - **Ghost Node Optimization**: Production ARC would store ghost nodes without `Option<V>` overhead,
//   saving memory. Here we use `Option<V>` for simplicity.
//
// Next Steps:
// 1. Optimize memory layout for Ghost Nodes.
// 2. Add concurrency support.
// 3. Implement LIRS (Low Inter-reference Recency Set) which is similar but often performs better.

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    #[test]
    fn test_arc_basic_put_get() {
        let mut cache = ARCCache::new(2);
        cache.put(1, 10);
        cache.put(2, 20);

        assert_eq!(cache.get(&1), Some(&10));
        assert_eq!(cache.get(&2), Some(&20));
    }

    #[test]
    fn test_arc_lru_behavior_initially() {
        // Initially p=0, behaves like LRU (mostly)
        let mut cache = ARCCache::new(2);
        cache.put(1, 10);
        cache.put(2, 20);
        cache.put(3, 30); // Evicts 1 (LRU of T1)

        assert_eq!(cache.get(&1), None);
        assert_eq!(cache.get(&2), Some(&20));
        assert_eq!(cache.get(&3), Some(&30));
    }

    #[test]
    fn test_arc_adaptation_to_frequency() {
        let mut cache = ARCCache::new(2);

        // Fill cache [1, 2] in T1
        cache.put(1, 10);
        cache.put(2, 20);

        // Access 1 twice -> Moves to T2
        cache.get(&1);
        cache.put(1, 10);

        // Add 3 -> Evicts 2 (LRU of T1). 2 goes to B1.
        cache.put(3, 30);

        // State: T2: [1], T1: [3], B1: [2]

        // Access 2 (Ghost Hit in B1).
        // Should increase p.
        // Move 2 to T2.
        // As p=1, T1 size matches p. replace(false) will evict from T2 to make space.
        // 1 (in T2) evicted to B2.

        cache.put(2, 25);

        assert_eq!(cache.get(&2), Some(&25));
        assert_eq!(cache.get(&1), None); // 1 evicted to B2
        assert_eq!(cache.get(&3), Some(&30)); // 3 stays in T1 (matches p=1)

        // Now demonstrate adaptation back (favor frequency)
        // Hit 1 in B2.
        cache.put(1, 15);

        // p should decrease.
        // replace(true) is called.
        // Evicts T1 (3) to B1.
        // Moves 1 to T2.

        assert_eq!(cache.get(&1), Some(&15)); // Back in cache
        assert_eq!(cache.get(&2), None); // Evicted from T2 (was LRU in T2 after 3 moved to T2)
        assert_eq!(cache.get(&3), Some(&30)); // 3 is in T2 (MRU position relative to 2)
    }

    #[test]
    fn test_arc_scan_resistance() {
        // Scan: many one-time items.
        // Frequent items should stay.
        let mut cache = ARCCache::new(2);

        // Frequent item
        cache.put(1, 10);
        cache.get(&1); // Move to T2

        // Scan
        cache.put(2, 20);
        cache.put(3, 30);
        cache.put(4, 40);

        // 1 should persist in T2 because T1 absorbs the scan.
        assert_eq!(cache.get(&1), Some(&10));

        // 2 and 3 should be gone (evicted from T1/B1)
        // 4 is recent.
        assert_eq!(cache.get(&4), Some(&40));
    }

    #[test]
    fn test_arc_capacity_limits() {
        let mut cache = ARCCache::new(2);

        cache.put(1, 1);
        cache.put(2, 2);
        cache.put(3, 3);
        cache.put(4, 4);
        cache.put(5, 5);

        // Live items
        let mut live_count = 0;
        if cache.get(&1).is_some() {
            live_count += 1;
        }
        if cache.get(&2).is_some() {
            live_count += 1;
        }
        if cache.get(&3).is_some() {
            live_count += 1;
        }
        if cache.get(&4).is_some() {
            live_count += 1;
        }
        if cache.get(&5).is_some() {
            live_count += 1;
        }

        assert!(live_count <= 2);
    }

    #[test]
    fn test_arc_benchmark_proxy() {
        // Simple benchmark to ensure O(1) behavior isn't grossly violated.
        let mut cache = ARCCache::new(1000);
        let start = Instant::now();
        for i in 0..10_000 {
            cache.put(i, i);
        }
        let duration = start.elapsed();
        // Just printing for visibility, assert is mainly that it finishes quickly.
        println!("Inserted 10k items in {:?}", duration);
        assert!(duration.as_secs() < 1);
    }
}
