//! # W-TinyLFU Cache Implementation
//!
//! Implements a state-of-the-art caching algorithm that combines the scan-resistance of LFU
//! with the recent-event handling of LRU. W-TinyLFU (Window Tiny Least Frequently Used)
//! represents a modern approach to cache admission and eviction policies.
//!
//! **Replaces Crates:** `moka`, `caffeine` (Java)
//!
//! **Real-world Usage:**
//! - High-performance JVM caches (Caffeine)
//! - Rust async caches (Moka)
//! - Database query caches
//! - Content Delivery Networks (CDNs)
//!
//! **Why build it yourself?**
//! W-TinyLFU is a fascinating algorithm that uses an embedded CountMinSketch for frequency estimation,
//! achieving LFU-like performance with minimal memory overhead. Implementing this teaches you how
//! to build a complex, multi-component cache architecture without relying on unsafe pointers or
//! slow `Rc<RefCell>` wrappers. You will learn the Arena-allocator pattern for managing complex
//! graph-like structures (doubly linked lists) purely through indices.

// =========================================================================================
// Architecture
// =========================================================================================
//
// The W-TinyLFU cache consists of three main components:
// 1. A CountMinSketch for frequency estimation.
// 2. A Window LRU (W-LRU) for recent items.
// 3. A Main Space (Probabilistic Cache) divided into Probation and Protected segments.
//
// Admission Policy:
// When an item is evicted from the Window LRU, it must compete with the victim of the
// Main Space. The item with the higher estimated frequency (via CountMinSketch) is kept.
//
// Data Structure Diagram:
//
// Incoming Items
//       │
//       ▼
// ┌───────────┐    Evicted
// │ Window LRU├─────────────┐
// └───────────┘             │
//                           ▼
//                     ┌───────────┐
//                     │ Admission │◄── CountMinSketch Frequency
//                     └─────┬─────┘
//                           │
//       ┌───────────────────┴───────────────────┐
//       ▼                                       │
// ┌───────────┐       Promote             ┌─────┴─────┐
// │ Protected │◄──────────────────────────┤ Probation │
// └───────────┘                           └───────────┘
//       │                Demote                 │
//       └───────────────────────────────────────┘
//                                               │
//                                            Evicted
//
// Invariants:
// 1. The total size of all segments combined does not exceed max capacity.
// 2. The Window LRU is bounded (typically 1% of capacity).
// 3. The Main Space is bounded (typically 99% of capacity).
// 4. The Protected segment is bounded (typically 80% of Main Space).
//
// Complexity:
// ┌───────────┬────────┬──────────────────────────┐
// │ Operation │ Time   │ Space                    │
// ├───────────┼────────┼──────────────────────────┤
// │ get       │ O(1)   │ O(1) per item            │
// │ put       │ O(1)   │ O(1) per item            │
// └───────────┴────────┴──────────────────────────┘
//
// Design Decisions:
// - **Arena Allocation**: We use a central vector `entries: Vec<Entry>` to store items.
//   Nodes form a doubly linked list using `usize` indices instead of `NonNull` or `Rc`.
//   This is 100% safe Rust and highly cache-friendly.
// - **CountMinSketch**: Uses a 4-bit counter array to save memory, allowing decay via bitwise shift.

use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// A generalized Cache trait to enable swappable caching strategies.
pub trait Cache<K, V> {
    fn get(&mut self, key: &K) -> Option<&V>;
    fn put(&mut self, key: K, val: V);
}

/// A 4-bit CountMinSketch for frequency estimation.
/// It uses multiple hash functions to estimate the frequency of events.
/// The 4-bit counters allow for memory efficiency and easy decay (divide by 2 via shift).
pub struct CountMinSketch {
    // Array of 64-bit integers. Each u64 holds 16 x 4-bit counters.
    table: Vec<u64>,
    width: usize,
    sample_size: usize,
    additions: usize,
}

impl CountMinSketch {
    pub fn new(capacity: usize) -> Self {
        // RUST INSIGHT: We size the sketch relative to the cache capacity to bound error.
        // A typical heuristic is a width of 4-8x the capacity.
        let width = std::cmp::max(capacity.next_power_of_two() * 8, 16);
        let num_u64 = width.div_ceil(16);
        Self {
            table: vec![0; num_u64],
            width,
            sample_size: capacity * 10,
            additions: 0,
        }
    }

