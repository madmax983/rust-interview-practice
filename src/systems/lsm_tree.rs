//! # Log-Structured Merge-Tree (LSM Tree) Implementation
//!
//! # Header
//!
//! *   **Problem Name**: Log-Structured Merge-Tree (LSM Tree) Core
//! *   **Difficulty**: Hard (Systems)
//! *   **Replaces Crates**: `rocksdb`, `leveldb`, `sled` (partially)
//! *   **Real-world Usage**:
//!     *   **Cassandra / ScyllaDB**: Core storage engine for high write throughput.
//!     *   **RocksDB / LevelDB**: Embedded key-value stores used everywhere (from Kafka to TiDB).
//!     *   **InfluxDB**: Time-series databases heavily rely on LSM semantics.
//! *   **Why build it yourself?**
//!     Implementing an LSM Tree teaches you how modern databases achieve insanely high write throughput.
//!     You learn how to trade off read speed (which might need to search multiple files) for write speed
//!     (which only writes to memory and appends to disk). You will grapple with `BTreeMap`s (MemTable),
//!     immutable disk structures (SSTables), and the concept of Tombstones for deletion.
//!
//! # Architecture
//!
//! An LSM Tree buffers writes in memory and periodically flushes them to disk as immutable, sorted files.
//!
//! **Components:**
//! 1.  **MemTable (In-Memory)**: A balanced tree (e.g., `BTreeMap`) holding the most recent writes.
//! 2.  **SSTable (Disk)**: Sorted String Table. Immutable files on disk containing sorted Key-Value pairs.
//! 3.  **WAL (Write-Ahead Log)**: (Omitted here for simplicity, but crucial for durability in production).
//!
//! **Write Path (`put`):**
//! 1.  Insert into the MemTable.
//! 2.  If the MemTable exceeds a threshold size, flush it to disk as an SSTable and clear the MemTable.
//!
//! **Read Path (`get`):**
//! 1.  Check the MemTable. If found, return the value.
//! 2.  If not found, search the SSTables (from newest to oldest).
//!
//! **Deletion:**
//! 1.  Since SSTables are immutable, we cannot physically delete a record.
//! 2.  Instead, we write a **Tombstone** (a special marker) to the MemTable.
//! 3.  During a read, if a Tombstone is found, we return `None`.
//!
//! **Complexity:**
//!
//! | Operation | Time            | Space        |
//! | :---      | :---            | :---         |
//! | Write     | O(log M)        | O(1)         |
//! | Read      | O(log M + K * log S) | O(1)    |
//! | Space     | -               | O(Total Data)|
//!
//! *M = items in MemTable, K = number of SSTables, S = items per SSTable.*
//!
use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufReader, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, RwLock};

// =========================================================================================
// Data Structures
// =========================================================================================

/// Defines the core operations of a Key-Value store.
///
/// RUST INSIGHT: Defining a trait first allows us to swap out storage engines
/// (e.g., swapping a simple HashMap for this LSM Tree) without changing the
/// consuming application code.
pub trait KeyValueStore<K, V> {
    fn put(&self, key: K, value: V) -> io::Result<()>;
    fn get(&self, key: &K) -> io::Result<Option<Value>>;
    fn delete(&self, key: K) -> io::Result<()>;
}

pub type Key = Vec<u8>;
pub type Value = Vec<u8>;

/// Represents the value stored in the LSM tree.
/// It can either be actual data or a Tombstone indicating deletion.
#[derive(Debug, Clone, PartialEq)]
pub enum Entry {
    Data(Value),
    Tombstone,
}

/// The in-memory buffer for the LSM tree.
///
/// RUST INSIGHT: Rust's standard library provides a highly optimized `BTreeMap`
/// which is perfect for our MemTable.
struct MemTable {
    map: BTreeMap<Key, Entry>,
    estimated_size: usize,
}

impl MemTable {
    fn new() -> Self {
        Self {
            map: BTreeMap::new(),
            estimated_size: 0,
        }
    }

    fn put(&mut self, key: Key, entry: Entry) {
        // Rough size estimation (ignoring BTreeMap overhead)
        let key_len = key.len();
        let val_len = match &entry {
            Entry::Data(v) => v.len(),
            Entry::Tombstone => 0,
        };

        if let Some(old) = self.map.insert(key, entry) {
            let old_val_len = match old {
                Entry::Data(v) => v.len(),
                Entry::Tombstone => 0,
            };
            self.estimated_size = self.estimated_size + val_len - old_val_len;
        } else {
            self.estimated_size += key_len + val_len;
        }
    }

    fn get(&self, key: &Key) -> Option<Entry> {
        self.map.get(key).cloned()
    }

    fn clear(&mut self) {
        self.map.clear();
        self.estimated_size = 0;
    }
}

