//! # TTL Cache Implementation
//!
//! Implements a Time-To-Live (TTL) cache where items expire after a set duration.
//! It supports both lazy expiration (on access) and active expiration (cleanup).
//!
//! **Replaces Crates:** `cached`, `expiring_map`, `ttl_cache`
//!
//! **Real-world Usage:**
//! - Session storage (e.g., user login sessions that expire after 30 mins).
//! - DNS caching (records have a TTL).
//! - API response caching (short-lived cache to reduce load).
//! - Authentication tokens (JWTs often have built-in expiry, but a revocation list needs TTL).
//!
//! **Why build it yourself?**
//! Implementing a TTL cache teaches you about time management in systems.
//! You learn the trade-offs between:
//! 1. **Lazy Expiration**: O(1) checks on access, but memory is never freed if items aren't accessed.
//! 2. **Active Expiration**: Background cleanup tasks (complex concurrency) vs. random sampling (probabilistic cleanup like Redis).
//! 3. **Data Structure**: Using a `HashMap` vs. an ordered structure (like a `BTreeMap` by expiration time) for O(1) expiration finding.

use std::collections::HashMap;
use std::hash::Hash;
use std::time::{Duration, Instant};

// =========================================================================================
// Architecture
// =========================================================================================
//
// Data Structure:
//
//      HashMap (Key -> (Value, Expiry))
//
// Invariants:
// 1. `get(key)` returns `None` if `now > expiry`, even if the key is in the map.
// 2. `len()` returns the number of items *currently stored*, potentially including expired ones (lazy).
//
// Complexity:
// ┌───────────────┬─────────────┬─────────────┐
// │ Operation     │ Time        │ Space       │
// ├───────────────┼─────────────┼─────────────┤
// │ get           │ O(1)        │ O(1)        │
// │ put           │ O(1)        │ O(1)        │
// │ cleanup (full)│ O(N)        │ O(1)        │
// └───────────────┴─────────────┴─────────────┘
//
// Design Decisions:
// - **Storage**: `HashMap<K, (V, Instant)>`. Simple and effective for O(1) access.
// - **Expiration Strategy**:
//   - *Lazy*: On `get`, check if expired. If so, remove and return None.
//   - *Active*: `cleanup()` method iterates all keys to remove expired ones. User can call this periodically or in a background thread.
//   - *Alternative*: Use a separate `BTreeMap<Instant, Vec<K>>` to efficiently find expired keys (O(log N)).
//     This doubles memory usage and complicates `put` (need to update both structures).
//     We chose `HashMap` for simplicity and lower overhead, assuming `cleanup` is infrequent or acceptable O(N).

/// A Time-To-Live (TTL) Cache.
pub struct TTLCache<K, V> {
    map: HashMap<K, (V, Instant)>,
    ttl: Duration,
}

