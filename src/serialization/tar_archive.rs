//! # Tar Archive Implementation
//!
//! Implements a basic USTAR (Unix Standard TAR) archive reader and writer.
//!
//! **Replaces Crates:** `tar`
//!
//! **Real-world Usage:**
//! - Unix `tar` utility.
//! - Docker image layer packaging.
//! - npm package publishing (`.tgz` files are gzipped tarballs).
//!
//! **Why build it yourself?**
//! Tar is surprisingly simple yet full of historical quirks. Building a tar implementation
//! teaches you about 512-byte block alignment, octal string encoding for metadata (why?!),
//! and how early file systems represented directories and permissions. It’s an exercise
//! in strictly adhering to a legacy binary format.

use std::io::{self, Read, Write};

// =========================================================================================
// Architecture
// =========================================================================================
//
// Data Structure:
//
//     [Header Block (512 bytes)]
//     [Data Block 1 (512 bytes)]
//     [Data Block 2 (512 bytes)]
//     ...
//     [Padding to 512 bytes]
//     [Header Block (512 bytes)]
//     [Data Block 1 (512 bytes)]
//     ...
//     [End of Archive (1024 zero bytes)]
//
// USTAR Header Layout (512 bytes total):
// 0   - 99  : File name (100 bytes)
// 100 - 107 : File mode (8 bytes, octal string)
// 108 - 115 : Owner UID (8 bytes, octal string)
// 116 - 123 : Group GID (8 bytes, octal string)
// 124 - 135 : File size (12 bytes, octal string)
// 136 - 147 : Last modification time (12 bytes, octal string)
// 148 - 155 : Checksum (8 bytes, octal string)
// 156       : Type flag (1 byte)
// 157 - 256 : Link name (100 bytes)
// 257 - 262 : USTAR indicator "ustar\0" (6 bytes)
// 263 - 264 : USTAR version "00" (2 bytes)
// 265 - 296 : Owner user name (32 bytes)
// 297 - 328 : Owner group name (32 bytes)
// 329 - 336 : Device major number (8 bytes)
// 337 - 344 : Device minor number (8 bytes)
// 345 - 499 : Filename prefix (155 bytes)
// 500 - 511 : Padding (12 bytes)
//
// Invariants:
// 1. Every header and data section is aligned to a 512-byte boundary.
// 2. Metadata values (size, mode, etc.) are ASCII strings representing octal numbers, terminated by space or NUL.
// 3. The checksum is calculated by summing all bytes of the header, with the checksum field itself treated as 8 spaces.
//
// Complexity:
// ┌─────────────┬─────────────┬─────────────┐
// │ Operation   │ Time        │ Space       │
// ├─────────────┼─────────────┼─────────────┤
// │ write_file  │ O(N)        │ O(1)        │
// │ read_file   │ O(N)        │ O(N)        │
// └─────────────┴─────────────┴─────────────┘
// N is the file size. Reading buffers into memory here for simplicity, though a real crate would stream it.
//
// Design Decisions:
// - **In-Memory Reading**: `read_archive` loads the entire file contents into memory.
//   - *Alternative*: Returning an Iterator yielding `impl Read` for each file, which is what the real `tar` crate does to avoid OOM on huge archives.
// - **Octal parsing**: Manual parsing to handle legacy idiosyncrasies (e.g., spaces vs NULs).

/// Represents an entry in a Tar archive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TarEntry {
    pub filename: String,
    pub size: usize,
    pub mode: u32,
    pub mtime: u64,
    pub type_flag: u8,
    pub data: Vec<u8>,
}

/// Helper to write an octal string into a fixed-size buffer.
/// The string is right-aligned, padded with zeros, and terminated with a NUL or space.
fn format_octal(buf: &mut [u8], value: u64) {
    let s = format!("{value:0width$o}", width = buf.len() - 1);
    let bytes = s.as_bytes();
    // Copy the octal string, leaving the last byte as 0 (NUL)
    let len = bytes.len().min(buf.len() - 1);
    buf[..len].copy_from_slice(&bytes[..len]);
    buf[len] = b' '; // Many tar implementations use space instead of NUL for some fields, we'll use space as terminator for safety.
}

/// Helper to parse an octal string from a buffer, stopping at NUL or space.
fn parse_octal(buf: &[u8]) -> io::Result<u64> {
    let mut s = String::new();
    for &b in buf {
        if b == 0 || b == b' ' {
            break;
        }
        if b.is_ascii_digit() {
            s.push(b as char);
        }
    }
    if s.is_empty() {
        return Ok(0);
    }
    u64::from_str_radix(&s, 8).map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Invalid octal"))
}

