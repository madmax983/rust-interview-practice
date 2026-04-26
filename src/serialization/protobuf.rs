//! # Protobuf / Binary Serialization Implementation
//!
//! Implements a minimal, zero-allocation binary serializer/deserializer modeled after Protocol Buffers.
//! It supports Varint encoding, ZigZag encoding for signed integers, and tag-based field resolution.
//!
//! **Replaces Crates:** `prost`, `protobuf`
//!
//! **Real-world Usage:**
//! - High-performance gRPC microservices.
//! - Compact data storage (e.g., in a time-series database or LSM tree).
//! - Cross-language telemetry streaming.
//!
//! **Why build it yourself?**
//! Understanding binary wire formats makes you better at diagnosing network overhead.
//! Implementing Varint and ZigZag encoding teaches you bitwise manipulation and how to pack integers efficiently.
//! You'll learn how schema evolution (adding/removing fields) works via field tags instead of rigid struct layouts.

// =========================================================================================
// Architecture
// =========================================================================================
//
// Wire Format:
//
// Each field is encoded as:
// [Tag (Field Number << 3 | Wire Type)] -> [Value Data]
//
// Wire Types:
// 0: Varint (int32, int64, uint32, uint64, sint32, sint64, bool, enum)
// 1: 64-bit (fixed64, sfixed64, double)
// 2: Length-delimited (string, bytes, embedded messages, packed repeated fields)
// 5: 32-bit (fixed32, sfixed32, float)
//
// Varint Encoding:
// 7 bits of data per byte. The Most Significant Bit (MSB) is the continuation bit.
// 1 = more bytes to follow, 0 = last byte.
//
// ZigZag Encoding:
// Maps signed integers to unsigned integers so that numbers with a small absolute value
// have a small varint encoded value.
// Formula: (n << 1) ^ (n >> 31)
//
// Invariants:
// 1. Tags must be > 0.
// 2. Decoder must skip unknown fields safely based on their wire type.
// 3. Serialized size should be exactly pre-computable to avoid reallocations.

use std::convert::TryFrom;

/// Protobuf Wire Types.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum WireType {
    Varint = 0,
    Fixed64 = 1,
    LengthDelimited = 2,
    Fixed32 = 5,
}

impl TryFrom<u8> for WireType {
    type Error = &'static str;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value & 0x07 {
            0 => Ok(WireType::Varint),
            1 => Ok(WireType::Fixed64),
            2 => Ok(WireType::LengthDelimited),
            5 => Ok(WireType::Fixed32),
            _ => Err("Invalid wire type"),
        }
    }
}

/// A trait for message types that can be serialized to and deserialized from a binary format.
pub trait Message: Sized + Default {
    /// Encodes the message to the given buffer.
    fn encode(&self, buf: &mut Vec<u8>);

    /// Decodes the message from the given buffer slice. Returns the number of bytes read.
    fn decode(buf: &[u8]) -> Result<(Self, usize), &'static str>;

    /// Computes the exact encoded size of this message.
    fn encoded_len(&self) -> usize;
}

/// Helper functions for Varint encoding.
pub struct Varint;

impl Varint {
    /// Computes the size of a u64 encoded as a varint.
    #[must_use]
    pub fn encoded_len(mut val: u64) -> usize {
        if val == 0 {
            return 1;
        }
        let mut len = 0;
        while val > 0 {
            len += 1;
            val >>= 7;
        }
        len
    }

    /// Encodes a u64 as a varint into the buffer.
    pub fn encode(mut val: u64, buf: &mut Vec<u8>) {
        loop {
            let mut byte = (val & 0x7F) as u8;
            val >>= 7;
            if val != 0 {
                byte |= 0x80; // Set continuation bit
                buf.push(byte);
            } else {
                buf.push(byte);
                break;
            }
        }
    }

    /// Decodes a u64 varint from the buffer. Returns the value and bytes read.
    pub fn decode(buf: &[u8]) -> Result<(u64, usize), &'static str> {
        let mut val = 0u64;
        let mut shift = 0;

