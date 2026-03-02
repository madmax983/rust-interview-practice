//! # Concurrent Hash Map Implementation
//!
//! A thread-safe, highly concurrent hash map using lock sharding.
//!
//! **Replaces Crates:** `dashmap`, `chashmap`
//!
//! **Real-world Usage:**
//! - High-throughput in-memory caches.
//! - State management in multi-threaded web servers (e.g., active user sessions).
//! - Concurrent frequency counters.
//!
//! **Why build it yourself?**
//! Wrapping a standard `HashMap` in an `Arc<RwLock>` creates a massive bottleneck: any write locks out all other readers and writers.
//! Building a sharded concurrent map teaches you how to reduce lock contention by partitioning data.
//! You'll learn how to pre-hash keys, route them to specific shards, and understand the trade-offs of returning owned values versus lock guards.

use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::RwLock;

// =========================================================================================
// Architecture
// =========================================================================================
//
// Data Structure:
//
//      Hash(Key) % N
//           │
//           ▼
//      ┌─────────┐
//      │ Shard 0 │ ──► RwLock<HashMap<K, V>>
//      ├─────────┤
//      │ Shard 1 │ ──► RwLock<HashMap<K, V>>
//      ├─────────┤
//      │   ...   │
//      ├─────────┤
//      │ Shard N │ ──► RwLock<HashMap<K, V>>
//      └─────────┘
//
// Invariants:
// 1. A key always maps to the exact same shard.
// 2. Operations on different shards do not block each other.
// 3. Shard count should ideally be a power of 2 for performance, though any N > 0 works.
//
// Complexity:
// ┌───────────┬──────────────┬────────┐
// │ Operation │ Time         │ Space  │
// ├───────────┼──────────────┼────────┤
// │ get       │ O(1) expected│ O(1)   │
// │ insert    │ O(1) expected│ O(1)   │
// │ remove    │ O(1) expected│ O(1)   │
// └───────────┴──────────────┴────────┘
// Note: Time complexity includes acquiring the shard's lock, which may block depending on contention.
//
// Design Decisions:
// - **Sharding Factor**: Number of shards is fixed at creation. Resizing shards requires a global lock and rehashing, which we skip for simplicity.
// - **Return Types**: `get()` returns a cloned value instead of a lock guard.
//   - *Alternative*: Returning a custom `Ref` guard holding the `RwLockReadGuard` (like `dashmap`). This is complex due to self-referential lifetimes and can lead to deadlocks if the consumer holds the guard too long.

/// A concurrent map trait that allows for swappable implementations.
pub trait ConcurrentMap<K, V> {
    /// Inserts a key-value pair into the map.
    fn insert(&self, key: K, value: V);

    /// Returns a cloned copy of the value corresponding to the key.
    fn get(&self, key: &K) -> Option<V>
    where
        V: Clone;

    /// Removes a key from the map, returning the value at the key if it was present.
    fn remove(&self, key: &K) -> Option<V>;

    /// Checks if the map contains the specified key.
    fn contains_key(&self, key: &K) -> bool;
}

/// A concurrent, sharded hash map.
pub struct ConcurrentHashMap<K, V> {
    shards: Vec<RwLock<HashMap<K, V>>>,
    num_shards: usize,
}

impl<K: Hash + Eq, V> ConcurrentHashMap<K, V> {
    /// Creates a new `ConcurrentHashMap` with the specified number of shards.
    /// A power of 2 is recommended.
    #[must_use]
    pub fn new(num_shards: usize) -> Self {
        assert!(num_shards > 0, "Number of shards must be greater than 0");

        let mut shards = Vec::with_capacity(num_shards);
        for _ in 0..num_shards {
            shards.push(RwLock::new(HashMap::new()));
        }

        Self { shards, num_shards }
    }

    /// Default constructor creating 16 shards.
    #[must_use]
    pub fn default_shards() -> Self {
        Self::new(16)
    }

    /// Determines which shard should handle the given key.
    fn get_shard_index(&self, key: &K) -> usize {
        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        let hash = hasher.finish();

        // RUST INSIGHT: We use modulo arithmetic. If num_shards is a power of 2,
        // bitwise AND (hash & (num_shards - 1)) is faster. For simplicity, modulo is used here.
        (hash as usize) % self.num_shards
    }
}

impl<K: Hash + Eq, V> ConcurrentMap<K, V> for ConcurrentHashMap<K, V> {
    fn insert(&self, key: K, value: V) {
        let shard_idx = self.get_shard_index(&key);
        // GOTCHA: We must lock only the specific shard.
        let mut shard = self.shards[shard_idx].write().unwrap();
        shard.insert(key, value);
    }

    /// PRODUCTION NOTE: `dashmap` returns a `Ref` struct holding the `RwLockReadGuard`.
    /// Returning a clone here avoids potential deadlocks caused by consumers holding the guard
    /// for an extended time, but requires `V: Clone`.
    fn get(&self, key: &K) -> Option<V>
    where
        V: Clone,
    {
        let shard_idx = self.get_shard_index(key);
        let shard = self.shards[shard_idx].read().unwrap();
        shard.get(key).cloned()
    }

