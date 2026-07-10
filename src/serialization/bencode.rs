//! # Bencode Serialization Framework
//!
//! Implements a parser and serializer for the Bencode format from scratch.
//!
//! **Replaces Crates:** `bendy`, `bencode`
//!
//! **Real-world Usage:**
//! - Core serialization format for the `BitTorrent` protocol.
//! - Used to encode `.torrent` files and peer-to-peer tracker messages.
//! - Used wherever deterministic encoding (lexicographical sorting of keys) is required for hashing.
//!
//! **Why build it yourself?**
//! Implementing a Bencode parser teaches you how to handle binary-safe strings mixed with ASCII markers.
//! Unlike JSON, Bencode strings are just raw bytes preceded by their length, allowing them to safely contain binary data (like SHA-1 hashes) without escaping.
//! It also emphasizes the importance of deterministic serialization: Bencode dictates that dictionary keys MUST be sorted lexicographically, allowing for exact cryptographic hashing of the encoded data.

use std::collections::BTreeMap;

// =========================================================================================
// Architecture
// =========================================================================================
//
// Data Structure Diagram:
//
//      BencodeValue (Enum)
//      ├── Integer(i64)                          -> e.g., i42e
//      ├── ByteString(Vec<u8>)                   -> e.g., 4:spam
//      ├── List(Vec<BencodeValue>)               -> e.g., l4:spami42ee
//      └── Dictionary(BTreeMap<String, BencodeValue>) -> e.g., d3:cow3:moo4:spam4:eggse
//
// Invariants:
// 1. Strings are length-prefixed base-10 strings followed by a colon and the raw bytes (e.g., `4:spam`).
// 2. Integers are enclosed in `i` and `e` (e.g., `i42e`, `i-42e`). Negative zero (`i-0e`) is invalid. Leading zeros (`i03e`) are invalid, except for zero itself (`i0e`).
// 3. Lists are enclosed in `l` and `e`.
// 4. Dictionaries are enclosed in `d` and `e`. Keys MUST be byte strings, and they MUST be sorted lexicographically by their raw byte values.
//
// Complexity:
// ┌───────────────────┬────────┬────────┐
// │ Operation         │ Time   │ Space  │
// ├───────────────────┼────────┼────────┤
// │ Encode            │ O(N)   │ O(N)   │
// │ Decode            │ O(N)   │ O(N)   │
// └───────────────────┴────────┴────────┘
// N is the length of the data / encoded string.
//
// Design Decisions:
// - **Dictionary Keys**: Bencode allows any byte string as a dictionary key, but in practice, they are almost always UTF-8 strings.
//   We enforce `String` for keys here for ergonomics, but a perfectly spec-compliant version would use `Vec<u8>` keys.
//   We use `BTreeMap` instead of `HashMap` to enforce the invariant that keys are strictly sorted lexicographically,
//   which is required by the BitTorrent spec for deterministic hashing (e.g., computing the Info Hash).
// - **Byte Strings vs UTF-8**: Bencode does not require strings to be valid UTF-8. They are just byte arrays. We represent them as `Vec<u8>`.

/// Represents a value in the Bencode format.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BencodeValue {
    /// An integer, typically i64 in `BitTorrent`.
    Integer(i64),
    /// A raw byte string (not necessarily UTF-8).
    ByteString(Vec<u8>),
    /// A list of Bencode values.
    List(Vec<Self>),
    /// A dictionary mapping strings to Bencode values, ordered lexicographically.
    Dictionary(BTreeMap<String, Self>),
}

/// A trait for types that can be serialized into Bencode format.
pub trait BencodeEncode {
    /// Encodes the value into a raw byte vector.
    fn bencode(&self) -> Vec<u8> {
        let mut buffer = Vec::new();
        self.bencode_into(&mut buffer);
        buffer
    }
    /// Recursively encodes the value into the provided buffer to avoid allocations.
    fn bencode_into(&self, buffer: &mut Vec<u8>);
}

/// A trait for types that can be deserialized from Bencode format.
pub trait BencodeDecode: Sized {
    /// Decodes a value from a raw byte slice, returning the value and remaining bytes.
    fn bdecode(bytes: &[u8]) -> Result<(Self, &[u8]), String>;
}

// Implement the traits for the generic BencodeValue DOM
impl BencodeEncode for BencodeValue {
    fn bencode_into(&self, buffer: &mut Vec<u8>) {
        self.encode_into(buffer);
    }
}

