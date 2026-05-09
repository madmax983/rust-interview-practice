//! # MessagePack Serialization Implementation
//!
//! Implements a minimal MessagePack serializer and deserializer for binary data exchange.
//!
//! **Replaces Crates:** `rmp`, `rmp-serde`
//!
//! **Real-world Usage:**
//! - High-performance RPC frameworks where JSON parsing overhead is too high.
//! - Caching layers (e.g., storing structured data in Redis).
//! - IoT telemetry where network bandwidth and memory are constrained.
//!
//! **Why build it yourself?**
//! Building a MessagePack implementation teaches you how self-describing binary formats work. You'll learn
//! how type tags and compact integer representations save space compared to JSON, while still retaining
//! dynamic typing (unlike Protobuf or Cap'n Proto). It also provides practice with byte-level manipulation
//! and traits for serialization/deserialization.

// =========================================================================================
// Architecture
// =========================================================================================
//
// Data Structure Diagram:
//
//   JSON:  {"id": 42, "name": "foo"} -> 25 bytes
//
//   MsgPack:
//   ┌──────┬──────┬──────┬──────┬──────┬──────┬──────┬──────┬──────┬──────┐
//   │ 0x82 │ 0xa2 │ 'i'  │ 'd'  │ 0x2a │ 0xa4 │ 'n'  │ 'a'  │ 'm'  │ 'e'  │
//   └──────┴──────┴──────┴──────┴──────┴──────┴──────┴──────┴──────┴──────┘
//   │      │      │             │      │      │                           │
//   │      │      │             │      │      │                           │
//   ▼      ▼      ▼             ▼      ▼      ▼                           ▼
//  map(2) str(2) "id"          42     str(4) "name"
//
//   -> 13 bytes (nearly 50% smaller)
//
// Format Specifications (Simplified subset):
// - Positive FixInt: 0x00 - 0x7f
// - FixMap: 0x80 - 0x8f
// - FixArray: 0x90 - 0x9f
// - FixStr: 0xa0 - 0xbf
// - Nil: 0xc0
// - False: 0xc2
// - True: 0xc3
//
// Invariants:
// 1. Serialization must produce valid MessagePack byte sequences according to the spec.
// 2. Deserialization must consume exactly the number of bytes required for the parsed type.
// 3. String encodings must be valid UTF-8.
//
// Complexity:
// ┌────────────────┬──────────┬──────────┐
// │ Operation      │ Time     │ Space    │
// ├────────────────┼──────────┼──────────┤
// │ Serialize      │ O(N)*    │ O(N)     │
// │ Deserialize    │ O(N)*    │ O(N)     │
// └────────────────┴──────────┴──────────┘
// * N is the size of the data structure.
//
// Design Decisions & Tradeoffs:
// - We implement a simplified subset of MessagePack focusing on FixInt, FixStr, FixArray, and FixMap.
// - We use custom `Serializer` and `Deserializer` traits instead of implementing `serde` directly to focus
//   on the core formatting logic without `serde`'s macro complexity.

use std::collections::HashMap;

#[derive(Debug, PartialEq)]
pub enum MsgPackError {
    UnexpectedEof,
    InvalidMarker(u8),
    InvalidUtf8,
    NotImplemented,
}

pub trait Serializer {
    fn serialize<W: std::io::Write>(&self, writer: &mut W) -> std::io::Result<()>;
}

pub trait Deserializer: Sized {
    fn deserialize(bytes: &[u8]) -> Result<(Self, usize), MsgPackError>;
}

#[derive(Debug, PartialEq, Clone)]
pub enum Value {
    Nil,
    Boolean(bool),
    Integer(u64), // Using u64 for simplicity; a full spec needs i64/f64 etc.
    String(String),
    Array(Vec<Value>),
    Map(HashMap<String, Value>), // Keys are strings for simplicity
}

