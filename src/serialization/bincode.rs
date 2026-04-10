//! # Binary Serialization Engine
//!
//! **What this implements:** A custom binary serialization format and traits.
//! **Replaces Crates:** `bincode`, `rmp` (MessagePack), `postcard`.
//!
//! **Real-world Usage:**
//! - Fast Inter-Process Communication (IPC).
//! - Game state saves and network replication (where bandwidth/storage is premium).
//! - Storing structured data in embedded databases (like RocksDB or LMDB).
//!
//! **Why build it yourself?**
//! Text formats like JSON are slow to parse and bulky. Building a binary serializer teaches you
//! about endianness, memory layouts, zero-cost abstractions, and how to safely reconstruct complex
//! types directly from raw bytes. You learn exactly what overhead `serde` is hiding from you.
//!
//! # Architecture
//!
//! **Encoding Rules (Little Endian):**
//! - Primitives (`u8`, `u32`, `u64`, etc.): Raw little-endian bytes.
//! - `String`: 8-byte length prefix (u64) followed by raw UTF-8 bytes.
//! - `Vec<T>`: 8-byte length prefix (u64) followed by serialized elements.
//!
//! **Invariants:**
//! 1. The serializer will always produce little-endian output for cross-platform consistency.
//! 2. Strings must be valid UTF-8; the deserializer validates this before returning.
//! 3. Slices read during deserialization will bounds-check early to prevent panics.
//!
//! **Complexity:**
//! | Operation     | Time     | Space       |
//! |---------------|----------|-------------|
//! | Serialize     | O(N)     | O(N)        |
//! | Deserialize   | O(N)     | O(N)        |
//!
//! *Where N is the size of the data being serialized/deserialized. Space complexity is O(N) to hold the output buffer.*
//!
//! **Design Decisions:**
//! - **Trait-based Interfaces:** `Serialize` and `Deserialize` traits allow extending support to any custom type.
//! - **Cursor-less deserialization:** We use `&mut &[u8]` as the reader. Advancing the slice is an incredibly cheap and ergonomic way to consume bytes.
//! - **Pre-allocation:** Length prefixes allow the deserializer to pre-allocate exact capacities for vectors and strings, avoiding dynamic reallocation.

use std::convert::TryInto;
use std::fmt;
use std::string::FromUtf8Error;

/// Errors that can occur during deserialization.
#[derive(Debug, PartialEq)]
pub enum DecodeError {
    /// The buffer did not contain enough bytes.
    UnexpectedEof,
    /// A string contained invalid UTF-8 bytes.
    InvalidUtf8,
    /// An unsupported or corrupted length was encountered.
    CapacityOverflow,
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedEof => write!(f, "Unexpected end of file/buffer"),
            Self::InvalidUtf8 => write!(f, "Invalid UTF-8 sequence"),
            Self::CapacityOverflow => {
                write!(f, "Requested capacity exceeds limits or is corrupted")
            }
        }
    }
}

impl std::error::Error for DecodeError {}

impl From<FromUtf8Error> for DecodeError {
    fn from(_: FromUtf8Error) -> Self {
        Self::InvalidUtf8
    }
}

/// The core `Serialize` trait.
/// Any type implementing this can be encoded into a binary stream.
pub trait Serialize {
    /// Appends the serialized representation of `self` to the provided byte vector.
    fn serialize(&self, buffer: &mut Vec<u8>);
}

/// The core `Deserialize` trait.
/// Any type implementing this can be decoded from a binary stream.
pub trait Deserialize: Sized {
    /// Attempts to read `Self` from the given byte slice, advancing the slice.
    fn deserialize(bytes: &mut &[u8]) -> Result<Self, DecodeError>;
}

// ============================================================================
// Primitive Implementations
// ============================================================================

// RUST INSIGHT: We can use a macro to implement these traits for all numeric types,
// keeping the code DRY while leaning on Rust's `to_le_bytes` and `from_le_bytes`.
macro_rules! impl_serialize_for_num {
    ($type:ty) => {
        impl Serialize for $type {
            fn serialize(&self, buffer: &mut Vec<u8>) {
                buffer.extend_from_slice(&self.to_le_bytes());
            }
        }

        impl Deserialize for $type {
            fn deserialize(bytes: &mut &[u8]) -> Result<Self, DecodeError> {
                let size = std::mem::size_of::<$type>();
                if bytes.len() < size {
                    return Err(DecodeError::UnexpectedEof);
                }

                // RUST INSIGHT: `split_at` safely divides the slice. No raw pointer math needed.
                let (chunk, rest) = bytes.split_at(size);
                *bytes = rest;

                // GOTCHA: `from_le_bytes` expects a fixed-size array. `try_into()` converts
                // the dynamic slice into an array, relying on the bounds check we just did.
                let arr = chunk.try_into().unwrap();
                Ok(<$type>::from_le_bytes(arr))
            }
        }
    };
}

