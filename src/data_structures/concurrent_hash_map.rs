//! # Concurrent Hash Map Implementation
//!
//! Implements a thread-safe, sharded hash map for highly concurrent reads and writes.
//!
//! **Recrates Replaced:** `dashmap`, `chashmap`, `scc` (partially)
//!
//! **Real-world Usage:**
//! - High-throughput web server session stores (e.g., maintaining state across concurrent requests).
//! - In-memory caches shared across many worker threads.
//! - Metrics aggregation engines where multiple threads update counters simultaneously.
//!
//! **Why build it yourself?**
//! Building a concurrent hash map teaches you how to balance consistency, contention, and complexity.
//! You'll learn why a single `RwLock<HashMap>` becomes a bottleneck and how "lock sharding" (segmenting
//! the data into multiple independent locks) drastically improves throughput. It also forces you to
//! navigate Rust's borrowing rules in a concurrent context, particularly why returning references
//! (like `Ref` or `RefMut`) out of locked collections is incredibly difficult and often deadlock-prone
//! without advanced lifetimes and guard objects.
//!
//! # Architecture
//!
//! **Data Structure Diagram:**
//!
//! ```text
//! ConcurrentHashMap
//! ├── Shard 0: RwLock<HashMap<K, V>>
//! ├── Shard 1: RwLock<HashMap<K, V>>
//! ├── ...
//! └── Shard N: RwLock<HashMap<K, V>>
//! ```
//!
//! **Invariants:**
//! 1. A key `K` must always hash to the same shard `N`.
//! 2. The number of shards must be a power of two (optional but common for fast modulo via bitwise AND).
//! 3. No thread may hold locks on multiple shards simultaneously unless a strict global ordering is
//!    enforced (to prevent deadlocks). This implementation avoids multiple locks entirely.
//!
//! **Complexity:**
//! | Operation | Time (Average) | Time (Worst Case) | Space | Contention |
//! |-----------|----------------|-------------------|-------|------------|
//! | `get`     | O(1)           | O(N) in a shard   | O(1)  | Low (Shared Read Lock) |
//! | `insert`  | O(1)           | O(N) in a shard   | O(1)  | Medium (Exclusive Write Lock on 1 shard) |
//! | `remove`  | O(1)           | O(N) in a shard   | O(1)  | Medium (Exclusive Write Lock on 1 shard) |
//! | `len`     | O(S)           | O(S)              | O(1)  | High (Locks all shards) |
//! *(Where `S` is the number of shards, and `N` is the number of elements in a single shard).*
//!
//! **Design Decisions & Tradeoffs:**
//! - **Cloning vs. Guards:** We return `Option<V>` (requiring `V: Clone`) instead of returning
//!   a lock guard (like `dashmap::Ref`).
//!   - *Tradeoff:* Returning cloned values avoids complex, lifetime-bound lock guards that can
//!     trivially cause deadlocks if a user holds a read guard while trying to write to the same shard.
//!     However, it incurs a performance penalty if `V` is large to clone.
//! - **Hashing:** We use a `DefaultHasher` to determine the shard index.
//! - **Shard Count:** A fixed number of shards (default 64) is used to balance memory overhead vs. contention.

use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::RwLock;

/// The trait defining operations for a concurrent map.
pub trait ConcurrentMap<K, V> {
    /// Inserts a key-value pair into the map.
    fn insert(&self, key: K, value: V) -> Option<V>;

    /// Retrieves a clone of the value associated with the key.
    fn get(&self, key: &K) -> Option<V>;

    /// Removes a key from the map, returning its value if it existed.
    fn remove(&self, key: &K) -> Option<V>;

    /// Returns the total number of elements across all shards.
    fn len(&self) -> usize;

    /// Returns `true` if the map contains no elements.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// A highly concurrent, sharded hash map.
pub struct ConcurrentHashMap<K, V> {
    shards: Vec<RwLock<HashMap<K, V>>>,
    shard_count: usize,
}

impl<K, V> ConcurrentHashMap<K, V> {
    /// Creates a new `ConcurrentHashMap` with a default number of shards (64).
    #[must_use]
    pub fn new() -> Self {
        Self::with_shards(64)
    }

