//! # Tar Archive (USTAR format)
//!
//! ## What this implements and what it replaces
//! This implements a basic USTAR (Unix Standard TAR) archive reader and writer.
//! It replaces the `tar` crate for foundational tarball packing/unpacking capabilities.
//!
//! ## Real-world systems that use this
//! Almost every Unix-like OS, container image format (Docker uses tar to pack image layers),
//! and language package manager (e.g., npm) relies on tar.
//!
//! ## Why build it yourself?
//! The tar format is notoriously quirky—it relies on 512-byte block alignment and
//! ASCII octal strings for metadata. Building it reveals how legacy standards are
//! maintained and how byte-level alignment impacts parsing logic.
//!
//! ## Architecture
//! ```text
//! [ Header Block (512 bytes) ]
//! [ Data Block 1 (512 bytes) ]
//! [ Data Block 2 (512 bytes) ]
//! [ Padding to fill 512-byte boundary ]
//! ...
//! [ End of Archive: Two 512-byte blocks of nulls ]
//! ```
//!
//! ### Invariants
//! - All blocks must be exactly 512 bytes.
//! - The end of an archive is marked by at least two consecutive 512-byte blocks of null bytes.
//! - Metadata numbers (size, mode, mtime, checksum) are encoded as ASCII octal strings terminated by a null or space.
//!
//! ### Time / Space Complexity
//! - **Appending/Reading File**: Time `O(N)` where `N` is the file size. Space `O(512)`.
//!
//! ### Design Decisions & Tradeoffs
//! - **Memory vs Streaming**: We provide basic memory-based structures. A production reader (`tar` crate) streams from an `io::Read` source directly to disk.
//! - **Checksums**: We implement the checksum calculation which requires treating the checksum field as spaces during computation.
use std::io::{self, Write};
/// Standard size of a TAR block.
pub const BLOCK_SIZE: usize = 512;
/// A simple in-memory tar archive writer.
pub struct TarWriter<W: Write> {
    inner: W,
    written_bytes: u64,
}
impl<W: Write> TarWriter<W> {
    pub const fn new(inner: W) -> Self {
        Self {
            inner,
            written_bytes: 0,
        }
    }
    /// Appends a file to the archive.
    ///
    /// # Errors
    /// Returns an `io::Result` error if writing to the underlying writer fails.
    pub fn append_file(&mut self, name: &str, content: &[u8]) -> io::Result<()> {
        let mut header = [0u8; BLOCK_SIZE];
        // 0-99: File name
        let name_bytes = name.as_bytes();
        let name_len = name_bytes.len().min(100);
        header[0..name_len].copy_from_slice(&name_bytes[0..name_len]);
        // 100-107: File mode (octal)
        Self::write_octal(&mut header[100..108], 0o644);
        // 108-115: Owner's numeric user ID
        Self::write_octal(&mut header[108..116], 1000);
        // 116-123: Group's numeric user ID
        Self::write_octal(&mut header[116..124], 1000);
        // 124-135: File size in bytes (octal)
        Self::write_octal(&mut header[124..136], content.len() as u64);
        // 136-147: Last modification time in numeric Unix time format (octal)
        Self::write_octal(&mut header[136..148], 0); // Simplified
        // 156: Type flag (0 or '0' for normal file)
        header[156] = b'0';
        // 257-262: USTAR indicator "ustar\0"
        header[257..263].copy_from_slice(b"ustar\0");
        // 263-264: USTAR version "00"
        header[263..265].copy_from_slice(b"00");
        // Calculate and write checksum (148-155)
        // GOTCHA: Checksum calculation assumes the checksum field itself is filled with spaces.
        for byte in &mut header[148..156] {
            *byte = b' ';
        }
        let checksum: u32 = header.iter().map(|&b| u32::from(b)).sum();
        Self::write_octal(&mut header[148..155], u64::from(checksum));
        header[155] = b' '; // often space or null terminated
        // Write header
        self.inner.write_all(&header)?;
        self.written_bytes += BLOCK_SIZE as u64;
        // Write content
        self.inner.write_all(content)?;
        self.written_bytes += content.len() as u64;
        // Write padding
        let padding = BLOCK_SIZE - (content.len() % BLOCK_SIZE);
        if padding < BLOCK_SIZE {
            let pad_buf = vec![0u8; padding];
            self.inner.write_all(&pad_buf)?;
            self.written_bytes += padding as u64;
        }
        Ok(())
    }
    /// Finishes the archive by writing the required EOF markers.
    ///
    /// # Errors
    /// Returns an `io::Result` error if writing to the underlying writer fails.
    pub fn finish(mut self) -> io::Result<W> {
        let eof = [0u8; BLOCK_SIZE * 2];
        self.inner.write_all(&eof)?;
        Ok(self.inner)
    }
    /// Helper to write octal strings into a buffer.
    fn write_octal(buf: &mut [u8], value: u64) {
        let s = format!("{value:o}");
        let len = s.len();
        let buf_len = buf.len();
        // Typically padded with zeros, ended with null or space
        for (i, byte) in buf.iter_mut().enumerate() {
            if i < buf_len - len - 1 {
                *byte = b'0'; // padding
            } else if i < buf_len - 1 {
                *byte = s.as_bytes()[i - (buf_len - len - 1)];
            } else {
                *byte = 0; // null terminator
            }
        }
    }
}
/// ## Footer
///
/// ### Comparison to Canonical Crates
/// The `tar` crate is robust, handles long filenames (GNU extensions or POSIX PAX headers),
/// hard links, symlinks, and properly unescapes/validates paths against traversal attacks.
///
/// ### Missing vs Production
/// - **Path traversal protection**: A real reader must prevent extracting files outside the target dir (e.g., `../../etc/passwd`).
/// - **Extended headers**: We don't support file sizes > 8GB which require base256 encoding or PAX headers.
/// - **Reader implementation**: A full reader would parse these blocks safely.
///
/// ### Benchmarking Notes
/// The `append_file` operation can be benchmarked using `criterion` by writing numerous
/// small files or a few extremely large files to a `TarWriter` wrapping an in-memory `Vec<u8>`
/// to isolate logic performance from disk I/O overhead.
///
/// ### Suggested Next Steps
/// - Implement `TarReader` to parse these blocks.
/// - Add PAX header support for long file paths.
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_tar_writer_basic() {
        let mut buf = Vec::new();
        let mut writer = TarWriter::new(&mut buf);
        writer.append_file("test.txt", b"hello world").unwrap();
        writer.finish().unwrap();
        // 1 header block + 1 data block (padded) + 2 EOF blocks = 4 blocks
        assert_eq!(buf.len(), BLOCK_SIZE * 4);
        // Check file name
        assert_eq!(&buf[0..8], b"test.txt");
        // Check USTAR magic
        assert_eq!(&buf[257..262], b"ustar");
        // Check data block
        assert_eq!(&buf[BLOCK_SIZE..BLOCK_SIZE + 11], b"hello world");
    }
    #[test]
    fn test_octal_formatting() {
        let mut buf = [0u8; 8];
        TarWriter::<Vec<u8>>::write_octal(&mut buf, 0o644);
        assert_eq!(&buf, b"0000644\0");
    }
}
