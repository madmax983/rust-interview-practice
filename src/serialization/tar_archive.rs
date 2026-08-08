//! # Tar Archive Implementation
//!
//! Implements a basic USTAR (Uniform Standard Tape Archive) reader/writer from scratch.
//!
//! **Replaces Crates:** `tar`
//!
//! **Real-world Usage:**
//! - Package distribution (npm `.tgz`, Rust crates)
//! - Docker image layers
//! - Backup systems and file archiving
//!
//! **Why build it yourself?**
//! Implementing a tar archive teaches you about fixed-size block alignment (512 bytes) and
//! historical encoding quirks (like using ASCII octal strings for metadata instead of binary
//! integers). It's a great exercise in parsing continuous byte streams where data and metadata
//! are interleaved.

use std::io::{self, Read, Write};

// =========================================================================================
// Architecture
// =========================================================================================
//
// Structure of a USTAR archive:
// A tar file consists of a series of file entries, terminated by two 512-byte blocks of zeros.
// Each file entry consists of:
// 1. A 512-byte header block containing metadata (name, size, mode, checksum, etc.).
// 2. N blocks of 512 bytes containing the file data (padded with zeros to reach the 512-byte boundary).
//
// Header Block Layout (POSIX USTAR):
// ┌────────────┬──────┬───────────────────────────────┐
// │ Field      │ Size │ Description                   │
// ├────────────┼──────┼───────────────────────────────┤
// │ name       │ 100  │ File name                     │
// │ mode       │ 8    │ File mode (octal)             │
// │ uid        │ 8    │ User ID (octal)               │
// │ gid        │ 8    │ Group ID (octal)              │
// │ size       │ 12   │ File size in bytes (octal)    │
// │ mtime      │ 12   │ Modification time (octal)     │
// │ chksum     │ 8    │ Header checksum (octal)       │
// │ typeflag   │ 1    │ File type ('0' for normal)    │
// │ linkname   │ 100  │ Name of linked file           │
// │ magic      │ 6    │ "ustar\0"                     │
// │ version    │ 2    │ "00"                          │
// │ uname      │ 32   │ User name                     │
// │ gname      │ 32   │ Group name                    │
// │ devmajor   │ 8    │ Device major number           │
// │ devminor   │ 8    │ Device minor number           │
// │ prefix     │ 155  │ File name prefix              │
// │ (padding)  │ 12   │ Zeros to reach 512 bytes      │
// └────────────┴──────┴───────────────────────────────┘
//
// Invariants:
// 1. All operations must read/write in multiples of 512 bytes.
// 2. Numeric metadata fields are encoded as ASCII octal strings terminated by a space or null byte.
// 3. The checksum is the sum of all bytes in the header, assuming the checksum field itself is filled with spaces.

const BLOCK_SIZE: usize = 512;

/// Represents an entry in a tar archive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TarEntry {
    pub name: String,
    pub mode: u32,
    pub size: u64,
    pub mtime: u64,
    pub typeflag: u8,
    pub data: Vec<u8>,
}

impl TarEntry {
    /// Creates a new regular file entry.
    pub fn new_file(name: &str, data: Vec<u8>) -> Self {
        Self {
            name: name.to_string(),
            mode: 0o644,
            size: data.len() as u64,
            mtime: 0,
            typeflag: b'0',
            data,
        }
    }
}

// Helper to format a number as an octal ASCII string in a fixed-size buffer
fn format_octal(buf: &mut [u8], val: u64) {
    let s = format!("{:0>width$o}", val, width = buf.len() - 1);
    let bytes = s.as_bytes();
    let len = bytes.len().min(buf.len() - 1);
    buf[..len].copy_from_slice(&bytes[bytes.len() - len..]);
    buf[len] = 0; // null terminator
}

// Helper to parse an octal ASCII string from a buffer
fn parse_octal(buf: &[u8]) -> Option<u64> {
    let s = std::str::from_utf8(buf).ok()?;
    let trimmed = s.trim_matches(|c| c == ' ' || c == '\0');
    if trimmed.is_empty() {
        return Some(0);
    }
    u64::from_str_radix(trimmed, 8).ok()
}

/// A basic USTAR tar archive writer.
pub struct TarWriter<W: Write> {
    inner: W,
}

impl<W: Write> TarWriter<W> {
    pub fn new(inner: W) -> Self {
        Self { inner }
    }

    /// Writes an entry to the archive.
    pub fn append(&mut self, entry: &TarEntry) -> io::Result<()> {
        let mut header = [0u8; BLOCK_SIZE];

        // Name (100 bytes)
        let name_bytes = entry.name.as_bytes();
        let name_len = name_bytes.len().min(100);
        header[0..name_len].copy_from_slice(&name_bytes[..name_len]);

        // Mode (8 bytes)
        format_octal(&mut header[100..108], entry.mode as u64);

        // UID/GID (8 bytes each, mocked as 0)
        format_octal(&mut header[108..116], 0);
        format_octal(&mut header[116..124], 0);

        // Size (12 bytes)
        format_octal(&mut header[124..136], entry.size);

        // Mtime (12 bytes)
        format_octal(&mut header[136..148], entry.mtime);

        // Typeflag (1 byte)
        header[156] = entry.typeflag;

        // Magic and Version (USTAR)
        header[257..263].copy_from_slice(b"ustar\0");
        header[263..265].copy_from_slice(b"00");

        // Calculate and set checksum (8 bytes)
        // First fill checksum field with spaces
        header[148..156].fill(b' ');
        let checksum: u32 = header.iter().map(|&b| b as u32).sum();

        // Format checksum as octal, ending with a null and space or just null
        let checksum_str = format!("{:06o}\0 ", checksum);
        header[148..156].copy_from_slice(checksum_str.as_bytes());

        // Write header block
        self.inner.write_all(&header)?;

        // Write data blocks
        self.inner.write_all(&entry.data)?;

        // Pad data to 512-byte boundary
        let remainder = entry.data.len() % BLOCK_SIZE;
        if remainder > 0 {
            let padding = vec![0u8; BLOCK_SIZE - remainder];
            self.inner.write_all(&padding)?;
        }

        Ok(())
    }

