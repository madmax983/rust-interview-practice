//! # Tar Archive Implementation
//!
//! Implements a minimal USTAR archive reader and writer from scratch.
//! It handles reading and writing 512-byte aligned blocks and octal string encoding for metadata.
//!
//! **Replaces Crates:** `tar`
//!
//! **Real-world Usage:**
//! - Package distribution (NPM, Cargo, pip).
//! - Docker image layers.
//! - Backup systems and file archiving.
//!
//! **Why build it yourself?**
//! Implementing a tar archive teaches you about historical binary formats, the necessity of padding
//! and block alignment (512-byte blocks), and how to encode numeric metadata (permissions, sizes, timestamps)
//! as null-terminated ASCII octal strings. You will understand how simple streaming formats work without a central index.

use std::io::{self, Read, Error, ErrorKind};

// =========================================================================================
// Architecture
// =========================================================================================
//
// Tar Archive Structure (Stream of 512-byte blocks):
//
// ┌──────────────────────────┐
// │ File 1 Header (512B)     │
// ├──────────────────────────┤
// │ File 1 Content (Block 1) │
// ├──────────────────────────┤
// │ File 1 Content (Block 2) │
// ├──────────────────────────┤
// │ File 2 Header (512B)     │
// ├──────────────────────────┤
// │ File 2 Content (Block 1) │
// ├──────────────────────────┤
// │ Zero Block (512B)        │
// ├──────────────────────────┤
// │ Zero Block (512B)        │
// └──────────────────────────┘
//
// Invariants:
// 1. Every header and content block is exactly 512 bytes.
// 2. The end of the archive is marked by at least two consecutive 512-byte blocks of zeros.
// 3. Numbers (size, mtime, mode) are encoded as ASCII octal strings padded with spaces or zeros.
//
// Complexity:
// ┌────────────────┬────────────────┬────────────────┐
// │ Operation      │ Time           │ Space          │
// ├────────────────┼────────────────┼────────────────┤
// │ Read Header    │ O(1)           │ O(1)           │
// │ Read File      │ O(Size)        │ O(Size)        │
// │ Write File     │ O(Size)        │ O(Size)        │
// └────────────────┴────────────────┴────────────────┘
//
// Design Decisions:
// - **In-Memory Buffering**: We read file contents into memory (`Vec<u8>`).
//   - *Tradeoff*: Simple but uses a lot of memory for large files.
//   - *Alternative*: Return a reader object that implements `Read` over the underlying stream.

const BLOCK_SIZE: usize = 512;

/// Formats a number as a null-terminated octal string into the given buffer.
fn format_octal(buf: &mut [u8], value: u64) {
    let s = format!("{:0width$o}", value, width = buf.len() - 1);
    let bytes = s.as_bytes();
    let len = bytes.len().min(buf.len() - 1);

    // Copy the octal string, leaving the last byte as 0 (null terminator)
    buf[..len].copy_from_slice(&bytes[..len]);
    buf[len] = b' '; // some implementations use space before null
    if len + 1 < buf.len() {
         buf[len + 1] = 0;
    }
}

/// Parses a null- or space-terminated octal string from a byte slice.
fn parse_octal(buf: &[u8]) -> Option<u64> {
    let mut s = String::new();
    for &b in buf {
        if b == 0 || b == b' ' {
            break;
        }
        s.push(b as char);
    }
    u64::from_str_radix(s.trim(), 8).ok()
}

/// Computes the checksum of a header block.
fn compute_checksum(block: &[u8; BLOCK_SIZE]) -> u64 {
    let mut sum: u64 = 0;
    for (i, &b) in block.iter().enumerate() {
        // The checksum field itself (offset 148, length 8) is treated as spaces (0x20)
        if (148..156).contains(&i) {
            sum += b' ' as u64;
        } else {
            sum += b as u64;
        }
    }
    sum
}

/// A trait defining the behavior of a Tar Archive Entry.
/// RUST INSIGHT: By defining a trait, we enable swappable implementations, such as an entry backed
/// by memory (like `MemoryTarEntry`) or an entry backed by a streaming file reader for large files.
pub trait ArchiveEntry {
    fn name(&self) -> &str;
    fn serialize(&self) -> Vec<u8>;
}

