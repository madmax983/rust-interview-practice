//! # MessagePack Serialization
//!
//! ## Purpose
//! Implements a self-describing binary serialization format (MessagePack) from scratch.
//!
//! ## Replaces
//! - `rmp`
//! - `rmp-serde`
//!
//! ## Real-world Usage
//! - Used in Redis for scripting.
//! - Used in RPC protocols (like Msgpack-RPC) where JSON parsing overhead is too high.
//! - Ideal for IoT or low-bandwidth environments where binary compactness is necessary
//!   but dynamic typing is still required.
//!
//! ## Why build it yourself?
//! Understanding MessagePack forces you to deal with bit-level encoding schemes,
//! recognizing how length-prefixed arrays and variable-width integers drastically
//! reduce payload sizes compared to plain text formats.
//!
//! ## Architecture
//!
//! MessagePack encodes values as a type tag followed by data.
//! For many small values, the tag and data are packed into a single byte.
//!
//! ### Invariants
//! 1. Integers should be packed into the smallest possible representation.
//! 2. Strings and Arrays must correctly encode their length before their elements.
//!
//! ### Complexity
//! - **Time:** O(N) where N is the size of the data structure.
//! - **Space:** O(N) to build the output buffer.

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Nil,
    Bool(bool),
    Integer(i64),
    Float(f64),
    String(String),
    Array(Vec<Value>),
    Map(Vec<(Value, Value)>),
}

#[derive(Debug)]
pub enum EncodeError {
    NotImplemented,
}

#[derive(Debug)]
pub enum DecodeError {
    UnexpectedEof,
    InvalidMarker(u8),
    NotImplemented,
    InvalidString,
}

/// A trait for types that can be serialized to a byte vector.
pub trait Serializer {
    /// Serializes the value into a byte vector.
    ///
    /// # Errors
    /// Returns an `EncodeError` if serialization fails.
    fn to_bytes(&self) -> Result<Vec<u8>, EncodeError>;
}

/// A trait for types that can be deserialized from a byte slice.
pub trait Deserializer: Sized {
    /// Deserializes a value from a byte slice.
    /// Returns the deserialized value and the number of bytes read.
    ///
    /// # Errors
    /// Returns a `DecodeError` if deserialization fails.
    fn from_bytes(bytes: &[u8]) -> Result<(Self, usize), DecodeError>;
}

impl Serializer for Value {
    fn to_bytes(&self) -> Result<Vec<u8>, EncodeError> {
        let mut buf = Vec::new();
        self.encode_to(&mut buf)?;
        Ok(buf)
    }
}

impl Value {
    fn encode_to(&self, buf: &mut Vec<u8>) -> Result<(), EncodeError> {
        match self {
            Value::Nil => buf.push(0xc0),
            Value::Bool(false) => buf.push(0xc2),
            Value::Bool(true) => buf.push(0xc3),
            Value::Integer(n) => {
                let val = *n;
                if (0..=127).contains(&val) {
                    // positive fixint
                    buf.push(val as u8);
                } else if (-32..=0).contains(&val) {
                    // negative fixint
                    // RUST INSIGHT:
                    // Using bitwise casting `as u8` on a negative `i64` safely truncates
                    // to the two's complement representation in 8 bits.
                    buf.push(val as u8);
                } else if (i8::MIN as i64..=i8::MAX as i64).contains(&val) {
                    buf.push(0xd0);
                    buf.push(val as i8 as u8);
                } else if (i16::MIN as i64..=i16::MAX as i64).contains(&val) {
                    buf.push(0xd1);
                    buf.extend_from_slice(&(val as i16).to_be_bytes());
                } else if (i32::MIN as i64..=i32::MAX as i64).contains(&val) {
                    buf.push(0xd2);
                    buf.extend_from_slice(&(val as i32).to_be_bytes());
                } else {
                    buf.push(0xd3);
                    buf.extend_from_slice(&val.to_be_bytes());
                }
            }
            Value::Float(f) => {
                buf.push(0xcb);
                buf.extend_from_slice(&f.to_be_bytes());
            }
            Value::String(s) => {
                let bytes = s.as_bytes();
                let len = bytes.len();
                if len <= 31 {
                    // fixstr
                    buf.push(0xa0 | (len as u8));
                } else if len <= u8::MAX as usize {
                    buf.push(0xd9);
                    buf.push(len as u8);
                } else if len <= u16::MAX as usize {
                    buf.push(0xda);
                    buf.extend_from_slice(&(len as u16).to_be_bytes());
                } else if len <= u32::MAX as usize {
                    buf.push(0xdb);
                    buf.extend_from_slice(&(len as u32).to_be_bytes());
                } else {
                    return Err(EncodeError::NotImplemented);
                }
                buf.extend_from_slice(bytes);
            }
            Value::Array(arr) => {
                let len = arr.len();
                if len <= 15 {
                    // fixarray
                    buf.push(0x90 | (len as u8));
                } else if len <= u16::MAX as usize {
                    buf.push(0xdc);
                    buf.extend_from_slice(&(len as u16).to_be_bytes());
                } else if len <= u32::MAX as usize {
                    buf.push(0xdd);
                    buf.extend_from_slice(&(len as u32).to_be_bytes());
                } else {
                    return Err(EncodeError::NotImplemented);
                }
                for item in arr {
                    item.encode_to(buf)?;
                }
            }
            Value::Map(map) => {
                let len = map.len();
                if len <= 15 {
                    // fixmap
                    buf.push(0x80 | (len as u8));
                } else if len <= u16::MAX as usize {
                    buf.push(0xde);
                    buf.extend_from_slice(&(len as u16).to_be_bytes());
                } else if len <= u32::MAX as usize {
                    buf.push(0xdf);
                    buf.extend_from_slice(&(len as u32).to_be_bytes());
                } else {
                    return Err(EncodeError::NotImplemented);
                }
                for (k, v) in map {
                    k.encode_to(buf)?;
                    v.encode_to(buf)?;
                }
            }
        }
        Ok(())
    }

}

