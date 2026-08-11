//! # Tar Archive Implementation (USTAR)
//!
//! **Replaces Crates:** `tar`
//!
//! **Real-world systems:** GNU `tar`, `libarchive`, container image tools (Docker images are just layered tarballs).
//!
//! **Why build it yourself?** To understand legacy file formats, binary serialization, octal string encoding,
//! and handling fixed-size 512-byte block alignment. Tar is deceptively simple but requires strict attention
//! to block padding and metadata encoding rules that are common in low-level systems programming.
//!
//! ## Architecture
//!
//! A tar archive is a sequence of files. Each file consists of a 512-byte header block followed by the
//! file data, which is also padded to a multiple of 512 bytes. The archive ends with at least two empty
//! 512-byte blocks.
//!
//! ```text
//! +---------------------+
//! | Header (512 bytes)  | -> Contains metadata (name, size, mode, etc.) as ASCII octal strings
//! +---------------------+
//! | File Data Block 1   | -> (512 bytes)
//! +---------------------+
//! | File Data Block 2   | -> (512 bytes)
//! +---------------------+
//! | ... padding ...     | -> Null bytes to reach 512 byte boundary
//! +---------------------+
//! | Header 2 ...        |
//! +---------------------+
//! | ...                 |
//! +---------------------+
//! | End of Archive      | -> Two 512-byte blocks of null bytes
//! +---------------------+
//! ```
//!
//! ### Invariants
//! * Every block read or written must be exactly 512 bytes long.
//! * Header numeric fields (size, mode, etc.) must be ASCII octal strings, null or space-terminated.
//! * Checksum must be calculated by treating the checksum field itself as all spaces.
//!
//! ### Time/Space Complexity
//! | Operation | Time Complexity | Space Complexity |
//! |-----------|-----------------|------------------|
//! | Write     | O(N)            | O(1) buffer      |
//! | Read      | O(N)            | O(1) buffer      |
//!
//! ## Footer
//!
//! **Comparison to `tar` crate:** The canonical `tar` crate supports sparse files, long names (GNU and PAX extensions),
//! directory traversal, and seamless file-system extraction. Our implementation is limited to basic USTAR
//! and supports only standard file names (up to 100 characters).
//!
//! **Missing from production:**
//! - PAX extended headers for long paths and large files.
//! - GNU tar extensions (e.g., sparse files).
//! - Streaming file extraction directly to the filesystem.
//! - Robust error recovery for corrupt archives.
//!
//! **Next steps:**
//! - Implement PAX headers to support file names longer than 100 characters and files larger than 8GB.
//! - Add a directory extraction utility with permission preservation.

use std::io::{self, Read, Write};

const BLOCK_SIZE: usize = 512;

/// File type flags defined by the USTAR specification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileType {
    NormalFile,
    HardLink,
    SymbolicLink,
    CharacterSpecial,
    BlockSpecial,
    Directory,
    Fifo,
    Unknown(u8),
}

impl FileType {
    fn from_byte(b: u8) -> Self {
        match b {
            b'0' | 0 => FileType::NormalFile,
            b'1' => FileType::HardLink,
            b'2' => FileType::SymbolicLink,
            b'3' => FileType::CharacterSpecial,
            b'4' => FileType::BlockSpecial,
            b'5' => FileType::Directory,
            b'6' => FileType::Fifo,
            _ => FileType::Unknown(b),
        }
    }

    fn to_byte(self) -> u8 {
        match self {
            FileType::NormalFile => b'0',
            FileType::HardLink => b'1',
            FileType::SymbolicLink => b'2',
            FileType::CharacterSpecial => b'3',
            FileType::BlockSpecial => b'4',
            FileType::Directory => b'5',
            FileType::Fifo => b'6',
            FileType::Unknown(b) => b,
        }
    }
}

/// A parsed entry header from a Tar archive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TarHeader {
    pub name: String,
    pub mode: u32,
    pub uid: u32,
    pub gid: u32,
    pub size: u64,
    pub mtime: u64,
    pub typeflag: FileType,
    pub linkname: String,
    pub uname: String,
    pub gname: String,
}

impl Default for TarHeader {
    fn default() -> Self {
        Self {
            name: String::new(),
            mode: 0o644,
            uid: 1000,
            gid: 1000,
            size: 0,
            mtime: 0,
            typeflag: FileType::NormalFile,
            linkname: String::new(),
            uname: "root".to_string(),
            gname: "root".to_string(),
        }
    }
}