    /// Hashes the key to 4 different positions.
    fn hashes<K: Hash>(key: &K) -> [usize; 4] {
        // PRODUCTION NOTE: For a real cache, you would use a faster, non-cryptographic
        // hasher like `ahash` or `xxhash`.
        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        let hash1 = hasher.finish();

        // Perturb the hash to get a second independent hash
        hasher.write_u64(0x9e3779b97f4a7c15);
        let hash2 = hasher.finish();

        // Generate 4 independent indices using linear combinations to cover full usize space
        // hash_i = hash1 + i * hash2
        [
            hash1 as usize,
            hash1.wrapping_add(hash2) as usize,
            hash1.wrapping_add(hash2.wrapping_mul(2)) as usize,
            hash1.wrapping_add(hash2.wrapping_mul(3)) as usize,
        ]
    }

    /// Increments the frequency of the given key.
    pub fn add<K: Hash>(&mut self, key: &K) {
        let h = Self::hashes(key);
        for &idx in &h {
            let index = idx % self.width;
            let u64_idx = index / 16;
            let shift = (index % 16) * 4;

            let val = (self.table[u64_idx] >> shift) & 0xF;
            if val < 15 {
                // Increment without carrying over to the next 4-bit block
                self.table[u64_idx] += 1 << shift;
            }
        }

        self.additions += 1;
        if self.additions >= self.sample_size {
            self.reset();
        }
    }

    /// Estimates the frequency of the given key.
    pub fn estimate<K: Hash>(&self, key: &K) -> u8 {
        let h = Self::hashes(key);
        let mut min_val = 15;
        for &idx in &h {
            let index = idx % self.width;
            let u64_idx = index / 16;
            let shift = (index % 16) * 4;
            let val = ((self.table[u64_idx] >> shift) & 0xF) as u8;
            if val < min_val {
                min_val = val;
            }
        }
        min_val
    }

    /// Decays all counters by half (shift right by 1).
    fn reset(&mut self) {
        // GOTCHA: We must shift each 4-bit block individually, but we can do it in parallel
        // using bitwise operations on the whole u64!
        // Mask for the 4th bit of each 4-bit block: 0x8888...
        // Mask for the lower 3 bits: 0x7777...
        for val in &mut self.table {
            // Shift right by 1, and clear the top bit of each 4-bit nibble so we don't bleed
            *val = (*val >> 1) & 0x7777777777777777;
        }
        self.additions /= 2;
    }
}

/// Identifies which segment an entry belongs to.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum Segment {
    Window,
    Probation,
    Protected,
}

/// A node in the doubly linked list, managed via Arena allocation.
struct Entry<K, V> {
    key: K,
    val: V,
    segment: Segment,
    prev: usize,
    next: usize,
}

/// A doubly-linked list managed via indices in the Arena.
#[derive(Default)]
struct List {
    head: usize,
    tail: usize,
    len: usize,
}

pub struct WTinyLfuCache<K, V> {
    capacity: usize,

    // Limits
    window_capacity: usize,
    protected_capacity: usize,

    // Storage
    // RUST INSIGHT: The Arena allocator pattern! Instead of self-referential
    // pointer structures, we use `Vec` indices. This provides 100% safe Rust,
    // excellent cache locality, and zero reference counting overhead.
    entries: Vec<Entry<K, V>>,
    free_list: Vec<usize>,
    map: HashMap<K, usize>,

    // Lists
    window: List,
    probation: List,
    protected: List,

    // Frequency estimator
    sketch: CountMinSketch,
}

const NULL: usize = usize::MAX;

