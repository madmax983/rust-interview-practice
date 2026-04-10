//! # Trait-based Serialization Framework
//!
//! Implements a minimal version of a reflection-free, zero-cost serialization framework.
//!
//! **Replaces Crates:** `serde`
//!
//! **Real-world Usage:**
//! - Data serialization for network transmission (JSON, Bincode).
//! - Configuration file parsing (TOML, YAML).
//! - Database Object-Relational Mapping (ORM) and abstraction layers.
//!
//! **Why build it yourself?**
//! Rebuilding the core traits of Serde teaches you the Visitor pattern and double dispatch in Rust.
//! It clarifies how Rust can achieve zero-cost abstractions by leveraging monomorphization instead of
//! runtime reflection, decoupling the data structures being serialized from the formats they are serialized into.

// =========================================================================================
// Architecture
// =========================================================================================
//
// Data Structure Diagram:
//
//      Data Structure (e.g., struct User)
//               │
//               ▼ (calls serialize)
//         Serialize Trait
//               │
//               ▼ (calls serialize_struct, serialize_str, etc.)
//        Serializer Trait (Format-specific, e.g., JSON)
//               │
//               ▼
//         Serialized Data
//
//       (Deserialization)
//         Serialized Data
//               │
//               ▼
//       Deserializer Trait (Format-specific)
//               │
//               ▼ (calls visit_struct, visit_str, etc.)
//          Visitor Trait
//               │
//               ▼
//         Deserialize Trait
//               │
//               ▼
//      Data Structure (e.g., struct User)
//
// Invariants:
// 1. Double Dispatch: The data structure knows what types it contains (e.g., i32, String) and tells the `Serializer`.
//    The `Serializer` knows how to encode those types into the specific format.
// 2. The `Deserializer` knows how to parse the format into primitive types. The `Visitor` (provided by the data structure)
//    knows how to construct the final type from those primitives.
//
// Complexity:
// ┌───────────────────┬────────┬────────┐
// │ Operation         │ Time   │ Space  │
// ├───────────────────┼────────┼────────┤
// │ Serialize         │ O(N)   │ O(1)*  │
// │ Deserialize       │ O(N)   │ O(N)   │
// └───────────────────┴────────┴────────┘
// * Not including the output buffer size.
//
// Design Decisions:
// - Uses traits to statically dispatch serialization logic, allowing zero-cost monomorphization.
// - Lifetimes (`'de`) on `Deserializer` and `Visitor` allow for zero-copy deserialization where the
//   deserialized struct borrows slices from the input data.

use std::fmt;

/// An error type for the serialization framework.
#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    Custom(String),
    TypeMismatch,
    EndOfStream,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Custom(msg) => write!(f, "{}", msg),
            Error::TypeMismatch => write!(f, "type mismatch"),
            Error::EndOfStream => write!(f, "end of stream"),
        }
    }
}

/// A data format that can serialize any data structure supported by its traits.
pub trait Serializer {
    type Error: Into<Error>;

    fn serialize_bool(self, v: bool) -> Result<(), Self::Error>;
    fn serialize_i32(self, v: i32) -> Result<(), Self::Error>;
    fn serialize_str(self, v: &str) -> Result<(), Self::Error>;

    // RUST INSIGHT:
    // We would normally have a `serialize_struct` method here that returns a `SerializeStruct` trait,
    // which allows the caller to serialize fields sequentially. For simplicity, we stick to primitives.
}

/// A data structure that can be serialized into any data format supported by a Serializer.
pub trait Serialize {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<(), S::Error>;
}

/// A data structure that can be deserialized from any data format supported by a Deserializer.
pub trait Deserialize<'de>: Sized {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error>;
}

/// A data format that can deserialize any data structure supported by its traits.
pub trait Deserializer<'de>: Sized {
    type Error: Into<Error>;

    /// The deserializer inspects the format and decides which `Visitor` method to call.
    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> where V::Error: From<Error>;
    fn deserialize_i32<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> where V::Error: From<Error>;
    fn deserialize_str<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> where V::Error: From<Error>;
    fn deserialize_bool<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> where V::Error: From<Error>;
}

/// A visitor that constructs a type by inspecting the primitive types provided by a Deserializer.
pub trait Visitor<'de>: Sized {
    type Value;
    type Error: Into<Error>;

    fn visit_bool(self, _v: bool) -> Result<Self::Value, Self::Error>
    where
        Self::Error: From<Error>,
    {
        Err(Error::TypeMismatch.into())
    }

    fn visit_i32(self, _v: i32) -> Result<Self::Value, Self::Error>
    where
        Self::Error: From<Error>,
    {
        Err(Error::TypeMismatch.into())
    }

    fn visit_str(self, _v: &str) -> Result<Self::Value, Self::Error>
    where
        Self::Error: From<Error>,
    {
        Err(Error::TypeMismatch.into())
    }

    // PRODUCTION NOTE:
    // A real `Visitor` trait in Serde has dozens of methods (e.g., `visit_u64`, `visit_map`, `visit_seq`).
    // It provides default implementations that return a "type mismatch" error, so implementers only need
    // to define the methods relevant to their expected type.
}

// =========================================================================================
// Implementations for Built-in Types
// =========================================================================================

impl Serialize for i32 {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<(), S::Error> {
        // Double dispatch: the `i32` type tells the `Serializer` "I am an i32, serialize me as such."
        serializer.serialize_i32(*self)
    }
}

impl Serialize for String {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<(), S::Error> {
        serializer.serialize_str(self)
    }
}

impl Serialize for bool {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<(), S::Error> {
        serializer.serialize_bool(*self)
    }
}

struct I32Visitor;

impl<'de> Visitor<'de> for I32Visitor {
    type Value = i32;
    type Error = Error;