/// Helper function to parse an octal ASCII string into a u64.
///
/// // GOTCHA: Tar octal strings can be terminated by a null byte or a space, and may contain
/// leading spaces or nulls.
fn parse_octal(bytes: &[u8]) -> Option<u64> {
    let mut val = 0;
    let mut started = false;

    for &b in bytes {
        if b == 0 || b == b' ' {
            if started {
                break;
            }
            continue;
        }
        // RUST INSIGHT: Pattern matching on byte literals is a clean, zero-cost abstraction for ASCII parsing.
        if (b'0'..=b'7').contains(&b) {
            val = val * 8 + (b - b'0') as u64;
            started = true;
        } else {
            return None; // Invalid octal character
        }
    }
    Some(val)
}

/// Helper to write a number as an ASCII octal string, null-terminated.
fn write_octal(buf: &mut [u8], mut val: u64) {
    let len = buf.len();
    buf[len - 1] = 0; // Null terminator
    for i in (0..len - 1).rev() {
        buf[i] = b'0' + (val & 7) as u8;
        val >>= 3;
    }
}

/// Reads a null-terminated string from a byte slice.
fn read_string(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).into_owned()
}

/// Writes a string to a byte slice, padded with nulls.
fn write_string(buf: &mut [u8], s: &str) {
    let bytes = s.as_bytes();
    let copy_len = bytes.len().min(buf.len());
    buf[..copy_len].copy_from_slice(&bytes[..copy_len]);
    buf[copy_len..].fill(0);
}

/// Calculates the USTAR header checksum.
fn calculate_checksum(header: &[u8; BLOCK_SIZE]) -> u32 {
    let mut sum: u32 = 0;
    for (i, &b) in header.iter().enumerate() {
        // The checksum field (bytes 148-155) is treated as all spaces (32) during calculation.
        if (148..156).contains(&i) {
            sum += b' ' as u32;
        } else {
            sum += b as u32;
        }
    }
    sum
}

/// A writer for creating USTAR archives.
pub struct TarWriter<W: Write> {
    inner: W,
    finished: bool,
}

impl<W: Write> TarWriter<W> {
    pub fn new(inner: W) -> Self {
        Self {
            inner,
            finished: false,
        }
    }

    /// Appends a new file entry to the archive.
    pub fn append(&mut self, header: &TarHeader, data: &[u8]) -> io::Result<()> {
        if self.finished {
            return Err(io::Error::other("Archive already finished"));
        }

        let mut block = [0u8; BLOCK_SIZE];

        write_string(&mut block[0..100], &header.name);
        write_octal(&mut block[100..108], header.mode as u64);
        write_octal(&mut block[108..116], header.uid as u64);
        write_octal(&mut block[116..124], header.gid as u64);
        write_octal(&mut block[124..136], header.size);
        write_octal(&mut block[136..148], header.mtime);

        block[156] = header.typeflag.to_byte();
        write_string(&mut block[157..257], &header.linkname);

        // USTAR magic and version
        block[257..263].copy_from_slice(b"ustar\0");
        block[263..265].copy_from_slice(b"00");

        write_string(&mut block[265..297], &header.uname);
        write_string(&mut block[297..329], &header.gname);

        // Calculate and write checksum
        let checksum = calculate_checksum(&block);
        // GOTCHA: Checksum is terminated by a null and a space in some older tars, but
        // standard USTAR is 6 octal digits followed by null and space.
        let mut chksum_buf = [b'0'; 8];
        write_octal(&mut chksum_buf[0..7], checksum as u64);
        chksum_buf[6] = 0;
        chksum_buf[7] = b' ';
        block[148..156].copy_from_slice(&chksum_buf);

        self.inner.write_all(&block)?;

        // Write data
        self.inner.write_all(data)?;

        // Padding
        let remainder = data.len() % BLOCK_SIZE;
        if remainder != 0 {
            let padding = BLOCK_SIZE - remainder;
            let pad_buf = vec![0u8; padding];
            self.inner.write_all(&pad_buf)?;
        }

        Ok(())
    }

    /// Finishes the archive by writing the required two empty EOF blocks.
    pub fn finish(&mut self) -> io::Result<()> {
        if !self.finished {
            let eof_blocks = [0u8; BLOCK_SIZE * 2];
            self.inner.write_all(&eof_blocks)?;
            self.finished = true;
        }
        Ok(())
    }

    // PRODUCTION NOTE: Real tar implementations usually impl `Drop` to ensure the archive
    // is finalized, or consume `self` in `finish()`. We use an explicit finish method here
    // for simplicity and error bubbling.
}

/// Represents an entry read from a Tar archive.
pub struct TarEntry {
    pub header: TarHeader,
    pub data: Vec<u8>,
}

