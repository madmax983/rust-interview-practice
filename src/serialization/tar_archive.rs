//! # Tar Archive Implementation
//!
//! **Replaces Crate:** `tar`
//!
//! **Real-world systems that use this:** Docker (image layers), Linux package managers (pacman, dpkg),
//! backup systems, and CI/CD pipelines (cache archives).
//!
//! **Why build it yourself?**
//! Tar (Tape Archive) is one of the oldest and simplest archive formats still heavily in use today.
//! Implementing it teaches you how to handle 512-byte block alignment, parse zero-padded octal ASCII
//! (a bizarre historical artifact), and stream data sequentially without seeking (crucial for pipes).
//! It highlights the difference between file metadata (headers) and file content (payloads) in storage.

use std::io::{self, Read, Write};

// =========================================================================================
// Architecture
// =========================================================================================
//
// A POSIX USTAR archive consists of a sequence of file entries.
// Each file entry is a 512-byte Header, followed by the File Content (padded to a multiple of 512 bytes).
// The archive ends with two consecutive 512-byte blocks of zeros (the End of Archive marker).
//
//      ┌───────────────────────────────┐
//      │ Header (512 bytes)            │
//      ├───────────────────────────────┤
//      │ File Content (N bytes)        │
//      │ + Padding to 512-byte bound   │
//      ├───────────────────────────────┤
//      │ Header (512 bytes)            │
//      ├───────────────────────────────┤
//      │ ...                           │
//      ├───────────────────────────────┤
//      │ Zero Block (512 bytes)        │
//      ├───────────────────────────────┤
//      │ Zero Block (512 bytes)        │
//      └───────────────────────────────┘
//
// Invariants:
// 1. All headers and content payloads must be exactly aligned to 512-byte boundaries.
// 2. Numeric fields in the header (size, mode, mtime, checksum) are ASCII strings representing octal numbers, terminated by space or NUL.
// 3. The checksum field is calculated as the sum of all bytes in the header, assuming the checksum field itself is filled with spaces (0x20).
//
// Time/Space Complexity:
// - **Read/Write Entry:** Time O(N) where N is the size of the file, Space O(1) buffer overhead.
// - **Checksum Calc:** Time O(1) (fixed 512 bytes), Space O(1).
//
// Design Decisions:
// - We implement a simplified subset of the USTAR format (POSIX.1-1988) focusing on normal files.
// - We rely heavily on exact-size byte arrays (`[u8; 512]`) to enforce alignment at the type level.

/// Size of a Tar block (512 bytes).
pub const BLOCK_SIZE: usize = 512;

/// A Tar Header structure representing a single file entry in the archive.
#[derive(Debug, PartialEq, Eq)]
pub struct TarHeader {
    pub name: String,
    pub size: u64,
    pub mode: u32,
    // Ignoring mtime, uid, gid, etc., for this simplified exercise
}

/// A reader for Tar archives.
pub struct TarReader<R: Read> {
    reader: R,
    is_eof: bool,
}

impl<R: Read> TarReader<R> {
    /// Creates a new `TarReader` wrapping the provided `Read` implementation.
    pub fn new(reader: R) -> Self {
        Self {
            reader,
            is_eof: false,
        }
    }

    /// Reads the next entry header from the archive.
    /// Returns `Ok(None)` if the end of the archive (two zero blocks) is reached.
    pub fn next_header(&mut self) -> io::Result<Option<TarHeader>> {
        if self.is_eof {
            return Ok(None);
        }

        let mut header_buf = [0u8; BLOCK_SIZE];
        let bytes_read = self.reader.read(&mut header_buf)?;

        if bytes_read == 0 {
            // Unexpected EOF, but we can treat it as end of archive
            self.is_eof = true;
            return Ok(None);
        }

        if bytes_read < BLOCK_SIZE {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "Incomplete header block",
            ));
        }

        // Check for end of archive (block of all zeros)
        if header_buf.iter().all(|&b| b == 0) {
            // RUST INSIGHT: The canonical tar format requires TWO zero blocks to mark EOF.
            // We read one, we should ideally read the second, but effectively one zero block
            // means we are done reading active entries.
            self.is_eof = true;
            return Ok(None);
        }

        // Validate Checksum
        let checksum_bytes = &header_buf[148..156];
        let parsed_checksum = parse_octal(checksum_bytes).map_err(|_| {
            io::Error::new(io::ErrorKind::InvalidData, "Invalid header checksum format")
        })?;

        let calculated_checksum = calculate_checksum(&header_buf);
        if parsed_checksum != calculated_checksum {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Header checksum mismatch",
            ));
        }

        // Parse Name (bytes 0..100)
        let name_bytes = &header_buf[0..100];
        let name_len = name_bytes.iter().position(|&b| b == 0).unwrap_or(100);
        let name = String::from_utf8(name_bytes[0..name_len].to_vec()).map_err(|_| {
            io::Error::new(io::ErrorKind::InvalidData, "Invalid UTF-8 in file name")
        })?;

        // Parse Mode (bytes 100..108)
        let mode = parse_octal(&header_buf[100..108]).unwrap_or(0) as u32;

        // Parse Size (bytes 124..136)
        let size = parse_octal(&header_buf[124..136]).unwrap_or(0);

        Ok(Some(TarHeader { name, size, mode }))
    }

    /// Reads exactly `size` bytes of file content, and consumes the remaining padding
    /// to advance the stream to the next 512-byte block boundary.
    pub fn read_content(&mut self, size: u64, buf: &mut Vec<u8>) -> io::Result<()> {
        // Read exact content
        let mut content = vec![0u8; size as usize];
        self.reader.read_exact(&mut content)?;
        buf.extend_from_slice(&content);

        // Read and discard padding
        let padding = calculate_padding(size);
        if padding > 0 {
            let mut pad_buf = vec![0u8; padding];
            self.reader.read_exact(&mut pad_buf)?;
        }

        Ok(())
    }
}