/// Represents a Tar Archive entry (file) stored completely in memory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TarEntry {
    pub name: String,
    pub mode: u64,
    pub uid: u64,
    pub gid: u64,
    pub size: u64,
    pub mtime: u64,
    pub typeflag: u8,
    pub linkname: String,
    // GOTCHA: Storing the entire content in a Vec<u8> is a design tradeoff that is easy to write
    // but dangerous for large files.
    pub content: Vec<u8>,
}

impl ArchiveEntry for TarEntry {
    fn name(&self) -> &str {
        &self.name
    }

    /// Serializes this entry into tar format (Header block + Data blocks).
    fn serialize(&self) -> Vec<u8> {
        let mut data = Vec::new();

        // 1. Write Header
        let mut header = [0u8; BLOCK_SIZE];

        // Name (100 bytes)
        let name_bytes = self.name.as_bytes();
        let name_len = name_bytes.len().min(100);
        header[0..name_len].copy_from_slice(&name_bytes[0..name_len]);

        // Mode (8 bytes)
        format_octal(&mut header[100..108], self.mode);
        // UID (8 bytes)
        format_octal(&mut header[108..116], self.uid);
        // GID (8 bytes)
        format_octal(&mut header[116..124], self.gid);
        // Size (12 bytes)
        format_octal(&mut header[124..136], self.size);
        // MTime (12 bytes)
        format_octal(&mut header[136..148], self.mtime);

        // Typeflag (1 byte)
        header[156] = self.typeflag;

        // Linkname (100 bytes)
        let link_bytes = self.linkname.as_bytes();
        let link_len = link_bytes.len().min(100);
        header[157..157 + link_len].copy_from_slice(&link_bytes[0..link_len]);

        // USTAR Magic (6 bytes)
        header[257..263].copy_from_slice(b"ustar\0");
        // USTAR Version (2 bytes)
        header[263..265].copy_from_slice(b"00");

        // Compute and write checksum (8 bytes)
        let checksum = compute_checksum(&header);
        format_octal(&mut header[148..156], checksum);

        data.extend_from_slice(&header);

        // 2. Write Content
        data.extend_from_slice(&self.content);

        // 3. Padding
        let padding_needed = BLOCK_SIZE - (self.content.len() % BLOCK_SIZE);
        if padding_needed != BLOCK_SIZE {
            data.resize(data.len() + padding_needed, 0);
        }

        data
    }
}

impl TarEntry {
    /// Creates a new TarEntry for a regular file.
    pub fn new_file(name: &str, content: Vec<u8>) -> Self {
        Self {
            name: name.to_string(),
            mode: 0o644,
            uid: 1000,
            gid: 1000,
            size: content.len() as u64,
            mtime: 0,
            typeflag: b'0', // Regular file
            linkname: String::new(),
            content,
        }
    }

    /// Parses a single TarEntry from a reader.
    pub fn parse<R: Read>(reader: &mut R) -> io::Result<Option<Self>> {
        let mut header = [0u8; BLOCK_SIZE];
        let mut bytes_read = 0;

        while bytes_read < BLOCK_SIZE {
            let n = reader.read(&mut header[bytes_read..])?;
            if n == 0 {
                // EOF reached
                if bytes_read == 0 {
                    return Ok(None);
                } else {
                    return Err(Error::new(ErrorKind::UnexpectedEof, "Incomplete tar header"));
                }
            }
            bytes_read += n;
        }

        // Check if it's an empty block (marks end of archive)
        if header.iter().all(|&b| b == 0) {
            return Ok(None);
        }

        let name = {
            let mut s = String::new();
            for &b in &header[0..100] {
                if b == 0 { break; }
                s.push(b as char);
            }
            s
        };

        let mode = parse_octal(&header[100..108]).unwrap_or(0);
        let uid = parse_octal(&header[108..116]).unwrap_or(0);
        let gid = parse_octal(&header[116..124]).unwrap_or(0);
        let size = parse_octal(&header[124..136]).unwrap_or(0);
        let mtime = parse_octal(&header[136..148]).unwrap_or(0);
        let typeflag = header[156];

        let linkname = {
            let mut s = String::new();
            for &b in &header[157..257] {
                if b == 0 { break; }
                s.push(b as char);
            }
            s
        };

        // Read content
        let mut content = vec![0u8; size as usize];
        reader.read_exact(&mut content)?;

        // Read padding
        let padding_needed = BLOCK_SIZE - (size as usize % BLOCK_SIZE);
        if padding_needed != BLOCK_SIZE {
            let mut padding = vec![0u8; padding_needed];
            reader.read_exact(&mut padding)?;
        }

        Ok(Some(Self {
            name,
            mode,
            uid,
            gid,
            size,
            mtime,
            typeflag,
            linkname,
            content,
        }))
    }
}

