//! # Tar Archive Implementation
//!
//! Implements a basic USTAR (Unix Standard TAR) archive reader and writer.
//! Demonstrates 512-byte block alignment, octal string encoding for metadata,
//! and parsing binary layouts.
//!
//! **Replaces Crates:** `tar`
//!
//! **Real-world Usage:**
//! - Docker image layer extraction.
//! - Software distribution (tarballs).
//! - Backup systems.
//!
//! **Why build it yourself?**
//! Tar is a surprisingly simple, append-only format that relies strictly on fixed-size
//! 512-byte blocks. Building it teaches you how to map structs to raw byte buffers,
//! handle zero-padded strings, and understand legacy Unix file permissions and types.

use std::collections::HashMap;
use std::io::{self, Read, Write};

// =========================================================================================
// Architecture
// =========================================================================================
//
// Format:
//
//   [Header Block (512 bytes)]
//   [Data Block 1 (512 bytes)]
//   [Data Block 2 (512 bytes)]
//   ... (padding to 512) ...
//   [Header Block (512 bytes)]
//   [Data Block 1 (512 bytes)]
//   [End of Archive (1024 bytes of zeros)]
//
// Header Layout (USTAR):
// Name (100) | Mode (8) | UID (8) | GID (8) | Size (12) | MTime (12) | Checksum (8) | Type (1) ...
// All numeric fields are ASCII octal strings, space or NUL terminated.
//
// Invariants:
// 1. Every entry starts on a 512-byte boundary.
// 2. Data is padded with zeros to the next 512-byte boundary.
// 3. The archive is terminated by two consecutive empty (zero-filled) 512-byte blocks.
//
// Complexity:
// ┌──────────────────┬─────────────┬─────────────┐
// │ Operation        │ Time        │ Space       │
// ├──────────────────┼─────────────┼─────────────┤
// │ Create entry     │ O(N)        │ O(N)        │
// │ Read entry       │ O(N)        │ O(N)        │
// └──────────────────┴─────────────┴─────────────┘
// N: Size of the file payload.

const BLOCK_SIZE: usize = 512;

/// A simple Tar Archive representation.
#[derive(Debug, Default)]
pub struct TarArchive {
    entries: HashMap<String, Vec<u8>>,
}

impl TarArchive {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a file to the archive.
    pub fn add_file(&mut self, path: &str, data: Vec<u8>) {
        self.entries.insert(path.to_string(), data);
    }

    /// Retrieves a file from the archive.
    #[must_use]
    pub fn get_file(&self, path: &str) -> Option<&[u8]> {
        self.entries.get(path).map(std::vec::Vec::as_slice)
    }

    /// Writes the archive out to a writer.
    ///
    /// # Errors
    /// Returns an I/O error if writing fails.
    pub fn write_to<W: Write>(&self, mut writer: W) -> io::Result<()> {
        for (path, data) in &self.entries {
            // Write Header
            let header = Self::create_header(path, data.len());
            writer.write_all(&header)?;

            // Write Data
            writer.write_all(data)?;

            // Pad data to 512 bytes
            let padding_len = (BLOCK_SIZE - (data.len() % BLOCK_SIZE)) % BLOCK_SIZE;
            if padding_len > 0 {
                writer.write_all(&vec![0; padding_len])?;
            }
        }

        // End of archive marker: Two 512-byte blocks of zeros.
        writer.write_all(&[0; BLOCK_SIZE * 2])?;
        Ok(())
    }

    /// Reads an archive from a reader.
    ///
    /// # Errors
    /// Returns an I/O error if reading fails or the format is invalid.
    pub fn read_from<R: Read>(mut reader: R) -> io::Result<Self> {
        let mut archive = Self::new();
        let mut block = [0u8; BLOCK_SIZE];

        loop {
            let bytes_read = reader.read(&mut block)?;
            if bytes_read == 0 {
                break; // EOF
            }
            if bytes_read != BLOCK_SIZE {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "Incomplete block",
                ));
            }

            // Check for end of archive (all zeros)
            if block.iter().all(|&b| b == 0) {
                // Technically need to read one more zero block, but we can just stop.
                break;
            }

            // Parse header
            let name = Self::parse_string(&block[0..100]);
            let size = Self::parse_octal(&block[124..136]);

            // RUST INSIGHT: We use `take` on a `by_ref` reader to read exactly `size` bytes,
            // preventing us from over-reading into the next block.
            let mut data = vec![0; size];
            reader.read_exact(&mut data)?;

            archive.entries.insert(name, data);

