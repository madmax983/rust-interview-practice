//! Write-Ahead Log (WAL) implementation.
//!
//! # Header
//!
//! *   **Problem Name**: Write-Ahead Log (WAL)
//! *   **Difficulty**: Hard
//! *   **Link**: <https://en.wikipedia.org/wiki/Write-ahead_logging>
//! *   **Why this matters in Rust**: Critical for database durability (ACID) and crash recovery.
//!
//! # Architecture
//!
//! A WAL provides durability by appending mutations to a log file before applying them to the in-memory state.
//! On crash, the system replays the log to restore the state.
//!
//! **Format per Entry:**
//! `[Checksum (8 bytes) | Length (8 bytes) | Payload (N bytes)]`
//!
//! *   **Checksum**: `u64` hash of the payload using `DefaultHasher`.
//! *   **Length**: `u64` length of the payload.
//! *   **Payload**: The actual data.
//!
//! **Invariants:**
//! *   Entries are immutable once written.
//! *   `flush` guarantees data is on stable storage.
//! *   Corrupted entries (mismatched checksum) stop replay.
//!
//! **Complexity:**
//!
//! | Operation | Time | Space |
//! | :--- | :--- | :--- |
//! | Append | O(1) | O(1) |
//! | Flush | O(Disk Latency) | O(1) |
//! | Replay | O(N) | O(1) |