/// A Tar Archive Builder.
pub struct TarBuilder {
    data: Vec<u8>,
}

impl TarBuilder {
    pub fn new() -> Self {
        Self { data: Vec::new() }
    }

    pub fn append_entry<E: ArchiveEntry>(&mut self, entry: &E) {
        self.data.extend_from_slice(&entry.serialize());
    }

    pub fn finish(mut self) -> Vec<u8> {
        // End of archive is marked by two empty blocks
        self.data.extend_from_slice(&[0u8; BLOCK_SIZE * 2]);
        self.data
    }
}

/// Reads a Tar Archive and returns all entries.
pub fn read_tar_archive<R: Read>(mut reader: R) -> io::Result<Vec<TarEntry>> {
    let mut entries = Vec::new();
    while let Some(entry) = TarEntry::parse(&mut reader)? {
        entries.push(entry);
    }
    Ok(entries)
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `tar`: The standard Rust crate for tar archives. It handles streaming reading/writing efficiently,
//   supports PAX extensions (for long filenames), and robustly handles paths.
//
// Missing vs. Production:
// - **PAX Extensions**: We only support USTAR, meaning filenames are limited to 100 characters.
// - **Streaming**: We load entire file contents into memory instead of using streaming readers/writers.
// - **Path Handling**: No protection against Zip Slip (directory traversal attacks) when extracting.
//
// Next Steps:
// 1. Implement streaming content readers.
// 2. Add PAX extended header support for long paths and larger files.
// 3. Add safety checks for extraction paths to prevent directory traversal.

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_format_parse_octal() {
        let mut buf = [0u8; 8];
        format_octal(&mut buf, 0o644);
        // Expect: "0000644 " or similar, we wrote exactly size-1
        // Actually, format!("{:0width$o}", value) with width=7 for value 0o644 -> "0000644"
        assert_eq!(&buf[0..7], b"0000644");

        let val = parse_octal(&buf).unwrap();
        assert_eq!(val, 0o644);
    }

    #[test]
    fn test_tar_entry_serialize_parse() {
        let content = b"Hello, World!\n".to_vec();
        let original_entry = TarEntry::new_file("test.txt", content);

        let serialized = original_entry.serialize();

        // Size should be header (512) + block (512)
        assert_eq!(serialized.len(), 1024);

        let mut cursor = Cursor::new(&serialized);
        let parsed_entry = TarEntry::parse(&mut cursor).unwrap().unwrap();

        assert_eq!(original_entry.name, parsed_entry.name);
        assert_eq!(original_entry.size, parsed_entry.size);
        assert_eq!(original_entry.content, parsed_entry.content);
        assert_eq!(original_entry.typeflag, parsed_entry.typeflag);
        assert_eq!(original_entry.mode, parsed_entry.mode);
    }

    #[test]
    fn test_tar_builder_and_reader() {
        let mut builder = TarBuilder::new();
        builder.append_entry(&TarEntry::new_file("file1.txt", b"First file".to_vec()));
        builder.append_entry(&TarEntry::new_file("file2.txt", b"Second file contents".to_vec()));
        let archive_data = builder.finish();

        let cursor = Cursor::new(&archive_data);
        let entries = read_tar_archive(cursor).unwrap();

        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].name, "file1.txt");
        assert_eq!(entries[0].content, b"First file");

        assert_eq!(entries[1].name, "file2.txt");
        assert_eq!(entries[1].content, b"Second file contents");
    }
}
