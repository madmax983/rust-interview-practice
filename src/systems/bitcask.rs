//! # Bitcask (Key-Value Store) Implementation
//!
//! # Header
//!
//! *   **Problem Name**: Bitcask Storage Engine
//! *   **Difficulty**: Hard (Systems)
//! *   **Link**: <https://github.com/basho/bitcask/blob/develop/docs/bitcask-intro.pdf>
//! *   **Why this matters in Rust**: Implementing a persistent KV store teaches file I/O, serialization, and index management. Bitcask is the backend for Riak.
//!
//! # Architecture
//!
//! Bitcask is a Log-Structured Hash Table.
//!
//! **Components:**
//! 1.  **Data File (Active)**: Append-only file where all writes go.
//! 2.  **`KeyDir` (In-Memory)**: A Hash Map mapping `Key -> (FileId, ValueSize, ValuePos, Timestamp)`.
//! 3.  **Hint File**: Acceleration structure to rebuild `KeyDir` faster on startup (omitted for brevity).
//!
//! **Write Path:**
//! 1.  Serialize Key, Value, Metadata.
//! 2.  Append to active file.
//! 3.  Update `KeyDir`.
//!
//! **Read Path:**
//! 1.  Lookup Key in `KeyDir` to get position.
//! 2.  Seek to position in file.
//! 3.  Read and deserialize value.
//!
//! **Complexity:**
//!
//! | Operation | Time | Space |
//! | :--- | :--- | :--- |
//! | Write | O(1) | O(1) |
//! | Read | O(1) | O(K) RAM |
//!
//! **Limitations:**
//! *   All keys must fit in RAM.
//! *   Compaction (Merge) is required to reclaim space from updated/deleted keys.

// Truncating usize sizes into the 32-bit on-disk length fields is intentional.
#![allow(clippy::cast_possible_truncation)]
// The keydir/active-file locks are intentionally held across scan/read/write critical sections.
#![allow(clippy::significant_drop_tightening)]

use crc32fast::Hasher;
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

// =========================================================================================
// Data Structures
// =========================================================================================

type Key = Vec<u8>;
type Value = Vec<u8>;

/// Pointer to a value on disk.
#[derive(Debug, Clone, Copy)]
struct EntryLocation {
    value_sz: u32,
    value_pos: u64,
}

/// The main Bitcask instance.
///
/// # Lock-order invariant
///
/// `Bitcask` holds two independent mutexes (`active_file` and `keydir`). To
/// prevent an ABBA deadlock between the read path (`get`) and the write path
/// (`put`/`delete`), **any code that needs both locks MUST acquire `active_file`
/// BEFORE `keydir`**, and never the other way around. Release `keydir` as early
/// as possible when only the file is still needed.
pub struct Bitcask {
    /// In-memory index: Key -> Location
    keydir: Arc<Mutex<HashMap<Key, EntryLocation>>>,
    /// Active file for writing
    active_file: Arc<Mutex<File>>,
}

/// Represents an entry in the log file.
/// Format: [CRC (4) | Tstamp (8) | `KeySz` (4) | `ValSz` (4) | Key | Value]
struct EntryHeader {
    crc: u32,
    timestamp: u64,
    key_sz: u32,
    value_sz: u32,
}

const HEADER_SIZE: usize = 4 + 8 + 4 + 4;
// Use u32::MAX as a sentinel for deletion (Tombstone).
// This means we can't store a value of exactly 4GB size, which is a reasonable limitation.
const TOMBSTONE_VALUE_SZ: u32 = u32::MAX;

impl Bitcask {
    /// Opens or creates a Bitcask store at the given path.
    ///
    /// # Errors
    /// Returns `Err` if the directory cannot be created, the data file cannot be opened,
    /// or the on-disk log cannot be read while rebuilding the in-memory index.
    pub fn open(path: impl AsRef<Path>) -> io::Result<Self> {
        let path = path.as_ref().to_path_buf();
        if !path.exists() {
            std::fs::create_dir_all(&path)?;
        }

        // For simplicity, we just use one file "data.bitcask"
        // In reality, we'd scan directory for existing files to rebuild KeyDir.
        // We'll simulate a fresh start or single file append for this edu implementation.
        let file_path = path.join("data.bitcask");
        let file = OpenOptions::new()
            .create(true)
            .read(true)
            .append(true)
            .open(&file_path)?;

        let store = Self {
            keydir: Arc::new(Mutex::new(HashMap::new())),
            active_file: Arc::new(Mutex::new(file)),
        };

        // If file exists, we should replay it to build KeyDir.
        store.rebuild_keydir(&file_path)?;

        Ok(store)
    }