        for (i, &byte) in buf.iter().enumerate() {
            if shift >= 64 {
                return Err("Varint too long");
            }

            // Extract the 7 bits of data
            val |= u64::from(byte & 0x7F) << shift;
            shift += 7;

            // Check continuation bit
            if byte & 0x80 == 0 {
                return Ok((val, i + 1));
            }
        }
        Err("Buffer exhausted before varint completed")
    }

    /// ZigZag encodes a signed 64-bit integer to an unsigned 64-bit integer.
    #[must_use]
    pub fn zigzag_encode(val: i64) -> u64 {
        // RUST INSIGHT: Arithmetic shift right (`>>`) on a signed integer duplicates the sign bit.
        // `val >> 63` will be all 1s (-1) if negative, or all 0s (0) if positive.
        // XORing with this mask effectively flips the bits if it was negative.
        ((val << 1) ^ (val >> 63)) as u64
    }

    /// ZigZag decodes an unsigned 64-bit integer back to a signed 64-bit integer.
    #[must_use]
    pub fn zigzag_decode(val: u64) -> i64 {
        // Shift right logical, then XOR with the negation of the least significant bit.
        let right_shifted = val >> 1;
        let lsb = val & 1;
        let mask = if lsb == 1 { !0u64 } else { 0u64 };
        (right_shifted ^ mask) as i64
    }
}

/// Helper for encoding fields with tags.
pub struct Field;

impl Field {
    /// Encodes a tag (field number and wire type).
    pub fn encode_tag(field_number: u32, wire_type: WireType, buf: &mut Vec<u8>) {
        let tag = (field_number << 3) | (wire_type as u32);
        Varint::encode(u64::from(tag), buf);
    }

