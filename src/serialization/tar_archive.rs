//! # Tar Archive Implementation
//!
//! Implements a minimal USTAR archive reader and writer from scratch.
//!
//! **Replaces Crates:** `tar`
//!
//! **Real-world Usage:**
//! - Package distribution (NPM, Cargo, Docker images)
//! - Archival tooling and backups (`tar` command line)
//! - Stream-based archiving without seeking
//!
//! **Why build it yourself?**
//! Implementing a `tar` parser demystifies how files are packed together into a single stream.
//! It teaches you about 512-byte block alignment, octal string encoding for metadata,
//! and legacy fixed-size buffer layouts. You learn how stream-based serialization
//! allows processing massive archives without loading everything into memory.
//!
//! # Architecture
//!
//! **Flow:**
//! ```text
//! [ Metadata ] -> [ USTAR Header (512 bytes) ]
//! [ File Content ] -> [ Blocks of 512 bytes (padded) ]
//! ```
//!
//! **Invariants:**
//! 1. All headers and file data blocks must be aligned to 512 bytes.
//! 2. Numeric values in the header are ASCII octal strings, null-terminated.
//! 3. The checksum is calculated assuming the checksum field itself is filled with spaces.
//! 4. The archive is terminated by two consecutive empty (all zero) 512-byte blocks.
//!
//! **Complexity:**
//! ┌───────────────┬────────────┬─────────────┐
//! │ Operation     │ Time       │ Space       │
//! ├───────────────┼────────────┼─────────────┤
//! │ Read Block    │ O(1)       │ O(B)        │
//! │ Write Block   │ O(1)       │ O(B)        │
//! └───────────────┴────────────┴─────────────┘
//! * B = Block size (512 bytes). Time is proportional to file length when fully reading.
//!
//! **Design Decisions & Tradeoffs:**
//! - **Encoding:** We only support basic USTAR format, ignoring PAX extended headers for simplicity.
//! - **I/O:** The implementation is entirely in-memory using `Vec<u8>` to simulate a file stream, avoiding real I/O complexities but mimicking the block logic.
//!
//! **Comparison to canonical crates:**
//! - The `tar` crate supports PAX extended headers, sparse files, and extensive I/O traits (`Read`/`Write`).

// =========================================================================================
// Tar USTAR Definitions
// =========================================================================================

const BLOCK_SIZE: usize = 512;

/// A Tar archive entry header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TarHeader {
    pub name: String,
    pub mode: u32,
    pub uid: u32,
    pub gid: u32,
    pub size: u64,
    pub mtime: u64,
    pub typeflag: u8,
    pub linkname: String,
    pub uname: String,
    pub gname: String,
}

impl Default for TarHeader {
    fn default() -> Self {
        Self {
            name: String::new(),
            mode: 0o644,
            uid: 0,
            gid: 0,
            size: 0,
            mtime: 0,
            typeflag: b'0', // Regular file
            linkname: String::new(),
            uname: String::new(),
            gname: String::new(),
        }
    }
}

impl TarHeader {
    /// Serializes the header into a 512-byte block.
    #[must_use]
    pub fn serialize(&self) -> [u8; BLOCK_SIZE] {
        let mut block = [0u8; BLOCK_SIZE];

        // 0-99: name
        let name_bytes = self.name.as_bytes();
        let name_len = name_bytes.len().min(100);
        block[..name_len].copy_from_slice(&name_bytes[..name_len]);

        // 100-107: mode
        write_octal(&mut block[100..108], self.mode as u64);

        // 108-115: uid
        write_octal(&mut block[108..116], self.uid as u64);

        // 116-123: gid
        write_octal(&mut block[116..124], self.gid as u64);

        // 124-135: size
        write_octal_12(&mut block[124..136], self.size);

        // 136-147: mtime
        write_octal_12(&mut block[136..148], self.mtime);

        // 148-155: chksum (temporarily filled with spaces)
        block[148..156].copy_from_slice(b"        ");

        // 156: typeflag
        block[156] = self.typeflag;

        // 157-256: linkname
        let linkname_bytes = self.linkname.as_bytes();
        let linkname_len = linkname_bytes.len().min(100);
        block[157..157 + linkname_len].copy_from_slice(&linkname_bytes[..linkname_len]);

        // 257-262: magic
        block[257..263].copy_from_slice(b"ustar\0");

        // 263-264: version
        block[263..265].copy_from_slice(b"00");

        // 265-296: uname
        let uname_bytes = self.uname.as_bytes();
        let uname_len = uname_bytes.len().min(32);
        block[265..265 + uname_len].copy_from_slice(&uname_bytes[..uname_len]);

        // 297-328: gname
        let gname_bytes = self.gname.as_bytes();
        let gname_len = gname_bytes.len().min(32);
        block[297..297 + gname_len].copy_from_slice(&gname_bytes[..gname_len]);

        // Calculate and write checksum
        let mut chksum = 0u32;
        for &byte in &block {
            chksum += byte as u32;
        }
        write_octal(&mut block[148..156], chksum as u64);
        // Tar checksums end with null and space historically
        block[154] = 0;
        block[155] = b' ';

        block
    }

