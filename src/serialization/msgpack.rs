//! `MessagePack` Serialization Format
//!
//! What this implements and what crate(s) it replaces:
//! This implements a robust encoder and decoder for the `MessagePack` binary serialization format.
//! It replaces canonical crates like `rmp` and `rmp-serde` to demonstrate how self-describing
//! binary formats achieve minimal overhead compared to JSON while retaining dynamic typing.
//!
//! Real-world systems that use this:
//! - Redis (as an alternative to raw JSON for faster internal parsing and caching)
//! - Fluentd (log data streaming)
//! - Various RPC frameworks (Neovim uses MsgPack-RPC)
//!
//! Why build it yourself?
//! Implementing `MessagePack` teaches you byte-level protocol design, masking/bit-shifting for
//! space optimization (like storing a tiny integer and its type tag in a single byte), and how
//! zero-copy deserialization works in Rust by borrowing from the input buffer.
//!
//! Architecture
//! ------------
//!
//! `MessagePack` packs types tightly. A single byte often contains both the type information
//! (the "tag") and the value itself if it's small enough.
//!
//! Example Layouts:
//! - `FixInt` (0..127): Tag is `0xxxxxxx`. The byte *is* the value.
//! - `FixStr` (length up to 31): Tag is `101xxxxx`. The 5 bits are the length, followed by utf-8 bytes.
//! - `Uint8`: Tag is `0xcc`, followed by 1 byte value.
//!
//! Invariants:
//! - Parsed sizes (for strings, arrays, maps) must not exceed the remaining buffer length.
//! - String deserialization must yield valid UTF-8.
//! - The encoder must always choose the most compact representation possible for integers.
//!
//! Complexity:
//! - Encoding/Decoding Time: O(N) where N is the size of the data structure.
//! - Space: O(N) for encoding. O(1) extra space for decoding if zero-copy (yielding `&str`).
//!
//! Design Decisions:
//! - We use an explicit `Value` enum for the AST representation, allowing completely dynamic types.
//! - We borrow strings (`&'a str`) from the input byte slice to demonstrate zero-copy deserialization.
//! - We implement `std::io::Write` for encoding to support streams, but decoding operates on `&[u8]` for zero-copy.

// Byte/word truncation and reinterpretation are intentional in this serialization code.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap
)]

use std::convert::TryInto;
use std::io::{self, Write};
use std::str;

/// Represents a dynamically typed `MessagePack` value.
/// The `&'a str` lifetime binds the string values to the original byte slice.
#[derive(Debug, Clone, PartialEq)]
pub enum Value<'a> {
    Nil,
    Bool(bool),
    /// Unsigned integers up to 64-bit
    Integer(u64),
    /// Signed integers up to 64-bit
    NegativeInteger(i64),
    /// Floating point numbers
    Float(f64),
    /// Borrowed UTF-8 string (zero-copy)
    String(&'a str),
    /// Array of values
    Array(Vec<Self>),
    /// Map of key-value pairs (using Vec to preserve order and simplify dynamic keys)
    Map(Vec<(Self, Self)>),
}

/// Errors that can occur during decoding.
#[derive(Debug, PartialEq, Eq)]
pub enum DecodeError {
    /// The buffer ended unexpectedly.
    UnexpectedEof,
    /// An unknown or unsupported type tag was encountered.
    InvalidMarker(u8),
    /// A string payload was not valid UTF-8.
    InvalidUtf8,
    /// An integer conversion failed.
    Overflow,
    /// The nesting depth of arrays/maps exceeded `MAX_DEPTH`.
    DepthLimitExceeded,
}

/// Maximum nesting depth for arrays/maps. Bounds recursion so that adversarial
/// deeply nested input (e.g. a long run of fixarray markers) returns a recoverable
/// error instead of aborting the process via stack overflow.
///
/// The `decode` stack frame is large (the marker match plus `Value` construction),
/// so the cap is kept well below the point where legitimate maximum-depth input
/// could itself exhaust a default (2 MiB) thread stack.
const MAX_DEPTH: usize = 128;