impl Serializer for Value {
    fn serialize<W: std::io::Write>(&self, writer: &mut W) -> std::io::Result<()> {
        match self {
            Value::Nil => {
                writer.write_all(&[0xc0])?;
            }
            Value::Boolean(b) => {
                let marker = if *b { 0xc3 } else { 0xc2 };
                writer.write_all(&[marker])?;
            }
            Value::Integer(i) => {
                if *i <= 0x7f {
                    // positive fixint
                    writer.write_all(&[*i as u8])?;
                } else {
                    // RUST INSIGHT: A complete implementation would handle u8, u16, u32, u64 boundaries.
                    // For brevity, we just implement FixInt and then jump to 64-bit for larger numbers
                    // just to show the structure, though a real crate optimizes this heavily.
                    writer.write_all(&[0xcf])?; // uint 64
                    writer.write_all(&i.to_be_bytes())?;
                }
            }
            Value::String(s) => {
                let len = s.len();
                if len <= 31 {
                    // fixstr
                    let marker = 0xa0 | (len as u8);
                    writer.write_all(&[marker])?;
                } else {
                    // str 32
                    writer.write_all(&[0xdb])?;
                    writer.write_all(&(len as u32).to_be_bytes())?;
                }
                writer.write_all(s.as_bytes())?;
            }
            Value::Array(arr) => {
                let len = arr.len();
                if len <= 15 {
                    // fixarray
                    let marker = 0x90 | (len as u8);
                    writer.write_all(&[marker])?;
                } else {
                    // array 32
                    writer.write_all(&[0xdd])?;
                    writer.write_all(&(len as u32).to_be_bytes())?;
                }
                for item in arr {
                    item.serialize(writer)?;
                }
            }
            Value::Map(map) => {
                let len = map.len();
                if len <= 15 {
                    // fixmap
                    let marker = 0x80 | (len as u8);
                    writer.write_all(&[marker])?;
                } else {
                    // map 32
                    writer.write_all(&[0xdf])?;
                    writer.write_all(&(len as u32).to_be_bytes())?;
                }
                // Note: We don't sort keys here, so binary representation of maps is non-deterministic
                for (k, v) in map {
                    Value::String(k.clone()).serialize(writer)?;
                    v.serialize(writer)?;
                }
            }
        }
        Ok(())
    }
}

impl Deserializer for Value {
    fn deserialize(bytes: &[u8]) -> Result<(Self, usize), MsgPackError> {
        if bytes.is_empty() {
            return Err(MsgPackError::UnexpectedEof);
        }

        let marker = bytes[0];
        match marker {
            0xc0 => Ok((Value::Nil, 1)),
            0xc2 => Ok((Value::Boolean(false), 1)),
            0xc3 => Ok((Value::Boolean(true), 1)),
            // positive fixint
            0x00..=0x7f => Ok((Value::Integer(marker as u64), 1)),
            // uint 64
            0xcf => {
                if bytes.len() < 9 {
                    return Err(MsgPackError::UnexpectedEof);
                }
                let mut arr = [0u8; 8];
                arr.copy_from_slice(&bytes[1..9]);
                Ok((Value::Integer(u64::from_be_bytes(arr)), 9))
            }
            // fixstr
            0xa0..=0xbf => {
                let len = (marker & 0x1f) as usize;
                if bytes.len() < 1 + len {
                    return Err(MsgPackError::UnexpectedEof);
                }
                let s = std::str::from_utf8(&bytes[1..1 + len])
                    .map_err(|_| MsgPackError::InvalidUtf8)?
                    .to_string();
                Ok((Value::String(s), 1 + len))
            }
            // fixarray
            0x90..=0x9f => {
                let len = (marker & 0x0f) as usize;
                let mut arr = Vec::with_capacity(len);
                let mut offset = 1;
                for _ in 0..len {
                    let (val, bytes_read) = Value::deserialize(&bytes[offset..])?;
                    arr.push(val);
                    offset += bytes_read;
                }
                Ok((Value::Array(arr), offset))
            }
            // fixmap
            0x80..=0x8f => {
                let len = (marker & 0x0f) as usize;
                let mut map = HashMap::with_capacity(len);
                let mut offset = 1;
                for _ in 0..len {
                    let (key_val, k_bytes_read) = Value::deserialize(&bytes[offset..])?;
                    offset += k_bytes_read;

                    let key_str = match key_val {
                        Value::String(s) => s,
                        _ => return Err(MsgPackError::NotImplemented), // Simplification: string keys only
                    };

                    let (val, v_bytes_read) = Value::deserialize(&bytes[offset..])?;
                    offset += v_bytes_read;

                    map.insert(key_str, val);
                }
                Ok((Value::Map(map), offset))
            }
            _ => Err(MsgPackError::InvalidMarker(marker)),
        }
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Canonical Comparisons:
// - `rmp` is the standard low-level MessagePack implementation in Rust, handling all edge cases,
//   number representations (i8-i64, u8-u64, f32/f64), and extensions.
// - `rmp-serde` binds `rmp` to the Serde framework, allowing transparent serialization of Rust structs.
//
// Missing Features:
// - Full number support (signed integers, floats).
// - Various sized string, array, and map encodings (e.g., str 8, str 16).
// - Binary data (`bin 8`, `bin 16`, `bin 32`).
// - Extension types (`ext`).
// - Serde integration.
//
// Next Steps:
// - Implement the remaining integer types and floating point formats.
// - Add `Serde` traits (`Serialize`, `Deserialize`) to allow use with standard Rust structs.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serialize_deserialize_nil() {
        let val = Value::Nil;
        let mut buf = Vec::new();
        val.serialize(&mut buf).unwrap();
        assert_eq!(buf, vec![0xc0]);

        let (deserialized, bytes_read) = Value::deserialize(&buf).unwrap();
        assert_eq!(deserialized, Value::Nil);
        assert_eq!(bytes_read, 1);
    }