    /// Deserializes a 512-byte block into a TarHeader. Returns None if the block is empty.
    #[must_use]
    pub fn deserialize(block: &[u8; BLOCK_SIZE]) -> Option<Self> {
        if block.iter().all(|&b| b == 0) {
            return None;
        }

        let name = read_string(&block[0..100]);
        let mode = read_octal(&block[100..108]) as u32;
        let uid = read_octal(&block[108..116]) as u32;
        let gid = read_octal(&block[116..124]) as u32;
        let size = read_octal(&block[124..136]);
        let mtime = read_octal(&block[136..148]);
        let typeflag = block[156];
        let linkname = read_string(&block[157..257]);

        let mut uname = String::new();
        let mut gname = String::new();

        // Only parse USTAR fields if magic matches
        if &block[257..262] == b"ustar" {
            uname = read_string(&block[265..297]);
            gname = read_string(&block[297..329]);
        }

        Some(Self {
            name,
            mode,
            uid,
            gid,
            size,
            mtime,
            typeflag,
            linkname,
            uname,
            gname,
        })
    }
}

fn write_octal(buf: &mut [u8], val: u64) {
    let s = format!("{:07o}", val);
    let bytes = s.as_bytes();
    let len = bytes.len().min(buf.len() - 1);
    // Pad with leading zeros if needed
    let start = (buf.len() - 1).saturating_sub(len);
    buf[..start].fill(b'0');
    buf[start..start + len].copy_from_slice(&bytes[..len]);
    buf[buf.len() - 1] = 0; // Null-terminated
}

fn write_octal_12(buf: &mut [u8], val: u64) {
    let s = format!("{:011o}", val);
    let bytes = s.as_bytes();
    let len = bytes.len().min(buf.len() - 1);
    let start = (buf.len() - 1).saturating_sub(len);
    buf[..start].fill(b'0');
    buf[start..start + len].copy_from_slice(&bytes[..len]);
    buf[buf.len() - 1] = 0; // Null-terminated
}

fn read_octal(buf: &[u8]) -> u64 {
    let mut val = 0;
    for &b in buf {
        if (b'0'..=b'7').contains(&b) {
            val = (val << 3) | ((b - b'0') as u64);
        } else if b == 0 || b == b' ' {
            // Null or space terminates
            if val != 0 {
                break;
            }
        }
    }
    val
}

fn read_string(buf: &[u8]) -> String {
    let len = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
    String::from_utf8_lossy(&buf[..len]).into_owned()
}

// =========================================================================================
// Tar Archive Builder / Reader
// =========================================================================================

/// An entry in a Tar archive.
#[derive(Debug, Clone)]
pub struct TarEntry {
    pub header: TarHeader,
    pub data: Vec<u8>,
}

/// Trait defining the behavior for building a Tar Archive.
pub trait ArchiveBuilder {
    /// Appends a file to the archive.
    fn append(&mut self, header: &TarHeader, data: &[u8]);
    /// Finalizes the archive, appending the two empty blocks.
    fn finish(self) -> Vec<u8>;
}

/// An in-memory Tar archive builder.
#[derive(Default)]
pub struct TarBuilder {
    // RUST INSIGHT: Real `tar` crates would use a writer implementing `std::io::Write`.
    // We use a Vec<u8> memory buffer to focus on the structure over I/O handling.
    archive: Vec<u8>,
}

impl TarBuilder {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl ArchiveBuilder for TarBuilder {
    fn append(&mut self, header: &TarHeader, data: &[u8]) {
        let mut hdr = header.clone();
        // GOTCHA: Tar archive size headers reflect the unpadded data length.
        hdr.size = data.len() as u64;

        // PRODUCTION NOTE: Appending to a Vec causes it to reallocate as it grows.
        // In a real stream, we would write blocks directly to a file descriptor.
        self.archive.extend_from_slice(&hdr.serialize());
        self.archive.extend_from_slice(data);

        // Pad data to BLOCK_SIZE
        let padding = (BLOCK_SIZE - (data.len() % BLOCK_SIZE)) % BLOCK_SIZE;
        self.archive.resize(self.archive.len() + padding, 0);
    }