    /// Finishes the archive by writing two 512-byte blocks of zeros.
    pub fn finish(mut self) -> io::Result<W> {
        let zeros = [0u8; BLOCK_SIZE * 2];
        self.inner.write_all(&zeros)?;
        Ok(self.inner)
    }
}

/// A basic USTAR tar archive reader.
pub struct TarReader<R: Read> {
    inner: R,
}

impl<R: Read> TarReader<R> {
    pub fn new(inner: R) -> Self {
        Self { inner }
    }

    /// Reads all entries from the archive.
    pub fn read_entries(&mut self) -> io::Result<Vec<TarEntry>> {
        let mut entries = Vec::new();

        loop {
            let mut header = [0u8; BLOCK_SIZE];
            let mut read_len = 0;

            while read_len < BLOCK_SIZE {
                let n = self.inner.read(&mut header[read_len..])?;
                if n == 0 {
                    break;
                }
                read_len += n;
            }

            // EOF or truncated
            if read_len == 0 {
                break;
            }
            if read_len < BLOCK_SIZE {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "Truncated tar header",
                ));
            }

            // Two consecutive zero blocks indicate the end of the archive
            if header.iter().all(|&b| b == 0) {
                break;
            }

            // Parse Header
            let name_end = header[0..100].iter().position(|&b| b == 0).unwrap_or(100);
            let name = String::from_utf8_lossy(&header[0..name_end]).to_string();

            let mode = parse_octal(&header[100..108]).unwrap_or(0) as u32;
            let size = parse_octal(&header[124..136]).unwrap_or(0);
            let mtime = parse_octal(&header[136..148]).unwrap_or(0);
            let typeflag = header[156];

            // Verify Checksum
            let mut chksum_header = header;
            chksum_header[148..156].fill(b' ');
            let expected_chksum: u32 = chksum_header.iter().map(|&b| b as u32).sum();
            let actual_chksum = parse_octal(&header[148..156]).unwrap_or(0) as u32;

            if expected_chksum != actual_chksum {
                // Return an error rather than panicking in production
                // For educational brevity we just log/ignore or error
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Checksum mismatch",
                ));
            }

            // Read Data
            let mut data = vec![0u8; size as usize];
            self.inner.read_exact(&mut data)?;

            // Consume Padding
            let remainder = size as usize % BLOCK_SIZE;
            if remainder > 0 {
                let padding_len = BLOCK_SIZE - remainder;
                let mut padding = vec![0u8; padding_len];
                self.inner.read_exact(&mut padding)?;
            }

            entries.push(TarEntry {
                name,
                mode,
                size,
                mtime,
                typeflag,
                data,
            });
        }

        Ok(entries)
    }
}

// =========================================================================================
// Footer
// =========================================================================================
// Comparison to canonical crates:
// - `tar` provides a full-featured, zero-copy (where possible) streaming reader/writer that
//   handles GNU/POSIX extensions (like long file names > 100 chars), hard links, and symlinks.
//
// What's missing:
// - Long file name support (GNU `././@LongLink` or POSIX PAX extended headers).
// - Directory creation and metadata preservation (ownership, extended attributes).
// - Streaming data extraction (this implementation buffers entire files into memory).

#[cfg(test)]
mod tests {
    use super::*;
    use std::hint::black_box;
    use std::time::Instant;

    #[test]
    fn test_format_parse_octal() {
        let mut buf = [0u8; 8];
        format_octal(&mut buf, 0o644);
        assert_eq!(std::str::from_utf8(&buf).unwrap(), "0000644\0");
        assert_eq!(parse_octal(&buf), Some(0o644));
    }

    #[test]
    fn test_tar_roundtrip() {
        let mut archive_data = Vec::new();

        let file1 = TarEntry::new_file("hello.txt", b"Hello, world!".to_vec());
        let file2 = TarEntry::new_file("script.sh", b"echo 'hi'".to_vec());

        {
            let mut writer = TarWriter::new(&mut archive_data);
            writer.append(&file1).unwrap();
            writer.append(&file2).unwrap();
            writer.finish().unwrap();
        }

        // Must be a multiple of 512
        assert_eq!(archive_data.len() % 512, 0);

        let mut reader = TarReader::new(archive_data.as_slice());
        let entries = reader.read_entries().unwrap();

        assert_eq!(entries.len(), 2);

        assert_eq!(entries[0].name, "hello.txt");
        assert_eq!(entries[0].data, b"Hello, world!");

        assert_eq!(entries[1].name, "script.sh");
        assert_eq!(entries[1].data, b"echo 'hi'");
    }

    #[test]
    fn benchmark_tar_creation() {
        let mut archive_data = Vec::with_capacity(1024 * 1024);
        let data = vec![0u8; 100 * 1024]; // 100KB file

        let start = Instant::now();

        let mut writer = TarWriter::new(&mut archive_data);
        for i in 0..10 {
            let entry = TarEntry::new_file(&format!("file_{}.bin", i), data.clone());
            writer.append(black_box(&entry)).unwrap();
        }
        writer.finish().unwrap();

        let elapsed = start.elapsed();
        // Benchmark note: Creating 10x 100KB entries should be extremely fast, primarily dominated
        // by memcpy.
        assert!(elapsed.as_millis() < 100);
    }
}