impl_serialize_for_num!(u8);
impl_serialize_for_num!(u16);
impl_serialize_for_num!(u32);
impl_serialize_for_num!(u64);
impl_serialize_for_num!(i8);
impl_serialize_for_num!(i16);
impl_serialize_for_num!(i32);
impl_serialize_for_num!(i64);
impl_serialize_for_num!(f32);
impl_serialize_for_num!(f64);

impl Serialize for bool {
    fn serialize(&self, buffer: &mut Vec<u8>) {
        buffer.push(if *self { 1 } else { 0 });
    }
}

impl Deserialize for bool {
    fn deserialize(bytes: &mut &[u8]) -> Result<Self, DecodeError> {
        if bytes.is_empty() {
            return Err(DecodeError::UnexpectedEof);
        }
        let val = bytes[0];
        *bytes = &bytes[1..];
        Ok(val != 0)
    }
}

// ============================================================================
// Compound Implementations
// ============================================================================

impl Serialize for String {
    fn serialize(&self, buffer: &mut Vec<u8>) {
        let len = self.len() as u64;
        len.serialize(buffer);
        buffer.extend_from_slice(self.as_bytes());
    }
}

impl Deserialize for String {
    fn deserialize(bytes: &mut &[u8]) -> Result<Self, DecodeError> {
        let len = u64::deserialize(bytes)? as usize;

        // PRODUCTION NOTE: In a real implementation (like bincode), you must limit
        // the maximum allocation size to prevent OOM attacks from malformed payloads
        // reporting a length of `u64::MAX`.
        if bytes.len() < len {
            return Err(DecodeError::UnexpectedEof);
        }

        let (chunk, rest) = bytes.split_at(len);
        *bytes = rest;

        // Zero-copy validation -> Allocation
        String::from_utf8(chunk.to_vec()).map_err(Into::into)
    }
}

impl<T: Serialize> Serialize for Vec<T> {
    fn serialize(&self, buffer: &mut Vec<u8>) {
        let len = self.len() as u64;
        len.serialize(buffer);
        for item in self {
            item.serialize(buffer);
        }
    }
}

impl<T: Deserialize> Deserialize for Vec<T> {
    fn deserialize(bytes: &mut &[u8]) -> Result<Self, DecodeError> {
        let len = u64::deserialize(bytes)? as usize;

        // Protect against huge allocations
        // Arbitrary sanity check for this educational implementation
        if len > 100_000_000 {
            return Err(DecodeError::CapacityOverflow);
        }

        // BOLT OPTIMIZATION: Pre-allocate capacity to eliminate dynamic heap reallocations.
        // Since we know exactly how many items to deserialize, we initialize the vector
        // with the precise capacity to prevent costly reallocations.
        let mut vec = Vec::with_capacity(len);
        for _ in 0..len {
            vec.push(T::deserialize(bytes)?);
        }
        Ok(vec)
    }
}

impl<T: Serialize> Serialize for Option<T> {
    fn serialize(&self, buffer: &mut Vec<u8>) {
        match self {
            Some(val) => {
                buffer.push(1);
                val.serialize(buffer);
            }
            None => {
                buffer.push(0);
            }
        }
    }
}

impl<T: Deserialize> Deserialize for Option<T> {
    fn deserialize(bytes: &mut &[u8]) -> Result<Self, DecodeError> {
        let is_some = bool::deserialize(bytes)?;
        if is_some {
            Ok(Some(T::deserialize(bytes)?))
        } else {
            Ok(None)
        }
    }
}