/// Encodes a `Value` into a writer using the most compact representation.
///
/// # Errors
///
/// Returns an [`io::Error`] if the writer fails or a string/array/map exceeds
/// the maximum length representable by the `MessagePack` format.
// The match covers every marker family, so the line count is inherent to the format.
#[allow(clippy::too_many_lines)]
pub fn encode<W: Write>(writer: &mut W, value: &Value) -> io::Result<()> {
    match value {
        Value::Nil => {
            writer.write_all(&[0xc0])?;
        }
        Value::Bool(false) => {
            writer.write_all(&[0xc2])?;
        }
        Value::Bool(true) => {
            writer.write_all(&[0xc3])?;
        }
        Value::Integer(val) => {
            // RUST INSIGHT: Match guards enable concise bounds checking for optimal encoding.
            // MessagePack requires us to use the smallest possible integer format.
            match *val {
                v if v <= 127 => writer.write_all(&[v as u8])?,
                v if u8::try_from(v).is_ok() => writer.write_all(&[0xcc, v as u8])?,
                v if u16::try_from(v).is_ok() => {
                    writer.write_all(&[0xcd])?;
                    writer.write_all(&(v as u16).to_be_bytes())?;
                }
                v if u32::try_from(v).is_ok() => {
                    writer.write_all(&[0xce])?;
                    writer.write_all(&(v as u32).to_be_bytes())?;
                }
                v => {
                    writer.write_all(&[0xcf])?;
                    writer.write_all(&v.to_be_bytes())?;
                }
            }
        }
        Value::NegativeInteger(val) => {
            match *val {
                v if v >= -32 => {
                    // Negative FixInt: 111xxxxx
                    writer.write_all(&[(v as i8) as u8])?;
                }
                v if v >= i64::from(i8::MIN) => {
                    writer.write_all(&[0xd0, (v as i8) as u8])?;
                }
                v if v >= i64::from(i16::MIN) => {
                    writer.write_all(&[0xd1])?;
                    writer.write_all(&(v as i16).to_be_bytes())?;
                }
                v if v >= i64::from(i32::MIN) => {
                    writer.write_all(&[0xd2])?;
                    writer.write_all(&(v as i32).to_be_bytes())?;
                }
                v => {
                    writer.write_all(&[0xd3])?;
                    writer.write_all(&v.to_be_bytes())?;
                }
            }
        }
        Value::Float(val) => {
            // Write as f64 (float 64)
            writer.write_all(&[0xcb])?;
            writer.write_all(&val.to_be_bytes())?;
        }
        Value::String(s) => {
            let len = s.len();
            if len <= 31 {
                // FixStr
                writer.write_all(&[0xa0 | (len as u8)])?;
            } else if u8::try_from(len).is_ok() {
                writer.write_all(&[0xd9, len as u8])?;
            } else if u16::try_from(len).is_ok() {
                writer.write_all(&[0xda])?;
                writer.write_all(&(len as u16).to_be_bytes())?;
            } else if u32::try_from(len).is_ok() {
                writer.write_all(&[0xdb])?;
                writer.write_all(&(len as u32).to_be_bytes())?;
            } else {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "String too long",
                ));
            }
            writer.write_all(s.as_bytes())?;
        }
        Value::Array(arr) => {
            let len = arr.len();
            if len <= 15 {
                writer.write_all(&[0x90 | (len as u8)])?;
            } else if u16::try_from(len).is_ok() {
                writer.write_all(&[0xdc])?;
                writer.write_all(&(len as u16).to_be_bytes())?;
            } else if u32::try_from(len).is_ok() {
                writer.write_all(&[0xdd])?;
                writer.write_all(&(len as u32).to_be_bytes())?;
            } else {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "Array too long",
                ));
            }
            for item in arr {
                encode(writer, item)?;
            }
        }
        Value::Map(map) => {
            let len = map.len();
            if len <= 15 {
                writer.write_all(&[0x80 | (len as u8)])?;
            } else if u16::try_from(len).is_ok() {
                writer.write_all(&[0xde])?;
                writer.write_all(&(len as u16).to_be_bytes())?;
            } else if u32::try_from(len).is_ok() {
                writer.write_all(&[0xdf])?;
                writer.write_all(&(len as u32).to_be_bytes())?;
            } else {
                return Err(io::Error::new(io::ErrorKind::InvalidInput, "Map too long"));
            }
            for (k, v) in map {
                encode(writer, k)?;
                encode(writer, v)?;
            }
        }
    }
    Ok(())
}

