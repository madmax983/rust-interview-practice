//! # Base64 Encoder/Decoder
//!
//! Implements standard Base64 encoding and decoding as defined in RFC 4648.
//! It supports the standard alphabet and padding with `=`.
//!
//! **Replaces Crates:** `base64`
//!
//! **Real-world Usage:**
//! - Encoding binary data in email (MIME).
//! - Embedding images in HTML/CSS.
//! - Authorization headers (Basic Auth).
//! - WebSocket handshake keys.
//!
//! **Why build it yourself?**
//! Implementing Base64 demystifies how binary data is represented in ASCII.
//! You learn about bitwise manipulation (shifting, masking), padding logic,
//! and the trade-off between space efficiency (33% overhead) and text-safety.

// =========================================================================================
// Architecture
// =========================================================================================
//
// Flow:
//
//      Binary Data (Vec<u8>)
//           │
//           ▼
//      [Group into 24-bit chunks (3 bytes)]
//           │
//           ▼
//      [Split into four 6-bit indices]
//           │
//           ▼
//      [Map indices to Alphabet Chars]
//           │
//           ▼
//      Base64 String
//
//
// Invariants:
// 1. Every 3 bytes of input produce 4 characters of output.
// 2. If input length is not divisible by 3, output is padded with `=` to a multiple of 4.
// 3. Decoding ignores whitespace (optional, but good for robustness).
//
// Complexity:
// ┌───────────────┬─────────────┬─────────────┐
// │ Operation     │ Time        │ Space       │
// ├───────────────┼─────────────┼─────────────┤
// │ Encode        │ O(N)        │ O(N)        │
// ├───────────────┼─────────────┼─────────────┤
// │ Decode        │ O(N)        │ O(N)        │
// └───────────────┴─────────────┴─────────────┘
//
// Design Decisions:
// - **Standard Alphabet**: `A-Z, a-z, 0-9, +, /`. This is the most common variant.
// - **Padding**: Mandatory. Some implementations make it optional, but RFC 4648 standard requires it.
// - **Error Handling**: Returns `Result` for decoding errors (invalid char, bad length).

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Encodes binary data into a Base64 string.
pub fn encode<T: AsRef<[u8]>>(input: T) -> String {
    let bytes = input.as_ref();
    let mut buffer = String::with_capacity((bytes.len() * 4 / 3) + 4);

    // RUST INSIGHT: Chunks
    // Iterate over 3-byte chunks.
    // If the last chunk is smaller than 3, we handle padding manually.
    for chunk in bytes.chunks(3) {
        let b1 = chunk[0];
        let b2 = *chunk.get(1).unwrap_or(&0);
        let b3 = *chunk.get(2).unwrap_or(&0);

        // Combine 3 bytes (24 bits) into a u32
        // RUST INSIGHT: Bitwise Operations
        // We shift and OR to pack the bits.
        let combined: u32 = (u32::from(b1) << 16) | (u32::from(b2) << 8) | u32::from(b3);

        // Extract four 6-bit indices
        let i1 = (combined >> 18) & 0x3F;
        let i2 = (combined >> 12) & 0x3F;
        let i3 = (combined >> 6) & 0x3F;
        let i4 = combined & 0x3F;

        // Map to characters
        buffer.push(ALPHABET[i1 as usize] as char);
        buffer.push(ALPHABET[i2 as usize] as char);

        if chunk.len() > 1 {
            buffer.push(ALPHABET[i3 as usize] as char);
        } else {
            buffer.push('=');
        }

        if chunk.len() > 2 {
            buffer.push(ALPHABET[i4 as usize] as char);
        } else {
            buffer.push('=');
        }
    }

    buffer
}

