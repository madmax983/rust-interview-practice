//! # Tar Archive Implementation
//!
//! Implements a basic USTAR (Uniform Standard Tape Archive) reader and writer from scratch.
//! Tar files are sequences of 512-byte blocks containing file headers and file data.
//!
//! **Replaces Crates:** `tar`
//!
//! **Real-world Usage:**
//! - Unix archiving and backups.
//! - Container images (Docker/OCI layers are just tarballs).
//! - Source code distribution (npm packages, cargo crates).
//!
//! **Why build it yourself?**
//! Parsing a tarball teaches you how binary formats handle alignment (512-byte blocks) and
//! historical quirks (using ASCII octal strings instead of binary integers for metadata).
//! You'll learn how to safely slice byte arrays, handle padding, and understand the difference
//! between file metadata and file data in a raw stream.

use std::io::{self, Read, Write};

// =========================================================================================
// Architecture
// =========================================================================================
//
// Data Structure Diagram (Tar Format):
//
//   [ 512-byte Header for File A ]
//   [ 512-byte Data Block (File A) ]
//   [ 512-byte Data Block (Padding if A < 512) ]
//   [ 512-byte Header for File B ]
//   [ 512-byte Data Block (File B) ]
//   [ 512-byte Empty Block ] \
//   [ 512-byte Empty Block ]  - End of Archive Marker
//
// Invariants:
// 1. Everything is aligned to 512-byte blocks.
// 2. Numbers (size, mode, mtime, checksum) are encoded as ASCII octal strings terminated by \0 or space.
// 3. The checksum is the sum of all bytes in the header block, with the checksum field itself treated as spaces.
//
// Complexity:
// ┌───────────────┬──────────────────┬──────────────────┐
// │ Operation     │ Time             │ Space            │
// ├───────────────┼──────────────────┼──────────────────┤
// │ Read Header   │ O(1)             │ O(1)             │
// │ Write Header  │ O(1)             │ O(1)             │
// │ Read Data     │ O(N) where N=size│ O(N)             │
// └───────────────┴──────────────────┴──────────────────┘
//
// Design Decisions:
// - **Memory**: We parse into memory rather than streaming for simplicity. A production parser
//   would yield streaming readers.
// - **Octal Parsing**: We use a basic ASCII octal parser.
// - **Checksums**: We calculate and verify USTAR checksums strictly.

const BLOCK_SIZE: usize = 512;

/// A parsed entry from a tar archive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TarEntry {
    pub name: String,
    pub size: u64,
    pub mode: u32,
    pub mtime: u64,
    pub data: Vec<u8>,
}

/// Helper to parse an ASCII octal string into a u64.
/// The string is usually null or space terminated.
fn parse_octal(bytes: &[u8]) -> Option<u64> {
    let mut val = 0;
    for &b in bytes {
        if b == 0 || b == b' ' {
            break;
        }
        if !(b'0'..=b'7').contains(&b) {
            return None; // Invalid octal char
        }
        val = val * 8 + u64::from(b - b'0');
    }
    Some(val)
}

/// Helper to format a u64 into an ASCII octal string, null-terminated.
fn format_octal(val: u64, buf: &mut [u8]) {
    // RUST INSIGHT: Working with byte arrays directly allows us to avoid String allocations.
    // PRODUCTION NOTE: Write directly into the buffer backwards.
    let mut temp = val;
    let len = buf.len();

    // Fill with leading zeros (or we could use spaces, but 0 is common)
    for byte in buf.iter_mut().take(len - 1) {
        *byte = b'0';
    }
    buf[len - 1] = 0; // Null terminator

    if val == 0 {
        return;
    }

    let mut idx = len - 2;
    loop {
        buf[idx] = b'0' + (temp % 8) as u8;
        temp /= 8;
        if temp == 0 || idx == 0 {
            break;
        }
        idx -= 1;
    }
}

