//! # Multi-Version Concurrency Control (MVCC)
//!
//! MVCC is a concurrency control method commonly used by database management systems to provide
//! concurrent access to the database. It maintains multiple versions of data, allowing readers
//! to read from a consistent snapshot without blocking writers, and writers to write without blocking readers.
//!
//! **Replaces Crates:** The transactional core of crates like `sled`, `rocksdb`, `fjall`
//! **Real-world systems:** `PostgreSQL`, `MySQL` (`InnoDB`), `CockroachDB`, `RocksDB`
//!
//! **Why build it yourself?**
//! Understanding MVCC is essential for distributed systems and database engineering. It shifts
//! your mindset from "lock everything when mutating" to "append new versions and reconcile."
//! It also demonstrates Rust's RAII (Drop trait) for transaction rollback and `Arc`/`RwLock` for
//! fine-grained concurrent state management.
//!
//! ## Architecture
//!
//! The system consists of:
//! 1. `MvccStore`: The global state containing all versions of all keys, an active transaction set, and a global TX ID counter.
//! 2. `Transaction`: A local view (Snapshot) of the database. It buffers writes locally until `commit`.
//!
//! When a transaction starts:
//! - It receives a unique `tx_id`.
//! - It receives the current `commit_id` (snapshot). It can only see writes committed *before* or *at* this `commit_id`.
//! - It adds itself to the `active_txs` set.
//!
//! On `get(key)`:
//! - Check local write buffer first.
//! - If not found, look up the key in the global store. Find the latest version where the writing `tx_id`
//!   is `<= snapshot_id` AND the writing `tx_id` is NOT in the active transactions set (meaning it was committed).
//!
//! On `set(key, value)`:
//! - Buffer the write locally.
//!
//! On `commit()`:
//! - Lock the global store.
//! - **Conflict Detection:** For every key written locally, check if the global store has a newer version
//!   (a version written by a `tx_id > our_snapshot_id`). If so, we have a write-write conflict -> ABORT.
//! - If no conflict, append all local writes to the global store with our `tx_id`.
//! - Remove our `tx_id` from the `active_txs` set.
//!
//! On `drop()` (Rollback):
//! - If not committed, simply remove our `tx_id` from the `active_txs` set.
//!
//! **Invariants:**
//! - Readers never block writers. Writers never block readers.
//! - Write-Write conflicts abort the transaction that commits second (First-Commiter-Wins).

// The global-store lock is intentionally held across conflict detection and write application.
#![allow(clippy::significant_drop_tightening)]

use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, RwLock};

/// API for the global MVCC Store.
pub trait MvccStoreApi<K, V> {
    type Tx<'a>: MvccTransactionApi<K, V>
    where
        Self: 'a;

    /// Begins a new transaction, providing snapshot isolation.
    fn begin(&self) -> Self::Tx<'_>;
}

/// API for an MVCC Transaction.
pub trait MvccTransactionApi<K, V> {
    /// Gets a value from the transaction's snapshot or local write buffer.
    fn get(&self, key: &K) -> Option<V>;

    /// Buffers a write to be applied on commit.
    fn set(&mut self, key: K, val: V);

    /// Attempts to commit the transaction. Returns `Err` on write-write conflicts.
    ///
    /// # Errors
    /// Returns `Err` if a write-write conflict is detected (another transaction committed
    /// a change to a key in this transaction's write set after this transaction started).
    fn commit(self) -> Result<(), &'static str>;
}

// ============================================================================
// Core MVCC Implementation
// ============================================================================

/// Represents a single version of a value in the store.
#[derive(Debug, Clone)]
pub struct Version<V> {
    /// The transaction ID that wrote this version.
    pub tx_id: u64,
    /// The actual value.
    pub value: V,
}

/// The Global MVCC Store.
pub struct MvccStore<K, V> {
    /// The global monotonically increasing transaction ID counter.
    next_tx_id: Mutex<u64>,

    /// The set of currently active (uncommitted) transactions.
    /// This is used for visibility checks. A transaction cannot see writes from active transactions.
    active_txs: Mutex<HashSet<u64>>,

    /// The core data structure mapping keys to a list of versions (ordered by `tx_id` ascending).
    /// We use a `RwLock` to allow multiple readers to scan versions concurrently.
    data: RwLock<HashMap<K, Vec<Version<V>>>>,
}

impl<K: std::cmp::Eq + std::hash::Hash + Clone, V: Clone> MvccStore<K, V> {
    /// Creates a new, empty MVCC store.
    #[must_use] 
    pub fn new() -> Self {
        Self {
            next_tx_id: Mutex::new(1), // tx_id 0 is reserved for "no transaction" or system init
            active_txs: Mutex::new(HashSet::new()),
            data: RwLock::new(HashMap::new()),
        }
    }
}