/// Represents an immutable Sorted String Table on disk.
/// Format: Sequence of [Key Length (4 bytes) | Value Length (4 bytes) | Key | Value]
/// If Value Length is `u32::MAX`, it represents a Tombstone.
struct SSTable {
    path: PathBuf,
}

const TOMBSTONE_MARKER: u32 = u32::MAX;

impl SSTable {
    /// Creates a new SSTable from a MemTable.
    fn flush_from(memtable: &MemTable, path: PathBuf) -> io::Result<Self> {
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&path)?;

        let mut writer = BufWriter::new(file);

        for (key, entry) in &memtable.map {
            // Write Key Length
            writer.write_all(&(key.len() as u32).to_le_bytes())?;

            match entry {
                Entry::Data(val) => {
                    // Write Value Length
                    writer.write_all(&(val.len() as u32).to_le_bytes())?;
                    // Write Key
                    writer.write_all(key)?;
                    // Write Value
                    writer.write_all(val)?;
                }
                Entry::Tombstone => {
                    // Write Tombstone Marker as Value Length
                    writer.write_all(&TOMBSTONE_MARKER.to_le_bytes())?;
                    // Write Key
                    writer.write_all(key)?;
                    // No Value payload
                }
            }
        }

        writer.flush()?;
        // PRODUCTION NOTE: Should call `sync_all()` for durability.
        writer.into_inner().unwrap().sync_all()?;

        Ok(Self { path })
    }

    /// Searches for a key in this SSTable.
    /// Returns `Ok(Some(Entry))` if found, `Ok(None)` if not found.
    // PRODUCTION NOTE: A real SSTable uses an index (e.g., Sparse Index + Bloom Filter)
    // to find keys in O(log S) or O(1) time without full sequential scans.
    // We implement a basic sequential scan for educational simplicity.
    fn search(&self, target_key: &Key) -> io::Result<Option<Entry>> {
        let file = File::open(&self.path)?;
        let mut reader = BufReader::new(file);

        loop {
            // Read Key Length
            let mut k_len_buf = [0u8; 4];
            match reader.read_exact(&mut k_len_buf) {
                Ok(_) => {}
                Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
                Err(e) => return Err(e),
            }
            let k_len = u32::from_le_bytes(k_len_buf) as usize;

            // Read Value Length (or Tombstone marker)
            let mut v_len_buf = [0u8; 4];
            reader.read_exact(&mut v_len_buf)?;
            let v_len_marker = u32::from_le_bytes(v_len_buf);

            // Read Key
            let mut key = vec![0u8; k_len];
            reader.read_exact(&mut key)?;

            // Compare Key
            if &key == target_key {
                if v_len_marker == TOMBSTONE_MARKER {
                    return Ok(Some(Entry::Tombstone));
                } else {
                    let mut val = vec![0u8; v_len_marker as usize];
                    reader.read_exact(&mut val)?;
                    return Ok(Some(Entry::Data(val)));
                }
            } else {
                // Skip Value if not target
                if v_len_marker != TOMBSTONE_MARKER {
                    reader.seek(SeekFrom::Current(v_len_marker as i64))?;
                }
            }
        }
    }
}

/// The main LSM Tree database struct.
pub struct LsmTree {
    memtable: RwLock<MemTable>,
    sstables: RwLock<Vec<SSTable>>,
    base_path: PathBuf,
    memtable_flush_threshold: usize,
    // Using a counter to generate unique SSTable filenames
    next_sstable_id: Mutex<usize>,
}

impl<K: AsRef<[u8]>, V: AsRef<[u8]>> KeyValueStore<K, V> for LsmTree {
    /// Puts a key-value pair into the database.
    fn put(&self, key: K, value: V) -> io::Result<()> {
        self.insert_entry(key.as_ref().to_vec(), Entry::Data(value.as_ref().to_vec()))
    }

    /// Retrieves a value by key.
    fn get(&self, key: &K) -> io::Result<Option<Value>> {
        let key_bytes = key.as_ref().to_vec();
        // 1. Check MemTable
        {
            let mem = self.memtable.read().unwrap();
            if let Some(entry) = mem.get(&key_bytes) {
                return match entry {
                    Entry::Data(v) => Ok(Some(v)),
                    Entry::Tombstone => Ok(None),
                };
            }
        }

        // 2. Check SSTables (Newest to Oldest)
        let ssts = self.sstables.read().unwrap();
        for sst in ssts.iter() {
            // GOTCHA: End-of-File Handling: When searching an SSTable, catching `UnexpectedEof`
            // is a normal part of reading sequentially if an index isn't present.
            if let Some(entry) = sst.search(&key_bytes)? {
                return match entry {
                    Entry::Data(v) => Ok(Some(v)),
                    // GOTCHA: Tombstone Propagation: If a key is deleted, the tombstone
                    // must mask older values in older SSTables until compaction physically removes them.
                    Entry::Tombstone => Ok(None),
                };
            }
        }

        Ok(None)
    }