impl BencodeDecode for BencodeValue {
    fn bdecode(bytes: &[u8]) -> Result<(Self, &[u8]), String> {
        decode(bytes)
    }
}

// Example of implementing the traits for native Rust types directly
impl BencodeEncode for String {
    fn bencode_into(&self, buffer: &mut Vec<u8>) {
        use std::io::Write;
        write!(buffer, "{}:", self.len()).expect("Write failed");
        buffer.extend_from_slice(self.as_bytes());
    }
}

impl BencodeDecode for String {
    fn bdecode(bytes: &[u8]) -> Result<(Self, &[u8]), String> {
        let (val, remaining) = decode_byte_string(bytes)?;
        match val {
            BencodeValue::ByteString(b) => {
                let s = Self::from_utf8(b).map_err(|_| "Invalid UTF-8")?;
                Ok((s, remaining))
            }
            _ => Err("Expected byte string".to_string()),
        }
    }
}

impl BencodeValue {
    /// Encodes the `BencodeValue` into a raw byte vector.
    #[must_use] 
    pub fn encode(&self) -> Vec<u8> {
        let mut buffer = Vec::new();
        self.encode_into(&mut buffer);
        buffer
    }

    /// Recursively encodes the value into the provided buffer.
    ///
    /// # RUST INSIGHT:
    /// We use `&mut Vec<u8>` to avoid intermediate heap allocations during recursive encoding.
    /// This is a common zero-cost abstraction pattern in Rust serializers.
    fn encode_into(&self, buffer: &mut Vec<u8>) {
        match self {
            Self::Integer(i) => {
                // GOTCHA: It's tempting to use `format!("i{}e", i).into_bytes()`, but that allocates
                // a new String and Vec on the heap for every integer. Using `itoa` or `write!` into
                // the buffer is much more efficient.
                use std::io::Write;
                write!(buffer, "i{i}e").expect("Writing to Vec should never fail");
            }
            Self::ByteString(bytes) => {
                use std::io::Write;
                // Length prefix, colon, then raw bytes
                write!(buffer, "{}:", bytes.len()).expect("Writing to Vec should never fail");
                buffer.extend_from_slice(bytes);
            }
            Self::List(list) => {
                buffer.push(b'l');
                for item in list {
                    item.encode_into(buffer);
                }
                buffer.push(b'e');
            }
            Self::Dictionary(dict) => {
                buffer.push(b'd');
                // BTreeMap automatically iterates over keys in sorted order.
                for (key, value) in dict {
                    // Keys are just ByteStrings in Bencode
                    use std::io::Write;
                    write!(buffer, "{}:", key.len()).expect("Writing to Vec should never fail");
                    buffer.extend_from_slice(key.as_bytes());
                    value.encode_into(buffer);
                }
                buffer.push(b'e');
            }
        }
    }
}

/// Decodes a `BencodeValue` from a raw byte slice.
/// Returns the parsed value and the remaining unparsed bytes.
const MAX_DEPTH: usize = 512;

pub fn decode(bytes: &[u8]) -> Result<(BencodeValue, &[u8]), String> {
    decode_internal(bytes, 0)
}

fn decode_internal(bytes: &[u8], depth: usize) -> Result<(BencodeValue, &[u8]), String> {
    if depth > MAX_DEPTH {
        return Err("Max recursion depth exceeded".to_string());
    }

    if bytes.is_empty() {
        return Err("Unexpected EOF".to_string());
    }

    match bytes[0] {
        b'i' => decode_integer(bytes),
        b'l' => decode_list(bytes, depth),
        b'd' => decode_dictionary(bytes, depth),
        b'0'..=b'9' => decode_byte_string(bytes),
        _ => Err(format!("Invalid bencode prefix: {}", bytes[0] as char)),
    }
}