impl<K: Hash + Eq + Clone, V> WTinyLfuCache<K, V> {
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "Capacity must be greater than 0");

        // PRODUCTION NOTE: W-TinyLFU typically assigns 1% to Window and 99% to Main.
        // Within Main, 80% goes to Protected and 20% to Probation.
        // For small capacities, we ensure at least 1 item per segment.
        let window_capacity = std::cmp::max(1, capacity / 100);
        let main_capacity = capacity - window_capacity;
        let protected_capacity = std::cmp::max(1, (main_capacity * 80) / 100);

        Self {
            capacity,
            window_capacity,
            protected_capacity,
            entries: Vec::with_capacity(capacity + 1), // +1 for transient victims
            free_list: Vec::new(),
            map: HashMap::with_capacity(capacity),
            window: List {
                head: NULL,
                tail: NULL,
                len: 0,
            },
            probation: List {
                head: NULL,
                tail: NULL,
                len: 0,
            },
            protected: List {
                head: NULL,
                tail: NULL,
                len: 0,
            },
            sketch: CountMinSketch::new(capacity),
        }
    }

    /// Links an entry to the front of the specified list.
    fn link_front(&mut self, list_type: Segment, idx: usize) {
        let list = match list_type {
            Segment::Window => &mut self.window,
            Segment::Probation => &mut self.probation,
            Segment::Protected => &mut self.protected,
        };

        self.entries[idx].prev = NULL;
        self.entries[idx].next = list.head;
        self.entries[idx].segment = list_type;

        if list.head != NULL {
            self.entries[list.head].prev = idx;
        }
        list.head = idx;
        if list.tail == NULL {
            list.tail = idx;
        }
        list.len += 1;
    }

    /// Unlinks an entry from its current list.
    fn unlink(&mut self, idx: usize) {
        let entry = &self.entries[idx];
        let prev = entry.prev;
        let next = entry.next;
        let segment = entry.segment;

        let list = match segment {
            Segment::Window => &mut self.window,
            Segment::Probation => &mut self.probation,
            Segment::Protected => &mut self.protected,
        };

        if prev != NULL {
            self.entries[prev].next = next;
        } else {
            list.head = next;
        }

        if next != NULL {
            self.entries[next].prev = prev;
        } else {
            list.tail = prev;
        }

        list.len -= 1;
    }

    /// Moves an entry to the front of its current list.
    fn move_to_front(&mut self, idx: usize) {
        let segment = self.entries[idx].segment;
        self.unlink(idx);
        self.link_front(segment, idx);
    }

    /// Frees an entry, removing it from the map and returning its index to the free list.
    fn free_entry(&mut self, idx: usize) {
        self.unlink(idx);
        let key = self.entries[idx].key.clone();
        self.map.remove(&key);
        self.free_list.push(idx);
    }

    /// The W-TinyLFU eviction policy.
    fn evict(&mut self) {
        // If window is over capacity, move its tail to probation
        if self.window.len > self.window_capacity {
            let window_tail = self.window.tail;
            self.unlink(window_tail);
            self.link_front(Segment::Probation, window_tail);
        }

        // If total size is within capacity, we are done
        if self.map.len() <= self.capacity {
            return;
        }

        // We need to evict one item.
        // The candidate is the tail of the probation list.
        let candidate_idx = self.probation.tail;
        if candidate_idx == NULL {
            // Should not happen if size > capacity, but fallback to window
            let window_tail = self.window.tail;
            self.free_entry(window_tail);
            return;
        }

        // W-TinyLFU Admission:
        // We have an item just demoted from Window (now at head of probation, or incoming)
        // competing with the Probation tail.
        // But W-TinyLFU standard policy states: incoming item competes with probation victim.
        // Since we already inserted the incoming item into Window, the true victim is the probation tail.
        // Wait, if Window overflowed, it pushed to Probation.
        // We evaluate if the Probation head (newcomer) is better than Probation tail (victim).
        // Actually, Moka and Caffeine do admission *before* inserting into the Main Space.
        // Here we simulate it: The victim is `probation.tail`. We need to free someone.

        // To simplify: we compare probation.head (most recently pushed from window)
        // with probation.tail (LRU of probation).
        let newcomer_idx = self.probation.head;
        if newcomer_idx != candidate_idx {
            let newcomer_freq = self.sketch.estimate(&self.entries[newcomer_idx].key);
            let victim_freq = self.sketch.estimate(&self.entries[candidate_idx].key);

            if newcomer_freq < victim_freq {
                // Reject newcomer
                self.free_entry(newcomer_idx);
                return;
            }
        }

        // Otherwise, evict the candidate (victim)
        self.free_entry(candidate_idx);
    }
}

