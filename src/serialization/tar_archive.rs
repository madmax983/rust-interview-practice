//! # Tar Archive Implementation
//!
//! Implements a basic USTAR (Unix Standard TAR) archive reader and writer.
//! It demonstrates 512-byte block alignment, octal string metadata encoding,
//! and raw checksum computation for header integrity.
//!
//! **Replaces Crates:** `tar`
//!
//! **Real-world Usage:**
//! - Package distribution (npm, pip, cargo)
//! - Docker image layers
//! - Backup systems
//!
//! **Why build it yourself?**
//! Implementing TAR is the perfect exercise for understanding raw byte manipulation,
//! fixed-size buffering, and legacy format constraints (like octal strings for integers).
//! You'll learn how to write a format that requires strict 512-byte alignment and
//! how to manage state across stream reads and writes in Rust.

use std::io::{self, Read, Write};

// =========================================================================================
// Architecture
// =========================================================================================
//
// Format Structure (USTAR):
// A TAR file is a sequence of files. Each file consists of:
// - A 512-byte Header block.
// - 0 or more 512-byte Data blocks (padded with zeros).
//
// The archive is terminated by at least two consecutive 512-byte blocks filled with zeros.
//
// Header Layout (512 bytes):
// - Name (100)
// - Mode (8) - Octal string + \0
// - UID (8)
// - GID (8)
// - Size (12) - Octal string + \0
// - MTime (12)
// - Checksum (8) - Octal string + \0 + space
// - TypeFlag (1)
// - LinkName (100)
// - Magic (6) - "ustar\0"
// - Version (2) - "00"
// - UName (32)
// - GName (32)
// - DevMajor (8)
// - DevMinor (8)
// - Prefix (155)
// - Padding (12)
//
// Time/Space Complexity:
// - Writer: O(N) time and O(1) extra space (streaming), where N is the length of data block. 512 bytes space to compose header.
// - Reader: O(N) time to read stream payload into O(N) `Vec<u8>`, where N is the payload size.
//
// Design Decisions & Tradeoffs:
// - Strings: TAR uses null-terminated ASCII. We strictly truncate/pad rather than fail
//   on oversized strings for simplicity, though a robust parser would return an error.
// - Streaming: We consume `std::io::Read` and `std::io::Write` trait objects to allow
//   for in-memory or file-backed streaming, avoiding fully buffering large archives.

pub const BLOCK_SIZE: usize = 512;

/// Supported types of entries in the TAR archive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntryType {
    Regular,
    Directory,
}

impl EntryType {
    fn to_byte(&self) -> u8 {
        match self {
            EntryType::Regular => b'0',
            EntryType::Directory => b'5',
        }
    }

    fn from_byte(b: u8) -> Self {
        match b {
            b'5' => EntryType::Directory,
            _ => EntryType::Regular, // Treat '0', '\0', or anything else as Regular
        }
    }
}

/// Metadata for a single file or directory in the archive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Header {
    pub name: String,
    pub size: u64,
    pub mode: u32,
    pub entry_type: EntryType,
}

// =========================================================================================
// Traits
// =========================================================================================

// RUST INSIGHT:
// By abstracting over `io::Read` and `io::Write` instead of concrete types,
// our writer/reader can work seamlessly with files, network streams, or in-memory vectors.

pub trait ArchiveWriter {
    /// Appends a new file entry and its contents to the archive.
    fn append_data(&mut self, header: &Header, data: &[u8]) -> io::Result<()>;
    /// Finalizes the archive by writing the required two zero-filled blocks.
    fn finish(&mut self) -> io::Result<()>;
}

pub trait ArchiveReader {
    /// Reads the next entry's header and payload from the archive.
    /// Returns `None` when the end of the archive (zero blocks) is reached.
    fn next_entry(&mut self) -> io::Result<Option<(Header, Vec<u8>)>>;
}

// =========================================================================================
// Writer Implementation
// =========================================================================================

pub struct TarWriter<W: Write> {
    writer: W,
    finished: bool,
}

impl<W: Write> TarWriter<W> {
    pub fn new(writer: W) -> Self {
        Self {
            writer,
            finished: false,
        }
    }
}