impl Deserializer for Value {
    fn from_bytes(bytes: &[u8]) -> Result<(Self, usize), DecodeError> {
        if bytes.is_empty() {
            return Err(DecodeError::UnexpectedEof);
        }

        let marker = bytes[0];
        let mut offset = 1;

        // RUST INSIGHT:
        // By structuring decoding logically via exhaustive or categorized matches,
        // we map directly from the MessagePack spec to Rust's control flow.
        if marker <= 0x7f {
            // positive fixint
            return Ok((Value::Integer(marker as i64), offset));
        }

        if marker >= 0xe0 {
            // negative fixint (11100000 to 11111111)
            // Need to cast to i8 to get the negative value, then to i64
            let val = (marker as i8) as i64;
            return Ok((Value::Integer(val), offset));
        }

        if (0xa0..=0xbf).contains(&marker) {
            // fixstr
            let len = (marker & 0x1f) as usize;
            if bytes.len() < offset + len {
                return Err(DecodeError::UnexpectedEof);
            }
            let s = String::from_utf8(bytes[offset..offset + len].to_vec())
                .map_err(|_| DecodeError::InvalidString)?;
            offset += len;
            return Ok((Value::String(s), offset));
        }

        if (0x90..=0x9f).contains(&marker) {
            // fixarray
            let len = (marker & 0x0f) as usize;
            let mut arr = Vec::with_capacity(len);
            for _ in 0..len {
                let (val, read) = Self::from_bytes(&bytes[offset..])?;
                arr.push(val);
                offset += read;
            }
            return Ok((Value::Array(arr), offset));
        }

        if (0x80..=0x8f).contains(&marker) {
            // fixmap
            let len = (marker & 0x0f) as usize;
            let mut map = Vec::with_capacity(len);
            for _ in 0..len {
                let (k, read_k) = Self::from_bytes(&bytes[offset..])?;
                offset += read_k;
                let (v, read_v) = Self::from_bytes(&bytes[offset..])?;
                offset += read_v;
                map.push((k, v));
            }
            return Ok((Value::Map(map), offset));
        }

        match marker {
            0xc0 => Ok((Value::Nil, offset)),
            0xc2 => Ok((Value::Bool(false), offset)),
            0xc3 => Ok((Value::Bool(true), offset)),
            0xd0 => {
                // int 8
                if bytes.len() < offset + 1 {
                    return Err(DecodeError::UnexpectedEof);
                }
                let val = bytes[offset] as i8 as i64;
                offset += 1;
                Ok((Value::Integer(val), offset))
            }
            0xd1 => {
                // int 16
                if bytes.len() < offset + 2 {
                    return Err(DecodeError::UnexpectedEof);
                }
                let val = i16::from_be_bytes([bytes[offset], bytes[offset + 1]]) as i64;
                offset += 2;
                Ok((Value::Integer(val), offset))
            }
            0xd2 => {
                // int 32
                if bytes.len() < offset + 4 {
                    return Err(DecodeError::UnexpectedEof);
                }
                let val = i32::from_be_bytes(bytes[offset..offset + 4].try_into().unwrap()) as i64;
                offset += 4;
                Ok((Value::Integer(val), offset))
            }
            0xd3 => {
                // int 64
                if bytes.len() < offset + 8 {
                    return Err(DecodeError::UnexpectedEof);
                }
                let val = i64::from_be_bytes(bytes[offset..offset + 8].try_into().unwrap());
                offset += 8;
                Ok((Value::Integer(val), offset))
            }
            0xcb => {
                // float 64
                if bytes.len() < offset + 8 {
                    return Err(DecodeError::UnexpectedEof);
                }
                let val = f64::from_be_bytes(bytes[offset..offset + 8].try_into().unwrap());
                offset += 8;
                Ok((Value::Float(val), offset))
            }
            0xd9 => {
                // str 8
                if bytes.len() < offset + 1 {
                    return Err(DecodeError::UnexpectedEof);
                }
                let len = bytes[offset] as usize;
                offset += 1;
                if bytes.len() < offset + len {
                    return Err(DecodeError::UnexpectedEof);
                }
                let s = String::from_utf8(bytes[offset..offset + len].to_vec())
                    .map_err(|_| DecodeError::InvalidString)?;
                offset += len;
                Ok((Value::String(s), offset))
            }
            // More complex sizes could be added here (str 16, str 32, array 16, array 32, etc.)
            _ => Err(DecodeError::InvalidMarker(marker)),
        }
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `rmp`: Full compliance with the MessagePack spec, highly optimized, avoids allocations
//   when decoding streams.
// - `rmp-serde`: Integrates `rmp` with `serde`, allowing Rust structs to seamlessly serialize
//   to MessagePack without manual enum mapping.
//
// Missing vs. Production:
// - **Unsigned Integers**: We map all integers to `i64` for simplicity. The spec supports unsigned.
// - **Binary Ext Types**: The MessagePack spec supports arbitrary binary blobs (`bin 8`, `bin 16`, etc.) and extensions.
// - **Reader/Writer Traits**: A real crate implements streaming over `std::io::Read` and `std::io::Write`.
//
// Next Steps:
// 1. Add support for `u64` via a `Value::UInteger(u64)` variant.
// 2. Implement streaming decoding over `std::io::Read`.
//
// Benchmarking:
// To benchmark serialization performance against `rmp` or `serde_json`,
// use the `criterion` crate:
// ```rust
// pub fn msgpack_benchmark(c: &mut Criterion) {
//     c.bench_function("msgpack_encode", |b| {
//         b.iter(|| {
//             let val = Value::String("hello world".to_string());
//             black_box(val.to_bytes().unwrap());
//         })
//     });
// }
// ```

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nil_bool() {
        let nil = Value::Nil;
        assert_eq!(nil.to_bytes().unwrap(), vec![0xc0]);
        assert_eq!(Value::from_bytes(&[0xc0]).unwrap().0, Value::Nil);

        let t = Value::Bool(true);
        assert_eq!(t.to_bytes().unwrap(), vec![0xc3]);
        assert_eq!(Value::from_bytes(&[0xc3]).unwrap().0, Value::Bool(true));
    }

    #[test]
    fn test_fixint() {
        let pos = Value::Integer(42);
        assert_eq!(pos.to_bytes().unwrap(), vec![42]);
        assert_eq!(Value::from_bytes(&[42]).unwrap().0, Value::Integer(42));

        let neg = Value::Integer(-15);
        assert_eq!(neg.to_bytes().unwrap(), vec![0xf1]); // 0xf1 is -15 as i8 cast to u8
        assert_eq!(Value::from_bytes(&[0xf1]).unwrap().0, Value::Integer(-15));
    }

    #[test]
    fn test_string() {
        let s = Value::String("hello".to_string());
        let encoded = s.to_bytes().unwrap();
        // 0xa5 is 0xa0 | 5
        assert_eq!(encoded, vec![0xa5, b'h', b'e', b'l', b'l', b'o']);
        assert_eq!(Value::from_bytes(&encoded).unwrap().0, s);
    }

    #[test]
    fn test_array() {
        let arr = Value::Array(vec![Value::Integer(1), Value::Integer(2)]);
        let encoded = arr.to_bytes().unwrap();
        // 0x92 is 0x90 | 2
        assert_eq!(encoded, vec![0x92, 1, 2]);
        assert_eq!(Value::from_bytes(&encoded).unwrap().0, arr);
    }

    #[test]
    fn test_map() {
        let map = Value::Map(vec![(
            Value::String("key".to_string()),
            Value::Integer(100),
        )]);
        let encoded = map.to_bytes().unwrap();
        assert_eq!(Value::from_bytes(&encoded).unwrap().0, map);
    }
}