    fn remove(&self, key: &K) -> Option<V> {
        let shard_idx = self.get_shard_index(key);
        let mut shard = self.shards[shard_idx].write().unwrap();
        shard.remove(key)
    }

    fn contains_key(&self, key: &K) -> bool {
        let shard_idx = self.get_shard_index(key);
        let shard = self.shards[shard_idx].read().unwrap();
        shard.contains_key(key)
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `dashmap`: `dashmap` implements a highly optimized sharded hash map. It dynamically
//   manages resizing, returns safe reference guards (`Ref`, `RefMut`) instead of cloning,
//   and provides iterator support across all shards safely.
// - `chashmap`: Similar to `dashmap`, but with slightly different iteration and lock
//   semantics.
//
// Missing vs. Production:
// - **Zero-Copy Reads**: A true production map uses reference guards (`dashmap::Ref`) to
//   avoid requiring `V: Clone` and the allocation overhead of cloning values.
// - **Iterators**: We do not provide `.iter()` across shards. Implementing a concurrent
//   iterator involves careful multi-lock strategies or snapshotting.
// - **Resizing**: `std::collections::HashMap` manages its own internal resizing, but if
//   we wanted to dynamically grow the number of *shards*, we would need a global lock.
//
// Next Steps:
// 1. Implement `Ref` and `RefMut` wrapper structs holding the `RwLock` guards.
// 2. Add an iterator that snapshots keys or locks shards sequentially.
// 3. Optimize hashing (e.g., using `ahash` instead of `DefaultHasher`).
//
// Benchmarking Note:
// To benchmark this structure against `std::collections::HashMap` behind a single `RwLock`
// and the `dashmap` crate, use `criterion`. Create varying workloads (e.g., 90% reads / 10% writes,
// 50% reads / 50% writes) and scale the number of threads from 1 to `num_cpus`. You will observe that
// the single `RwLock` falls over quickly under contention, while `ConcurrentHashMap` maintains throughput.
// Utilize `std::hint::black_box` to prevent the compiler from optimizing away reads.

impl<K: Hash + Eq, V> Default for ConcurrentHashMap<K, V> {
    fn default() -> Self {
        Self::default_shards()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn test_insert_and_get() {
        let map = ConcurrentHashMap::new(4);
        map.insert("key1".to_string(), 100);
        map.insert("key2".to_string(), 200);

        assert_eq!(map.get(&"key1".to_string()), Some(100));
        assert_eq!(map.get(&"key2".to_string()), Some(200));
        assert_eq!(map.get(&"key3".to_string()), None);
    }

    #[test]
    fn test_remove() {
        let map = ConcurrentHashMap::default();
        map.insert(1, "one");
        assert_eq!(map.get(&1), Some("one"));

        let removed = map.remove(&1);
        assert_eq!(removed, Some("one"));
        assert_eq!(map.get(&1), None);
    }

    #[test]
    fn test_contains_key() {
        let map = ConcurrentHashMap::new(8);
        map.insert("rust", "fast");

        assert!(map.contains_key(&"rust"));
        assert!(!map.contains_key(&"python"));
    }

    #[test]
    fn test_concurrent_inserts() {
        let map = Arc::new(ConcurrentHashMap::new(8));
        let mut handles = vec![];

        for i in 0..10 {
            let map_clone = Arc::clone(&map);
            handles.push(thread::spawn(move || {
                for j in 0..100 {
                    let key = i * 100 + j;
                    map_clone.insert(key, key * 2);
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        for i in 0..10 {
            for j in 0..100 {
                let key = i * 100 + j;
                assert_eq!(map.get(&key), Some(key * 2));
            }
        }
    }

    #[test]
    fn test_concurrent_reads_and_writes() {
        let map = Arc::new(ConcurrentHashMap::new(16));

        // Pre-populate some keys
        for i in 0..500 {
            map.insert(i, i.to_string());
        }

        let mut handles = vec![];

        // Writers
        for i in 500..1000 {
            let map_clone = Arc::clone(&map);
            handles.push(thread::spawn(move || {
                map_clone.insert(i, i.to_string());
            }));
        }

        // Readers
        for i in 0..500 {
            let map_clone = Arc::clone(&map);
            handles.push(thread::spawn(move || {
                let val = map_clone.get(&i);
                assert_eq!(val, Some(i.to_string()));
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        // Verify writers succeeded
        for i in 500..1000 {
            assert_eq!(map.get(&i), Some(i.to_string()));
        }
    }

    #[test]
    #[should_panic(expected = "Number of shards must be greater than 0")]
    fn test_zero_shards_panics() {
        let _map: ConcurrentHashMap<i32, i32> = ConcurrentHashMap::new(0);
    }
}