/// Decodes a Base64 string into binary data.
///
/// # Errors
///
/// Returns an `Err` with a descriptive message if the input length is not a
/// multiple of four, contains an invalid character, or has malformed padding.
pub fn decode<T: AsRef<str>>(input: T) -> Result<Vec<u8>, String> {
    let input = input.as_ref();
    // Filter out whitespace/newlines if we want to be robust, but RFC 4648 implies strictness.
    // We'll be strict but allow ignoring padding logic during the loop if we handle it at the end?
    // Actually, let's just strip whitespace first or iterate carefully.
    // For simplicity and strictness, we assume no whitespace.

    let input_bytes = input.as_bytes();

    if input_bytes.len() % 4 != 0 {
        return Err("Invalid input length: must be multiple of 4".to_string());
    }

    let mut output = Vec::with_capacity(input.len() * 3 / 4);

    // Padding (`=`) is only valid in the final 4-char quantum; track the last index.
    let num_chunks = input_bytes.len() / 4;

    // Build reverse map for O(1) lookups?
    // Or just a helper function. Helper is O(1) effectively.
    // A 256-byte array for lookup is faster than a match or scan.
    // PRODUCTION NOTE: A static lookup table [u8; 256] initialized with 0xFF (invalid)
    // is the standard optimization.

    // Decoding logic iterating by 4 chars
    for (chunk_index, chunk) in input_bytes.chunks(4).enumerate() {
        if chunk.len() != 4 {
            return Err("Invalid chunk length".to_string());
        }

        let mut combined: u32 = 0;
        let mut padding_count = 0;

        for (i, &byte) in chunk.iter().enumerate() {
            if byte == b'=' {
                padding_count += 1;
                continue;
            }

            // GOTCHA: If padding appears in the middle of a chunk, it's invalid.
            // But standard decoders usually just stop or handle trailing padding.
            // Here, we check strictly.
            if padding_count > 0 {
                return Err("Unexpected character after padding".to_string());
            }

            let val = decode_char(byte)?;
            // Shift into position
            // i=0: top 6 bits (18..24)
            // i=1: next 6 bits (12..18)
            // i=2: next 6 bits (6..12)
            // i=3: low 6 bits (0..6)
            combined |= u32::from(val) << (18 - i * 6);
        }

        // Padding is only permitted in the terminal quantum. `Zg==Zg==` (padding in a
        // non-final chunk) must be rejected rather than silently decoded.
        if padding_count > 0 && chunk_index != num_chunks - 1 {
            return Err("Invalid padding: '=' only allowed in the final chunk".to_string());
        }

        // A single quantum can carry at most 2 padding characters. `====` / `A===`
        // (3 or 4 pads) are malformed and must not produce output.
        if padding_count > 2 {
            return Err("Invalid padding: too many '=' in chunk".to_string());
        }

        // Extract bytes
        // 24 bits total.
        // Byte 1: bits 16..24
        // Byte 2: bits 8..16
        // Byte 3: bits 0..8

        output.push(((combined >> 16) & 0xFF) as u8);

        if padding_count < 2 {
            output.push(((combined >> 8) & 0xFF) as u8);
        }

        if padding_count < 1 {
            output.push((combined & 0xFF) as u8);
        }
    }

    Ok(output)
}

fn decode_char(byte: u8) -> Result<u8, String> {
    match byte {
        b'A'..=b'Z' => Ok(byte - b'A'),
        b'a'..=b'z' => Ok(byte - b'a' + 26),
        b'0'..=b'9' => Ok(byte - b'0' + 52),
        b'+' => Ok(62),
        b'/' => Ok(63),
        _ => Err(format!("Invalid character: {}", byte as char)),
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `base64`: Highly optimized (SIMD support), configurable alphabets (URL-safe, bcrypt, etc.),
//   and streaming support. Ours is a simple, correct, scalar implementation.
//
// Missing vs. Production:
// - **Performance**: No SIMD or lookup tables for decoding.
// - **Configurability**: Hardcoded to standard alphabet.
// - **Streaming**: Requires loading all data into memory.
//
// Next Steps:
// 1. Add URL-safe variant support.
// 2. Implement a `DecoderReader` for streaming decoding.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_rfc_examples() {
        // RFC 4648 test vectors
        assert_eq!(encode(b""), "");
        assert_eq!(encode(b"f"), "Zg==");
        assert_eq!(encode(b"fo"), "Zm8=");
        assert_eq!(encode(b"foo"), "Zm9v");
        assert_eq!(encode(b"foob"), "Zm9vYg==");
        assert_eq!(encode(b"fooba"), "Zm9vYmE=");
        assert_eq!(encode(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn test_decode_rfc_examples() {
        assert_eq!(decode("").unwrap(), b"");
        assert_eq!(decode("Zg==").unwrap(), b"f");
        assert_eq!(decode("Zm8=").unwrap(), b"fo");
        assert_eq!(decode("Zm9v").unwrap(), b"foo");
        assert_eq!(decode("Zm9vYg==").unwrap(), b"foob");
        assert_eq!(decode("Zm9vYmE=").unwrap(), b"fooba");
        assert_eq!(decode("Zm9vYmFy").unwrap(), b"foobar");
    }

    #[test]
    fn test_decode_invalid() {
        assert!(decode("Z").is_err()); // Invalid length
        assert!(decode("Zg").is_err()); // Invalid length
        assert!(decode("Zg=").is_err()); // Invalid length (missing one char)
        assert!(decode("$$$$").is_err()); // Invalid chars
        assert!(decode("Zg=Z").is_err()); // Padding in middle (logic check)
    }

    #[test]
    fn test_decode_invalid_padding() {
        // Regression: over-padded quanta and padding in a non-final chunk were
        // previously accepted and produced output. They must now be rejected.
        assert!(decode("====").is_err()); // 4 padding chars
        assert!(decode("A===").is_err()); // 3 padding chars
        assert!(decode("Zg==Zg==").is_err()); // padding in a non-final chunk

        // Sanity: legitimate terminal padding still round-trips.
        assert_eq!(decode("Zg==").unwrap(), b"f");
    }

    #[test]
    fn test_binary_data() {
        let data = vec![0u8, 255u8, 127u8, 64u8];
        let encoded = encode(&data);
        let decoded = decode(&encoded).unwrap();
        assert_eq!(data, decoded);
    }
}