    fn finish(mut self) -> Vec<u8> {
        self.archive.resize(self.archive.len() + 2 * BLOCK_SIZE, 0);
        self.archive
    }
}

/// Parses a Tar archive into entries.
#[must_use]
pub fn parse_tar(data: &[u8]) -> Vec<TarEntry> {
    let mut entries = Vec::new();
    let mut offset = 0;

    while offset + BLOCK_SIZE <= data.len() {
        let mut block = [0u8; BLOCK_SIZE];
        block.copy_from_slice(&data[offset..offset + BLOCK_SIZE]);
        offset += BLOCK_SIZE;

        if let Some(header) = TarHeader::deserialize(&block) {
            let size = header.size as usize;
            if offset + size > data.len() {
                break; // Corrupt archive
            }

            let file_data = data[offset..offset + size].to_vec();
            entries.push(TarEntry {
                header,
                data: file_data,
            });

            // Advance offset, including padding
            let padding = (BLOCK_SIZE - (size % BLOCK_SIZE)) % BLOCK_SIZE;
            offset += size + padding;
        } else {
            // Null block marks the end of the archive
            break;
        }
    }

    entries
}

// =========================================================================================
// Tests
// =========================================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tar_header_serialization() {
        let header = TarHeader {
            name: "test.txt".to_string(),
            size: 123,
            uname: "jules".to_string(),
            ..TarHeader::default()
        };

        let block = header.serialize();
        let parsed = TarHeader::deserialize(&block).unwrap();

        assert_eq!(parsed.name, "test.txt");
        assert_eq!(parsed.size, 123);
        assert_eq!(parsed.uname, "jules");
        assert_eq!(parsed.typeflag, b'0');
    }

    #[test]
    fn test_tar_builder_and_parser() {
        let mut builder = TarBuilder::new();

        let h1 = TarHeader {
            name: "file1.txt".to_string(),
            ..TarHeader::default()
        };
        builder.append(&h1, b"hello world");

        let h2 = TarHeader {
            name: "file2.bin".to_string(),
            ..TarHeader::default()
        };
        let data2 = vec![0x01, 0x02, 0x03, 0x04];
        builder.append(&h2, &data2);

        let archive = builder.finish();

        // Minimal valid archive should be:
        // h1(512) + d1(11 + 501 pad = 512) + h2(512) + d2(4 + 508 pad = 512) + 2*empty(1024) = 3072 bytes
        assert_eq!(archive.len(), 3072);

        let entries = parse_tar(&archive);
        assert_eq!(entries.len(), 2);

        assert_eq!(entries[0].header.name, "file1.txt");
        assert_eq!(entries[0].data, b"hello world");

        assert_eq!(entries[1].header.name, "file2.bin");
        assert_eq!(entries[1].data, vec![0x01, 0x02, 0x03, 0x04]);
    }

    #[test]
    fn test_tar_benchmark() {
        use std::hint::black_box;
        use std::time::Instant;

        let mut builder = TarBuilder::new();
        let data = vec![0u8; 1024 * 1024]; // 1MB buffer

        let start = Instant::now();
        builder.append(black_box(&TarHeader::default()), black_box(&data));
        let archive = builder.finish();
        let duration = start.elapsed();

        assert!(archive.len() > 1024 * 1024);
        println!("Appended 1MB to tar in {:?}", duration);
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// **Comparison to canonical crates:**
// - The `tar` crate supports reading and writing streaming I/O (`std::io::Read` and `std::io::Write`),
//   whereas this implementation requires loading the entire archive into memory (`Vec<u8>`).
// - The canonical crate supports PAX extended headers which allow for arbitrarily long file names
//   and large file sizes beyond the legacy 8GB USTAR limit.
//
// **What's missing vs. production:**
// - **PAX Headers**: Long names, huge files, and non-ASCII names are not supported.
// - **Streaming**: Cannot process a tarball linearly without keeping it all in RAM.
// - **Sparse Files**: Missing support for files with holes (GNU extensions).
// - **Error Handling**: Currently uses `Option` or drops invalid structures; a real crate returns `std::io::Error`.
//
// **Suggested next steps:**
// - Implement a `Read` and `Write` trait interface instead of byte slices.
// - Add parsing support for PAX extended headers to support long paths.