    #[test]
    fn test_serialize_deserialize_boolean() {
        let val = Value::Boolean(true);
        let mut buf = Vec::new();
        val.serialize(&mut buf).unwrap();
        assert_eq!(buf, vec![0xc3]);

        let (deserialized, _) = Value::deserialize(&buf).unwrap();
        assert_eq!(deserialized, Value::Boolean(true));
    }

    #[test]
    fn test_serialize_deserialize_fixint() {
        let val = Value::Integer(42);
        let mut buf = Vec::new();
        val.serialize(&mut buf).unwrap();
        assert_eq!(buf, vec![42]);

        let (deserialized, _) = Value::deserialize(&buf).unwrap();
        assert_eq!(deserialized, Value::Integer(42));
    }

    #[test]
    fn test_serialize_deserialize_fixstr() {
        let val = Value::String("hello".to_string());
        let mut buf = Vec::new();
        val.serialize(&mut buf).unwrap();

        let mut expected = vec![0xa0 | 5]; // fixstr of length 5
        expected.extend_from_slice(b"hello");
        assert_eq!(buf, expected);

        let (deserialized, _) = Value::deserialize(&buf).unwrap();
        assert_eq!(deserialized, Value::String("hello".to_string()));
    }

    #[test]
    fn test_serialize_deserialize_fixarray() {
        let val = Value::Array(vec![
            Value::Integer(1),
            Value::Integer(2),
            Value::Integer(3),
        ]);
        let mut buf = Vec::new();
        val.serialize(&mut buf).unwrap();

        assert_eq!(buf, vec![0x90 | 3, 1, 2, 3]);

        let (deserialized, _) = Value::deserialize(&buf).unwrap();
        assert_eq!(deserialized, val);
    }

    // Benchmarking Note:
    // To benchmark this implementation against `rmp`, one would use the `criterion` crate:
    // ```rust
    // pub fn bench_serialize(c: &mut Criterion) {
    //     let val = Value::Map(HashMap::new()); // Build a complex value
    //     c.bench_function("msgpack serialize", |b| b.iter(|| {
    //         let mut buf = Vec::new();
    //         val.serialize(&mut buf).unwrap();
    //         std::hint::black_box(buf);
    //     }));
    // }
    // ```

    #[test]
    fn test_serialize_deserialize_fixmap() {
        let mut map = HashMap::new();
        map.insert("a".to_string(), Value::Integer(1));

        let val = Value::Map(map);
        let mut buf = Vec::new();
        val.serialize(&mut buf).unwrap();

        let (deserialized, _) = Value::deserialize(&buf).unwrap();
        assert_eq!(deserialized, val);
    }
}