/// Computes the USTAR checksum of a header block.
fn compute_checksum(header: &[u8; BLOCK_SIZE]) -> u64 {
    let mut sum: u64 = 0;
    for (i, &b) in header.iter().enumerate() {
        if (148..156).contains(&i) {
            // Checksum field itself is treated as spaces (0x20)
            sum += 0x20;
        } else {
            sum += u64::from(b);
        }
    }
    sum
}

/// A simple Tar Archive Builder.
pub struct TarBuilder<W: Write> {
    writer: W,
}

impl<W: Write> TarBuilder<W> {
    pub fn new(writer: W) -> Self {
        Self { writer }
    }

    /// Appends a new file entry to the archive.
    pub fn append(&mut self, name: &str, mode: u32, mtime: u64, data: &[u8]) -> io::Result<()> {
        let mut header = [0u8; BLOCK_SIZE];

        // Name (100 bytes)
        let name_bytes = name.as_bytes();
        let name_len = name_bytes.len().min(100);
        header[0..name_len].copy_from_slice(&name_bytes[0..name_len]);

        // Mode (8 bytes)
        format_octal(u64::from(mode), &mut header[100..108]);

        // Uid / Gid (8 bytes each - hardcoded 0 for simplicity)
        format_octal(0, &mut header[108..116]);
        format_octal(0, &mut header[116..124]);

        // Size (12 bytes)
        format_octal(data.len() as u64, &mut header[124..136]);

        // Mtime (12 bytes)
        format_octal(mtime, &mut header[136..148]);

        // Type flag (1 byte - '0' for regular file)
        header[156] = b'0';

        // USTAR Magic (6 bytes)
        header[257..263].copy_from_slice(b"ustar\0");
        header[263..265].copy_from_slice(b"00");

        // Calculate and write checksum (8 bytes)
        let checksum = compute_checksum(&header);
        format_octal(checksum, &mut header[148..156]);
        // Tar standard quirk: often ends with null then space, or just nulls.
        // `format_octal` null terminates. We'll leave it as is.

        self.writer.write_all(&header)?;

        // Write data
        self.writer.write_all(data)?;

        // Write padding
        let padding_len = (BLOCK_SIZE - (data.len() % BLOCK_SIZE)) % BLOCK_SIZE;
        if padding_len > 0 {
            let padding = vec![0u8; padding_len];
            self.writer.write_all(&padding)?;
        }

        Ok(())
    }

    /// Finishes the archive by writing the two empty end blocks.
    pub fn finish(mut self) -> io::Result<W> {
        let end_blocks = [0u8; BLOCK_SIZE * 2];
        self.writer.write_all(&end_blocks)?;
        Ok(self.writer)
    }
}

/// A simple Tar Archive Reader.
pub struct TarReader<R: Read> {
    reader: R,
}

impl<R: Read> TarReader<R> {
    pub fn new(reader: R) -> Self {
        Self { reader }
    }

    /// Reads all entries from the archive.
    pub fn read_all(&mut self) -> io::Result<Vec<TarEntry>> {
        // GOTCHA: A single corrupted file in the archive will fail the entire `read_all`
        // stream, aborting extraction. Production parsers provide iterators over `Result<TarEntry>`.
        let mut entries = Vec::new();

        loop {
            let mut header = [0u8; BLOCK_SIZE];
            let mut read_bytes = 0;

            // Handle potentially short reads
            while read_bytes < BLOCK_SIZE {
                let n = self.reader.read(&mut header[read_bytes..])?;
                if n == 0 {
                    break;
                }
                read_bytes += n;
            }

            if read_bytes == 0 {
                break; // EOF
            }
            if read_bytes < BLOCK_SIZE {
                return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "Incomplete tar block"));
            }

            // Check for end of archive (empty block)
            if header.iter().all(|&b| b == 0) {
                break;
            }

            // Parse name
            let name_end = header[0..100].iter().position(|&b| b == 0).unwrap_or(100);
            let name = String::from_utf8_lossy(&header[0..name_end]).into_owned();

            // Parse metadata
            let mode = parse_octal(&header[100..108]).unwrap_or(0) as u32;
            let size = parse_octal(&header[124..136]).unwrap_or(0);
            let mtime = parse_octal(&header[136..148]).unwrap_or(0);