/// Calculates the checksum of a header block.
fn calculate_checksum(header: &[u8; 512]) -> u64 {
    let mut sum: u64 = 0;
    for (i, &b) in header.iter().enumerate() {
        if (148..156).contains(&i) {
            sum += b' ' as u64;
        } else {
            sum += b as u64;
        }
    }
    sum
}

/// Trait representing a strategy for reading and writing Tar archives.
///
/// Using a trait allows swapping out implementations (e.g., an in-memory streaming strategy vs.
/// one that buffers entire files).
pub trait TarStrategy {
    /// Writes a single file entry into the archive stream.
    fn write_entry<W: Write>(&self, writer: &mut W, filename: &str, data: &[u8], mode: u32, mtime: u64) -> io::Result<()>;

    /// Writes the end-of-archive marker.
    fn write_finish<W: Write>(&self, writer: &mut W) -> io::Result<()>;

    /// Reads all entries from the archive stream.
    fn read_archive<R: Read>(&self, reader: &mut R) -> io::Result<Vec<TarEntry>>;
}

/// A basic Tar archiving strategy that operates in memory.
pub struct BasicTarStrategy;

impl TarStrategy for BasicTarStrategy {
    fn write_entry<W: Write>(
        &self,
        writer: &mut W,
        filename: &str,
        data: &[u8],
        mode: u32,
        mtime: u64,
    ) -> io::Result<()> {
        write_entry(writer, filename, data, mode, mtime)
    }

    fn write_finish<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        write_finish(writer)
    }

    fn read_archive<R: Read>(&self, reader: &mut R) -> io::Result<Vec<TarEntry>> {
        read_archive(reader)
    }
}

/// Writes a single file entry into a TAR archive stream.
///
/// # Errors
/// Returns an `io::Result` if the underlying writer fails.
pub fn write_entry<W: Write>(
    writer: &mut W,
    filename: &str,
    data: &[u8],
    mode: u32,
    mtime: u64,
) -> io::Result<()> {
    let mut header = [0u8; 512];

    // Filename (0-99)
    let name_bytes = filename.as_bytes();
    let name_len = name_bytes.len().min(100);
    header[..name_len].copy_from_slice(&name_bytes[..name_len]);

    // Mode (100-107)
    format_octal(&mut header[100..108], mode as u64);

    // UID (108-115) - Default to 0 (root)
    format_octal(&mut header[108..116], 0);

    // GID (116-123) - Default to 0 (root)
    format_octal(&mut header[116..124], 0);

    // Size (124-135)
    format_octal(&mut header[124..136], data.len() as u64);

    // MTime (136-147)
    format_octal(&mut header[136..148], mtime);

    // Type flag (156) - '0' for normal file
    header[156] = b'0';

    // USTAR indicator (257-262)
    header[257..263].copy_from_slice(b"ustar\0");

    // USTAR version (263-264)
    header[263..265].copy_from_slice(b"00");

    // Checksum (148-155)
    let checksum = calculate_checksum(&header);
    // Format checksum: 6 digits, NUL, space
    let chk_str = format!("{checksum:06o}");
    header[148..154].copy_from_slice(chk_str.as_bytes());
    header[154] = 0;
    header[155] = b' ';

    // Write header
    writer.write_all(&header)?;

    // Write data
    writer.write_all(data)?;

    // Write padding to align to 512 bytes
    let padding_len = (512 - (data.len() % 512)) % 512;
    if padding_len > 0 {
        let padding = vec![0u8; padding_len];
        writer.write_all(&padding)?;
    }

    Ok(())
}

/// Writes the end-of-archive marker (two 512-byte blocks of zeros).
///
/// # Errors
/// Returns an `io::Result` if the underlying writer fails.
pub fn write_finish<W: Write>(writer: &mut W) -> io::Result<()> {
    writer.write_all(&[0u8; 1024])
}