    /// Decodes a tag from the buffer. Returns (field_number, wire_type, bytes_read).
    pub fn decode_tag(buf: &[u8]) -> Result<(u32, WireType, usize), &'static str> {
        let (tag, read) = Varint::decode(buf)?;
        let wire_type = WireType::try_from((tag & 0x07) as u8)?;
        let field_number = (tag >> 3) as u32;
        Ok((field_number, wire_type, read))
    }

    /// Skips a field based on its wire type. Returns the number of bytes to skip.
    pub fn skip(wire_type: WireType, buf: &[u8]) -> Result<usize, &'static str> {
        match wire_type {
            WireType::Varint => {
                let (_, read) = Varint::decode(buf)?;
                Ok(read)
            }
            WireType::Fixed64 => {
                if buf.len() >= 8 {
                    Ok(8)
                } else {
                    Err("Buffer too short for Fixed64")
                }
            }
            WireType::Fixed32 => {
                if buf.len() >= 4 {
                    Ok(4)
                } else {
                    Err("Buffer too short for Fixed32")
                }
            }
            WireType::LengthDelimited => {
                let (len, read) = Varint::decode(buf)?;
                let total = read + (len as usize);
                if buf.len() >= total {
                    Ok(total)
                } else {
                    Err("Buffer too short for LengthDelimited data")
                }
            }
        }
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `prost`: `prost` generates highly optimized Rust structs directly from `.proto` files
//   using a build script (`prost-build`). It handles the full spec (oneof, repeated, maps).
//   Our implementation provides the core primitives that a code generator would output.
// - `protobuf`: The other major crate, slightly heavier and more feature-rich regarding
//   reflection and unknown fields.
//
// Missing vs. Production:
// - **Code Generation**: Real protobuf uses a compiler (`protoc`) to generate the `Message` impls.
// - **Repeated Fields / Maps**: No built-in helpers for packed repeated fields or maps.
// - **Unknown Field Retention**: Production parsers often keep unknown fields in a byte buffer
//   so they aren't lost if the message is re-serialized. We just skip them.
//
// Next Steps:
// 1. Write a macro `#[derive(Message)]` to automate `encode`/`decode` implementations.
// 2. Implement string and embedded message helpers.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_varint_encode_decode() {
        let cases = [0, 1, 127, 128, 300, 16384, u64::MAX];

        for &val in &cases {
            let mut buf = Vec::new();
            Varint::encode(val, &mut buf);

            assert_eq!(buf.len(), Varint::encoded_len(val));

            let (decoded, read) = Varint::decode(&buf).unwrap();
            assert_eq!(decoded, val);
            assert_eq!(read, buf.len());
        }
    }

    #[test]
    fn test_zigzag() {
        let cases = [0, -1, 1, -2, 2, i64::MIN, i64::MAX];

        for &val in &cases {
            let encoded = Varint::zigzag_encode(val);
            let decoded = Varint::zigzag_decode(encoded);
            assert_eq!(decoded, val);
        }

        // Ensure small negative numbers map to small positive numbers
        assert_eq!(Varint::zigzag_encode(-1), 1);
        assert_eq!(Varint::zigzag_encode(1), 2);
        assert_eq!(Varint::zigzag_encode(-2), 3);
    }

    #[test]
    fn test_tag_encoding() {
        let mut buf = Vec::new();
        Field::encode_tag(15, WireType::LengthDelimited, &mut buf);

        let (field_num, wire_type, _) = Field::decode_tag(&buf).unwrap();
        assert_eq!(field_num, 15);
        assert_eq!(wire_type, WireType::LengthDelimited);
    }

    #[test]
    fn test_skip_unknown_field() {
        let mut buf = Vec::new();

        // Encode a tag and a varint value
        Field::encode_tag(99, WireType::Varint, &mut buf);
        Varint::encode(12345, &mut buf);

        // Let's say we read the tag
        let (_, wire_type, read) = Field::decode_tag(&buf).unwrap();

        // Now skip the value
        let skip_len = Field::skip(wire_type, &buf[read..]).unwrap();

        assert_eq!(read + skip_len, buf.len());
    }

    // A dummy message for testing the trait concept
    #[derive(Default, PartialEq, Debug)]
    struct Person {
        id: u64, // Field 1
    }

    impl Message for Person {
        fn encode(&self, buf: &mut Vec<u8>) {
            if self.id != 0 {
                Field::encode_tag(1, WireType::Varint, buf);
                Varint::encode(self.id, buf);
            }
        }

        fn decode(buf: &[u8]) -> Result<(Self, usize), &'static str> {
            let mut person = Person::default();
            let mut offset = 0;

            while offset < buf.len() {
                let (field_number, wire_type, read) = Field::decode_tag(&buf[offset..])?;
                offset += read;

                match field_number {
                    1 => {
                        if wire_type != WireType::Varint {
                            return Err("Type mismatch");
                        }
                        let (val, read) = Varint::decode(&buf[offset..])?;
                        person.id = val;
                        offset += read;
                    }
                    _ => {
                        // Unknown field, skip it
                        let read = Field::skip(wire_type, &buf[offset..])?;
                        offset += read;
                    }
                }
            }

            Ok((person, offset))
        }

        fn encoded_len(&self) -> usize {
            if self.id == 0 {
                0
            } else {
                1 + Varint::encoded_len(self.id)
            }
        }
    }

    #[test]
    fn test_message_trait() {
        let person = Person { id: 1000 };
        let mut buf = Vec::new();
        person.encode(&mut buf);

        assert_eq!(buf.len(), person.encoded_len());

        let (decoded, read) = Person::decode(&buf).unwrap();
        assert_eq!(decoded, person);
        assert_eq!(read, buf.len());
    }
}

// Benchmarking Note:
// To benchmark `Protobuf`, use `Criterion.rs` to measure the speed of encoding and decoding.
// Example:
// ```rust
// pub fn criterion_benchmark(c: &mut Criterion) {
//     c.bench_function("protobuf_varint_encode", |b| b.iter(|| {
//         let mut buf = Vec::new();
//         Varint::encode(std::hint::black_box(16384), std::hint::black_box(&mut buf));
//     }));
// }
// ```