fn decode_integer(bytes: &[u8]) -> Result<(BencodeValue, &[u8]), String> {
    // Expected format: i<number>e
    let e_pos = bytes
        .iter()
        .position(|&b| b == b'e')
        .ok_or("Unterminated integer")?;

    // Extract the bytes between 'i' and 'e'
    let num_bytes = &bytes[1..e_pos];
    if num_bytes.is_empty() {
        return Err("Empty integer".to_string());
    }

    // Check for negative zero (i-0e)
    if num_bytes == b"-0" {
        return Err("Negative zero is not allowed".to_string());
    }

    // Check for leading zeros (e.g., i03e), but allow exactly i0e
    if num_bytes.len() > 1 && num_bytes[0] == b'0' {
        return Err("Leading zeros are not allowed".to_string());
    }
    if num_bytes.len() > 2 && num_bytes[0] == b'-' && num_bytes[1] == b'0' {
        return Err("Leading zeros are not allowed".to_string());
    }

    let num_str = std::str::from_utf8(num_bytes).map_err(|_| "Invalid UTF-8 in integer")?;
    let num = num_str
        .parse::<i64>()
        .map_err(|_| "Invalid integer format")?;

    Ok((BencodeValue::Integer(num), &bytes[e_pos + 1..]))
}

fn decode_byte_string(bytes: &[u8]) -> Result<(BencodeValue, &[u8]), String> {
    // Expected format: <length>:<raw bytes>
    let colon_pos = bytes
        .iter()
        .position(|&b| b == b':')
        .ok_or("Missing colon for byte string")?;

    let len_bytes = &bytes[..colon_pos];
    if len_bytes.is_empty() {
        return Err("Empty length prefix".to_string());
    }

    let len_str = std::str::from_utf8(len_bytes).map_err(|_| "Invalid UTF-8 in string length")?;
    let len = len_str
        .parse::<usize>()
        .map_err(|_| "Invalid string length format")?;

    let string_start = colon_pos + 1;
    let end_pos = string_start
        .checked_add(len)
        .ok_or("String length causes integer overflow")?;

    if bytes.len() < end_pos {
        return Err("String length exceeds available bytes".to_string());
    }

    let string_bytes = bytes[string_start..end_pos].to_vec();

    Ok((BencodeValue::ByteString(string_bytes), &bytes[end_pos..]))
}

fn decode_list(mut bytes: &[u8], depth: usize) -> Result<(BencodeValue, &[u8]), String> {
    // Skip 'l'
    bytes = &bytes[1..];

    let mut list = Vec::new();

    while !bytes.is_empty() && bytes[0] != b'e' {
        let (val, remaining) = decode_internal(bytes, depth + 1)?;
        list.push(val);
        bytes = remaining;
    }

    if bytes.is_empty() {
        return Err("Unterminated list".to_string());
    }

    // Skip 'e'
    Ok((BencodeValue::List(list), &bytes[1..]))
}

