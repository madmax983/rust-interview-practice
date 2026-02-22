//! # Log-Structured Merge Tree (LSM-Tree) Sketch
//!
//! # Header
//!
//! *   **Problem Name**: LSM-Tree Storage Engine
//! *   **Difficulty**: Hard (Systems Design)
//! *   **Link**: <https://en.wikipedia.org/wiki/Log-structured_merge-tree>
//! *   **Why this matters in Rust**: Basis of modern K-V stores (RocksDB, LevelDB, Cassandra).
//!
//! # Architecture
//!
//! This is a minimal implementation of an LSM-Tree.
//!
//! **Components:**
//! 1.  **Memtable**: In-memory `BTreeMap`. Accepts writes. Sorted by key.
//! 2.  **SSTable (Sorted String Table)**: Immutable, on-disk file. Created when Memtable is full.
//! 3.  **WAL**: Write-Ahead Log for durability.
//! 4.  **Bloom Filter**: Probabilistic structure to skip SSTables.
//!
//! **Read Path (`get`):**
//! 1.  Check Memtable.
//! 2.  Check SSTables (Newest -> Oldest).
//!
//! **Write Path (`put`):**
//! 1.  Write to Memtable.
//! 2.  If Memtable size > Threshold -> Flush to new SSTable.
//!
//! **Format:**
//! SSTables are simple text files: `key,value\n`.
//!
//! # Invariants
//!
//! *   Memtable is always sorted.
//! *   SSTables are immutable and sorted.
//! *   Newer data shadows older data.
//!
//! # Rust Insight
//!
//! *   **BTreeMap**: Provides the sorted in-memory structure out of the box.
//! *   **File I/O**: We use `BufReader` for efficient scanning.
//!
//! # Production Note
//!
//! Real LSM-Trees use:
//! *   **Sparse Index**: In-memory map of `Key -> FileOffset` to allow binary search in blocks.
//! *   **Compaction**: Background threads merging SSTables to reclaim space and improve read speed. (Our `compact` is manual).
//! *   **Binary Format**: Protobuf or custom binary for space efficiency.

use std::collections::{BTreeMap, HashMap};
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::systems::wal::Wal;
use crate::data_structures::bloom_filter::BloomFilter;

pub struct LsmTree {
    memtable: BTreeMap<String, String>,
    memtable_size: usize,
    threshold: usize,
    dir: PathBuf,
    sstables: Vec<PathBuf>, // Sorted Oldest -> Newest (append order).
    wal: Wal,
    bloom_filters: HashMap<PathBuf, BloomFilter<String>>,
}

impl LsmTree {
    /// Opens or creates an LSM Tree in the given directory.
    pub fn new<P: AsRef<Path>>(dir: P, threshold_bytes: usize) -> io::Result<Self> {
        let dir = dir.as_ref().to_path_buf();
        fs::create_dir_all(&dir)?;

        // 1. Open/Create WAL
        let wal_path = dir.join("wal.log");
        let mut wal = Wal::open(&wal_path)?;

        // 2. Replay WAL into Memtable
        // WAL Format: [Key Len (8)] [Key Bytes] [Value Bytes]
        // Value len is implicit: Entry Len - 8 - Key Len.
        let mut memtable = BTreeMap::new();
        let mut memtable_size = 0;

        let wal_entries = wal.replay()?;
        for entry in wal_entries {
            if entry.len() < 8 {
                continue;
            }
            let key_len_bytes: [u8; 8] = entry[0..8].try_into().unwrap();
            let key_len = u64::from_le_bytes(key_len_bytes) as usize;

            if entry.len() < 8 + key_len {
                continue;
            }

            let key_bytes = &entry[8..8+key_len];
            let val_bytes = &entry[8+key_len..];

            let key = String::from_utf8_lossy(key_bytes).to_string();
            let val = String::from_utf8_lossy(val_bytes).to_string();

            if let Some(old_val) = memtable.insert(key.clone(), val.clone()) {
                memtable_size -= old_val.len();
                memtable_size += val.len();
            } else {
                memtable_size += key.len() + val.len();
            }
        }

        // 3. Load existing SSTables
        let mut sstables = Vec::new();
        let mut bloom_filters = HashMap::new();

        if let Ok(entries) = fs::read_dir(&dir) {
            let mut paths: Vec<PathBuf> = entries
                .filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| p.extension().map_or(false, |ext| ext == "sst"))
                .collect();

            // Sort by timestamp extracted from filename
            paths.sort_by_key(|p| {
                let filename = p.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                let timestamp_str = if filename.starts_with("compacted_") {
                    &filename["compacted_".len()..]
                } else {
                    filename
                };
                timestamp_str.parse::<u128>().unwrap_or(0)
            });
            sstables = paths;
        }