    /// Creates a new `ConcurrentHashMap` with the specified number of shards.
    ///
    /// # Panics
    /// Panics if `shard_count` is 0.
    #[must_use]
    pub fn with_shards(shard_count: usize) -> Self {
        assert!(shard_count > 0, "Shard count must be greater than 0");

        // RUST INSIGHT: We use `Vec::with_capacity` and a loop instead of `vec![RwLock::new(HashMap::new()); shard_count]`
        // because `RwLock<HashMap<K, V>>` does not implement `Clone`. This is a deliberate
        // design in Rust: synchronization primitives shouldn't be implicitly cloned, as that
        // would create entirely new, unlinked locks rather than sharing the existing one.
        let mut shards = Vec::with_capacity(shard_count);
        for _ in 0..shard_count {
            shards.push(RwLock::new(HashMap::new()));
        }

        Self {
            shards,
            shard_count,
        }
    }

    /// Calculates the shard index for a given key.
    fn get_shard_index(&self, key: &K) -> usize
    where
        K: Hash,
    {
        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        let hash = hasher.finish();

        // Use modulo to find the shard. If shard_count is a power of 2,
        // a bitwise AND (`hash & (shard_count - 1)`) would be faster.
        (hash as usize) % self.shard_count
    }
}

impl<K, V> Default for ConcurrentHashMap<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K, V> ConcurrentMap<K, V> for ConcurrentHashMap<K, V>
where
    K: Hash + Eq,
    V: Clone,
{
    fn insert(&self, key: K, value: V) -> Option<V> {
        let shard_idx = self.get_shard_index(&key);

        // GOTCHA: We must acquire a write lock here. If another thread is holding
        // a read lock OR a write lock on THIS SPECIFIC SHARD, we will block.
        // However, threads accessing other shards proceed without any contention.
        let mut shard = self.shards[shard_idx]
            .write()
            .expect("RwLock poisoned (a thread panicked while holding the write lock)");

        shard.insert(key, value)
    }

    fn get(&self, key: &K) -> Option<V> {
        let shard_idx = self.get_shard_index(key);

        // RUST INSIGHT: We only need a read lock here. Multiple threads can read
        // from the same shard simultaneously.
        let shard = self.shards[shard_idx].read().expect("RwLock poisoned");

        // PRODUCTION NOTE: We return `cloned()` here. A true production crate like `dashmap`
        // returns a custom `Ref<'a, K, V>` type that holds the lock guard and a reference
        // to the value. We avoid that because it leads to complex lifetimes and potential
        // deadlocks if the user isn't careful. By cloning, we trade a little performance
        // (depending on `V`) for immense API simplicity and safety.
        shard.get(key).cloned()
    }

    fn remove(&self, key: &K) -> Option<V> {
        let shard_idx = self.get_shard_index(key);

        let mut shard = self.shards[shard_idx].write().expect("RwLock poisoned");

        shard.remove(key)
    }

    fn len(&self) -> usize {
        // GOTCHA: Calculating the exact length of a concurrent map is inherently racy.
        // By the time this function returns, another thread might have inserted/removed items.
        // We lock and read each shard sequentially, meaning the total count represents
        // a loose "point in time" estimate, not a strict transactional snapshot, unless
        // we lock ALL shards simultaneously (which would destroy throughput).
        self.shards
            .iter()
            .map(|shard| shard.read().expect("RwLock poisoned").len())
            .sum()
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `dashmap`: DashMap is the standard for concurrent hash maps in Rust. It uses highly
//   optimized locking mechanisms (often parking_lot), returns lock guards (`Ref`/`RefMut`)
//   to avoid cloning, and handles rehashing/resizing transparently without global locking.
// - `scc`: Scalable Concurrent Collections use lock-free or wait-free techniques,
//   relying heavily on atomic operations and hazard pointers rather than sharded locks.
//
// Missing vs. Production:
// - **Guards instead of Clones:** Real implementations use advanced unsafe Rust to return
//   a guard object so you don't have to require `V: Clone`.
// - **Adaptive Sharding:** Hardcoding 64 shards is naive. Production maps adjust the
//   shard count based on CPU cores or contention.
// - **Iteration:** We omitted `iter()`. Implementing iteration over a concurrent map requires
//   either snapshotting the entire map (expensive) or yielding items while incrementally
//   locking shards (which can yield duplicate or missed items if concurrent resizing happens).
// - **Advanced Hashing:** We use `DefaultHasher` which is slow (SipHash) but DoS resistant.
//   `ahash` or `fxhash` are often preferred for internal caches.
//
// Next Steps:
// 1. Swap `std::sync::RwLock` for `parking_lot::RwLock` for better performance.
// 2. Add an `update` method that takes a closure to modify a value in place (avoiding the get-modify-insert race condition).
// 3. Implement `iter()` that locks one shard at a time.

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn test_basic_operations() {
        let map = ConcurrentHashMap::new();

        assert_eq!(map.len(), 0);
        assert!(map.is_empty());

        assert_eq!(map.insert("a", 1), None);
        assert_eq!(map.insert("b", 2), None);

        assert_eq!(map.len(), 2);

        assert_eq!(map.get(&"a"), Some(1));
        assert_eq!(map.get(&"b"), Some(2));
        assert_eq!(map.get(&"c"), None);

        assert_eq!(map.insert("a", 10), Some(1)); // Overwrite
        assert_eq!(map.get(&"a"), Some(10));

        assert_eq!(map.remove(&"b"), Some(2));
        assert_eq!(map.get(&"b"), None);
        assert_eq!(map.len(), 1);
    }

    #[test]
    fn test_concurrent_inserts() {
        let map = Arc::new(ConcurrentHashMap::new());
        let mut handles = vec![];

        let thread_count = 10;
        let inserts_per_thread = 1000;

        for i in 0..thread_count {
            let map_clone = Arc::clone(&map);
            let handle = thread::spawn(move || {
                for j in 0..inserts_per_thread {
                    let key = format!("key_{}_{}", i, j);
                    map_clone.insert(key, i * j);
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        assert_eq!(map.len(), thread_count * inserts_per_thread);

        // Verify a specific key
        assert_eq!(map.get(&"key_5_500".to_string()), Some(5 * 500));
    }

    #[test]
    fn test_concurrent_reads_and_writes() {
        let map = Arc::new(ConcurrentHashMap::with_shards(16));

        // Pre-populate
        for i in 0..100 {
            map.insert(i, i * 10);
        }

        let mut handles = vec![];

        // Writers
        for i in 100..200 {
            let map_clone = Arc::clone(&map);
            handles.push(thread::spawn(move || {
                map_clone.insert(i, i * 10);
            }));
        }

        // Readers
        for i in 0..100 {
            let map_clone = Arc::clone(&map);
            handles.push(thread::spawn(move || {
                // It's guaranteed to be there because we pre-populated it
                assert_eq!(map_clone.get(&i), Some(i * 10));
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        assert_eq!(map.len(), 200);
    }

    /// Example of how you would benchmark this vs a standard Mutex<HashMap>
    /// using standard library tools. For serious benchmarking, use `criterion`.
    #[test]
    #[ignore = "Run manually as a basic benchmark: cargo test --release -- --ignored test_benchmark_note"]
    fn test_benchmark_note() {
        use std::time::Instant;

        let map = Arc::new(ConcurrentHashMap::with_shards(64));
        let mut handles = vec![];
        let start = Instant::now();

        for i in 0..8 {
            let map_clone = Arc::clone(&map);
            handles.push(thread::spawn(move || {
                for j in 0..100_000 {
                    let key = i * 100_000 + j;
                    map_clone.insert(key, j);
                    std::hint::black_box(map_clone.get(&key));
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        println!("Sharded map took: {:?}", start.elapsed());
    }
}