/// Decodes a `MessagePack` value from a byte slice.
/// Returns a tuple of the parsed `Value` and the remaining unparsed bytes.
///
/// # Errors
///
/// Returns a [`DecodeError`] if the buffer ends early, contains an invalid
/// marker or non-UTF-8 string, or exceeds the maximum nesting depth.
pub fn decode(input: &[u8]) -> Result<(Value<'_>, &[u8]), DecodeError> {
    decode_depth(input, 0)
}

/// Internal decode that tracks the current nesting depth to bound recursion.
// One match arm per marker family; the length is inherent to the wire format.
#[allow(clippy::too_many_lines)]
fn decode_depth(input: &[u8], depth: usize) -> Result<(Value<'_>, &[u8]), DecodeError> {
    if depth > MAX_DEPTH {
        return Err(DecodeError::DepthLimitExceeded);
    }
    if input.is_empty() {
        return Err(DecodeError::UnexpectedEof);
    }

    let marker = input[0];
    let mut rest = &input[1..];

    // Helper macro to read an exact number of bytes.
    macro_rules! read_bytes {
        ($len:expr) => {{
            if rest.len() < $len {
                return Err(DecodeError::UnexpectedEof);
            }
            let (bytes, new_rest) = rest.split_at($len);
            rest = new_rest;
            bytes
        }};
    }

    match marker {
        // Positive FixInt: 0xxxxxxx
        0x00..=0x7f => Ok((Value::Integer(u64::from(marker)), rest)),

        // Negative FixInt: 111xxxxx
        0xe0..=0xff => Ok((Value::NegativeInteger(i64::from(marker as i8)), rest)),

        // FixMap: 1000xxxx
        0x80..=0x8f => {
            let len = (marker & 0x0f) as usize;
            let (map, rest) = decode_map(len, rest, depth)?;
            Ok((Value::Map(map), rest))
        }

        // FixArray: 1001xxxx
        0x90..=0x9f => {
            let len = (marker & 0x0f) as usize;
            let (arr, rest) = decode_array(len, rest, depth)?;
            Ok((Value::Array(arr), rest))
        }

        // FixStr: 101xxxxx
        0xa0..=0xbf => {
            let len = (marker & 0x1f) as usize;
            let bytes = read_bytes!(len);
            let s = str::from_utf8(bytes).map_err(|_| DecodeError::InvalidUtf8)?;
            Ok((Value::String(s), rest))
        }

        0xc0 => Ok((Value::Nil, rest)),
        0xc2 => Ok((Value::Bool(false), rest)),
        0xc3 => Ok((Value::Bool(true), rest)),

        // Uint 8
        0xcc => {
            let val = u64::from(read_bytes!(1)[0]);
            Ok((Value::Integer(val), rest))
        }
        // Uint 16
        0xcd => {
            let bytes = read_bytes!(2).try_into().unwrap();
            Ok((Value::Integer(u64::from(u16::from_be_bytes(bytes))), rest))
        }
        // Uint 32
        0xce => {
            let bytes = read_bytes!(4).try_into().unwrap();
            Ok((Value::Integer(u64::from(u32::from_be_bytes(bytes))), rest))
        }
        // Uint 64
        0xcf => {
            let bytes = read_bytes!(8).try_into().unwrap();
            Ok((Value::Integer(u64::from_be_bytes(bytes)), rest))
        }

        // Int 8
        0xd0 => {
            let val = i64::from(read_bytes!(1)[0] as i8);
            Ok((Value::NegativeInteger(val), rest))
        }
        // Int 16
        0xd1 => {
            let bytes = read_bytes!(2).try_into().unwrap();
            Ok((
                Value::NegativeInteger(i64::from(i16::from_be_bytes(bytes))),
                rest,
            ))
        }
        // Int 32
        0xd2 => {
            let bytes = read_bytes!(4).try_into().unwrap();
            Ok((
                Value::NegativeInteger(i64::from(i32::from_be_bytes(bytes))),
                rest,
            ))
        }
        // Int 64
        0xd3 => {
            let bytes = read_bytes!(8).try_into().unwrap();
            Ok((Value::NegativeInteger(i64::from_be_bytes(bytes)), rest))
        }

        // Float 32 (we promote to f64 for simplicity in AST)
        0xca => {
            let bytes = read_bytes!(4).try_into().unwrap();
            Ok((Value::Float(f64::from(f32::from_be_bytes(bytes))), rest))
        }
        // Float 64
        0xcb => {
            let bytes = read_bytes!(8).try_into().unwrap();
            Ok((Value::Float(f64::from_be_bytes(bytes)), rest))
        }

        // Str 8
        0xd9 => {
            let len = read_bytes!(1)[0] as usize;
            let bytes = read_bytes!(len);
            let s = str::from_utf8(bytes).map_err(|_| DecodeError::InvalidUtf8)?;
            Ok((Value::String(s), rest))
        }
        // Str 16
        0xda => {
            let len_bytes = read_bytes!(2).try_into().unwrap();
            let len = u16::from_be_bytes(len_bytes) as usize;
            let bytes = read_bytes!(len);
            let s = str::from_utf8(bytes).map_err(|_| DecodeError::InvalidUtf8)?;
            Ok((Value::String(s), rest))
        }
        // Str 32
        0xdb => {
            let len_bytes = read_bytes!(4).try_into().unwrap();
            let len = u32::from_be_bytes(len_bytes) as usize;
            let bytes = read_bytes!(len);
            let s = str::from_utf8(bytes).map_err(|_| DecodeError::InvalidUtf8)?;
            Ok((Value::String(s), rest))
        }

        // Array 16
        0xdc => {
            let len_bytes = read_bytes!(2).try_into().unwrap();
            let len = u16::from_be_bytes(len_bytes) as usize;
            let (arr, rest) = decode_array(len, rest, depth)?;
            Ok((Value::Array(arr), rest))
        }
        // Array 32
        0xdd => {
            let len_bytes = read_bytes!(4).try_into().unwrap();
            let len = u32::from_be_bytes(len_bytes) as usize;
            let (arr, rest) = decode_array(len, rest, depth)?;
            Ok((Value::Array(arr), rest))
        }

        // Map 16
        0xde => {
            let len_bytes = read_bytes!(2).try_into().unwrap();
            let len = u16::from_be_bytes(len_bytes) as usize;
            let (map, rest) = decode_map(len, rest, depth)?;
            Ok((Value::Map(map), rest))
        }
        // Map 32
        0xdf => {
            let len_bytes = read_bytes!(4).try_into().unwrap();
            let len = u32::from_be_bytes(len_bytes) as usize;
            let (map, rest) = decode_map(len, rest, depth)?;
            Ok((Value::Map(map), rest))
        }

        _ => Err(DecodeError::InvalidMarker(marker)),
    }
}