        // 4. Load or Rebuild Bloom Filters
        for sst_path in &sstables {
            let filter_path = sst_path.with_extension("filter");
            if filter_path.exists() {
                if let Ok(bf) = BloomFilter::load_from_file(&filter_path) {
                    bloom_filters.insert(sst_path.clone(), bf);
                }
            } else {
                // Self-healing: Rebuild Bloom Filter from SSTable
                if let Ok(file) = File::open(sst_path) {
                    let mut bf = BloomFilter::new(1000, 0.01); // Default sizing
                    let reader = BufReader::new(file);
                    for line in reader.lines() {
                        if let Ok(line) = line {
                            if let Some((k, _)) = line.split_once(',') {
                                bf.add(&k.to_string());
                            }
                        }
                    }
                    let _ = bf.save_to_file(&filter_path);
                    bloom_filters.insert(sst_path.clone(), bf);
                }
            }
        }

        Ok(Self {
            memtable,
            memtable_size,
            threshold: threshold_bytes,
            dir,
            sstables,
            wal,
            bloom_filters,
        })
    }

    /// Writes a key-value pair.
    pub fn put(&mut self, key: String, value: String) -> io::Result<()> {
        let entry_size = key.len() + value.len();

        if self.memtable_size + entry_size > self.threshold && !self.memtable.is_empty() {
            self.flush()?;
        }

        // 1. Write to WAL
        // Format: [Key Len (8)] [Key Bytes] [Value Bytes]
        let mut wal_entry = Vec::new();
        wal_entry.extend_from_slice(&(key.len() as u64).to_le_bytes());
        wal_entry.extend_from_slice(key.as_bytes());
        wal_entry.extend_from_slice(value.as_bytes());

        self.wal.append(&wal_entry)?;

        // 2. Insert into Memtable
        if let Some(old_val) = self.memtable.insert(key.clone(), value.clone()) {
            self.memtable_size -= old_val.len();
            self.memtable_size += value.len();
        } else {
            self.memtable_size += key.len() + value.len();
        }

        Ok(())
    }

    /// Reads a value.
    pub fn get(&self, key: &str) -> io::Result<Option<String>> {
        // 1. Check Memtable
        if let Some(val) = self.memtable.get(key) {
            return Ok(Some(val.clone()));
        }

        // 2. Check SSTables (Reverse order: Newest first)
        for sst_path in self.sstables.iter().rev() {
            // Check Bloom Filter first
            if let Some(bf) = self.bloom_filters.get(sst_path) {
                if !bf.contains(&key.to_string()) {
                    continue;
                }
            }

            if let Some(val) = self.scan_sstable(sst_path, key)? {
                return Ok(Some(val));
            }
        }

        Ok(None)
    }

    /// Manually flush memtable to disk.
    pub fn flush(&mut self) -> io::Result<()> {
        if self.memtable.is_empty() {
            return Ok(());
        }

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();

        let filename = format!("{}.sst", timestamp);
        let sst_path = self.dir.join(filename);

        // 1. Write SSTable
        let file = OpenOptions::new()
            .write(true)
            .create(true)
            .open(&sst_path)?;

        let mut writer = std::io::BufWriter::new(file);

        // We also build the Bloom Filter as we iterate
        let mut bf = BloomFilter::new(self.memtable.len(), 0.01);

        for (k, v) in &self.memtable {
            writeln!(writer, "{},{}", k, v)?;
            bf.add(k);
        }

        writer.flush()?;

        // 2. Write Bloom Filter
        let filter_path = sst_path.with_extension("filter");
        bf.save_to_file(&filter_path)?;

        self.bloom_filters.insert(sst_path.clone(), bf);
        self.sstables.push(sst_path);

        // 3. Clear Memtable and WAL
        self.memtable.clear();
        self.memtable_size = 0;
        self.wal.clear()?;

        Ok(())
    }

    /// Compaction (Sketch): Merge all SSTables into one.
    /// In reality, this would be leveled or tiered compaction.
    pub fn compact(&mut self) -> io::Result<()> {
        if self.sstables.is_empty() {
            return Ok(());
        }

        // 1. Load all data into a giant in-memory map (Naive)
        // Production: K-way merge sort using iterators to avoid RAM explosion.
        let mut merged_map: BTreeMap<String, String> = BTreeMap::new();

        for path in &self.sstables {
            let file = File::open(path)?;
            let reader = BufReader::new(file);
            for line in reader.lines() {
                let line = line?;
                if let Some((k, v)) = line.split_once(',') {
                    merged_map.insert(k.to_string(), v.to_string());
                }
            }
        }

        // 2. Write new SSTable and Bloom Filter
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let new_filename = format!("compacted_{}.sst", timestamp);
        let new_path = self.dir.join(new_filename);

        let file = OpenOptions::new().write(true).create(true).open(&new_path)?;
        let mut writer = std::io::BufWriter::new(file);

        let mut bf = BloomFilter::new(merged_map.len().max(100), 0.01);

        for (k, v) in &merged_map {
            writeln!(writer, "{},{}", k, v)?;
            bf.add(k);
        }
        writer.flush()?;

        let filter_path = new_path.with_extension("filter");
        bf.save_to_file(&filter_path)?;

        // 3. Delete old SSTables and Filters
        for path in &self.sstables {
            fs::remove_file(path)?;
            let fp = path.with_extension("filter");
            if fp.exists() {
                 fs::remove_file(fp)?;
            }
            // Remove from bloom filters map
            self.bloom_filters.remove(path);
        }

        // 4. Update list
        self.sstables.clear();
        self.sstables.push(new_path.clone());
        self.bloom_filters.insert(new_path, bf);

        Ok(())
    }

    // Naive linear scan of SSTable
    fn scan_sstable(&self, path: &Path, key: &str) -> io::Result<Option<String>> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);

        // Since SSTable is sorted, we could stop early if current_key > key.
        for line in reader.lines() {
            let line = line?;
            if let Some((k, v)) = line.split_once(',') {
                if k == key {
                    return Ok(Some(v.to_string()));
                }
                if k > key {
                    // Sorted: if we passed it, it's not here.
                    return Ok(None);
                }
            }
        }

        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_dir() -> String {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        format!("lsm_test_{}", now)
    }

    #[test]
    fn test_memtable_get() {
        let dir = temp_dir();
        let mut lsm = LsmTree::new(&dir, 1000).unwrap();

        lsm.put("key1".to_string(), "val1".to_string()).unwrap();
        assert_eq!(lsm.get("key1").unwrap(), Some("val1".to_string()));

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_flush_and_read() {
        let dir = temp_dir();
        // Low threshold to force flush
        let mut lsm = LsmTree::new(&dir, 5).unwrap();

        // "key1" (4) + "val1" (4) = 8 bytes > 5. Should insert then flush?
        // Logic: if size + entry > threshold -> flush.
        // Initially size 0. 0 + 8 > 5. But !memtable.is_empty() is false.
        // So inserts to memtable. Size = 8.
        lsm.put("key1".to_string(), "val1".to_string()).unwrap();

        // Memtable has key1. Size 8.
        // Next insert:
        lsm.put("key2".to_string(), "val2".to_string()).unwrap();
        // 8 + 8 > 5 and memtable not empty. Flushes key1.
        // Memtable now has key2.

        assert_eq!(lsm.sstables.len(), 1);

        // Read key1 (from SST)
        assert_eq!(lsm.get("key1").unwrap(), Some("val1".to_string()));
        // Read key2 (from Memtable)
        assert_eq!(lsm.get("key2").unwrap(), Some("val2".to_string()));

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_persistence() {
        let dir = temp_dir();
        {
            let mut lsm = LsmTree::new(&dir, 10).unwrap();
            lsm.put("persistent".to_string(), "data".to_string()).unwrap();
            lsm.flush().unwrap();
        }

        // Reopen
        {
            let lsm = LsmTree::new(&dir, 10).unwrap();
            assert_eq!(lsm.get("persistent").unwrap(), Some("data".to_string()));
            assert_eq!(lsm.sstables.len(), 1);
        }

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_compaction() {
        let dir = temp_dir();
        let mut lsm = LsmTree::new(&dir, 100).unwrap();

        lsm.put("a".to_string(), "1".to_string()).unwrap();
        lsm.flush().unwrap();

        lsm.put("a".to_string(), "2".to_string()).unwrap(); // Overwrite in new SST
        lsm.flush().unwrap();

        assert_eq!(lsm.sstables.len(), 2);
        assert_eq!(lsm.get("a").unwrap(), Some("2".to_string()));

        lsm.compact().unwrap();

        assert_eq!(lsm.sstables.len(), 1);
        assert_eq!(lsm.get("a").unwrap(), Some("2".to_string()));

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_crash_recovery() {
        let dir = temp_dir();
        {
            let mut lsm = LsmTree::new(&dir, 100).unwrap();
            lsm.put("key1".to_string(), "val1".to_string()).unwrap();
            // We do NOT flush. Data is only in Memtable and WAL.
        }

        // Simulating crash (lsm dropped). Reopen.
        {
            let lsm = LsmTree::new(&dir, 100).unwrap();
            // Data should be recovered from WAL.
            assert_eq!(lsm.get("key1").unwrap(), Some("val1".to_string()));
            // Should be in memtable
            assert!(!lsm.memtable.is_empty());
        }

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_bloom_filter_integration() {
        let dir = temp_dir();
        let mut lsm = LsmTree::new(&dir, 100).unwrap();

        lsm.put("exists".to_string(), "yes".to_string()).unwrap();
        lsm.flush().unwrap();

        assert_eq!(lsm.get("exists").unwrap(), Some("yes".to_string()));
        assert_eq!(lsm.get("missing").unwrap(), None);

        // Verify .filter file exists
        let sst_path = &lsm.sstables[0];
        let filter_path = sst_path.with_extension("filter");
        assert!(filter_path.exists());

        fs::remove_dir_all(&dir).unwrap();
    }
}