    /// Rebuilds the in-memory index by scanning the file.
    fn rebuild_keydir(&self, file_path: &Path) -> io::Result<()> {
        let mut file = File::open(file_path)?;
        let mut pos = 0;
        let len = file.metadata()?.len();

        let mut keydir = self.keydir.lock().unwrap();

        while pos < len {
            // Read header
            let mut header_buf = [0u8; HEADER_SIZE];
            if file.read_exact(&mut header_buf).is_err() {
                break; // EOF or partial write
            }

            let header = decode_header(&header_buf);

            // Read Key
            let mut key = vec![0u8; header.key_sz as usize];
            file.read_exact(&mut key)?;

            let value = if header.value_sz == TOMBSTONE_VALUE_SZ {
                // Tombstone has no value payload
                Vec::new()
            } else {
                // Read Value to verify CRC
                let mut buf = vec![0u8; header.value_sz as usize];
                file.read_exact(&mut buf)?;
                buf
            };
            let entry_sz = if header.value_sz == TOMBSTONE_VALUE_SZ {
                HEADER_SIZE as u64 + u64::from(header.key_sz)
            } else {
                HEADER_SIZE as u64 + u64::from(header.key_sz) + u64::from(header.value_sz)
            };

            let mut hasher = Hasher::new();
            hasher.update(&header.timestamp.to_be_bytes());
            hasher.update(&header.key_sz.to_be_bytes());
            hasher.update(&header.value_sz.to_be_bytes());
            hasher.update(&key);
            if header.value_sz != TOMBSTONE_VALUE_SZ {
                hasher.update(&value);
            }
            let computed_crc = hasher.finalize();

            if computed_crc == header.crc {
                // BUG FIX: Check for Tombstone sentinel
                if header.value_sz == TOMBSTONE_VALUE_SZ {
                    keydir.remove(&key);
                } else {
                    // Value position is start + header + key
                    let value_pos = pos + HEADER_SIZE as u64 + u64::from(header.key_sz);

                    keydir.insert(
                        key,
                        EntryLocation {
                            value_sz: header.value_sz,
                            value_pos,
                        },
                    );
                }
            } else {
                // Optional: log CRC mismatch during recovery
                // We just skip corrupt entries and proceed
            }

            pos += entry_sz;
        }

        Ok(())
    }

    /// Stores a key-value pair.
    ///
    /// # Errors
    /// Returns `Err` if writing the entry to the active data file fails.
    ///
    /// # Panics
    /// Panics if the system clock is before the UNIX epoch or if an internal mutex is poisoned.
    // Owned key/value match the standard key-value store API even though this impl only needs borrows.
    #[allow(clippy::needless_pass_by_value)]
    pub fn put(&self, key: Key, value: Value) -> io::Result<()> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let mut hasher = Hasher::new();
        hasher.update(&timestamp.to_be_bytes());
        hasher.update(&(key.len() as u32).to_be_bytes());
        hasher.update(&(value.len() as u32).to_be_bytes());
        hasher.update(&key);
        hasher.update(&value);
        let crc = hasher.finalize();

        let header = EntryHeader {
            crc,
            timestamp,
            key_sz: key.len() as u32,
            value_sz: value.len() as u32,
        };

        let mut file = self.active_file.lock().unwrap();

        // Current position is where this entry starts
        let start_pos = file.seek(SeekFrom::End(0))?;

        // Write Header
        file.write_all(&encode_header(&header))?;
        // Write Key
        file.write_all(&key)?;
        // Write Value
        file.write_all(&value)?;
        file.flush()?;

        // Update KeyDir
        let mut keydir = self.keydir.lock().unwrap();
        // Value position is start + header + key
        let value_pos = start_pos + HEADER_SIZE as u64 + key.len() as u64;

        keydir.insert(
            key,
            EntryLocation {
                value_sz: header.value_sz,
                value_pos,
            },
        );