// ============================================================================
// Footer
// ============================================================================
//
// Comparison to Canonical Crates:
// - `bincode`: The standard for Rust binary serialization. It ties directly into `serde`,
//   meaning it can serialize almost any Rust struct automatically using macros (`#[derive(Serialize)]`).
//   Our implementation requires manual trait implementation.
// - `postcard`: A `#![no_std]` focused binary serializer that uses variable-length integers (varints)
//   to compress small numbers. We use fixed-width Little Endian integers for simplicity.
//
// Missing vs. Production:
// - **Derive Macros**: Real crates use `proc_macros` to automatically generate `serialize`/`deserialize` implementations for structs.
// - **Zero-Copy Deserialization**: Production systems often allow deserializing into `&'a str` referencing the original buffer, avoiding allocations entirely.
// - **VarInts**: Numbers are always taking their full size here (e.g., `1u64` takes 8 bytes).
//
// Suggested Next Steps:
// 1. Add `VarInt` encoding for `u64` to save space.
// 2. Implement `Serialize` and `Deserialize` for `HashMap` and tuples.
// 3. Write a declarative macro to generate implementations for simple structs.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_primitives() {
        let mut buffer = Vec::new();

        42u32.serialize(&mut buffer);
        (-15i64).serialize(&mut buffer);
        true.serialize(&mut buffer);
        std::f64::consts::PI.serialize(&mut buffer);

        let mut bytes = buffer.as_slice();
        assert_eq!(u32::deserialize(&mut bytes).unwrap(), 42);
        assert_eq!(i64::deserialize(&mut bytes).unwrap(), -15);
        assert!(bool::deserialize(&mut bytes).unwrap());
        assert_eq!(f64::deserialize(&mut bytes).unwrap(), std::f64::consts::PI);
        assert!(bytes.is_empty());
    }

    #[test]
    fn test_string() {
        let original = "Hello, Rust! 🦀".to_string();
        let mut buffer = Vec::new();
        original.serialize(&mut buffer);

        let mut bytes = buffer.as_slice();
        let decoded = String::deserialize(&mut bytes).unwrap();

        assert_eq!(original, decoded);
    }

    #[test]
    fn test_vec_of_strings() {
        let original = vec![
            "Apple".to_string(),
            "Banana".to_string(),
            "Cherry".to_string(),
        ];

        let mut buffer = Vec::new();
        original.serialize(&mut buffer);

        let mut bytes = buffer.as_slice();
        let decoded = Vec::<String>::deserialize(&mut bytes).unwrap();

        assert_eq!(original, decoded);
    }

    #[test]
    fn test_option() {
        let some_val: Option<u32> = Some(123);
        let none_val: Option<u32> = None;

        let mut buffer = Vec::new();
        some_val.serialize(&mut buffer);
        none_val.serialize(&mut buffer);

        let mut bytes = buffer.as_slice();
        assert_eq!(Option::<u32>::deserialize(&mut bytes).unwrap(), Some(123));
        assert_eq!(Option::<u32>::deserialize(&mut bytes).unwrap(), None);
    }

    #[test]
    fn test_unexpected_eof() {
        let buffer = vec![1, 2, 3]; // Only 3 bytes
        let mut bytes = buffer.as_slice();

        let err = u32::deserialize(&mut bytes).unwrap_err();
        assert_eq!(err, DecodeError::UnexpectedEof);
    }

    #[test]
    fn test_struct_manual_impl() {
        #[derive(Debug, PartialEq)]
        struct Player {
            id: u32,
            name: String,
            score: f32,
        }

        impl Serialize for Player {
            fn serialize(&self, buffer: &mut Vec<u8>) {
                self.id.serialize(buffer);
                self.name.serialize(buffer);
                self.score.serialize(buffer);
            }
        }

        impl Deserialize for Player {
            fn deserialize(bytes: &mut &[u8]) -> Result<Self, DecodeError> {
                Ok(Player {
                    id: u32::deserialize(bytes)?,
                    name: String::deserialize(bytes)?,
                    score: f32::deserialize(bytes)?,
                })
            }
        }

        let p1 = Player {
            id: 99,
            name: "JohnDoe".to_string(),
            score: 15.5,
        };

        let mut buffer = Vec::new();
        p1.serialize(&mut buffer);

        let mut bytes = buffer.as_slice();
        let p2 = Player::deserialize(&mut bytes).unwrap();

        assert_eq!(p1, p2);
    }

    // Benchmark Note:
    // To properly benchmark this system against `bincode`, you can use `criterion`:
    // ```rust
    // pub fn bench_serialization(c: &mut Criterion) {
    //     let data = vec!["Rust".to_string(); 1000];
    //     c.bench_function("custom serialize", |b| b.iter(|| {
    //         let mut buf = Vec::with_capacity(8192);
    //         std::hint::black_box(&data).serialize(&mut buf);
    //         std::hint::black_box(buf);
    //     }));
    // }
    // ```
}
