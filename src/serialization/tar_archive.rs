//! # Tar Archive Implementation
//!
//! Implements a basic USTAR (Unix Standard TAR) archive reader and writer.
//! It handles the serialization and deserialization of file metadata and contents
//! into 512-byte blocks.
//!
//! **Replaces Crates:** `tar`
//!
//! **Real-world Usage:**
//! - Docker image layers
//! - Package managers (npm `.tgz`, crates.io)
//! - Archival and backup utilities
//!
//! **Why build it yourself?**
//! Implementing TAR demystifies how multiple files and directories are packed into
//! a single contiguous stream. You'll learn about 512-byte block alignment, octal
//! string encoding for metadata (a quirk of early Unix), and how streaming formats
//! don't require an index at the end.

use std::io::{self, Read, Write};

// =========================================================================================
// Architecture
// =========================================================================================
//
// File Format Model:
// A TAR archive is simply a sequence of file entries, each consisting of a 512-byte
// header block followed by the file contents, which are padded with null bytes
// to a multiple of 512 bytes. An archive is terminated by two empty 512-byte blocks.
//
// ┌──────────────────────────────────────────────────┐
// │ Header (512 bytes)                               │
// │ - name (100)                                     │
// │ - mode (8)                                       │
// │ - uid (8), gid (8)                               │
// │ - size (12) (octal ascii)                        │
// │ - mtime (12)                                     │
// │ - chksum (8)                                     │
// │ - typeflag (1)                                   │
// │ - magic (6) "ustar\0"                            │
// │ - ...                                            │
// ├──────────────────────────────────────────────────┤
// │ File Content (padded to 512-byte boundary)       │
// ├──────────────────────────────────────────────────┤
// │ Header (512 bytes)                               │
// ├──────────────────────────────────────────────────┤
// │ File Content ...                                 │
// ├──────────────────────────────────────────────────┤
// │ Empty Block (512 null bytes)                     │
// ├──────────────────────────────────────────────────┤
// │ Empty Block (512 null bytes)                     │
// └──────────────────────────────────────────────────┘
//
// RUST INSIGHT:
// Using `std::io::Read` and `std::io::Write` traits allows our archive processor
// to stream data without loading everything into memory.
//
// GOTCHA:
// Tar fields are stored as ASCII octal strings terminated by a space or null, not
// raw binary numbers. The checksum must be calculated with the checksum field itself
// filled with ASCII spaces.

pub struct TarHeader {
    pub name: String,
    pub size: u64,
    pub mode: u32,
    pub typeflag: u8,
}

impl TarHeader {
    pub fn new(name: &str, size: u64) -> Self {
        Self {
            name: name.to_string(),
            size,
            mode: 0o644,
            typeflag: b'0', // '0' means regular file
        }
    }

    /// Serializes the header into a 512-byte block.
    #[must_use]
    pub fn to_block(&self) -> [u8; 512] {
        let mut block = [0u8; 512];

        // name (100)
        let name_bytes = self.name.as_bytes();
        let name_len = name_bytes.len().min(100);
        block[0..name_len].copy_from_slice(&name_bytes[0..name_len]);

        // mode (8)
        write_octal(&mut block[100..108], self.mode as u64);

        // uid (8), gid (8) - dummy values
        write_octal(&mut block[108..116], 1000);
        write_octal(&mut block[116..124], 1000);

        // size (12)
        write_octal(&mut block[124..136], self.size);

        // mtime (12) - dummy value
        write_octal(&mut block[136..148], 0);

        // chksum (8) - initially spaces
        block[148..156].copy_from_slice(b"        ");

        // typeflag (1)
        block[156] = self.typeflag;

        // magic (6) + version (2)
        block[257..263].copy_from_slice(b"ustar\0");
        block[263..265].copy_from_slice(b"00");

        // Calculate and write checksum
        let mut checksum = 0;
        for &byte in &block {
            checksum += byte as u64;
        }

        // Checksum is 6 octal digits followed by null and space
        let chk_str = format!("{:06o}\0 ", checksum);
        block[148..156].copy_from_slice(chk_str.as_bytes());

        block
    }

    /// Parses a header from a 512-byte block. Returns None if it's an empty block.
    pub fn from_block(block: &[u8; 512]) -> Option<Self> {
        if block.iter().all(|&b| b == 0) {
            return None;
        }

        let name = parse_string(&block[0..100]);
        let mode = parse_octal(&block[100..108]) as u32;
        let size = parse_octal(&block[124..136]);
        let typeflag = block[156];

        Some(Self {
            name,
            size,
            mode,
            typeflag,
        })
    }
}

// Helper to write an octal string into a fixed-size buffer
fn write_octal(buf: &mut [u8], value: u64) {
    let s = format!("{:o}", value);
    let len = s.len();
    let buf_len = buf.len();

    // Tar octal fields typically end in a space or null, and are zero/space padded
    // We pad with zeroes. Example for 12 bytes: "00000000123\0"
    for byte in buf.iter_mut().take(buf_len - len - 1) {
        *byte = b'0';
    }

    let start = buf_len - len - 1;
    buf[start..start + len].copy_from_slice(s.as_bytes());
    buf[buf_len - 1] = 0; // null terminator
}