impl<W: Write> ArchiveWriter for TarWriter<W> {
    fn append_data(&mut self, header: &Header, data: &[u8]) -> io::Result<()> {
        if self.finished {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                "Cannot write to finished archive",
            ));
        }

        let mut block = [0u8; BLOCK_SIZE];

        // 1. Name (100)
        let name_bytes = header.name.as_bytes();
        let name_len = name_bytes.len().min(99);
        block[0..name_len].copy_from_slice(&name_bytes[0..name_len]);

        // 2. Mode (8): e.g., "0000644\0"
        write_octal(&mut block[100..108], header.mode as u64);

        // 3. UID (8) - Hardcoded 0
        write_octal(&mut block[108..116], 0);

        // 4. GID (8) - Hardcoded 0
        write_octal(&mut block[116..124], 0);

        // 5. Size (12)
        write_octal(&mut block[124..136], header.size);

        // 6. MTime (12) - Hardcoded 0 for reproducibility
        write_octal(&mut block[136..148], 0);

        // 7. Checksum (8) - Placeholder spaces
        block[148..156].fill(b' ');

        // 8. TypeFlag (1)
        block[156] = header.entry_type.to_byte();

        // 9. LinkName (100) - Empty
        // 10. Magic (6) - "ustar\0"
        block[257..263].copy_from_slice(b"ustar\0");

        // 11. Version (2) - "00"
        block[263..265].copy_from_slice(b"00");

        // 12. UName (32) - "root"
        block[265..269].copy_from_slice(b"root");

        // 13. GName (32) - "root"
        block[297..301].copy_from_slice(b"root");

        // Calculate and write the checksum
        let checksum = compute_checksum(&block);
        let chksum_str = format!("{:06o}\0 ", checksum);
        block[148..156].copy_from_slice(chksum_str.as_bytes());

        // Write header
        self.writer.write_all(&block)?;

        // Write data if present
        if header.size > 0 {
            self.writer.write_all(data)?;

            // Pad up to 512 bytes
            let remainder = data.len() % BLOCK_SIZE;
            if remainder != 0 {
                let padding = BLOCK_SIZE - remainder;
                let zeroes = vec![0u8; padding];
                self.writer.write_all(&zeroes)?;
            }
        }

        Ok(())
    }

    fn finish(&mut self) -> io::Result<()> {
        if !self.finished {
            let zeros = [0u8; BLOCK_SIZE * 2];
            self.writer.write_all(&zeros)?;
            self.finished = true;
        }
        Ok(())
    }
}

// =========================================================================================
// Reader Implementation
// =========================================================================================

pub struct TarReader<R: Read> {
    reader: R,
}

impl<R: Read> TarReader<R> {
    pub fn new(reader: R) -> Self {
        Self { reader }
    }
}

impl<R: Read> ArchiveReader for TarReader<R> {
    fn next_entry(&mut self) -> io::Result<Option<(Header, Vec<u8>)>> {
        let mut block = [0u8; BLOCK_SIZE];

        // Try reading header block
        match self.reader.read_exact(&mut block) {
            Ok(_) => {}
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
            Err(e) => return Err(e),
        }

        // Two empty blocks mean EOF. If the name is null, we assume it's the end.
        if block[0] == 0 {
            return Ok(None);
        }

        // Parse Name (null-terminated)
        let name_len = block[0..100].iter().position(|&b| b == 0).unwrap_or(100);
        let name = String::from_utf8_lossy(&block[0..name_len]).into_owned();

        // Parse Mode
        let mode = read_octal(&block[100..108]).unwrap_or(0) as u32;

        // Parse Size
        let size = read_octal(&block[124..136]).unwrap_or(0);

        // Parse TypeFlag
        let entry_type = EntryType::from_byte(block[156]);

        let header = Header {
            name,
            size,
            mode,
            entry_type,
        };

        // Read payload
        let mut payload = vec![0u8; size as usize];
        if size > 0 {
            self.reader.read_exact(&mut payload)?;

            // Consume padding
            let remainder = (size as usize) % BLOCK_SIZE;
            if remainder != 0 {
                let padding = BLOCK_SIZE - remainder;
                let mut pad_buf = vec![0u8; padding];
                self.reader.read_exact(&mut pad_buf)?;
            }
        }

        Ok(Some((header, payload)))
    }
}

// =========================================================================================
// Helpers
// =========================================================================================

/// Writes a number as an octal string into the given buffer, terminated by `\0`.
fn write_octal(buf: &mut [u8], value: u64) {
    let len = buf.len();
    // Leave room for null terminator
    let octal_str = format!("{:0width$o}", value, width = len - 1);
    let bytes = octal_str.as_bytes();

    // Copy the bytes. If the value is too large, it truncates (unsafe in real TAR, fine for toy)
    let copy_len = bytes.len().min(len - 1);
    buf[len - 1 - copy_len..len - 1].copy_from_slice(&bytes[..copy_len]);
    buf[len - 1] = 0; // Null terminator
}

