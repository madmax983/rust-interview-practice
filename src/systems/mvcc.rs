//! # Multi-Version Concurrency Control (MVCC)
//!
//! A foundational transactional storage engine using Snapshot Isolation.
//!
//! **Replaces Crates:** The transactional core of `sled` and `rocksdb`.
//!
//! **Real-world Usage:**
//! - Core transactional engine in PostgreSQL and Oracle.
//! - Foundation of modern key-value stores like FoundationDB and TiKV.
//!
//! **Why build it yourself?**
//! MVCC solves the classic "readers block writers" problem. By building it, you learn how to
//! maintain multiple versions of a single key, implement lock-free concurrent reads using
//! snapshots, and detect write-write conflicts at commit time. It teaches you how databases
//! achieve ACID guarantees without grinding to a halt under contention.

use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

// =========================================================================================
// Architecture
// =========================================================================================
//
// Data Structure:
//
//      Global Storage: RwLock<HashMap<Key, Vec<Version>>>
//      Active Txs: Mutex<HashSet<TxId>>
//      Timestamp Generator: AtomicU64
//
// Flow:
//
//   [Tx1: Start] ──► Gets Read_Ts (e.g., 100)
//                        │
//                        ▼
//   [Tx1: Read]  ──► Scans `Versions` for largest Commit_Ts <= 100
//                        │
//                        ▼
//   [Tx1: Write] ──► Buffers locally in `write_set`
//                        │
//                        ▼
//   [Tx1: Commit]──► 1. Acquire Mutex
//                    2. Check Write-Write conflicts (has any key been updated since Read_Ts?)
//                    3. Get Commit_Ts (e.g., 105)
//                    4. Append new versions to Global Storage
//                    5. Release Mutex
//
// Invariants:
// 1. Snapshot Isolation: A transaction only sees committed data that existed at its `read_ts`.
// 2. First-Commiter-Wins: If TxA and TxB both read version 1, and TxA commits version 2, TxB must abort.
// 3. Readers never block writers, and writers never block readers.
//
// Complexity:
// - Read: `O(V)` where V is the number of versions for a key (can be optimized to O(log V) with binary search).
// - Write: `O(1)` local buffering.
// - Commit: `O(W * V)` where W is the number of writes and V is the versions to check for conflicts.

// =========================================================================================
// Implementation
// =========================================================================================

type Key = String;
type Value = String;
type Timestamp = u64;

#[derive(Clone, Debug)]
struct Version {
    commit_ts: Timestamp,
    value: Value,
}

pub struct MvccStore {
    // Global monotonic clock
    ts_generator: AtomicU64,

    // The actual data: Key -> List of historical versions, sorted by commit_ts ascending.
    // RUST INSIGHT: RwLock is perfect here because reads are heavily dominant and only
    // appending a new version requires a brief write lock.
    storage: RwLock<HashMap<Key, Vec<Version>>>,
    // In a real DB, we would track active transactions to know when older versions can be vacuumed.
    // For this educational implementation, we omit vacuuming.
}