/// Reads all entries from a TAR archive stream.
///
/// # Errors
/// Returns an `io::Result` if the underlying reader fails or if the archive is malformed.
pub fn read_archive<R: Read>(reader: &mut R) -> io::Result<Vec<TarEntry>> {
    let mut entries = Vec::new();

    loop {
        let mut header = [0u8; 512];
        let mut read_bytes = 0;

        while read_bytes < 512 {
            let n = reader.read(&mut header[read_bytes..])?;
            if n == 0 {
                // Unexpected EOF in middle of header, or clean EOF if read_bytes == 0
                if read_bytes == 0 {
                    return Ok(entries);
                }
                return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "Unexpected EOF reading header"));
            }
            read_bytes += n;
        }

        // Check for end of archive marker (all zeros)
        if header.iter().all(|&b| b == 0) {
            break;
        }

        // Parse filename
        let filename_end = header[0..100].iter().position(|&b| b == 0).unwrap_or(100);
        let filename = String::from_utf8_lossy(&header[0..filename_end]).into_owned();

        // Parse metadata
        let mode = parse_octal(&header[100..108])? as u32;
        let size = parse_octal(&header[124..136])? as usize;
        let mtime = parse_octal(&header[136..148])?;
        let type_flag = header[156];

        // RUST INSIGHT: Validating the checksum is crucial because a corrupted header might declare a massive size,
        // causing us to allocate gigabytes of memory or read endlessly.
        let declared_checksum = parse_octal(&header[148..156]).unwrap_or_else(|_| u64::MAX); // fallback if it's corrupt
        let actual_checksum = calculate_checksum(&header);

        // Some tars pad checksums with spaces, some with nulls. We just check numeric equality.
        if declared_checksum != actual_checksum {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "Checksum mismatch"));
        }

        // Read data
        let mut data = vec![0u8; size];
        reader.read_exact(&mut data)?;

        // Consume padding
        let padding_len = (512 - (size % 512)) % 512;
        if padding_len > 0 {
            let mut padding = vec![0u8; padding_len];
            reader.read_exact(&mut padding)?;
        }

        entries.push(TarEntry {
            filename,
            size,
            mode,
            mtime,
            type_flag,
            data,
        });
    }

    Ok(entries)
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `tar`: The canonical Rust crate for tar files. It handles long paths (GNU/POSIX extensions),
//   sparse files, hard links, and streaming extraction to disk securely (preventing zip-slip).
//
// Missing vs. Production:
// - GNU/POSIX Pax extensions for paths > 100 characters.
// - Streaming interface (currently buffers entire files into `Vec<u8>`).
// - Security checks (e.g., rejecting paths like `../../etc/passwd`).
//
// Next Steps:
// 1. Change `read_archive` to return an iterator of `Entry` objects that implement `Read`.
// 2. Add support for Pax extended headers for long file names.
// 3. Implement a safe `extract_to(path)` method that prevents directory traversal attacks.

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_format_parse_octal() {
        let mut buf = [0u8; 8];
        format_octal(&mut buf, 0o644);
        // Space terminated: "0000644 "
        assert_eq!(&buf, b"0000644 ");

        let parsed = parse_octal(&buf).unwrap();
        assert_eq!(parsed, 0o644);
    }

    #[test]
    fn test_tar_roundtrip() {
        let mut archive = Vec::new();

        // Write first file
        write_entry(
            &mut archive,
            "hello.txt",
            b"Hello, Tar!",
            0o644,
            1600000000,
        ).unwrap();

        // Write second file (requires padding)
        let data2 = vec![b'A'; 600];
        write_entry(
            &mut archive,
            "big_file.bin",
            &data2,
            0o755,
            1600000001,
        ).unwrap();

        write_finish(&mut archive).unwrap();

        // Read archive
        let mut cursor = Cursor::new(archive);
        let entries = read_archive(&mut cursor).unwrap();

        assert_eq!(entries.len(), 2);

        assert_eq!(entries[0].filename, "hello.txt");
        assert_eq!(entries[0].size, 11);
        assert_eq!(entries[0].mode, 0o644);
        assert_eq!(entries[0].mtime, 1600000000);
        assert_eq!(entries[0].data, b"Hello, Tar!");

        assert_eq!(entries[1].filename, "big_file.bin");
        assert_eq!(entries[1].size, 600);
        assert_eq!(entries[1].mode, 0o755);
        assert_eq!(entries[1].mtime, 1600000001);
        assert_eq!(entries[1].data, data2);
    }

    #[test]
    fn test_checksum_validation() {
        let mut archive = Vec::new();
        write_entry(
            &mut archive,
            "test.txt",
            b"test",
            0o644,
            0,
        ).unwrap();

        // Corrupt the header checksum
        archive[148] = b'9';

        let mut cursor = Cursor::new(archive);
        let err = read_archive(&mut cursor).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
        assert_eq!(err.to_string(), "Checksum mismatch");
    }
}