/// A writer for Tar archives.
pub struct TarWriter<W: Write> {
    writer: W,
    finished: bool,
}

impl<W: Write> TarWriter<W> {
    /// Creates a new `TarWriter` wrapping the provided `Write` implementation.
    pub fn new(writer: W) -> Self {
        Self {
            writer,
            finished: false,
        }
    }

    /// Appends a new file entry (header + content) to the archive.
    pub fn append(&mut self, header: &TarHeader, content: &[u8]) -> io::Result<()> {
        if self.finished {
            return Err(io::Error::other(
                "Archive is already finished",
            ));
        }
        if content.len() as u64 != header.size {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Content length does not match header size",
            ));
        }

        let mut header_buf = [0u8; BLOCK_SIZE];

        // Write Name (max 100 chars, truncated for simplicity if longer)
        let name_bytes = header.name.as_bytes();
        let name_len = name_bytes.len().min(100);
        header_buf[0..name_len].copy_from_slice(&name_bytes[0..name_len]);

        // Write Mode
        let mode_str = format!("{:07o}", header.mode);
        header_buf[100..107].copy_from_slice(mode_str.as_bytes());

        // Write UID, GID (dummy values)
        header_buf[108..115].copy_from_slice(b"0000000");
        header_buf[116..123].copy_from_slice(b"0000000");

        // Write Size (11 octal digits + NUL/Space)
        // GOTCHA: Tar size field is 12 bytes long. It usually contains 11 octal characters
        // followed by a space or NUL. Formatting correctly is crucial for compatibility.
        let size_str = format!("{:011o}", header.size);
        header_buf[124..135].copy_from_slice(size_str.as_bytes());

        // Write Mtime (dummy value, e.g., 0)
        header_buf[136..147].copy_from_slice(b"00000000000");

        // Write Typeflag (0 or '0' for normal file)
        header_buf[156] = b'0';

        // Write USTAR Magic and Version
        header_buf[257..262].copy_from_slice(b"ustar");
        header_buf[263..265].copy_from_slice(b"00");

        // Calculate and Write Checksum
        // Must fill checksum field with spaces first
        for item in header_buf.iter_mut().take(156).skip(148) {
            *item = b' ';
        }
        let checksum = calculate_checksum(&header_buf);
        let checksum_str = format!("{:06o}\0 ", checksum);
        header_buf[148..156].copy_from_slice(checksum_str.as_bytes());

        // Write Header
        self.writer.write_all(&header_buf)?;

        // Write Content
        self.writer.write_all(content)?;

        // Write Padding
        let padding = calculate_padding(header.size);
        if padding > 0 {
            let pad_buf = vec![0u8; padding];
            self.writer.write_all(&pad_buf)?;
        }

        Ok(())
    }

    /// Finishes the archive by writing the two required zero blocks.
    pub fn finish(&mut self) -> io::Result<()> {
        if !self.finished {
            let zero_block = [0u8; BLOCK_SIZE * 2];
            self.writer.write_all(&zero_block)?;
            self.finished = true;
        }
        Ok(())
    }
}

impl<W: Write> Drop for TarWriter<W> {
    fn drop(&mut self) {
        let _ = self.finish();
    }
}

