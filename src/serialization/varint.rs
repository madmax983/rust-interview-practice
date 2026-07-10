//! Variable Integer Encoding (LEB128) implementation.
//!
//! # Header
//!
//! *   **Problem Name**: Variable Integer Encoding (LEB128)
//! *   **Difficulty**: Medium
//! *   **Link**: <https://en.wikipedia.org/wiki/LEB128>
//! *   **Why this matters in Rust**: Essential for binary serialization protocols (like Protobuf) to save space.
//!
//! # Architecture
//!
//! LEB128 (Little Endian Base 128) is a variable-length code compression for arbitrary sized integers.
//! It works by chopping the integer into 7-bit chunks. The 8th bit (MSB) of each byte indicates
//! if there are more bytes to follow.
//!
//! **Diagram:**
//!
//! ```text
//! Value: 624485 (0x98765)
//! Binary: 1001 1000 0111 0110 0101
//!
//! Chunk 1: 1110 0101 (0xE5) -> LSB 7 bits: 1100101, MSB 1 (more)
//! Chunk 2: 1000 1110 (0x8E) -> Next 7 bits: 0001110, MSB 1 (more)
//! Chunk 3: 0010 0110 (0x26) -> Next 7 bits: 0100110, MSB 0 (done)
//! ```
//!
//! **Invariants:**
//! *   The output byte stream encodes the exact value of the input integer.
//! *   Decoding must stop when a byte with MSB 0 is encountered.
//! *   Decoding must handle malformed input (e.g., incomplete stream).
//!
//! **Complexity:**
//!
//! | Operation | Time | Space |
//! | :--- | :--- | :--- |
//! | Encode | O(log N) | O(log N) |
//! | Decode | O(log N) | O(1) |

use std::io::{self, Read, Write};

/// Encodes an unsigned 64-bit integer into a writer using LEB128.
// RUST INSIGHT: Using `impl Write` allows this to work with any writer (File, TcpStream, Vec<u8>), making it highly reusable.
pub fn encode_u64<W: Write>(writer: &mut W, mut value: u64) -> io::Result<usize> {
    let mut bytes_written = 0;
    loop {
        let mut byte = (value & 0x7F) as u8;
        value >>= 7;
        if value != 0 {
            byte |= 0x80; // Set MSB to 1 to indicate more bytes
        }
        writer.write_all(&[byte])?;
        bytes_written += 1;
        if value == 0 {
            break;
        }
    }
    Ok(bytes_written)
}

/// Decodes an unsigned 64-bit integer from a reader using LEB128.
// RUST INSIGHT: `impl Read` allows us to decode from any source without loading the whole buffer into memory.
pub fn decode_u64<R: Read>(reader: &mut R) -> io::Result<u64> {
    let mut result = 0;
    let mut shift = 0;
    let mut buf = [0u8; 1];

    loop {
        // GOTCHA: We must handle potential EOF or read errors gracefully.
        reader.read_exact(&mut buf)?;
        let byte = buf[0];

        // PRODUCTION NOTE: In a real implementation, we might check for overflow if shift > 63.
        // Here we assume valid u64 LEB128.
        if shift >= 64 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "LEB128 overflow",
            ));
        }

        // On the 10th byte (shift == 63) only bit 0 of the 7-bit group fits in a u64.
        // Any higher bit would be silently dropped by the `<< 63` below, accepting an
        // overlong/overflowing varint as a wrong value. Reject it explicitly.
        if shift == 63 && (byte & 0x7F) > 0x01 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "LEB128 overflow",
            ));
        }

        result |= ((byte & 0x7F) as u64) << shift;
        if (byte & 0x80) == 0 {
            break;
        }
        shift += 7;
    }
    Ok(result)
}

/// Encodes a signed 64-bit integer into a writer using ZigZag LEB128.
/// ZigZag encoding maps signed integers to unsigned integers so that small negative numbers
/// become small unsigned numbers (e.g., -1 -> 1, 1 -> 2, -2 -> 3).
// RUST INSIGHT: ZigZag encoding is crucial for efficiency with signed integers because standard two's complement
// for small negative numbers (like -1) has all high bits set, which would result in max-length LEB128.
pub fn encode_i64<W: Write>(writer: &mut W, value: i64) -> io::Result<usize> {
    let zigzag = ((value << 1) ^ (value >> 63)) as u64;
    encode_u64(writer, zigzag)
}

/// Decodes a signed 64-bit integer from a reader using ZigZag LEB128.
pub fn decode_i64<R: Read>(reader: &mut R) -> io::Result<i64> {
    let zigzag = decode_u64(reader)?;
    let value = (zigzag >> 1) as i64 ^ -((zigzag & 1) as i64);
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_encode_decode_u64() {
        let test_cases = vec![
            (0, vec![0x00]),
            (1, vec![0x01]),
            (127, vec![0x7F]),
            (128, vec![0x80, 0x01]),
            (300, vec![0xAC, 0x02]),
            (
                u64::MAX,
                vec![0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x01],
            ),
        ];

        for (value, expected) in test_cases {
            let mut buf = Vec::new();
            encode_u64(&mut buf, value).unwrap();
            assert_eq!(buf, expected, "Failed encoding {}", value);

            let mut cursor = Cursor::new(buf);
            let decoded = decode_u64(&mut cursor).unwrap();
            assert_eq!(decoded, value, "Failed decoding {}", value);
        }
    }

    #[test]
    fn test_encode_decode_i64() {
        let test_cases = vec![
            (0, vec![0x00]),
            (-1, vec![0x01]),
            (1, vec![0x02]),
            (-2, vec![0x03]),
            (2, vec![0x04]),
            (
                i64::MAX,
                vec![0xFE, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x01],
            ),
            (
                i64::MIN,
                vec![0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x01],
            ),
        ];

        for (value, expected) in test_cases {
            let mut buf = Vec::new();
            encode_i64(&mut buf, value).unwrap();
            assert_eq!(buf, expected, "Failed encoding {}", value);

            let mut cursor = Cursor::new(buf);
            let decoded = decode_i64(&mut cursor).unwrap();
            assert_eq!(decoded, value, "Failed decoding {}", value);
        }
    }

    #[test]
    fn test_decode_overflow_tenth_byte() {
        // Regression: a 10th byte carrying more than one usable bit (shift == 63)
        // previously passed the `shift >= 64` guard and had its high bits silently
        // dropped by `<< 63`, accepting an overflowing varint as a wrong value.
        let buf = vec![0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x7F];
        let mut cursor = Cursor::new(buf);
        assert!(decode_u64(&mut cursor).is_err());

        // The canonical maximum u64 (10th byte == 0x01) must still decode correctly.
        let max = vec![0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x01];
        let mut cursor = Cursor::new(max);
        assert_eq!(decode_u64(&mut cursor).unwrap(), u64::MAX);
    }

    #[test]
    fn test_decode_overflow() {
        // Construct a buffer that would cause overflow (too many bytes with high bit set)
        let buf = vec![0x80; 12]; // 12 bytes with continuation bit is definitely too long for u64
        let mut cursor = Cursor::new(buf);
        assert!(decode_u64(&mut cursor).is_err());
    }
}

// Footer
//
// *   **Comparison**: This is similar to how `prost` or `quick-protobuf` handles varints internally.
// *   **Missing features**: Optimization for specific buffer types (like `BytesMut`), handling of `u128`.