impl MvccStore {
    #[must_use]
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            ts_generator: AtomicU64::new(1),
            storage: RwLock::new(HashMap::new()),
        })
    }

    /// Starts a new transaction, capturing the current timestamp as its read snapshot.
    pub fn begin_tx(self: &Arc<Self>) -> Transaction {
        let read_ts = self.ts_generator.load(Ordering::SeqCst);
        Transaction {
            store: Arc::clone(self),
            read_ts,
            write_set: BTreeMap::new(),
            status: TxStatus::Active,
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum TxStatus {
    Active,
    Committed,
    Aborted,
}

pub struct Transaction {
    store: Arc<MvccStore>,
    read_ts: Timestamp,

    // Local buffer for writes. Not visible to others until commit.
    write_set: BTreeMap<Key, Value>,

    status: TxStatus,
}

impl Transaction {
    /// Reads a key.
    /// 1. Checks local write_set first (Read-Your-Own-Writes).
    /// 2. Then checks global storage for the latest version <= read_ts.
    pub fn get(&self, key: &str) -> Option<Value> {
        if self.status != TxStatus::Active {
            return None;
        }

        // 1. Read your own writes
        if let Some(val) = self.write_set.get(key) {
            return Some(val.clone());
        }

        // 2. Read from global snapshot
        let storage = self.store.storage.read().unwrap();
        if let Some(versions) = storage.get(key) {
            // Find the most recent version that was committed *before or at* our read_ts.
            // Since versions are appended in chronological order, we iterate backwards.
            for v in versions.iter().rev() {
                if v.commit_ts <= self.read_ts {
                    return Some(v.value.clone());
                }
            }
        }

        None
    }

    /// Buffers a write locally.
    pub fn put(&mut self, key: Key, value: Value) {
        if self.status == TxStatus::Active {
            self.write_set.insert(key, value);
        }
    }

    /// Attempts to commit the transaction using First-Commiter-Wins conflict resolution.
    pub fn commit(mut self) -> Result<Timestamp, &'static str> {
        if self.status != TxStatus::Active {
            return Err("Transaction is not active");
        }

        if self.write_set.is_empty() {
            self.status = TxStatus::Committed;
            return Ok(self.read_ts); // Read-only txs commit instantly
        }

        // 1. Acquire global write lock to check conflicts and apply changes atomically.
        // GOTCHA: Holding this lock during commit is the primary bottleneck in simple MVCC.
        let mut storage = self.store.storage.write().unwrap();

        // 2. Conflict Detection (First-Commiter-Wins)
        // For every key we want to write, check if someone else has committed a newer version
        // since our read_ts. If they did, our read snapshot is stale, and we must abort.
        for key in self.write_set.keys() {
            if let Some(versions) = storage.get(key) {
                if let Some(latest) = versions.last() {
                    if latest.commit_ts > self.read_ts {
                        self.status = TxStatus::Aborted;
                        return Err("Write-Write Conflict (Serialization Failure)");
                    }
                }
            }
        }

        // 3. Get commit timestamp
        // RUST INSIGHT: Fetch_add guarantees a strictly monotonically increasing timestamp
        // across all threads safely.
        let commit_ts = self.store.ts_generator.fetch_add(1, Ordering::SeqCst) + 1;

        // 4. Apply writes to global storage
        for (key, value) in std::mem::take(&mut self.write_set) {
            let new_version = Version { commit_ts, value };
            storage
                .entry(key)
                .or_insert_with(Vec::new)
                .push(new_version);
        }

        self.status = TxStatus::Committed;
        Ok(commit_ts)
    }

    /// Explicitly aborts the transaction.
    pub fn abort(mut self) {
        self.status = TxStatus::Aborted;
    }
}

// RUST INSIGHT: RAII Drop Guard
// If a `Transaction` goes out of scope without `commit()` being called, it is automatically aborted.
// Because we took ownership in `commit(self)`, Drop only runs for uncommitted or explicitly aborted txs.
impl Drop for Transaction {
    fn drop(&mut self) {
        if self.status == TxStatus::Active {
            self.status = TxStatus::Aborted;
        }
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to `sled` / `rocksdb`:
// - Real engines persist these versions to disk (WAL + LSM Tree or B-Tree).
// - Real engines use a background thread for Garbage Collection (Vacuuming) to remove old
//   versions that are no longer visible to any active transaction. We leak memory here.
// - Real engines often use lock-free data structures (like `SkipList`) for the MemTable
//   instead of a giant `RwLock<HashMap>`.
//
// What's missing vs. production:
// 1. Garbage Collection (Vacuuming) of old versions.
// 2. Persistence (WAL logging before modifying the in-memory map).
// 3. Fine-grained locking (e.g., locking per-key or per-shard during commit instead of globally).
// 4. Serializable Snapshot Isolation (SSI) which detects Read-Write conflicts (Anti-Dependencies).

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_crud_and_isolation() {
        let store = MvccStore::new();

        // Tx1 writes A=1
        let mut tx1 = store.begin_tx();
        tx1.put("A".to_string(), "1".to_string());
        tx1.commit().unwrap();

        // Tx2 reads A=1, writes B=2
        let mut tx2 = store.begin_tx();
        assert_eq!(tx2.get("A"), Some("1".to_string()));
        tx2.put("B".to_string(), "2".to_string());

        // Tx3 starts *before* Tx2 commits. It should NOT see B=2.
        let tx3 = store.begin_tx();

        tx2.commit().unwrap();

        assert_eq!(tx3.get("B"), None);
        assert_eq!(tx3.get("A"), Some("1".to_string())); // Tx3 still sees A=1
    }

    #[test]
    fn test_read_your_own_writes() {
        let store = MvccStore::new();
        let mut tx = store.begin_tx();

        tx.put("A".to_string(), "1".to_string());
        assert_eq!(tx.get("A"), Some("1".to_string())); // Should see its own write before commit
        tx.commit().unwrap();
    }

    #[test]
    fn test_write_write_conflict() {
        let store = MvccStore::new();

        let mut tx1 = store.begin_tx();
        let mut tx2 = store.begin_tx(); // tx2 starts at same time as tx1

        tx1.put("A".to_string(), "1".to_string());
        tx1.commit().unwrap(); // tx1 wins

        tx2.put("A".to_string(), "2".to_string());
        let res = tx2.commit(); // tx2 fails because 'A' was modified after tx2 started

        assert!(res.is_err());
        assert_eq!(
            res.unwrap_err(),
            "Write-Write Conflict (Serialization Failure)"
        );
    }

    #[test]
    fn test_non_conflicting_concurrent_writes() {
        let store = MvccStore::new();

        let mut tx1 = store.begin_tx();
        let mut tx2 = store.begin_tx();

        tx1.put("A".to_string(), "1".to_string());
        tx2.put("B".to_string(), "2".to_string());

        // Both should commit successfully because they write to different keys
        assert!(tx1.commit().is_ok());
        assert!(tx2.commit().is_ok());

        let tx3 = store.begin_tx();
        assert_eq!(tx3.get("A"), Some("1".to_string()));
        assert_eq!(tx3.get("B"), Some("2".to_string()));
    }
}