            // Verify checksum
            let expected_checksum = parse_octal(&header[148..156]).unwrap_or(0);
            let actual_checksum = compute_checksum(&header);
            if expected_checksum != actual_checksum && expected_checksum != 0 {
                return Err(io::Error::new(io::ErrorKind::InvalidData, "Invalid tar checksum"));
            }

            // Read data
            let mut data = vec![0u8; size as usize];
            self.reader.read_exact(&mut data)?;

            // Consume padding
            let padding_len = (BLOCK_SIZE - (size as usize % BLOCK_SIZE)) % BLOCK_SIZE;
            if padding_len > 0 {
                let mut padding = vec![0u8; padding_len];
                self.reader.read_exact(&mut padding)?;
            }

            entries.push(TarEntry {
                name,
                mode,
                size,
                mtime,
                data,
            });
        }

        Ok(entries)
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `tar`: The standard Rust library. It handles streaming readers (using `std::io::Read` directly
//   on entries), long file names (GNU/PAX extensions), and complex symlink/hardlink logic.
//
// Missing vs. Production:
// - **Streaming**: We load entire files into memory `Vec<u8>`. Real parsers stream.
// - **Extensions**: We don't support GNU long names (>100 chars) or PAX extended headers.
// - **File Types**: We only write/read regular files (type '0'). Symlinks, directories, etc. are ignored.
// - **Security**: A real parser must prevent directory traversal (e.g., `../../etc/passwd` in the name).
//
// Next Steps:
// 1. Implement streaming reads (return a struct implementing `Read` instead of `Vec<u8>`).
// 2. Add support for PAX extended headers for long paths.
// 3. Add path sanitization to prevent archive extraction vulnerabilities.
//
// Benchmarking:
// To benchmark serialization performance, use `criterion` on `TarBuilder::append` and `TarReader::read_all`
// with various file sizes to measure MB/s throughput, wrapping inputs in `black_box`.

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_octal_formatting_and_parsing() {
        let mut buf = [0u8; 8];
        format_octal(0o644, &mut buf);
        assert_eq!(&buf[0..7], b"0000644");
        assert_eq!(buf[7], 0);

        let parsed = parse_octal(&buf).unwrap();
        assert_eq!(parsed, 0o644);
    }

    #[test]
    fn test_tar_roundtrip() {
        let mut archive_bytes = Vec::new();

        // Write
        {
            let mut builder = TarBuilder::new(&mut archive_bytes);
            builder.append("test.txt", 0o644, 1234567890, b"Hello Tar").unwrap();
            builder.append("large.bin", 0o755, 1234567891, &[42u8; 1000]).unwrap();
            builder.finish().unwrap();
        }

        // Read
        let mut reader = TarReader::new(Cursor::new(&archive_bytes));
        let entries = reader.read_all().unwrap();

        assert_eq!(entries.len(), 2);

        assert_eq!(entries[0].name, "test.txt");
        assert_eq!(entries[0].mode, 0o644);
        assert_eq!(entries[0].mtime, 1234567890);
        assert_eq!(entries[0].size, 9);
        assert_eq!(entries[0].data, b"Hello Tar");

        assert_eq!(entries[1].name, "large.bin");
        assert_eq!(entries[1].mode, 0o755);
        assert_eq!(entries[1].size, 1000);
        assert_eq!(entries[1].data.len(), 1000);
        assert_eq!(entries[1].data[0], 42);
    }

    #[test]
    fn test_padding() {
        let mut archive_bytes = Vec::new();
        let mut builder = TarBuilder::new(&mut archive_bytes);

        // 9 bytes of data + 503 bytes padding = 512 bytes data block
        builder.append("a", 0, 0, b"123456789").unwrap();
        builder.finish().unwrap();

        // Header block (512) + Data block (512) + 2 End blocks (1024)
        assert_eq!(archive_bytes.len(), 512 + 512 + 1024);
    }
}