    /// Deletes a key from the database.
    fn delete(&self, key: K) -> io::Result<()> {
        self.insert_entry(key.as_ref().to_vec(), Entry::Tombstone)
    }
}

impl LsmTree {
    /// Opens or creates an LSM tree in the specified directory.
    pub fn open<P: AsRef<Path>>(path: P, flush_threshold: usize) -> io::Result<Self> {
        let base_path = path.as_ref().to_path_buf();
        if !base_path.exists() {
            fs::create_dir_all(&base_path)?;
        }

        // PRODUCTION NOTE: In a real system, we would scan the directory for existing SSTables,
        // load them into `sstables` (ordered by creation time), and replay the WAL into the MemTable.
        // For this minimal version, we start fresh but point to the directory.
        let mut max_id = 0;
        let mut sst_paths = Vec::new();

        if base_path.is_dir() {
            for entry in fs::read_dir(&base_path)? {
                let entry = entry?;
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("sst")
                    && let Some(stem) = path.file_stem().and_then(|s| s.to_str())
                        && let Ok(id) = stem.parse::<usize>() {
                            sst_paths.push((id, path.clone()));
                            if id > max_id {
                                max_id = id;
                            }
                        }
            }
        }

        sst_paths.sort_by_key(|&(id, _)| std::cmp::Reverse(id)); // Newest first
        let sstables = sst_paths
            .into_iter()
            .map(|(_, p)| SSTable { path: p })
            .collect();

        Ok(Self {
            memtable: RwLock::new(MemTable::new()),
            sstables: RwLock::new(sstables),
            base_path,
            memtable_flush_threshold: flush_threshold,
            next_sstable_id: Mutex::new(max_id + 1),
        })
    }

    fn insert_entry(&self, key: Key, entry: Entry) -> io::Result<()> {
        let mut needs_flush = false;

        {
            let mut mem = self.memtable.write().unwrap();
            mem.put(key, entry);
            if mem.estimated_size >= self.memtable_flush_threshold {
                needs_flush = true;
            }
        }

        if needs_flush {
            self.flush_memtable()?;
        }

        Ok(())
    }

    /// Flushes the current MemTable to an SSTable on disk.
    fn flush_memtable(&self) -> io::Result<()> {
        let mut mem = self.memtable.write().unwrap();

        // Double check condition under lock
        if mem.map.is_empty() {
            return Ok(());
        }

        let mut next_id = self.next_sstable_id.lock().unwrap();
        let sst_path = self.base_path.join(format!("{}.sst", *next_id));
        *next_id += 1;
        drop(next_id);

        let sstable = SSTable::flush_from(&mem, sst_path)?;

        let mut ssts = self.sstables.write().unwrap();
        // Insert at the beginning (index 0) so newest SSTables are searched first.
        ssts.insert(0, sstable);

        mem.clear();

        Ok(())
    }