fn decode_dictionary(mut bytes: &[u8], depth: usize) -> Result<(BencodeValue, &[u8]), String> {
    // Skip 'd'
    bytes = &bytes[1..];

    let mut dict = BTreeMap::new();
    let mut last_key: Option<Vec<u8>> = None;

    while !bytes.is_empty() && bytes[0] != b'e' {
        // Keys must be byte strings
        let (key_val, remaining1) = decode_byte_string(bytes)?;
        let key_bytes = match key_val {
            BencodeValue::ByteString(b) => b,
            _ => return Err("Dictionary key must be a byte string".to_string()),
        };

        // Bencode spec: keys must be sorted lexicographically
        if let Some(ref last) = last_key
            && &key_bytes <= last {
                return Err("Dictionary keys must be strictly sorted lexicographically".to_string());
            }
        last_key = Some(key_bytes.clone());

        let key_str = String::from_utf8(key_bytes)
            .map_err(|_| "Dictionary keys must be valid UTF-8 for this implementation")?;

        let (val, remaining2) = decode_internal(remaining1, depth + 1)?;
        dict.insert(key_str, val);
        bytes = remaining2;
    }

    if bytes.is_empty() {
        return Err("Unterminated dictionary".to_string());
    }

    // Skip 'e'
    Ok((BencodeValue::Dictionary(dict), &bytes[1..]))
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `bendy`: A fast, robust bencode encoder/decoder. Uses `serde`.
// - `bencode`: Older, unmaintained crate.
//
// Missing vs. Production:
// - **Zero-copy decoding**: We allocate `Vec<u8>` for every ByteString. A production parser (like `serde` over `bendy`) could yield borrowed `&[u8]`.
// - **Serde Support**: It would be much more ergonomic if users could `#[derive(Serialize, Deserialize)]` and map directly to custom Rust structs instead of the DOM tree.
//
// Suggested Next Steps:
// 1. **Implement `serde` traits**: Write `Serializer` and `Deserializer` implementations for `serde` to allow direct struct mapping.
// 2. **Zero-copy strings**: Change `ByteString(Vec<u8>)` to `ByteString(Cow<'a, [u8]>)` to avoid allocation during decoding.
// 3. **Streaming Parser**: Implement a streaming decoder that does not require the entire message in memory at once.

// =========================================================================================
// Benchmarking Note
// =========================================================================================
// To benchmark this implementation against canonical crates:
// ```rust
// use criterion::{black_box, criterion_group, criterion_main, Criterion};
//
// fn bench_bencode(c: &mut Criterion) {
//     let data = b"d3:bar4:spam3:fooi42ee";
//     c.bench_function("bencode_decode", |b| b.iter(|| {
//         black_box(decode(black_box(data)).unwrap());
//     }));
// }
// ```

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_integer() {
        assert_eq!(BencodeValue::Integer(42).encode(), b"i42e");
        assert_eq!(BencodeValue::Integer(0).encode(), b"i0e");
        assert_eq!(BencodeValue::Integer(-42).encode(), b"i-42e");
    }

    #[test]
    fn test_encode_byte_string() {
        assert_eq!(
            BencodeValue::ByteString(b"spam".to_vec()).encode(),
            b"4:spam"
        );
        assert_eq!(BencodeValue::ByteString(b"".to_vec()).encode(), b"0:");
    }

    #[test]
    fn test_encode_list() {
        let list = BencodeValue::List(vec![
            BencodeValue::ByteString(b"spam".to_vec()),
            BencodeValue::Integer(42),
        ]);
        assert_eq!(list.encode(), b"l4:spami42ee");
    }

    #[test]
    fn test_encode_dictionary() {
        let mut map = BTreeMap::new();
        map.insert(
            "bar".to_string(),
            BencodeValue::ByteString(b"spam".to_vec()),
        );
        map.insert("foo".to_string(), BencodeValue::Integer(42));

        let dict = BencodeValue::Dictionary(map);
        // Keys are sorted: "bar" then "foo"
        assert_eq!(dict.encode(), b"d3:bar4:spam3:fooi42ee");
    }

    #[test]
    fn test_decode_integer() {
        assert_eq!(decode(b"i42e").unwrap().0, BencodeValue::Integer(42));
        assert_eq!(decode(b"i0e").unwrap().0, BencodeValue::Integer(0));
        assert_eq!(decode(b"i-42e").unwrap().0, BencodeValue::Integer(-42));

        assert!(decode(b"i-0e").is_err()); // Negative zero not allowed
        assert!(decode(b"i03e").is_err()); // Leading zero not allowed
        assert!(decode(b"ie").is_err());
    }

    #[test]
    fn test_decode_byte_string() {
        assert_eq!(
            decode(b"4:spam").unwrap().0,
            BencodeValue::ByteString(b"spam".to_vec())
        );
        assert_eq!(
            decode(b"0:").unwrap().0,
            BencodeValue::ByteString(b"".to_vec())
        );

        assert!(decode(b"4:spa").is_err()); // Too short
    }

    #[test]
    fn test_decode_list() {
        let list = decode(b"l4:spami42ee").unwrap().0;
        assert_eq!(
            list,
            BencodeValue::List(vec![
                BencodeValue::ByteString(b"spam".to_vec()),
                BencodeValue::Integer(42),
            ])
        );
    }

    #[test]
    fn test_decode_dictionary() {
        let dict = decode(b"d3:bar4:spam3:fooi42ee").unwrap().0;

        let mut map = BTreeMap::new();
        map.insert(
            "bar".to_string(),
            BencodeValue::ByteString(b"spam".to_vec()),
        );
        map.insert("foo".to_string(), BencodeValue::Integer(42));

        assert_eq!(dict, BencodeValue::Dictionary(map));
    }

    #[test]
    fn test_decode_dictionary_unsorted() {
        // "foo" comes before "bar", which violates Bencode specification
        assert!(decode(b"d3:fooi42e3:bar4:spame").is_err());
    }
    #[test]
    fn test_decode_stack_overflow_prevention() {
        let mut deeply_nested = Vec::new();
        for _ in 0..1000 {
            deeply_nested.push(b'l');
        }
        for _ in 0..1000 {
            deeply_nested.push(b'e');
        }
        let result = decode(&deeply_nested);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Max recursion depth exceeded");
    }

    #[test]
    fn test_decode_integer_overflow_prevention() {
        // String length is maximum u64
        let malicious_input = b"18446744073709551615:a";
        let result = decode(malicious_input);
        assert!(result.is_err());
    }
}
