//! # Concurrent LRU Cache Implementation
//!
//! A thread-safe, sharded LRU cache implementation.
//!
//! **Replaces Crates:** `moka`, `dashmap` (partial functionality)
//!
//! **Real-world Usage:**
//! - High-concurrency web servers (caching session data, API responses).
//! - Database query results.
//!
//! **Why build it yourself?**
//! A single global lock (Mutex) on an LRU cache creates a massive bottleneck because every read (get)
//! implies a write (updating the linked list order). Sharding splits the key space into `N` independent
//! segments, reducing contention by a factor of `N`. You'll learn how to map keys to shards and
//! coordinate locking.

// Truncating a 64-bit hash to a shard index is intentional.
#![allow(clippy::cast_possible_truncation)]

use crate::systems::lru_cache::LRUCache;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Mutex;

// =========================================================================================
// Architecture
// =========================================================================================
//
// Sharding:
// - The key space is partitioned into `num_shards` segments.
// - Each shard is an independent `LRUCache` protected by a `Mutex`.
// - `shard_index = hash(key) % num_shards`.
//
// Invariants:
// 1. Total capacity ~= sum of shard capacities.
// 2. Keys map deterministically to the same shard.
//
// Tradeoffs:
// - **Sharding vs Global Lock**: Reduces contention but doesn't eliminate it. Hot keys can still bottleneck a single shard.
// - **LRU Accuracy**: Strictly enforcing global LRU is impossible with sharding. We enforce local LRU per shard.
//   This is usually a good approximation (random load balancing).

pub struct ConcurrentLruCache<K, V> {
    shards: Vec<Mutex<LRUCache<K, V>>>,
    num_shards: usize,
}

impl<K: Hash + Eq + Clone + Send + 'static, V: Send + 'static> ConcurrentLruCache<K, V> {
    /// Creates a new Concurrent LRU Cache.
    /// `capacity` is the total capacity across all shards.
    /// `num_shards` should be a power of two for better distribution (though we use modulo here for simplicity).
    ///
    /// # Panics
    /// Panics if `capacity` or `num_shards` is 0.
    #[must_use]
    pub fn new(capacity: usize, num_shards: usize) -> Self {
        assert!(capacity > 0, "Capacity must be greater than 0");
        assert!(num_shards > 0, "Number of shards must be greater than 0");

        let shard_capacity = capacity.div_ceil(num_shards); // Ceiling division
        let mut shards = Vec::with_capacity(num_shards);

        for _ in 0..num_shards {
            shards.push(Mutex::new(LRUCache::new(shard_capacity)));
        }

        Self { shards, num_shards }
    }

    fn get_shard_index(&self, key: &K) -> usize {
        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        (hasher.finish() as usize) % self.num_shards
    }

    /// Gets the value associated with the key.
    ///
    /// # Panics
    /// Panics if the shard's mutex is poisoned (a thread panicked while holding the lock).
    pub fn get(&self, key: &K) -> Option<V>
    where
        V: Clone,
    {
        // RUST INSIGHT: We require V: Clone here because we can't return a reference `&V`
        // that outlives the MutexGuard of the shard. To return a reference, we'd need
        // to return a custom guard type (like `dashmap` does), which is complex.
        // Cloning is the safe, idiomatic path for simple concurrent caches.

        let idx = self.get_shard_index(key);
        let mut shard = self.shards[idx].lock().unwrap();
        shard.get(key).cloned()
    }

    /// Inserts a key-value pair into the cache.
    ///
    /// # Panics
    /// Panics if the shard's mutex is poisoned (a thread panicked while holding the lock).
    pub fn put(&self, key: K, val: V) {
        let idx = self.get_shard_index(&key);
        let mut shard = self.shards[idx].lock().unwrap();
        shard.put(key, val);
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `moka`: Uses atomic counters and batching to avoid locking on every read (TinyLFU).
// - `dashmap`: Similar sharded lock structure but for a plain HashMap (no eviction).
//
// Missing vs. Production:
// - **Scan Resistance**: Pure LRU is vulnerable to scans (one-time usage wiping the cache).
//   Production caches use LFU/TinyLFU.
// - **Zero-Copy Reads**: We clone values on `get`. `DashMap` uses `Ref` guards.
// - **Resize**: Cannot resize shards dynamically.

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn test_concurrent_basic() {
        let cache = ConcurrentLruCache::new(10, 4); // 4 shards, ~3 cap each
        cache.put("a", 1);
        cache.put("b", 2);

        assert_eq!(cache.get(&"a"), Some(1));
        assert_eq!(cache.get(&"b"), Some(2));
        assert_eq!(cache.get(&"c"), None);
    }

    #[test]
    fn test_concurrent_threads() {
        let cache = Arc::new(ConcurrentLruCache::new(100, 8));
        let mut handles = vec![];

        for i in 0..50 {
            let cache = cache.clone();
            handles.push(thread::spawn(move || {
                cache.put(i, i * 10);
            }));
        }

        for h in handles {
            h.join().unwrap();
        }

        // Verify some keys
        let mut found = 0;
        for i in 0..50 {
            if let Some(val) = cache.get(&i) {
                assert_eq!(val, i * 10);
                found += 1;
            }
        }
        // Since capacity is 100, we expect all 50 to be there (unless hash collisions piled up in one shard)
        // With 8 shards and 50 items, avg 6 items/shard. Capacity per shard is ceil(100/8) = 13.
        // It is extremely unlikely to evict unless hash function is broken.
        assert_eq!(found, 50);
    }

    #[test]
    fn test_eviction_per_shard() {
        // 2 shards, capacity 2 total -> 1 per shard.
        let cache = ConcurrentLruCache::new(2, 2);

        // We need to find keys that map to the same shard.
        // This is tricky with random DefaultHasher.
        // We can force it by iterating until we find collisions, or just spam enough keys.

        for i in 0..100 {
            cache.put(i, i);
        }

        // We put 100 items. Total capacity is 2.
        // Most items should be gone.
        let mut count = 0;
        for i in 0..100 {
            if cache.get(&i).is_some() {
                count += 1;
            }
        }

        assert!(count <= 2);
    }
}