            // Skip padding
            let padding_len = (BLOCK_SIZE - (size % BLOCK_SIZE)) % BLOCK_SIZE;
            if padding_len > 0 {
                let mut padding = vec![0; padding_len];
                reader.read_exact(&mut padding)?;
            }
        }

        Ok(archive)
    }

    /// Creates a 512-byte USTAR header block for a file.
    fn create_header(path: &str, size: usize) -> [u8; BLOCK_SIZE] {
        let mut header = [0u8; BLOCK_SIZE];

        // Name (100)
        let name_bytes = path.as_bytes();
        let name_len = name_bytes.len().min(100);
        header[0..name_len].copy_from_slice(&name_bytes[0..name_len]);

        // Mode (8) - Default to 0644 (octal)
        Self::write_octal(&mut header[100..108], 0o644);
        // UID (8) - Default 1000
        Self::write_octal(&mut header[108..116], 1000);
        // GID (8) - Default 1000
        Self::write_octal(&mut header[116..124], 1000);
        // Size (12)
        Self::write_octal(&mut header[124..136], size);
        // MTime (12) - Default 0
        Self::write_octal(&mut header[136..148], 0);

        // Type (1) - '0' for normal file
        header[156] = b'0';

        // Magic (6) + Version (2) (USTAR)
        header[257..263].copy_from_slice(b"ustar\0");
        header[263..265].copy_from_slice(b"00");

        // Calculate Checksum (8 bytes at 148, filled with spaces during calc)
        header[148..156].copy_from_slice(b"        ");
        let checksum: usize = header.iter().map(|&b| b as usize).sum();

        // Write the calculated checksum back into the header
        let checksum_str = format!("{checksum:06o}\0 ");
        header[148..156].copy_from_slice(checksum_str.as_bytes());

        header
    }

    /// Helper to write an octal string into a fixed buffer.
    fn write_octal(buf: &mut [u8], value: usize) {
        let octal_str = format!("{value:o}");
        let len = octal_str.len();
        let max_len = buf.len() - 1; // Leave room for NUL/Space

        if len > max_len {
            // Value too large, just fill with zeros
            buf.fill(b'0');
            buf[max_len] = 0;
            return;
        }

        // Pad with leading zeros
        let pad = max_len - len;
        for item in buf.iter_mut().take(pad) {
            *item = b'0';
        }

        // Write the octal digits
        buf[pad..max_len].copy_from_slice(octal_str.as_bytes());

        // Space termination
        buf[max_len] = b' ';
    }

    /// Parses a NUL-terminated string from a buffer.
    fn parse_string(buf: &[u8]) -> String {
        let end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
        String::from_utf8_lossy(&buf[0..end]).into_owned()
    }

    /// Parses an octal string from a buffer.
    fn parse_octal(buf: &[u8]) -> usize {
        let s = Self::parse_string(buf);
        let s = s.trim();
        usize::from_str_radix(s, 8).unwrap_or(0)
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `tar`: The standard tar crate handles long file names (GNU extensions),
//   hard links, symlinks, directory creation, and handles I/O transparently.
//
// Missing vs. Production:
// - **Path Length Limits**: This implementation truncates paths > 100 chars. Real tar
//   uses PAX extended headers or GNU LongName blocks.
// - **Streaming**: We load entire files into memory. Real archives stream blocks.
// - **File Types**: Only supports normal files, not directories or symlinks.
//
// Next Steps:
// 1. Support directories by handling typeflag '5'.
// 2. Change `entries` to stream instead of holding `Vec<u8>`.

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_tar_archive_roundtrip() {
        let mut archive = TarArchive::new();
        archive.add_file("hello.txt", b"Hello, World!".to_vec());
        archive.add_file("test/dir/file.bin", vec![0, 1, 2, 3, 4, 5]);

        let mut buf = Vec::new();
        archive.write_to(&mut buf).unwrap();

        // Should be at least two data blocks and two end blocks
        assert!(buf.len() >= BLOCK_SIZE * 4);

        let parsed_archive = TarArchive::read_from(Cursor::new(buf)).unwrap();

        assert_eq!(
            parsed_archive.get_file("hello.txt"),
            Some(b"Hello, World!".as_slice())
        );
        assert_eq!(
            parsed_archive.get_file("test/dir/file.bin"),
            Some(vec![0, 1, 2, 3, 4, 5].as_slice())
        );
    }

    #[test]
    fn test_tar_archive_large_file() {
        // Create a file larger than one block
        let data = vec![42; 1000];

        let mut archive = TarArchive::new();
        archive.add_file("large.dat", data.clone());

        let mut buf = Vec::new();
        archive.write_to(&mut buf).unwrap();

        let parsed_archive = TarArchive::read_from(Cursor::new(buf)).unwrap();
        assert_eq!(parsed_archive.get_file("large.dat"), Some(data.as_slice()));
    }
}