impl<K: std::cmp::Eq + std::hash::Hash + Clone, V: Clone> Default for MvccStore<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K: std::cmp::Eq + std::hash::Hash + Clone, V: Clone> MvccStoreApi<K, V> for MvccStore<K, V> {
    type Tx<'a>
        = Transaction<'a, K, V>
    where
        Self: 'a;

    fn begin(&self) -> Self::Tx<'_> {
        let tx_id = {
            let mut next = self.next_tx_id.lock().unwrap();
            let id = *next;
            *next += 1;
            id
        };

        // Snapshot Isolation: We record the *start* time of this transaction.
        let snapshot_id = tx_id;

        let start_active_txs = {
            let mut active = self.active_txs.lock().unwrap();
            let snapshot = active.clone();
            // Add to active transactions so others can't see our writes until we commit.
            active.insert(tx_id);
            snapshot
        };

        Transaction {
            store: self,
            tx_id,
            snapshot_id,
            start_active_txs,
            write_buffer: HashMap::new(),
            committed: false,
        }
    }
}

/// An MVCC Transaction providing Snapshot Isolation.
pub struct Transaction<'a, K, V> {
    /// Reference back to the global store.
    store: &'a MvccStore<K, V>,
    /// The unique ID of this transaction.
    tx_id: u64,
    /// The ID defining our snapshot visibility. We can only see writes committed <= `snapshot_id`.
    snapshot_id: u64,
    /// The snapshot of active transactions when this transaction started.
    start_active_txs: HashSet<u64>,
    /// Local writes buffer. These are applied to the global store atomically on commit.
    write_buffer: HashMap<K, V>,
    /// Tracks if we successfully committed to prevent the `Drop` impl from rolling back.
    committed: bool,
}

impl<K: std::cmp::Eq + std::hash::Hash + Clone, V: Clone> MvccTransactionApi<K, V>
    for Transaction<'_, K, V>
{
    fn get(&self, key: &K) -> Option<V> {
        // 1. Check local writes first (Read-Your-Own-Writes consistency).
        if let Some(val) = self.write_buffer.get(key) {
            return Some(val.clone());
        }

        // 2. Read from global store. We only take a Read lock.
        // RUST INSIGHT: `RwLock` allows multiple readers to do this step concurrently.
        let data = self.store.data.read().unwrap();
        let versions = data.get(key)?;

        // 3. Find the most recent visible version.
        // We iterate backwards (newest to oldest) and find the first version that:
        // - Was written by a transaction <= our snapshot_id.
        // - Is NOT in the `start_active_txs` set (meaning it successfully committed before we started).
        for version in versions.iter().rev() {
            // We can see our own writes (though we handled that above), and writes from transactions
            // that started before us AND had already committed when we started.
            if version.tx_id <= self.snapshot_id && !self.start_active_txs.contains(&version.tx_id)
            {
                return Some(version.value.clone());
            }
        }

        None
    }

    fn set(&mut self, key: K, val: V) {
        // Just buffer locally. MVCC readers don't block writers, and writers don't block readers.
        self.write_buffer.insert(key, val);
    }

    fn commit(mut self) -> Result<(), &'static str> {
        // We must take a WRITE lock to commit, ensuring serializable application of the buffer.
        let mut data = self.store.data.write().unwrap();

        // First-Commiter-Wins Conflict Detection:
        // For every key we want to write, check if someone else wrote a newer version after our snapshot.
        // A write-write conflict occurs if anyone committed a version of this key that wasn't visible
        // to us when we started. A version was NOT visible to us if:
        // 1. It was committed by a transaction that started AFTER us (`tx_id > snapshot_id`).
        // 2. It was committed by a transaction that started BEFORE us, but was still active
        //    (uncommitted) when we started (`start_active_txs.contains(tx_id)`).
        for key in self.write_buffer.keys() {
            if let Some(versions) = data.get(key)
                && let Some(latest_version) = versions.last()
            {
                // Conflict if the latest version is from a transaction that started after us,
                // OR from a transaction that was active when we started.
                if latest_version.tx_id > self.snapshot_id
                    || self.start_active_txs.contains(&latest_version.tx_id)
                {
                    return Err("Write-Write Conflict detected. Transaction aborted.");
                }
            }
        }

        // No conflicts detected. Apply all buffered writes.
        for (key, value) in self.write_buffer.drain() {
            data.entry(key).or_default().push(Version {
                tx_id: self.tx_id,
                value,
            });
        }

        // Mark as committed so `Drop` doesn't roll us back.
        self.committed = true;

        // Remove from active transactions so other readers can now see our writes.
        self.store.active_txs.lock().unwrap().remove(&self.tx_id);

        Ok(())
    }
}

