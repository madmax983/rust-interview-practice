//! # W-TinyLFU Cache Implementation
//!
//! A high-performance, scan-resistant cache admission policy and eviction strategy.
//!
//! **Replaces Crates:** `moka`, `caffeine` (Java)
//!
//! **Real-world Usage:**
//! - High-throughput database query caching where one-time table scans shouldn't evict hot data.
//! - HTTP edge caches (CDNs) serving mixed zipfian and flat distributions.
//! - General purpose, concurrency-friendly application caches.
//!
//! **Why build it yourself?**
//! LRU fails catastrophically when a sequential scan accesses more items than the cache capacity,
//! wiping out all frequently accessed (hot) items. Standard LFU solves this but suffers from
//! "cache pollution" (items that were hot once stick around forever) and requires `O(N)` metadata.
//! W-TinyLFU solves both by using a tiny Count-Min Sketch for frequency estimation, and a small
//! Window LRU cache to admit new items before promoting them to the Main cache.

use crate::data_structures::count_min_sketch::CountMinSketch;
use std::collections::HashMap;
use std::hash::Hash;

// =========================================================================================
// Architecture
// =========================================================================================
//
// W-TinyLFU Structure:
//
// Incoming Items
//       │
//       ▼
// ┌─────────────┐
// │ Window LRU  │ (1% capacity) --> Absorbs bursts of new, potentially cold items.
// └─────────────┘
//       │
//       ▼ (Victim W)
//  [Admission Filter] <--- Count-Min Sketch (tracks frequencies of all seen items)
//       │    compare freq(W) vs freq(P)
//       ▼ (If freq(W) > freq(P))
// ┌─────────────┐
// │  Main LRU   │
// │ ┌─────────┐ │
// │ │Protected│ │ (79% capacity) --> The hottest items. Promoted from Probation on hit.
// │ └─────────┘ │
// │      ▲      │
// │      │      │
// │ ┌─────────┐ │
// │ │Probation│ │ (20% capacity) --> New admissions land here. Victim P evicted if W is hotter.
// │ └─────────┘ │
// └─────────────┘
//
// Invariants:
// 1. The total items across Window, Probation, and Protected equals `capacity`.
// 2. An item is only in ONE of the three regions at any given time.
// 3. Every access (hit or miss) increments the Count-Min Sketch.
//
// Complexity:
// ┌───────────────┬─────────────┬─────────────┐
// │ Operation     │ Time        │ Space       │
// ├───────────────┼─────────────┼─────────────┤
// │ Get           │ O(1)*       │ O(capacity) │
// │ Put           │ O(1)*       │ O(capacity) │
// └───────────────┴─────────────┴─────────────┘
// * Sketch updates are O(depth), which is typically a small constant (e.g., 4).

/// Identifies which region a cached item currently resides in.
#[derive(Debug, PartialEq, Clone, Copy)]
enum Region {
    Window,
    Probation,
    Protected,
}

/// Represents a node in the arena-allocated doubly-linked list.
struct Node<K, V> {
    key: Option<K>,
    val: Option<V>,
    region: Region,
    prev: usize,
    next: usize,
}

/// W-TinyLFU Cache Implementation using an Arena Allocator.
pub struct WTinyLfuCache<K, V> {
    capacity: usize,

    // RUST INSIGHT: The Arena Pattern
    // Creating cyclic data structures (like three interconnected doubly-linked lists) in safe Rust
    // without `Rc<RefCell<T>>` overhead is achieved via an Arena. Nodes live in a `Vec`, and
    // "pointers" are just `usize` indices.
    nodes: Vec<Node<K, V>>,
    map: HashMap<K, usize>,
    free_list: Vec<usize>,

    // Count-Min Sketch for frequency estimation
    sketch: CountMinSketch<K>,

    // We need a mechanism to decay frequencies over time.
    // The standard approach halves all sketch counters when `sample_size` reaches `W`.
    sample_size: usize,
    window_size: usize, // e.g. 10 * capacity

    // List heads/tails for each region.
    // Dummy nodes are pre-allocated at initialization:
    // Window:    head = 0, tail = 1
    // Probation: head = 2, tail = 3
    // Protected: head = 4, tail = 5
    window_cap: usize,
    window_len: usize,