/// Reads a null or space-terminated octal string from a buffer.
fn read_octal(buf: &[u8]) -> Option<u64> {
    let mut end = buf.len();
    for (i, &b) in buf.iter().enumerate() {
        if b == 0 || b == b' ' {
            end = i;
            break;
        }
    }

    let s = std::str::from_utf8(&buf[..end]).ok()?;
    u64::from_str_radix(s.trim(), 8).ok()
}

/// Computes the USTAR checksum (sum of all bytes, with the checksum field itself treated as spaces).
fn compute_checksum(block: &[u8; BLOCK_SIZE]) -> u64 {
    let mut sum: u64 = 0;
    for (i, &b) in block.iter().enumerate() {
        if (148..156).contains(&i) {
            sum += b' ' as u64;
        } else {
            sum += b as u64;
        }
    }
    sum
}

// =========================================================================================
// Footer & Extensions
// =========================================================================================
//
// Comparison to `tar` crate:
// - The canonical `tar` crate handles sparse files, long file names (GNU or PAX extensions),
//   and hard/symlinks robustly.
// - It correctly propagates I/O errors and provides safe abstractions for path checking
//   (preventing zip-slip vulnerabilities).
//
// Missing vs Production:
// - Long name support (names > 100 chars).
// - Path sanitization on extraction to prevent directory traversal attacks.
// - Handling of sparse files.
//
// Suggested Next Steps:
// - Add support for Pax header extensions to support arbitrarily long file names.
// - Add a `tar` command line tool that can extract and compress directories using `std::fs`.

#[cfg(test)]
mod tests {
    use super::*;
    use std::hint::black_box;
    use std::time::Instant;

    #[test]
    fn test_tar_roundtrip() {
        let mut buf = Vec::new();

        let header1 = Header {
            name: "hello.txt".to_string(),
            size: 11,
            mode: 0o644,
            entry_type: EntryType::Regular,
        };
        let data1 = b"hello world";

        let header2 = Header {
            name: "dir/".to_string(),
            size: 0,
            mode: 0o755,
            entry_type: EntryType::Directory,
        };

        // Write
        {
            let mut writer = TarWriter::new(&mut buf);
            writer.append_data(&header1, data1).unwrap();
            writer.append_data(&header2, &[]).unwrap();
            writer.finish().unwrap();
        }

        assert!(
            buf.len() > BLOCK_SIZE * 2,
            "Archive should have content plus zero blocks"
        );

        // Read
        let mut reader = TarReader::new(io::Cursor::new(buf));

        let (read_h1, read_d1) = reader.next_entry().unwrap().unwrap();
        assert_eq!(read_h1, header1);
        assert_eq!(read_d1, data1);

        let (read_h2, read_d2) = reader.next_entry().unwrap().unwrap();
        assert_eq!(read_h2, header2);
        assert!(read_d2.is_empty());

        // EOF
        assert!(reader.next_entry().unwrap().is_none());
    }

    #[test]
    fn test_tar_checksum() {
        let mut buf = Vec::new();
        let mut writer = TarWriter::new(&mut buf);
        let header = Header {
            name: "test.txt".to_string(),
            size: 0,
            mode: 0,
            entry_type: EntryType::Regular,
        };
        writer.append_data(&header, &[]).unwrap();

        let sum = compute_checksum(&buf[0..BLOCK_SIZE].try_into().unwrap());
        // Verify the parsed checksum matches the block's checksum calculation
        let chksum_str = std::str::from_utf8(&buf[148..154]).unwrap();
        let parsed_sum = u64::from_str_radix(chksum_str, 8).unwrap();

        assert_eq!(sum, parsed_sum);
    }

    #[test]
    fn test_tar_benchmark() {
        let mut buf = Vec::with_capacity(BLOCK_SIZE * 20_000);
        let header = Header {
            name: "bench.txt".to_string(),
            size: 5,
            mode: 0o644,
            entry_type: EntryType::Regular,
        };
        let data = b"bench";

        let start = Instant::now();
        {
            let mut writer = TarWriter::new(&mut buf);
            for _ in 0..10_000 {
                black_box(
                    writer
                        .append_data(black_box(&header), black_box(data))
                        .unwrap(),
                );
            }
            writer.finish().unwrap();
        }
        let duration = start.elapsed();
        println!("tar_archive: wrote 10,000 files in {:?}", duration);
        assert!(duration.as_millis() < 5000);
    }
}