// RUST INSIGHT: RAII Rollback
// If a transaction panics, returns early via `?`, or explicitly aborts, it will be Dropped.
// The `Drop` trait guarantees that we clean up the active transactions set, effectively
// rolling back the transaction and preventing resource leaks or global deadlocks.
impl<K, V> Drop for Transaction<'_, K, V> {
    fn drop(&mut self) {
        // ALWAYS remove from active transactions on drop.
        // Even if committed, we must clean up.
        // Wait, in `commit`, we did:
        // `self.store.active_txs.lock().unwrap().remove(&self.tx_id);`
        // So it's already removed if committed. Doing it again is harmless.
        if !self.committed {
            // Rollback: Since we only buffered writes locally, we didn't touch global state.
        }

        if let Ok(mut active_txs) = self.store.active_txs.lock() {
            active_txs.remove(&self.tx_id);
        }
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `sled` or `rocksdb`: Full databases persist to disk (LSM Trees/B-Trees), handle crash recovery (WAL),
//   and implement Garbage Collection (Vacuuming) to remove old versions.
// - This implementation is purely in-memory and unbounded (versions grow infinitely).
//
// Missing vs. Production:
// - **Garbage Collection (Vacuum):** Old versions are never deleted here. A real MVCC system periodically
//   cleans up versions that are older than the oldest active transaction.
// - **Durability:** This is just concurrency control, not a persistent storage engine.
// - **Predicate Locking / Phantom Reads:** This simple implementation protects against write-write
//   conflicts on specific keys, but not range-based conflicts (phantom reads).

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_your_own_writes() {
        let store = MvccStore::new();
        let mut tx = store.begin();

        tx.set("key", "value");
        assert_eq!(tx.get(&"key"), Some("value"));

        tx.commit().unwrap();
    }

    #[test]
    fn test_snapshot_isolation() {
        let store = MvccStore::new();

        // Setup initial state
        let mut tx1 = store.begin();
        tx1.set("key", "v1");
        tx1.commit().unwrap();

        // tx2 starts, sees "v1"
        let tx2 = store.begin();
        assert_eq!(tx2.get(&"key"), Some("v1"));

        // tx3 starts, writes "v2", and commits
        let mut tx3 = store.begin();
        tx3.set("key", "v2");
        tx3.commit().unwrap();

        // tx2 should STILL see "v1" because it started before tx3 committed (Snapshot Isolation)
        assert_eq!(tx2.get(&"key"), Some("v1"));

        // tx4 starts now, should see "v2"
        let tx4 = store.begin();
        assert_eq!(tx4.get(&"key"), Some("v2"));
    }

    #[test]
    fn test_write_write_conflict() {
        let store = MvccStore::new();

        // tx1 and tx2 start concurrently
        let mut tx1 = store.begin();
        let mut tx2 = store.begin();

        // tx1 writes and commits
        tx1.set("key", "v1");
        assert!(tx1.commit().is_ok());

        // tx2 writes to the same key and tries to commit
        tx2.set("key", "v2");
        let result = tx2.commit();

        // tx2 MUST abort because tx1 already committed a write to "key" after tx2's snapshot
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            "Write-Write Conflict detected. Transaction aborted."
        );
    }

    #[test]
    fn test_no_conflict_different_keys() {
        let store = MvccStore::new();

        let mut tx1 = store.begin();
        let mut tx2 = store.begin();

        // They write to different keys
        tx1.set("key1", "v1");
        tx2.set("key2", "v2");

        // Both should commit successfully
        assert!(tx1.commit().is_ok());
        assert!(tx2.commit().is_ok());

        let tx3 = store.begin();
        assert_eq!(tx3.get(&"key1"), Some("v1"));
        assert_eq!(tx3.get(&"key2"), Some("v2"));
    }

    #[test]
    fn test_raii_rollback() {
        let store = MvccStore::new();

        {
            let mut tx = store.begin();
            tx.set("key", "v1");
            // tx falls out of scope and is Dropped without calling `commit()`
        }

        let tx_check = store.begin();
        // The write should not exist in the global store
        assert_eq!(tx_check.get(&"key"), None);

        // Active transactions should only contain `tx_check` because `tx`'s Drop cleaned it up
        let active = store.active_txs.lock().unwrap();
        assert_eq!(active.len(), 1);
        assert!(active.contains(&tx_check.tx_id));
    }

    #[test]
    fn test_dirty_read_prevention() {
        let store = MvccStore::new();

        let mut tx1 = store.begin();
        tx1.set("key", "dirty_value"); // Written but NOT committed

        let tx2 = store.begin();
        // tx2 should not see tx1's uncommitted write
        assert_eq!(tx2.get(&"key"), None);
    }
}