        Ok(())
    }

    /// Retrieves a value by key.
    ///
    /// # Errors
    /// Returns `Err` if seeking/reading the value from the data file fails or the CRC check fails.
    ///
    /// # Panics
    /// Panics if an internal mutex is poisoned.
    pub fn get(&self, key: &Key) -> io::Result<Option<Value>> {
        // LOCK ORDER: acquire `active_file` BEFORE `keydir` (see the lock-order
        // invariant on `Bitcask`). Previously this method locked `keydir` first
        // and then `active_file`, the opposite of `put`/`delete`, which could
        // deadlock (ABBA) under concurrent access.
        let mut file = self.active_file.lock().unwrap();
        let keydir = self.keydir.lock().unwrap();

        // `EntryLocation` is `Copy`, so copy it out and release `keydir`
        // immediately; the rest of the read only needs the file.
        let Some(loc) = keydir.get(key).copied() else {
            return Ok(None);
        };
        drop(keydir);

        // Go to header position
        file.seek(SeekFrom::Start(
            loc.value_pos - key.len() as u64 - HEADER_SIZE as u64,
        ))?;

        let mut header_buf = [0u8; HEADER_SIZE];
        file.read_exact(&mut header_buf)?;
        let header = decode_header(&header_buf);

        // Go to value position
        file.seek(SeekFrom::Start(loc.value_pos))?;

        let mut value = vec![0u8; loc.value_sz as usize];
        file.read_exact(&mut value)?;

        let mut hasher = Hasher::new();
        hasher.update(&header.timestamp.to_be_bytes());
        hasher.update(&header.key_sz.to_be_bytes());
        hasher.update(&header.value_sz.to_be_bytes());
        hasher.update(key);
        hasher.update(&value);
        let computed_crc = hasher.finalize();

        if computed_crc != header.crc {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "CRC mismatch"));
        }

        Ok(Some(value))
    }

    /// Deletes a key (Writes a tombstone).
    ///
    /// # Errors
    /// Returns `Err` if writing the tombstone entry to the active data file fails.
    ///
    /// # Panics
    /// Panics if the system clock is before the UNIX epoch or if an internal mutex is poisoned.
    // Owned key matches the standard key-value store API even though this impl only needs a borrow.
    #[allow(clippy::needless_pass_by_value)]
    pub fn delete(&self, key: Key) -> io::Result<()> {
        // Write tombstone to file
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let mut hasher = Hasher::new();
        hasher.update(&timestamp.to_be_bytes());
        hasher.update(&(key.len() as u32).to_be_bytes());
        hasher.update(&TOMBSTONE_VALUE_SZ.to_be_bytes());
        hasher.update(&key);
        let crc = hasher.finalize();

        let header = EntryHeader {
            crc,
            timestamp,
            key_sz: key.len() as u32,
            value_sz: TOMBSTONE_VALUE_SZ, // Sentinel for deleted
        };

        let mut file = self.active_file.lock().unwrap();
        file.seek(SeekFrom::End(0))?;
        file.write_all(&encode_header(&header))?;
        file.write_all(&key)?;
        // No value payload for tombstone
        file.flush()?;

        let mut keydir = self.keydir.lock().unwrap();
        keydir.remove(&key);

        Ok(())
    }
}

// Helpers

fn encode_header(h: &EntryHeader) -> [u8; HEADER_SIZE] {
    let mut buf = [0u8; HEADER_SIZE];
    buf[0..4].copy_from_slice(&h.crc.to_be_bytes());
    buf[4..12].copy_from_slice(&h.timestamp.to_be_bytes());
    buf[12..16].copy_from_slice(&h.key_sz.to_be_bytes());
    buf[16..20].copy_from_slice(&h.value_sz.to_be_bytes());
    buf
}

