//! # Write-Through / Write-Back Cache Strategies
//!
//! # Header
//!
//! *   **Problem Name**: Cache Write Policies
//! *   **Difficulty**: Medium
//! *   **Link**: <https://en.wikipedia.org/wiki/Cache_(computing)#Writing_policies>
//! *   **Why this matters in Rust**: Understanding data consistency between cache and persistent storage.
//!
//! # Architecture
//!
//! This module implements a cache with pluggable write policies:
//!
//! 1.  **Write-Through**: Data is written to the cache and the backing store synchronously.
//!     *   *Pros*: Strong consistency, data is always in store.
//!     *   *Cons*: High write latency (limited by store speed).
//!
//! 2.  **Write-Back (Write-Behind)**: Data is written only to the cache initially. The store is updated later (e.g., on eviction or flush).
//!     *   *Pros*: Low write latency, write coalescing.
//!     *   *Cons*: Risk of data loss if cache crashes before flush; complex implementation (dirty tracking).
//!
//! **Components:**
//! *   `BackingStore`: Trait representing the slow persistence layer (Database, Disk).
//! *   `Cache`: The main struct managing memory and the store.
//!
//! # Rust Insight
//!
//! *   **Traits**: `BackingStore` allows mocking the database for tests.
//! *   **Hash Maps**: Used for O(1) storage. `HashSet` tracks dirty keys for Write-Back.
//!
//! # Production Note
//!
//! In a real system (like CPU caches or Redis), "Write-Back" is complex. It involves:
//! *   **Dirty Bits**: Tracking modified lines.
//! *   **checkpointing**: Periodic background flushing.
//! *   **WAL**: Often combined with a Write-Ahead Log to prevent data loss on crash.

use std::collections::{HashMap, HashSet};
use std::fmt::Debug;
use std::hash::Hash;

/// Abstraction for the persistent storage (Database, Disk, Network).
pub trait BackingStore<K, V> {
    fn load(&self, key: &K) -> Option<V>;
    fn save(&mut self, key: K, value: V);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WritePolicy {
    WriteThrough,
    WriteBack,
}

pub struct Cache<K, V, S> {
    store: S,
    policy: WritePolicy,
    data: HashMap<K, V>,
    dirty: HashSet<K>,
    capacity: usize,
}

impl<K, V, S> Cache<K, V, S>
where
    K: Eq + Hash + Clone + Debug,
    V: Clone + Debug,
    S: BackingStore<K, V>,
{
    pub fn new(store: S, policy: WritePolicy, capacity: usize) -> Self {
        Self {
            store,
            policy,
            data: HashMap::new(),
            dirty: HashSet::new(),
            capacity,
        }
    }

    /// Retrieves a value.
    /// If in cache, returns it.
    /// If not, loads from store, populates cache, and returns it.
    pub fn get(&mut self, key: &K) -> Option<V> {
        if let Some(val) = self.data.get(key) {
            return Some(val.clone());
        }

        // Cache miss - load from store
        if let Some(val) = self.store.load(key) {
            self.ensure_capacity();
            self.data.insert(key.clone(), val.clone());
            return Some(val);
        }

        None
    }

    /// Writes a value according to the policy.
    pub fn put(&mut self, key: K, value: V) {
        self.ensure_capacity();

        match self.policy {
            WritePolicy::WriteThrough => {
                // Write to store immediately
                self.store.save(key.clone(), value.clone());
                // Update cache
                self.data.insert(key, value);
            }
            WritePolicy::WriteBack => {
                // Update cache only
                self.data.insert(key.clone(), value);
                // Mark as dirty
                self.dirty.insert(key);
            }
        }
    }

    /// Forces a synchronization of all dirty items to the store.
    pub fn flush(&mut self) {
        if self.policy == WritePolicy::WriteBack {
            // We need to iterate dirty keys and save them.
            // But we need values from `data`.
            // We can't iterate `dirty` and borrow `data` mutably if not careful,
            // but `save` takes `&mut self.store`.

            // To satisfy borrow checker and logic:
            // 1. Drain the dirty set.
            // 2. For each key, get value from data, save to store.

            // ⚡ BOLT OPTIMIZATION: Avoid intermediate `.collect::<Vec<K>>()` allocation.
            // Modern Rust's borrow checker understands disjoint fields, allowing us to hold
            // a mutable borrow on `self.dirty` via `drain()` while simultaneously accessing
            // `self.data` and mutating `self.store` inside the loop. This preserves the
            // allocated capacity of the `HashSet` while avoiding an intermediate vector allocation.
            for key in self.dirty.drain() {
                if let Some(val) = self.data.get(&key) {
                    self.store.save(key.clone(), val.clone());
                }
            }
        }
    }

    /// Internal helper to maintain capacity.
    /// Simple eviction policy: Remove arbitrary element (keys iterator order).
    /// Real LRU would be better but this focuses on write policies.
    fn ensure_capacity(&mut self) {
        if self.data.len() >= self.capacity {
            // Pick a key to evict
            // Note: `keys().next()` is arbitrary but deterministic for same map state usually.
            let key_to_evict = self.data.keys().next().cloned();

            if let Some(key) = key_to_evict {
                self.evict(&key);
            }
        }
    }

    fn evict(&mut self, key: &K) {
        if let Some(val) = self.data.remove(key) {
            // On eviction, if Write-Back and dirty, we MUST save to store.
            if self.policy == WritePolicy::WriteBack && self.dirty.contains(key) {
                self.store.save(key.clone(), val);
                self.dirty.remove(key);
            }
        }
    }
}

// Drops should arguably flush for Write-Back to avoid data loss,
// but panicking in Drop is bad, and flush might fail (IO).
// We leave explicit flush to user.

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    // Mock Store using RefCell to track calls and state
    #[derive(Clone, Default)]
    struct MockStore {
        storage: Rc<RefCell<HashMap<String, String>>>,
        save_calls: Rc<RefCell<usize>>,
    }

