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
//! 3.  **WAL**: (Omitted for brevity, but crucial for durability).
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
//! *   **Bloom Filters**: To avoid reading SSTables for non-existent keys.
//! *   **Sparse Index**: In-memory map of `Key -> FileOffset` to allow binary search in blocks.
//! *   **Compaction**: Background threads merging SSTables to reclaim space and improve read speed.
//! *   **Binary Format**: Protobuf or custom binary for space efficiency.

use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub struct LsmTree {
    memtable: BTreeMap<String, String>,
    memtable_size: usize,
    threshold: usize,
    dir: PathBuf,
    sstables: Vec<PathBuf>, // Sorted Newest -> Oldest? No, Oldest -> Newest usually.
                            // But for search we want Newest first.
                            // Let's store Oldest -> Newest (append order).
}

impl LsmTree {
    /// Opens or creates an LSM Tree in the given directory.
    pub fn new<P: AsRef<Path>>(dir: P, threshold_bytes: usize) -> io::Result<Self> {
        let dir = dir.as_ref().to_path_buf();
        fs::create_dir_all(&dir)?;

        // Load existing SSTables
        let mut sstables = Vec::new();
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

        Ok(Self {
            memtable: BTreeMap::new(),
            memtable_size: 0,
            threshold: threshold_bytes,
            dir,
            sstables,
        })
    }

    /// Writes a key-value pair.
    pub fn put(&mut self, key: String, value: String) -> io::Result<()> {
        let entry_size = key.len() + value.len();

        // Check if we need to flush BEFORE inserting?
        // Usually we insert then check, or check then flush.
        // If we flush, we clear memtable.

        if self.memtable_size + entry_size > self.threshold && !self.memtable.is_empty() {
            self.flush()?;
        }

        // Insert into memtable
        // If key exists, we update size diff
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
        let path = self.dir.join(filename);

        let file = OpenOptions::new()
            .write(true)
            .create(true)
            .open(&path)?;

        let mut writer = std::io::BufWriter::new(file);

        for (k, v) in &self.memtable {
            // Simple CSV-like format: key,value
            // Escaping is needed for real production, here we assume alphanumeric simple keys for sketch.
            writeln!(writer, "{},{}", k, v)?;
        }

        writer.flush()?;

        self.sstables.push(path);
        self.memtable.clear();
        self.memtable_size = 0;

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

        // 2. Write new SSTable
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let new_filename = format!("compacted_{}.sst", timestamp);
        let new_path = self.dir.join(new_filename);

        let file = OpenOptions::new().write(true).create(true).open(&new_path)?;
        let mut writer = std::io::BufWriter::new(file);

        for (k, v) in &merged_map {
            writeln!(writer, "{},{}", k, v)?;
        }
        writer.flush()?;

        // 3. Delete old SSTables
        for path in &self.sstables {
            fs::remove_file(path)?;
        }

        // 4. Update list
        self.sstables.clear();
        self.sstables.push(new_path);

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
}