    probation_cap: usize,
    probation_len: usize,

    protected_cap: usize,
    protected_len: usize,
}

impl<K: Hash + Eq + Clone, V> WTinyLfuCache<K, V> {
    /// Creates a new `WTinyLfuCache` with the given capacity.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "Capacity must be greater than 0");

        // Sizing heuristics typical for W-TinyLFU:
        // Window: ~1% of capacity
        // Main: ~99% of capacity (80% Protected, 20% Probation)
        let window_cap = std::cmp::max(1, capacity / 100);
        let main_cap = std::cmp::max(1, capacity.saturating_sub(window_cap));
        let protected_cap = std::cmp::max(1, (main_cap * 80) / 100);
        let probation_cap = std::cmp::max(1, main_cap.saturating_sub(protected_cap));

        // Exact capacity might slightly differ due to integer division max(1) rounding,
        // but we enforce region caps locally.

        // Sketch width/depth tuned for the cache size.
        // A typical sketch has width ~ capacity and depth ~ 4.
        let width = std::cmp::max(128, capacity);
        let epsilon = 2.0 / (width as f64);
        let delta = 0.0625; // 1/16, which means depth = 4

        let mut nodes = Vec::with_capacity(capacity + 6);

        // Initialize dummy heads and tails for the 3 LRU lists.
        for region in [Region::Window, Region::Probation, Region::Protected] {
            let head_idx = nodes.len();
            let tail_idx = head_idx + 1;

            nodes.push(Node {
                key: None,
                val: None,
                region,
                prev: 0, // Placeholder
                next: tail_idx,
            });
            nodes.push(Node {
                key: None,
                val: None,
                region,
                prev: head_idx,
                next: 0, // Placeholder
            });
        }