    impl BackingStore<String, String> for MockStore {
        fn load(&self, key: &String) -> Option<String> {
            self.storage.borrow().get(key).cloned()
        }

        fn save(&mut self, key: String, value: String) {
            *self.save_calls.borrow_mut() += 1;
            self.storage.borrow_mut().insert(key, value);
        }
    }

    #[test]
    fn test_write_through() {
        let store = MockStore::default();
        let mut cache = Cache::new(store.clone(), WritePolicy::WriteThrough, 10);

        cache.put("key1".to_string(), "value1".to_string());

        // Should be in cache
        assert_eq!(cache.get(&"key1".to_string()), Some("value1".to_string()));

        // Should be in store immediately
        assert_eq!(
            store.storage.borrow().get("key1"),
            Some(&"value1".to_string())
        );
        assert_eq!(*store.save_calls.borrow(), 1);
    }

    #[test]
    fn test_write_back_deferred() {
        let store = MockStore::default();
        let mut cache = Cache::new(store.clone(), WritePolicy::WriteBack, 10);

        cache.put("key1".to_string(), "value1".to_string());

        // Should be in cache
        assert_eq!(cache.get(&"key1".to_string()), Some("value1".to_string()));

        // Should NOT be in store yet
        assert_eq!(store.storage.borrow().get("key1"), None);
        assert_eq!(*store.save_calls.borrow(), 0);

        // Explicit flush
        cache.flush();

        // Now should be in store
        assert_eq!(
            store.storage.borrow().get("key1"),
            Some(&"value1".to_string())
        );
        assert_eq!(*store.save_calls.borrow(), 1);
    }

    #[test]
    fn test_write_back_eviction() {
        let store = MockStore::default();
        // Capacity 1 to force eviction
        let mut cache = Cache::new(store.clone(), WritePolicy::WriteBack, 1);

        cache.put("key1".to_string(), "value1".to_string());
        // Cache: {k1: v1}, Dirty: {k1}

        // Insert k2, forcing eviction of k1
        cache.put("key2".to_string(), "value2".to_string());

        // k1 should be flushed to store
        assert_eq!(
            store.storage.borrow().get("key1"),
            Some(&"value1".to_string())
        );
        // k2 not yet
        assert_eq!(store.storage.borrow().get("key2"), None);
    }

    #[test]
    fn test_read_through() {
        let mut store = MockStore::default();
        store.save("key1".to_string(), "db_value".to_string());

        let mut cache = Cache::new(store.clone(), WritePolicy::WriteThrough, 10);

        // Cache miss -> Load from store
        assert_eq!(cache.get(&"key1".to_string()), Some("db_value".to_string()));

        // Should now be in cache (future reads don't hit store load, but we can't easily spy on load calls in this simple mock without adding more fields)
    }
}