impl<K: Hash + Eq + Clone, V> Cache<K, V> for WTinyLfuCache<K, V> {
    fn get(&mut self, key: &K) -> Option<&V> {
        self.sketch.add(key);

        if let Some(&idx) = self.map.get(key) {
            let segment = self.entries[idx].segment;

            if segment == Segment::Probation {
                // Promote to Protected
                self.unlink(idx);
                self.link_front(Segment::Protected, idx);

                // If Protected is full, demote its tail to Probation
                if self.protected.len > self.protected_capacity {
                    let protected_tail = self.protected.tail;
                    self.unlink(protected_tail);
                    self.link_front(Segment::Probation, protected_tail);
                }
            } else {
                // Already in Window or Protected, just move to front
                self.move_to_front(idx);
            }

            Some(&self.entries[idx].val)
        } else {
            None
        }
    }

    fn put(&mut self, key: K, val: V) {
        self.sketch.add(&key);

        if let Some(&idx) = self.map.get(&key) {
            self.entries[idx].val = val;
            let segment = self.entries[idx].segment;
            if segment == Segment::Probation {
                self.unlink(idx);
                self.link_front(Segment::Protected, idx);
                if self.protected.len > self.protected_capacity {
                    let protected_tail = self.protected.tail;
                    self.unlink(protected_tail);
                    self.link_front(Segment::Probation, protected_tail);
                }
            } else {
                self.move_to_front(idx);
            }
            return;
        }

        let idx = if let Some(free_idx) = self.free_list.pop() {
            self.entries[free_idx] = Entry {
                key: key.clone(),
                val,
                segment: Segment::Window,
                prev: NULL,
                next: NULL,
            };
            free_idx
        } else {
            self.entries.push(Entry {
                key: key.clone(),
                val,
                segment: Segment::Window,
                prev: NULL,
                next: NULL,
            });
            self.entries.len() - 1
        };

        self.map.insert(key, idx);
        self.link_front(Segment::Window, idx);

        self.evict();
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// **Canonical Crate Comparison:**
// - `moka`: A highly concurrent cache in Rust that uses W-TinyLFU. It uses asynchronous
//   maintenance threads and ring buffers (BP-Wrapper) to avoid blocking the caller. Our
//   implementation is synchronous.
// - `caffeine`: The Java gold standard. Uses similar concepts but relies on the JVM
//   garbage collector and handles concurrency heavily.
//
// **Missing Production Features:**
// - **Concurrency**: This implementation is `&mut self` and single-threaded. Production
//   caches use striped locks or background queues.
// - **Expirations**: No TTL or time-based eviction.
// - **Weighting**: Assumes all items have a weight of 1.
//
// **Next Steps:**
// - Implement a background thread that processes gets/puts via channels to make it concurrent.
// - Add TTL tracking per entry.
// - Benchmarking: Use `criterion` to compare hit rates against a standard LRU cache under Zipfian distribution workloads.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sketch() {
        let mut sketch = CountMinSketch::new(10);
        sketch.add(&"apple");
        sketch.add(&"apple");
        sketch.add(&"banana");

        assert_eq!(sketch.estimate(&"apple"), 2);
        assert_eq!(sketch.estimate(&"banana"), 1);
        assert_eq!(sketch.estimate(&"cherry"), 0);
    }

    #[test]
    fn test_cache_basic() {
        let mut cache = WTinyLfuCache::new(3);

        cache.put(1, 10);
        cache.put(2, 20);
        cache.put(3, 30);

        assert_eq!(cache.get(&1), Some(&10));
        assert_eq!(cache.get(&2), Some(&20));
        assert_eq!(cache.get(&3), Some(&30));

        // Push over capacity
        cache.put(4, 40);
        // We know 1, 2, 3 were accessed, so 4 might be evicted immediately if its
        // frequency is lower than the probation victim. However, if 4 is accessed
        // or the cache behaves dynamically, we just verify size bounds.
        assert!(cache.map.len() <= 3);
    }

    #[test]
    fn test_frequency_survival() {
        let mut cache = WTinyLfuCache::new(5);

        // Build up frequency for 1 and 2
        for _ in 0..5 {
            cache.put(1, 10);
            cache.put(2, 20);
        }

        cache.put(3, 30);
        cache.put(4, 40);
        cache.put(5, 50);

        // Push a bunch of one-off items
        cache.put(6, 60);
        cache.put(7, 70);
        cache.put(8, 80);
        cache.put(9, 90);

        // 1 and 2 should survive due to high frequency
        assert_eq!(cache.get(&1), Some(&10));
        assert_eq!(cache.get(&2), Some(&20));
    }
}