impl<K, V> TTLCache<K, V>
where
    K: Eq + Hash + Clone,
{
    /// Creates a new TTL Cache with the specified Time-To-Live duration.
    #[must_use]
    pub fn new(ttl: Duration) -> Self {
        Self {
            map: HashMap::new(),
            ttl,
        }
    }

    /// Inserts a key-value pair into the cache.
    /// The item will expire after `ttl` duration from now.
    pub fn put(&mut self, key: K, value: V) {
        let expiry = Instant::now() + self.ttl;
        self.map.insert(key, (value, expiry));
    }

    /// Gets the value associated with the key.
    /// Returns `None` if the key does not exist or has expired.
    ///
    /// # Lazy Expiration
    /// If the item is found but expired, it is removed from the cache immediately.
    pub fn get(&mut self, key: &K) -> Option<&V> {
        let is_expired = if let Some((_, expiry)) = self.map.get(key) {
            Instant::now() > *expiry
        } else {
            false
        };

        if is_expired {
            self.map.remove(key);
            return None;
        }

        // Return the value if valid
        self.map.get(key).map(|(v, _)| v)
    }

    /// Gets a mutable reference to the value associated with the key.
    /// Returns `None` if the key does not exist or has expired.
    pub fn get_mut(&mut self, key: &K) -> Option<&mut V> {
        let is_expired = if let Some((_, expiry)) = self.map.get(key) {
            Instant::now() > *expiry
        } else {
            false
        };

        if is_expired {
            self.map.remove(key);
            return None;
        }

        self.map.get_mut(key).map(|(v, _)| v)
    }

    /// Removes expired items from the cache.
    /// This is an O(N) operation where N is the number of items in the cache.
    ///
    /// # Active Expiration
    /// Call this method periodically (e.g., in a background thread or event loop)
    /// to free memory occupied by expired items that haven't been accessed.
    pub fn cleanup(&mut self) {
        let now = Instant::now();

        // RUST INSIGHT: `retain` is efficient for filtering a HashMap in-place.
        // It avoids the need to collect keys to remove and then iterate again.
        self.map.retain(|_, (_, expiry)| *expiry > now);
    }

    /// Returns the number of items currently in the cache.
    /// Note: This may include expired items that haven't been accessed or cleaned up yet.
    #[must_use]
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// Returns true if the cache is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Returns the configured TTL duration.
    #[must_use]
    pub const fn ttl(&self) -> Duration {
        self.ttl
    }

    /// Clear all items from the cache.
    pub fn clear(&mut self) {
        self.map.clear();
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `cached`: Focuses on function memoization (caching return values). Has TTL support but often tied to macros.
// - `expiring_map`: Similar to this implementation.
// - `ttl_cache`: Often uses a LinkedHashMap to also support LRU eviction along with TTL (capacity + time).
//
// Missing vs. Production:
// - **Per-item TTL**: This implementation uses a global TTL. Production caches (Redis) allow different TTLs per key.
// - **Capacity Limit**: No upper bound on memory. If cleanup isn't called and keys are unique, it will grow indefinitely.
// - **Efficient Expiration**: For large datasets, O(N) cleanup is too slow.
//   Production systems use:
//   1. **Random Sampling** (Redis): Check 20 random keys, remove expired. If >25% were expired, repeat.
//   2. **Bucketing**: Group items by expiry second/minute (Hierarchical Timing Wheels).
//
// Next Steps:
// 1. Add `put_with_ttl(key, value, duration)` for per-item TTL.
// 2. Implement `cleanup_random_sample(sample_size)` for efficient probabilistic cleanup.
// 3. Combine with `LRUCache` to have both capacity and time limits.

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_ttl_cache_lazy_expiration() {
        let mut cache = TTLCache::new(Duration::from_millis(50));

        cache.put("key", "value");

        // Immediate access should work
        assert_eq!(cache.get(&"key"), Some(&"value"));

        // Wait for expiration
        thread::sleep(Duration::from_millis(60));

        // Access should fail and remove item
        assert_eq!(cache.get(&"key"), None);
        assert_eq!(cache.len(), 0);
    }

    #[test]
    fn test_ttl_cache_active_cleanup() {
        let mut cache = TTLCache::new(Duration::from_millis(50));

        cache.put("key1", 1);
        cache.put("key2", 2);

        // Wait for expiration
        thread::sleep(Duration::from_millis(60));

        // Still in map before access/cleanup (technically implementation detail, but verifies lazy behavior)
        // Note: len() includes expired items
        assert_eq!(cache.len(), 2);

        // Run cleanup
        cache.cleanup();

        // Should be empty now
        assert_eq!(cache.len(), 0);
        assert_eq!(cache.get(&"key1"), None);
    }

    #[test]
    fn test_ttl_cache_updates_expiry() {
        let mut cache = TTLCache::new(Duration::from_millis(100));

        cache.put("key", 1);

        thread::sleep(Duration::from_millis(50));

        // Overwrite, should reset TTL
        cache.put("key", 2);

        thread::sleep(Duration::from_millis(60));

        // Total time 110ms, but reset at 50ms, so should still be valid (110 - 50 = 60 < 100)
        assert_eq!(cache.get(&"key"), Some(&2));

        thread::sleep(Duration::from_millis(50));

        // Now it should be expired (60 + 50 = 110 > 100)
        assert_eq!(cache.get(&"key"), None);
    }

    #[test]
    fn test_ttl_cache_mixed_expiration() {
        let mut cache = TTLCache::new(Duration::from_millis(100));

        cache.put("short", 1);

        thread::sleep(Duration::from_millis(60));

        cache.put("long", 2);

        thread::sleep(Duration::from_millis(50));

        // "short" expired (110ms > 100ms)
        // "long" valid (50ms < 100ms)

        cache.cleanup();

        assert_eq!(cache.get(&"short"), None);
        assert_eq!(cache.get(&"long"), Some(&2));
        assert_eq!(cache.len(), 1);
    }
}