    /// Explicitly forces a flush of the MemTable to disk.
    pub fn force_flush(&self) -> io::Result<()> {
        self.flush_memtable()
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `rocksdb`: The industry standard. Employs sophisticated Column Families, Block Caches, Bloom Filters, and background Compaction.
// - `sled`: A purely Rust implementation utilizing a latch-free Bw-tree and log-structured storage.
//
// Missing vs. Production:
// - **Compaction**: The most critical missing feature. SSTables will grow indefinitely. A background thread must merge them, removing tombstones and old values.
// - **Bloom Filters & Sparse Indices**: SSTable `search()` is currently a sequential scan O(N). Bloom filters avoid reading files unnecessarily, and sparse indices allow O(log N) binary search within a file block.
// - **WAL (Write-Ahead Log)**: Without a WAL, any data in the MemTable is lost on crash. Production systems append to a WAL *before* adding to the MemTable.
//
// Benchmarking Note:
// Use `criterion` to benchmark the write throughput. You will see that `put` is extremely fast
// compared to a B-Tree that writes directly to disk, because we only insert into a memory structure
// and optionally do a sequential flush. However, `get` performance will degrade as the number of SSTables
// grows, illustrating the need for Compaction and Bloom Filters.

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir() -> PathBuf {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static COUNTER: AtomicUsize = AtomicUsize::new(0);

        let mut dir = std::env::temp_dir();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let count = COUNTER.fetch_add(1, Ordering::Relaxed);
        dir.push(format!("lsm_test_{}_{}", now, count));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn test_memtable_basic() {
        let dir = temp_dir();
        // High threshold so it doesn't flush automatically
        let db = LsmTree::open(&dir, 1024).unwrap();

        <LsmTree as KeyValueStore<Vec<u8>, Vec<u8>>>::put(&db, b"k1".to_vec(), b"v1".to_vec())
            .unwrap();
        <LsmTree as KeyValueStore<Vec<u8>, Vec<u8>>>::put(&db, b"k2".to_vec(), b"v2".to_vec())
            .unwrap();

        assert_eq!(
            <LsmTree as KeyValueStore<Vec<u8>, Vec<u8>>>::get(&db, &b"k1".to_vec()).unwrap(),
            Some(b"v1".to_vec())
        );
        assert_eq!(
            <LsmTree as KeyValueStore<Vec<u8>, Vec<u8>>>::get(&db, &b"k2".to_vec()).unwrap(),
            Some(b"v2".to_vec())
        );
        assert_eq!(
            <LsmTree as KeyValueStore<Vec<u8>, Vec<u8>>>::get(&db, &b"k3".to_vec()).unwrap(),
            None
        );

        // Delete k1
        <LsmTree as KeyValueStore<Vec<u8>, Vec<u8>>>::delete(&db, b"k1".to_vec()).unwrap();
        assert_eq!(
            <LsmTree as KeyValueStore<Vec<u8>, Vec<u8>>>::get(&db, &b"k1".to_vec()).unwrap(),
            None
        );

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn test_flush_and_sstable_read() {
        let dir = temp_dir();
        // Threshold 0 to force flush on every put
        let db = LsmTree::open(&dir, 0).unwrap();

        <LsmTree as KeyValueStore<Vec<u8>, Vec<u8>>>::put(
            &db,
            b"disk_k1".to_vec(),
            b"disk_v1".to_vec(),
        )
        .unwrap();
        <LsmTree as KeyValueStore<Vec<u8>, Vec<u8>>>::put(
            &db,
            b"disk_k2".to_vec(),
            b"disk_v2".to_vec(),
        )
        .unwrap();

        // Data should now be in SSTables
        let mem = db.memtable.read().unwrap();
        assert!(mem.map.is_empty(), "MemTable should be empty after flush");
        drop(mem);

        let ssts = db.sstables.read().unwrap();
        assert_eq!(ssts.len(), 2, "Should have 2 SSTables");
        drop(ssts);

        assert_eq!(
            <LsmTree as KeyValueStore<Vec<u8>, Vec<u8>>>::get(&db, &b"disk_k1".to_vec()).unwrap(),
            Some(b"disk_v1".to_vec())
        );
        assert_eq!(
            <LsmTree as KeyValueStore<Vec<u8>, Vec<u8>>>::get(&db, &b"disk_k2".to_vec()).unwrap(),
            Some(b"disk_v2".to_vec())
        );

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn test_overwrite_and_tombstone_masking() {
        let dir = temp_dir();
        let db = LsmTree::open(&dir, 1024).unwrap();

        // 1. Put value and flush
        <LsmTree as KeyValueStore<Vec<u8>, Vec<u8>>>::put(&db, b"key".to_vec(), b"val1".to_vec())
            .unwrap();
        db.force_flush().unwrap();

        assert_eq!(
            <LsmTree as KeyValueStore<Vec<u8>, Vec<u8>>>::get(&db, &b"key".to_vec()).unwrap(),
            Some(b"val1".to_vec())
        );

        // 2. Overwrite and flush
        <LsmTree as KeyValueStore<Vec<u8>, Vec<u8>>>::put(&db, b"key".to_vec(), b"val2".to_vec())
            .unwrap();
        db.force_flush().unwrap();

        // Should return the newest value (val2) from the newer SSTable
        assert_eq!(
            <LsmTree as KeyValueStore<Vec<u8>, Vec<u8>>>::get(&db, &b"key".to_vec()).unwrap(),
            Some(b"val2".to_vec())
        );

        // 3. Delete and flush (Tombstone)
        <LsmTree as KeyValueStore<Vec<u8>, Vec<u8>>>::delete(&db, b"key".to_vec()).unwrap();
        db.force_flush().unwrap();

        // Tombstone in newest SSTable should mask older values
        assert_eq!(
            <LsmTree as KeyValueStore<Vec<u8>, Vec<u8>>>::get(&db, &b"key".to_vec()).unwrap(),
            None
        );

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn test_recovery() {
        let dir = temp_dir();

        {
            let db = LsmTree::open(&dir, 1024).unwrap();
            <LsmTree as KeyValueStore<Vec<u8>, Vec<u8>>>::put(
                &db,
                b"persist".to_vec(),
                b"data".to_vec(),
            )
            .unwrap();
            db.force_flush().unwrap();
        }

        // Reopen database
        let db = LsmTree::open(&dir, 1024).unwrap();
        assert_eq!(
            <LsmTree as KeyValueStore<Vec<u8>, Vec<u8>>>::get(&db, &b"persist".to_vec()).unwrap(),
            Some(b"data".to_vec())
        );

        let _ = fs::remove_dir_all(dir);
    }
}