// Helper to parse an octal string from a buffer, ignoring spaces and nulls
fn parse_octal(buf: &[u8]) -> u64 {
    let mut val = 0;
    for &b in buf {
        if (b'0'..=b'7').contains(&b) {
            val = (val << 3) | (b - b'0') as u64;
        } else if (b == 0 || b == b' ') && val != 0 {
            break;
        }
    }
    val
}

// Helper to parse a null-terminated string
fn parse_string(buf: &[u8]) -> String {
    let end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
    String::from_utf8_lossy(&buf[0..end]).into_owned()
}

pub struct TarWriter<W: Write> {
    writer: W,
}

impl<W: Write> TarWriter<W> {
    pub fn new(writer: W) -> Self {
        Self { writer }
    }

    pub fn append_file(&mut self, name: &str, data: &[u8]) -> io::Result<()> {
        let header = TarHeader::new(name, data.len() as u64);
        self.writer.write_all(&header.to_block())?;

        self.writer.write_all(data)?;

        // Pad to 512 bytes
        let padding = 512 - (data.len() % 512);
        if padding < 512 {
            let pad_buf = vec![0u8; padding];
            self.writer.write_all(&pad_buf)?;
        }

        Ok(())
    }

    pub fn finish(mut self) -> io::Result<()> {
        // Two empty blocks mark EOF
        let empty = [0u8; 1024];
        self.writer.write_all(&empty)
    }
}

pub struct TarReader<R: Read> {
    reader: R,
}

impl<R: Read> TarReader<R> {
    pub fn new(reader: R) -> Self {
        Self { reader }
    }

    pub fn next_entry(&mut self) -> io::Result<Option<(TarHeader, Vec<u8>)>> {
        let mut block = [0u8; 512];
        let n = self.reader.read(&mut block)?;
        if n < 512 {
            return Ok(None);
        }

        let header = match TarHeader::from_block(&block) {
            Some(h) => h,
            None => return Ok(None), // Empty block means EOF
        };

        let mut data = vec![0u8; header.size as usize];
        self.reader.read_exact(&mut data)?;

        // Skip padding
        let padding = 512 - (header.size as usize % 512);
        if padding < 512 {
            let mut pad_buf = vec![0u8; padding];
            self.reader.read_exact(&mut pad_buf)?;
        }

        Ok(Some((header, data)))
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `tar`: Comprehensive crate with support for sparse files, long names (GNU/POSIX extensions),
//   and hard/symlinks.
//
// Missing vs. Production:
// - **Long Names**: USTAR limits names to 100 chars (plus 155 char prefix). Modern tar uses Pax
//   headers or GNU extensions for unlimited lengths. We don't implement this.
// - **File Types**: We only handle regular files. Directories, symlinks, and device files are missing.
// - **Streaming Reader**: Our `next_entry` allocates the entire file content into a `Vec<u8>`. A production
//   implementation yields a reader object that limits reads to the file size.
//
// Benchmarking Note:
// To benchmark the TAR implementation, use `criterion` to measure throughput of
// `TarWriter::append_file` on large synthetic datasets, comparing it against the
// canonical `tar` crate, wrapped in `std::hint::black_box()`.
//
// Next Steps:
// 1. Return a `Read` implementation for entries instead of `Vec<u8>` to support large files without memory exhaustion.
// 2. Add support for POSIX.1-2001 (pax) extensions for long filenames.

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_octal_parsing() {
        let mut buf = [0u8; 12];
        write_octal(&mut buf, 12345);
        assert_eq!(parse_octal(&buf), 12345);

        let mut buf = [0u8; 12];
        write_octal(&mut buf, 0);
        assert_eq!(parse_octal(&buf), 0);
    }

    #[test]
    fn test_header_serialization() {
        let header = TarHeader::new("test.txt", 13);
        let block = header.to_block();

        let parsed = TarHeader::from_block(&block).unwrap();
        assert_eq!(parsed.name, "test.txt");
        assert_eq!(parsed.size, 13);
        assert_eq!(parsed.typeflag, b'0');
    }

    #[test]
    fn test_tar_writer_reader() {
        let mut buffer = Vec::new();

        // Write
        {
            let mut writer = TarWriter::new(&mut buffer);
            writer.append_file("hello.txt", b"Hello, World!").unwrap();
            writer.append_file("large.bin", &[1u8; 1000]).unwrap();
            writer.finish().unwrap();
        }

        // The size should be:
        // header (512) + file (512 padded) +
        // header (512) + file (1024 padded) +
        // EOF marker (1024) = 3584
        assert_eq!(buffer.len(), 3584);

        // Read
        let mut reader = TarReader::new(Cursor::new(&buffer));

        let (h1, d1) = reader.next_entry().unwrap().unwrap();
        assert_eq!(h1.name, "hello.txt");
        assert_eq!(h1.size, 13);
        assert_eq!(d1, b"Hello, World!");

        let (h2, d2) = reader.next_entry().unwrap().unwrap();
        assert_eq!(h2.name, "large.bin");
        assert_eq!(h2.size, 1000);
        assert_eq!(d2.len(), 1000);
        assert!(d2.iter().all(|&b| b == 1));

        let eof = reader.next_entry().unwrap();
        assert!(eof.is_none());
    }
}