/// Helper function to parse a tar octal ASCII string into a `u64`.
/// Tar octal fields are usually space or NUL padded/terminated.
fn parse_octal(bytes: &[u8]) -> Result<u64, ()> {
    let mut val: u64 = 0;
    let mut parsed_any = false;

    for &b in bytes {
        if b == b' ' || b == 0 {
            if parsed_any {
                break; // End of number
            }
            continue; // Leading space/NUL
        }

        // RUST INSIGHT: Resolving `clippy::manual_range_contains` by using range `.contains()`.
        if (b'0'..=b'7').contains(&b) {
            val = (val << 3) | u64::from(b - b'0');
            parsed_any = true;
        } else {
            return Err(()); // Invalid character
        }
    }

    if parsed_any {
        Ok(val)
    } else {
        Err(())
    }
}

/// Calculates the tar header checksum.
/// It is the sum of all bytes in the header, assuming the checksum field (bytes 148-155) is filled with spaces (0x20).
fn calculate_checksum(header: &[u8; BLOCK_SIZE]) -> u64 {
    let mut sum: u64 = 0;
    for (i, &b) in header.iter().enumerate() {
        if (148..156).contains(&i) {
            sum += u64::from(b' '); // Assume spaces for the checksum field itself
        } else {
            sum += u64::from(b);
        }
    }
    sum
}

/// Calculates the number of null bytes needed to pad the file content to a multiple of 512 bytes.
fn calculate_padding(size: u64) -> usize {
    let rem = size % (BLOCK_SIZE as u64);
    if rem == 0 {
        0
    } else {
        (BLOCK_SIZE as u64 - rem) as usize
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `tar`: The standard Rust crate for this is heavily robust. It handles GNU extensions (long names),
//   PAX extended headers, sparse files, and seamlessly integrates with `flate2` for `tar.gz`.
//
// Missing vs. Production:
// - **Long Names:** USTAR limits names to 100 chars (or 255 with prefix splitting). Modern tar uses GNU or PAX extensions to support arbitrarily long names. We just truncate/fail.
// - **Directory/Symlink Support:** We only implemented normal file types (`typeflag == '0'`). Real tar handles directories, symlinks, and hardlinks.
// - **Robust Octal Parsing:** GNU tar can sometimes store base-256 binary sizes in the size field if the file is too large for 11 octal digits. We only do strict ASCII octal.
//
// Suggested Next Steps:
// 1. Add support for creating and reading directory entries.
// 2. Implement PAX extended headers to support file names > 100 characters.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tar_archive_roundtrip() {
        let mut archive_data = Vec::new();

        // 1. Write Archive
        {
            let mut writer = TarWriter::new(&mut archive_data);

            let file1 = TarHeader {
                name: "hello.txt".to_string(),
                size: 13,
                mode: 0o644,
            };
            writer.append(&file1, b"Hello, World!").unwrap();

            let file2 = TarHeader {
                name: "test/data.bin".to_string(),
                size: 3,
                mode: 0o755,
            };
            writer.append(&file2, &[0x01, 0x02, 0x03]).unwrap();

            // writer.finish() is called automatically on Drop
        }

        // Output size should be:
        // 512 (header 1) + 512 (content 1 + pad) + 512 (header 2) + 512 (content 2 + pad) + 1024 (two zero blocks) = 3072 bytes.
        assert_eq!(archive_data.len(), 3072);

        // 2. Read Archive
        let mut reader = TarReader::new(archive_data.as_slice());

        // Read File 1
        let hdr1 = reader.next_header().unwrap().unwrap();
        assert_eq!(hdr1.name, "hello.txt");
        assert_eq!(hdr1.size, 13);
        assert_eq!(hdr1.mode, 0o644);

        let mut content1 = Vec::new();
        reader.read_content(hdr1.size, &mut content1).unwrap();
        assert_eq!(content1, b"Hello, World!");

        // Read File 2
        let hdr2 = reader.next_header().unwrap().unwrap();
        assert_eq!(hdr2.name, "test/data.bin");
        assert_eq!(hdr2.size, 3);
        assert_eq!(hdr2.mode, 0o755);

        let mut content2 = Vec::new();
        reader.read_content(hdr2.size, &mut content2).unwrap();
        assert_eq!(content2, &[0x01, 0x02, 0x03]);

        // End of Archive
        assert!(reader.next_header().unwrap().is_none());
    }

    #[test]
    fn test_padding_calculation() {
        assert_eq!(calculate_padding(0), 0);
        assert_eq!(calculate_padding(1), 511);
        assert_eq!(calculate_padding(511), 1);
        assert_eq!(calculate_padding(512), 0);
        assert_eq!(calculate_padding(513), 511);
    }

    #[test]
    fn test_octal_parsing() {
        assert_eq!(parse_octal(b"00000000015\0").unwrap(), 13);
        assert_eq!(parse_octal(b"777\0").unwrap(), 511);
        assert_eq!(parse_octal(b" \0 0123 \0").unwrap(), 83);
        assert!(parse_octal(b"999\0").is_err()); // Invalid octal digit
    }
}