fn decode_header(buf: &[u8; HEADER_SIZE]) -> EntryHeader {
    EntryHeader {
        crc: u32::from_be_bytes(buf[0..4].try_into().unwrap()),
        timestamp: u64::from_be_bytes(buf[4..12].try_into().unwrap()),
        key_sz: u32::from_be_bytes(buf[12..16].try_into().unwrap()),
        value_sz: u32::from_be_bytes(buf[16..20].try_into().unwrap()),
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `bitcask`: Official Rust port often exists but isn't as popular as RocksDB.
// - `sled`: A modern embedded DB in Rust, uses a more complex page-based LsM/bw-tree hybrid.
//
// Missing vs. Production:
// - **CRC Checks**: CRC32 checks are implemented, but recovery process does not fully handle partial records seamlessly (it skips and stops).
// - **Compaction**: The critical "Merge" process that reclaims space is missing.
// - **Multiple Files**: We force a single file. Real Bitcask rotates files when they reach size limit.
// - **Tombstones**: Deletion handling during recovery is simplified.

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::fs;
    use std::path::PathBuf;

    // Helper to create a temp dir without external crates
    fn temp_dir() -> PathBuf {
        let mut dir = env::temp_dir();
        dir.push(format!(
            "bitcask_test_{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn test_basic_put_get() {
        let dir = temp_dir();
        let store = Bitcask::open(&dir).unwrap();

        let key = b"hello".to_vec();
        let val = b"world".to_vec();

        store.put(key.clone(), val.clone()).unwrap();
        let retrieved = store.get(&key).unwrap();

        assert_eq!(retrieved, Some(val));

        // Cleanup
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn test_empty_value_vs_delete() {
        let dir = temp_dir();

        {
            let store = Bitcask::open(&dir).unwrap();
            // Store empty value
            store.put(b"empty".to_vec(), vec![]).unwrap();
            // Store normal value then delete
            store.put(b"deleted".to_vec(), b"val".to_vec()).unwrap();
            store.delete(b"deleted".to_vec()).unwrap();
        }

        // Re-open
        let store = Bitcask::open(&dir).unwrap();

        // Empty value should persist
        assert_eq!(store.get(&b"empty".to_vec()).unwrap(), Some(vec![]));

        // Deleted key should be gone
        assert_eq!(store.get(&b"deleted".to_vec()).unwrap(), None);

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn test_overwrite() {
        let dir = temp_dir();
        let store = Bitcask::open(&dir).unwrap();

        let key = b"key".to_vec();
        store.put(key.clone(), b"val1".to_vec()).unwrap();
        store.put(key.clone(), b"val2".to_vec()).unwrap();

        let retrieved = store.get(&key).unwrap();
        assert_eq!(retrieved, Some(b"val2".to_vec()));

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn test_delete() {
        let dir = temp_dir();
        let store = Bitcask::open(&dir).unwrap();

        let key = b"key".to_vec();
        store.put(key.clone(), b"val".to_vec()).unwrap();
        store.delete(key.clone()).unwrap();

        let retrieved = store.get(&key).unwrap();
        assert_eq!(retrieved, None);

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn test_recovery() {
        let dir = temp_dir();

        {
            let store = Bitcask::open(&dir).unwrap();
            store.put(b"k1".to_vec(), b"v1".to_vec()).unwrap();
            store.put(b"k2".to_vec(), b"v2".to_vec()).unwrap();
        } // store dropped, file closed

        // Re-open
        let store = Bitcask::open(&dir).unwrap();
        assert_eq!(store.get(&b"k1".to_vec()).unwrap(), Some(b"v1".to_vec()));
        assert_eq!(store.get(&b"k2".to_vec()).unwrap(), Some(b"v2".to_vec()));

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn test_recovery_with_delete() {
        let dir = temp_dir();

        {
            let store = Bitcask::open(&dir).unwrap();
            store.put(b"k1".to_vec(), b"v1".to_vec()).unwrap();
            store.delete(b"k1".to_vec()).unwrap();
        }

        // Re-open
        let store = Bitcask::open(&dir).unwrap();
        // Should be None because we deleted it
        assert_eq!(store.get(&b"k1".to_vec()).unwrap(), None);

        let _ = fs::remove_dir_all(dir);
    }

    /// Stress test: many threads concurrently `put`/`get`/`delete`. Before the
    /// lock-order fix, `get` (keydir -> active_file) and `put`/`delete`
    /// (active_file -> keydir) formed an ABBA cycle that could deadlock. We run
    /// the workload in a helper thread and wait on a channel with a timeout, so a
    /// regression fails the test instead of hanging forever.
    #[test]
    fn test_concurrent_get_put_no_deadlock() {
        use std::sync::mpsc;
        use std::thread;
        use std::time::Duration;

        let dir = temp_dir();
        let store = Arc::new(Bitcask::open(&dir).unwrap());

        let (tx, rx) = mpsc::channel();

        let store_for_worker = Arc::clone(&store);
        let coordinator = thread::spawn(move || {
            let mut handles = Vec::new();
            for t in 0..8u32 {
                let store = Arc::clone(&store_for_worker);
                handles.push(thread::spawn(move || {
                    for i in 0..250u32 {
                        let key = format!("k{}-{}", t, i % 16).into_bytes();
                        let val = format!("v{t}-{i}").into_bytes();
                        store.put(key.clone(), val).unwrap();
                        let _ = store.get(&key).unwrap();
                        if i % 5 == 0 {
                            store.delete(key).unwrap();
                        }
                    }
                }));
            }
            for h in handles {
                h.join().unwrap();
            }
            let _ = tx.send(());
        });

        match rx.recv_timeout(Duration::from_secs(30)) {
            Ok(()) => {
                coordinator.join().unwrap();
            }
            Err(_) => panic!(
                "concurrent get/put/delete did not finish within timeout (possible deadlock)"
            ),
        }

        let _ = fs::remove_dir_all(dir);
    }
}