        Self {
            capacity,
            nodes,
            map: HashMap::with_capacity(capacity),
            free_list: Vec::with_capacity(capacity),
            sketch: CountMinSketch::new(epsilon, delta),
            sample_size: 0,
            window_size: capacity * 10,
            window_cap,
            window_len: 0,
            probation_cap,
            probation_len: 0,
            protected_cap,
            protected_len: 0,
        }
    }

    // =========================================================================
    // List Operations (Internal)
    // =========================================================================

    /// Returns the dummy head index for a given region.
    fn head_idx(region: Region) -> usize {
        match region {
            Region::Window => 0,
            Region::Probation => 2,
            Region::Protected => 4,
        }
    }

    /// Returns the dummy tail index for a given region.
    fn tail_idx(region: Region) -> usize {
        match region {
            Region::Window => 1,
            Region::Probation => 3,
            Region::Protected => 5,
        }
    }

    /// Removes a node from its current doubly-linked list.
    fn remove_node(&mut self, idx: usize) {
        let prev = self.nodes[idx].prev;
        let next = self.nodes[idx].next;

        self.nodes[prev].next = next;
        self.nodes[next].prev = prev;
    }

    /// Adds a node to the front of the list (right after the dummy head) for its region.
    fn add_node_to_head(&mut self, idx: usize, region: Region) {
        let head = Self::head_idx(region);
        let next = self.nodes[head].next;

        self.nodes[idx].region = region;
        self.nodes[idx].prev = head;
        self.nodes[idx].next = next;

        self.nodes[head].next = idx;
        self.nodes[next].prev = idx;

        // Update lengths
        match region {
            Region::Window => self.window_len += 1,
            Region::Probation => self.probation_len += 1,
            Region::Protected => self.protected_len += 1,
        }
    }

    /// Moves an existing node to the front of its current region's list.
    fn move_to_head(&mut self, idx: usize) {
        self.remove_node(idx);
        let region = self.nodes[idx].region;

        // Temporarily adjust lengths since add_node_to_head increments them
        match region {
            Region::Window => self.window_len -= 1,
            Region::Probation => self.probation_len -= 1,
            Region::Protected => self.protected_len -= 1,
        }

        self.add_node_to_head(idx, region);
    }

    /// Pops the tail node (LRU position) from a region, returning its index.
    fn pop_tail(&mut self, region: Region) -> Option<usize> {
        let tail = Self::tail_idx(region);
        let lru_idx = self.nodes[tail].prev;

        let head = Self::head_idx(region);
        if lru_idx == head {
            return None; // List is empty
        }

        self.remove_node(lru_idx);

        match region {
            Region::Window => self.window_len -= 1,
            Region::Probation => self.probation_len -= 1,
            Region::Protected => self.protected_len -= 1,
        }

        Some(lru_idx)
    }

    // =========================================================================
    // Core Logic
    // =========================================================================

    /// Updates frequency in Count-Min Sketch and handles global halving.
    fn record_access(&mut self, key: &K) {
        self.sketch.add(key, 1);
        self.sample_size += 1;

        if self.sample_size >= self.window_size {
            // Decay process: halve all sketch counters
            // In a production implementation, this is often implemented dynamically or asynchronously.
            // Wait: CountMinSketch doesn't expose a halving method in our codebase.
            // PRODUCTION NOTE: We bypass actual decay here because `CountMinSketch` lacks a `halve_all` method.
            // A real W-TinyLFU cache (like Caffeine) uses a 4-bit CountMinSketch with a periodic halving process
            // to ensure stale hot items age out.
            self.sample_size = 0;
        }
    }

    /// Gets the value associated with the key, tracking its frequency and promoting it if necessary.
    #[must_use]
    pub fn get(&mut self, key: &K) -> Option<&V> {
        self.record_access(key);

        if let Some(&idx) = self.map.get(key) {
            let region = self.nodes[idx].region;

            if region == Region::Probation {
                // RUST INSIGHT: Match is exhaustive, but we handle the specific promotion logic.
                // Promote from Probation to Protected on second hit.
                self.remove_node(idx);
                self.probation_len -= 1;
                self.add_node_to_head(idx, Region::Protected);

                // If Protected is over capacity, demote its LRU to Probation
                if self.protected_len > self.protected_cap {
                    if let Some(demoted_idx) = self.pop_tail(Region::Protected) {
                        self.add_node_to_head(demoted_idx, Region::Probation);
                    }
                }
            } else {
                // For Window and Protected, just move to head (MRU)
                self.move_to_head(idx);
            }

            self.nodes[idx].val.as_ref()
        } else {
            None
        }
    }

    /// Puts a key-value pair into the cache, utilizing the W-TinyLFU admission policy.
    pub fn put(&mut self, key: K, value: V) {
        if self.capacity == 0 {
            return;
        }

        self.record_access(&key);

        if let Some(&idx) = self.map.get(&key) {
            // Update existing value and move to MRU of its region
            self.nodes[idx].val = Some(value);
            self.move_to_head(idx);

            // If it was in Probation, promote it
            if self.nodes[idx].region == Region::Probation {
                self.remove_node(idx);
                self.probation_len -= 1;
                self.add_node_to_head(idx, Region::Protected);

                if self.protected_len > self.protected_cap {
                    if let Some(demoted_idx) = self.pop_tail(Region::Protected) {
                        self.add_node_to_head(demoted_idx, Region::Probation);
                    }
                }
            }
            return;
        }

        // New item always goes to Window cache
        let new_idx = self.allocate_node(key.clone(), value);
        self.add_node_to_head(new_idx, Region::Window);
        self.map.insert(key, new_idx);

        // Enforce Window capacity
        if self.window_len > self.window_cap {
            if let Some(window_victim_idx) = self.pop_tail(Region::Window) {
                let window_victim_key = self.nodes[window_victim_idx].key.as_ref().unwrap().clone();

                // Attempt to admit Window Victim to Probation (Main Cache)
                self.admit_to_probation(window_victim_idx, window_victim_key);
            }
        }
    }

    /// Admission Policy: Compares the Window Victim against the Probation Victim using Sketch frequencies.
    fn admit_to_probation(&mut self, window_victim_idx: usize, window_victim_key: K) {
        // If Main cache (Probation + Protected) is not full, just add it to Probation
        if self.probation_len + self.protected_len < self.probation_cap + self.protected_cap {
            self.add_node_to_head(window_victim_idx, Region::Probation);
            return;
        }

        // Otherwise, find the LRU of the Probation segment
        // If Probation is empty, we must demote from Protected first
        if self.probation_len == 0 && self.protected_len > 0 {
            if let Some(demoted_idx) = self.pop_tail(Region::Protected) {
                self.add_node_to_head(demoted_idx, Region::Probation);
            }
        }

        // Peek at the Probation Victim
        let probation_tail = Self::tail_idx(Region::Probation);
        let probation_victim_idx = self.nodes[probation_tail].prev;

        // If Probation is still empty (capacity edge case), just discard window victim
        if probation_victim_idx == Self::head_idx(Region::Probation) {
            self.free_node(window_victim_idx);
            return;
        }

        let probation_victim_key = self.nodes[probation_victim_idx].key.as_ref().unwrap();

        // Retrieve estimated frequencies
        let freq_w = self.sketch.estimate(&window_victim_key);
        let freq_p = self.sketch.estimate(probation_victim_key);

        if freq_w > freq_p {
            // W is hotter than P. Evict P, Admit W.
            self.remove_node(probation_victim_idx);
            self.probation_len -= 1;
            self.free_node(probation_victim_idx);

            self.add_node_to_head(window_victim_idx, Region::Probation);
        } else {
            // P is hotter or equal to W. Keep P, Discard W.
            self.free_node(window_victim_idx);
        }
    }

    /// Allocates a new node either from the free list or by expanding the arena.
    fn allocate_node(&mut self, key: K, val: V) -> usize {
        if let Some(idx) = self.free_list.pop() {
            self.nodes[idx].key = Some(key);
            self.nodes[idx].val = Some(val);
            // Region/pointers are set during add_node_to_head
            idx
        } else {
            let idx = self.nodes.len();
            self.nodes.push(Node {
                key: Some(key),
                val: Some(val),
                region: Region::Window, // Placeholder
                prev: 0,
                next: 0,
            });
            idx
        }
    }

    /// Frees a node by removing it from the map and returning it to the free list.
    fn free_node(&mut self, idx: usize) {
        if let Some(k) = self.nodes[idx].key.take() {
            self.map.remove(&k);
        }
        self.nodes[idx].val = None;
        self.free_list.push(idx);
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `moka`: The leading Rust caching library uses a highly optimized concurrent W-TinyLFU with a 4-bit CMS.
// - `caffeine`: The Java inspiration for Moka.
//
// Missing vs. Production:
// - **Concurrency**: This implementation is single-threaded. Production systems like `moka` use sharding
//   and wait-free concurrent queues (lossy ring buffers) to record accesses asynchronously.
// - **Periodic Decay**: Real W-TinyLFU halves all sketch counters when the total access count reaches
//   a specific threshold (e.g. 10x capacity). We omit this here because our generic `CountMinSketch`
//   lacks a bulk mutation method.
// - **4-bit Sketch**: A standard `u64` per bucket is overkill. Real systems use a packed 4-bit array
//   to keep the admission filter extremely tiny.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wtinylfu_basic() {
        let mut cache = WTinyLfuCache::new(5);

        // Fill window
        cache.put(1, 10);
        cache.put(2, 20);
        cache.put(3, 30);
        cache.put(4, 40);
        cache.put(5, 50);

        assert_eq!(cache.get(&1), Some(&10));
        assert_eq!(cache.get(&2), Some(&20));

        // Scan resistance: A large number of one-time accesses shouldn't evict the hot items (1 and 2)
        for i in 100..120 {
            cache.put(i, i * 10);
        }

        // 1 and 2 should still be in the cache because they were hit multiple times
        // and have higher sketch frequencies than the one-time scan items.
        assert_eq!(cache.get(&1), Some(&10));
        assert_eq!(cache.get(&2), Some(&20));
    }

    #[test]
    fn test_update_existing_value() {
        let mut cache = WTinyLfuCache::new(2);
        cache.put(1, 10);
        cache.put(1, 20);

        assert_eq!(cache.get(&1), Some(&20));
    }
}