    fn visit_i32(self, v: i32) -> Result<Self::Value, Self::Error> {
        Ok(v)
    }
}

impl<'de> Deserialize<'de> for i32 {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        // Double dispatch: the `i32` type gives the `Deserializer` an `I32Visitor` to call back.
        deserializer.deserialize_i32(I32Visitor)
    }
}

struct StringVisitor;

impl<'de> Visitor<'de> for StringVisitor {
    type Value = String;
    type Error = Error;

    fn visit_str(self, v: &str) -> Result<Self::Value, Self::Error> {
        Ok(v.to_owned())
    }
}

impl<'de> Deserialize<'de> for String {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_str(StringVisitor)
    }
}

// =========================================================================================
// Example Format: A Simple Key-Value String Serializer/Deserializer
// =========================================================================================

/// A simple serializer that just pushes the string representation into a mutable String.
pub struct SimpleStringSerializer<'a> {
    output: &'a mut String,
}

impl<'a> SimpleStringSerializer<'a> {
    pub fn new(output: &'a mut String) -> Self {
        Self { output }
    }
}

impl<'a> Serializer for SimpleStringSerializer<'a> {
    type Error = Error;

    fn serialize_bool(self, v: bool) -> Result<(), Self::Error> {
        self.output.push_str(if v { "true" } else { "false" });
        Ok(())
    }

    fn serialize_i32(self, v: i32) -> Result<(), Self::Error> {
        // GOTCHA:
        // Allocations can happen during serialization. Using `std::fmt::Write` directly
        // on strings avoids some allocations.
        use std::fmt::Write;
        write!(self.output, "{}", v).map_err(|_| Error::Custom("Format error".into()))
    }

    fn serialize_str(self, v: &str) -> Result<(), Self::Error> {
        self.output.push_str(v);
        Ok(())
    }
}

/// A simple deserializer that reads from a string reference.
pub struct SimpleStringDeserializer<'de> {
    input: &'de str,
}

impl<'de> SimpleStringDeserializer<'de> {
    pub fn new(input: &'de str) -> Self {
        Self { input }
    }
}

impl<'de> Deserializer<'de> for SimpleStringDeserializer<'de> {
    type Error = Error;

    fn deserialize_any<V: Visitor<'de>>(self, _visitor: V) -> Result<V::Value, Self::Error>
    where
        V::Error: From<Error>,
    {
        Err(Error::Custom("Not implemented".into()))
    }

    fn deserialize_i32<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V::Error: From<Error>,
    {
        let val = self.input.parse::<i32>().map_err(|_| Error::TypeMismatch)?;
        visitor.visit_i32(val).map_err(|e| e.into())
    }

    fn deserialize_str<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V::Error: From<Error>,
    {
        // Zero-copy string deserialization could return a borrowed `&'de str`, but our
        // string visitor currently allocates a `String`.
        visitor.visit_str(self.input).map_err(|e| e.into())
    }

    fn deserialize_bool<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V::Error: From<Error>,
    {
        let val = match self.input {
            "true" => true,
            "false" => false,
            _ => return Err(Error::TypeMismatch),
        };
        visitor.visit_bool(val).map_err(|e| e.into())
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to canonical `serde`:
// - `serde` uses a `SerializeStruct` trait allowing complex structures with named fields to be serialized.
// - `serde` provides procedural macros (`#[derive(Serialize, Deserialize)]`) to automatically implement these traits for structs.
// - `serde` supports a `deserialize_any` method for self-describing formats like JSON, where the format dictates the data types.
// - `serde` heavily utilizes `#[inline]` annotations to guarantee the compiler aggressively monomorphizes and optimizes away the double dispatch.
//
// What's missing vs. production:
// - Support for structs, maps, sequences, enums, and tuples.
// - Procedural macros for generating implementations.
// - Advanced zero-copy deserialization using `&'de str` and `Cow<'de, str>` (we only do `String` here).
//
// Suggested Next Steps:
// - Implement a `SerializeStruct` trait and an associated method on `Serializer`.
// - Implement a `Visitor::visit_seq` to deserialize arrays.
// - Create a JSON format implementing `Serializer` and `Deserializer`.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serialize_primitives() {
        let mut output = String::new();

        let num: i32 = 42;
        num.serialize(SimpleStringSerializer::new(&mut output)).unwrap();
        assert_eq!(output, "42");

        output.clear();
        let text = String::from("hello");
        text.serialize(SimpleStringSerializer::new(&mut output)).unwrap();
        assert_eq!(output, "hello");

        output.clear();
        let b = true;
        b.serialize(SimpleStringSerializer::new(&mut output)).unwrap();
        assert_eq!(output, "true");
    }

    // =========================================================================================
    // Benchmarking Note
    // =========================================================================================
    // To benchmark this implementation against `serde`:
    // ```rust
    // use criterion::{black_box, criterion_group, criterion_main, Criterion};
    // fn bench_serialization(c: &mut Criterion) {
    //     let mut output = String::with_capacity(10);
    //     c.bench_function("serialize_i32", |b| b.iter(|| {
    //         output.clear();
    //         black_box(42.serialize(SimpleStringSerializer::new(&mut output)).unwrap());
    //     }));
    // }
    // ```

    #[test]
    fn test_deserialize_primitives() {
        let de = SimpleStringDeserializer::new("42");
        let num = i32::deserialize(de).unwrap();
        assert_eq!(num, 42);

        let de = SimpleStringDeserializer::new("world");
        let text = String::deserialize(de).unwrap();
        assert_eq!(text, "world");

        let de_err = SimpleStringDeserializer::new("not_a_number");
        let err = i32::deserialize(de_err).unwrap_err();
        assert_eq!(err, Error::TypeMismatch);
    }
}