/// A reader for extracting from USTAR archives.
pub struct TarReader<R: Read> {
    inner: R,
}

impl<R: Read> TarReader<R> {
    pub fn new(inner: R) -> Self {
        Self { inner }
    }

    /// Reads the next entry in the archive.
    /// Returns `None` if the end of the archive (empty blocks) is reached.
    pub fn read_next(&mut self) -> io::Result<Option<TarEntry>> {
        let mut block = [0u8; BLOCK_SIZE];
        let mut bytes_read = 0;

        while bytes_read < BLOCK_SIZE {
            let n = self.inner.read(&mut block[bytes_read..])?;
            if n == 0 {
                if bytes_read == 0 {
                    return Ok(None); // EOF
                }
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "Incomplete tar header block",
                ));
            }
            bytes_read += n;
        }

        // Check for empty block (end of archive marker)
        // RUST INSIGHT: `.iter().all()` is idiomatic and fast for checking properties of a slice.
        if block.iter().all(|&b| b == 0) {
            return Ok(None);
        }

        // Verify checksum
        let expected_checksum = parse_octal(&block[148..156]).unwrap_or(0) as u32;
        let actual_checksum = calculate_checksum(&block);
        if expected_checksum != actual_checksum {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Tar header checksum mismatch",
            ));
        }

        let header = TarHeader {
            name: read_string(&block[0..100]),
            mode: parse_octal(&block[100..108]).unwrap_or(0) as u32,
            uid: parse_octal(&block[108..116]).unwrap_or(0) as u32,
            gid: parse_octal(&block[116..124]).unwrap_or(0) as u32,
            size: parse_octal(&block[124..136]).unwrap_or(0),
            mtime: parse_octal(&block[136..148]).unwrap_or(0),
            typeflag: FileType::from_byte(block[156]),
            linkname: read_string(&block[157..257]),
            uname: read_string(&block[265..297]),
            gname: read_string(&block[297..329]),
        };

        let mut data = vec![0u8; header.size as usize];
        self.inner.read_exact(&mut data)?;

        // Consume padding
        let remainder = (header.size as usize) % BLOCK_SIZE;
        if remainder != 0 {
            let padding = BLOCK_SIZE - remainder;
            let mut pad_buf = vec![0u8; padding];
            self.inner.read_exact(&mut pad_buf)?;
        }

        Ok(Some(TarEntry { header, data }))
    }

    /// Reads all remaining entries into a vector.
    pub fn read_all(&mut self) -> io::Result<Vec<TarEntry>> {
        let mut entries = Vec::new();
        while let Some(entry) = self.read_next()? {
            entries.push(entry);
        }
        Ok(entries)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_octal_parsing() {
        let mut buf = [0u8; 8];
        write_octal(&mut buf, 0o644);
        assert_eq!(&buf[0..7], b"0000644");
        assert_eq!(buf[7], 0);

        assert_eq!(parse_octal(b"0000644\0").unwrap(), 0o644);
        assert_eq!(parse_octal(b"  00644 \0").unwrap(), 0o644);
    }

    #[test]
    fn test_round_trip() {
        let mut archive_data = Vec::new();

        let header1 = TarHeader {
            name: "hello.txt".to_string(),
            size: 13,
            ..Default::default()
        };
        let data1 = b"Hello, World!";

        let header2 = TarHeader {
            name: "test/dir/".to_string(),
            typeflag: FileType::Directory,
            size: 0,
            ..Default::default()
        };
        let data2 = b"";

        {
            let mut writer = TarWriter::new(&mut archive_data);
            writer.append(&header1, data1).unwrap();
            writer.append(&header2, data2).unwrap();
            writer.finish().unwrap();
        }

        // Total size should be:
        // 512 (header1) + 512 (data1 padded) + 512 (header2) + 0 (data2) + 1024 (EOF) = 2560 bytes
        assert_eq!(archive_data.len(), 512 + 512 + 512 + 1024);

        let mut reader = TarReader::new(Cursor::new(archive_data));
        let entries = reader.read_all().unwrap();

        assert_eq!(entries.len(), 2);

        assert_eq!(entries[0].header.name, "hello.txt");
        assert_eq!(entries[0].header.size, 13);
        assert_eq!(entries[0].data, b"Hello, World!");
        assert_eq!(entries[0].header.typeflag, FileType::NormalFile);

        assert_eq!(entries[1].header.name, "test/dir/");
        assert_eq!(entries[1].header.size, 0);
        assert_eq!(entries[1].data, b"");
        assert_eq!(entries[1].header.typeflag, FileType::Directory);
    }
}