fn decode_array(
    len: usize,
    mut input: &[u8],
    depth: usize,
) -> Result<(Vec<Value<'_>>, &[u8]), DecodeError> {
    // GOTCHA: Do not pre-allocate using `Vec::with_capacity(len)` blindly!
    // A malicious payload could specify a len of `u32::MAX` with a 5-byte file, causing an OOM panic.
    // Production parsers bound this or allocate incrementally.
    let cap = std::cmp::min(len, 1024);
    let mut arr = Vec::with_capacity(cap);
    for _ in 0..len {
        let (val, rest) = decode_depth(input, depth + 1)?;
        arr.push(val);
        input = rest;
    }
    Ok((arr, input))
}

/// Key/value entries produced by decoding a `MessagePack` map.
type MapEntries<'a> = Vec<(Value<'a>, Value<'a>)>;

fn decode_map(
    len: usize,
    mut input: &[u8],
    depth: usize,
) -> Result<(MapEntries<'_>, &[u8]), DecodeError> {
    let cap = std::cmp::min(len, 1024);
    let mut map = Vec::with_capacity(cap);
    for _ in 0..len {
        let (k, rest1) = decode_depth(input, depth + 1)?;
        let (v, rest2) = decode_depth(rest1, depth + 1)?;
        map.push((k, v));
        input = rest2;
    }
    Ok((map, input))
}