use std::collections::hash_map::DefaultHasher;
use std::fs::{File, OpenOptions};
use std::hash::{Hash, Hasher};
use std::io::{self, BufReader, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::Path;

pub struct Wal {
    file: File,
    writer: BufWriter<File>,
}

impl Wal {
    /// Opens or creates a WAL at the specified path.
    pub fn open<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(&path)?;

        let writer = BufWriter::new(file.try_clone()?);

        Ok(Self { file, writer })
    }

    /// Appends an entry to the WAL.
    // RUST INSIGHT: We take `&[u8]` which is a slice, avoiding ownership transfer until we write.
    pub fn append(&mut self, payload: &[u8]) -> io::Result<()> {
        let checksum = Self::calculate_checksum(payload);
        let len = payload.len() as u64;

        // Write Checksum
        self.writer.write_all(&checksum.to_le_bytes())?;
        // Write Length
        self.writer.write_all(&len.to_le_bytes())?;
        // Write Payload
        self.writer.write_all(payload)?;

        Ok(())
    }

    /// Flushes the WAL to disk, ensuring durability.
    // GOTCHA: `BufWriter::flush` only flushes to the OS cache. `File::sync_all` is needed for disk durability.
    pub fn flush(&mut self) -> io::Result<()> {
        self.writer.flush()?;
        self.writer.get_ref().sync_all()
    }

    /// Clears the WAL by truncating the file to 0 length.
    pub fn clear(&mut self) -> io::Result<()> {
        // Truncate file
        self.file.set_len(0)?;
        self.file.seek(std::io::SeekFrom::Start(0))?;
        // Re-create writer to reset internal buffer
        self.writer = BufWriter::new(self.file.try_clone()?);
        Ok(())
    }

    /// Replays entries from the WAL.
    // RUST INSIGHT: Returning an iterator would be ideal, but requires careful lifetime management with the file.
    // For simplicity, we return a Vec of payloads.
    // PRODUCTION NOTE: A real WAL would return an iterator to avoid loading everything into RAM.
    pub fn replay(&mut self) -> io::Result<Vec<Vec<u8>>> {
        // Ensure everything is flushed before reading
        self.flush()?;

        let mut reader = BufReader::new(self.file.try_clone()?);
        reader.seek(SeekFrom::Start(0))?;

        // Total on-disk length, used to bounds-check each entry's declared length below.
        let total_len = reader.get_ref().metadata()?.len();

        let mut entries = Vec::new();

        loop {
            // Read Checksum
            let mut checksum_bytes = [0u8; 8];
            match reader.read_exact(&mut checksum_bytes) {
                Ok(()) => {}
                Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => break, // End of file
                Err(e) => return Err(e),
            }
            let expected_checksum = u64::from_le_bytes(checksum_bytes);

            // Read Length
            let mut len_bytes = [0u8; 8];
            if let Err(e) = reader.read_exact(&mut len_bytes) {
                if e.kind() == io::ErrorKind::UnexpectedEof {
                    // Partial write at end of file (checksum written, length missing)
                    break;
                }
                return Err(e);
            }
            let len = u64::from_le_bytes(len_bytes);

            // BUG FIX: The length field is not covered by the checksum, so a corrupt length
            // would trigger an unbounded `vec![0u8; len]` allocation (potential OOM) before we
            // ever get a chance to detect the corruption via the checksum. Bounds-check the
            // declared length against the bytes actually remaining in the file first. If it
            // claims more than exists, treat it as a corrupt/partial entry and stop replay
            // cleanly, preserving the recovered good prefix.
            let pos = reader.stream_position()?;
            let remaining = total_len.saturating_sub(pos);
            if len > remaining {
                break;
            }

            // Read Payload
            let mut payload = vec![0u8; len as usize];
            if let Err(e) = reader.read_exact(&mut payload) {
                if e.kind() == io::ErrorKind::UnexpectedEof {
                    // Partial write at end of file (checksum and length written, payload missing/partial)
                    break;
                }
                return Err(e);
            }

            // Verify Checksum
            let actual_checksum = Self::calculate_checksum(&payload);
            if actual_checksum != expected_checksum {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Checksum mismatch",
                ));
            }

            entries.push(payload);
        }

        Ok(entries)
    }

    fn calculate_checksum(payload: &[u8]) -> u64 {
        let mut hasher = DefaultHasher::new();
        payload.hash(&mut hasher);
        hasher.finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    // Helper to get a temp file path
    fn temp_file() -> String {
        use std::time::{SystemTime, UNIX_EPOCH};
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        format!("wal_test_{}.log", now)
    }

    #[test]
    fn test_wal_append_and_replay() {
        let path = temp_file();
        {
            let mut wal = Wal::open(&path).unwrap();
            wal.append(b"entry1").unwrap();
            wal.append(b"entry2").unwrap();
            wal.flush().unwrap();

            let entries = wal.replay().unwrap();
            assert_eq!(entries.len(), 2);
            assert_eq!(entries[0], b"entry1");
            assert_eq!(entries[1], b"entry2");
        }
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn test_wal_persistence() {
        let path = temp_file();

        // Write and close
        {
            let mut wal = Wal::open(&path).unwrap();
            wal.append(b"persistent").unwrap();
            wal.flush().unwrap();
        }

        // Reopen and read
        {
            let mut wal = Wal::open(&path).unwrap();
            let entries = wal.replay().unwrap();
            assert_eq!(entries.len(), 1);
            assert_eq!(entries[0], b"persistent");
        }

        fs::remove_file(path).unwrap();
    }

    #[test]
    fn test_corrupted_wal() {
        let path = temp_file();

        // Write valid data
        {
            let mut wal = Wal::open(&path).unwrap();
            wal.append(b"valid").unwrap();
            wal.flush().unwrap();
        }

        // Corrupt the file manually
        {
            let mut file = OpenOptions::new().write(true).open(&path).unwrap();
            file.seek(SeekFrom::Start(0)).unwrap();
            file.write_all(b"00000000").unwrap(); // Overwrite checksum
        }

        // Reopen and try to replay
        {
            let mut wal = Wal::open(&path).unwrap();
            let result = wal.replay();
            assert!(result.is_err());
        }

        fs::remove_file(path).unwrap();
    }

    #[test]
    fn test_truncated_wal() {
        let path = temp_file();

        // Write valid data
        {
            let mut wal = Wal::open(&path).unwrap();
            wal.append(b"valid").unwrap();
            wal.flush().unwrap();
        }

        // Simulate a partial write at the end
        {
            let mut file = OpenOptions::new().append(true).open(&path).unwrap();
            // Write a checksum (8 bytes) but no length/payload
            file.write_all(&[1u8; 8]).unwrap();
        }

        // Reopen and replay
        {
            let mut wal = Wal::open(&path).unwrap();
            let entries = wal.replay().unwrap();
            assert_eq!(entries.len(), 1);
            assert_eq!(entries[0], b"valid");
        }

        fs::remove_file(path).unwrap();
    }

    #[test]
    fn test_corrupt_length_does_not_overallocate() {
        // Regression: a corrupt/oversized length field must NOT trigger a huge allocation.
        // Replay should stop cleanly at the bad entry and keep the recovered good prefix.
        let path = temp_file();

        // Write one valid entry.
        {
            let mut wal = Wal::open(&path).unwrap();
            wal.append(b"valid").unwrap();
            wal.flush().unwrap();
        }

        // Append a bogus entry header: 8-byte checksum + a length claiming a huge payload
        // that does not exist on disk, with no payload following.
        {
            let mut file = OpenOptions::new().append(true).open(&path).unwrap();
            file.write_all(&[0u8; 8]).unwrap(); // checksum
            file.write_all(&u64::MAX.to_le_bytes()).unwrap(); // absurd length
        }

        // Replay must return cleanly (no panic, no OOM) with just the good prefix.
        {
            let mut wal = Wal::open(&path).unwrap();
            let entries = wal.replay().unwrap();
            assert_eq!(entries.len(), 1);
            assert_eq!(entries[0], b"valid");
        }

        fs::remove_file(path).unwrap();
    }

    #[test]
    fn test_wal_clear() {
        let path = temp_file();
        let mut wal = Wal::open(&path).unwrap();

        wal.append(b"data").unwrap();
        wal.flush().unwrap();

        let entries = wal.replay().unwrap();
        assert_eq!(entries.len(), 1);

        wal.clear().unwrap();

        let entries = wal.replay().unwrap();
        assert_eq!(entries.len(), 0);

        fs::remove_file(path).unwrap();
    }
}

// Footer
//
// *   **Comparison**: This implements the core logic of `commitlog` or database-specific WALs (like Postgres's WAL).
// *   **Missing features**: Segment rotation (splitting logs into multiple files), concurrent appends, checksum offloading, compression.