// Canonical comparisons:
// - rmp: The foundational crate. It defines the raw format and implements `Read`/`Write` based serialization.
// - rmp-serde: Hooks into Serde's data model. Instead of parsing into an AST like `Value`, it drives the `Deserializer`
//   visitor trait, avoiding the intermediate `Vec` allocations we have here in `Value::Array` entirely.
//
// Missing vs Production:
// - Serde Support: This is a standalone AST. Production usage heavily relies on `#[derive(Serialize, Deserialize)]`.
// - Binary Type: MessagePack has a distinct "Bin" format for byte arrays (similar to strings but lacking utf-8 validation). We omit it for simplicity.
// - Extension Types: The spec allows custom type identifiers (FixExt).

#[cfg(test)]
mod tests {
    use super::*;

    // test helper: takes owned values so call sites can pass constructed `Value`s directly.
    #[allow(clippy::needless_pass_by_value)]
    fn assert_roundtrip(value: Value<'_>) {
        let mut buf = Vec::new();
        encode(&mut buf, &value).unwrap();
        let (decoded, rest) = decode(&buf).unwrap();
        assert_eq!(value, decoded);
        assert!(rest.is_empty());
    }

    #[test]
    fn test_primitives() {
        assert_roundtrip(Value::Nil);
        assert_roundtrip(Value::Bool(true));
        assert_roundtrip(Value::Bool(false));
    }

    #[test]
    fn test_integers() {
        assert_roundtrip(Value::Integer(0)); // FixInt
        assert_roundtrip(Value::Integer(127)); // FixInt
        assert_roundtrip(Value::Integer(200)); // Uint8
        assert_roundtrip(Value::Integer(60000)); // Uint16
        assert_roundtrip(Value::Integer(u64::from(u32::MAX))); // Uint32

        assert_roundtrip(Value::NegativeInteger(-1)); // FixInt
        assert_roundtrip(Value::NegativeInteger(-32)); // FixInt
        assert_roundtrip(Value::NegativeInteger(-100)); // Int8
        assert_roundtrip(Value::NegativeInteger(-30000)); // Int16
    }

    #[test]
    fn test_strings() {
        assert_roundtrip(Value::String(""));
        assert_roundtrip(Value::String("hello")); // FixStr

        let s200 = "a".repeat(200);
        assert_roundtrip(Value::String(&s200)); // Str8

        let s40000 = "a".repeat(40000);
        assert_roundtrip(Value::String(&s40000)); // Str16
    }

    #[test]
    fn test_deeply_nested_returns_err_no_crash() {
        // Regression: a long run of fixarray-of-1 markers (0x91) previously recursed
        // one level per byte with no depth guard, overflowing the stack. Now it must
        // return a recoverable error.
        let deep = vec![0x91u8; 100_000];
        assert_eq!(decode(&deep), Err(DecodeError::DepthLimitExceeded));
    }

    #[test]
    fn test_array_and_map() {
        let arr = Value::Array(vec![
            Value::Integer(1),
            Value::String("two"),
            Value::Bool(false),
        ]);
        assert_roundtrip(arr);

        let map = Value::Map(vec![
            (Value::String("key1"), Value::Integer(42)),
            (Value::String("key2"), Value::Array(vec![Value::Nil])),
        ]);
        assert_roundtrip(map);
    }
}
