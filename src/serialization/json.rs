//! # JSON Parser Implementation
//!
//! Implements a recursive descent JSON parser from scratch.
//! This implementation handles standard JSON types: Objects, Arrays, Strings, Numbers, Booleans, and Null.
//!
//! **Replaces Crates:** `serde_json`, `simd-json`
//!
//! **Real-world Usage:**
//! - Web APIs (REST/GraphQL).
//! - Configuration files.
//! - Data storage (NoSQL databases like MongoDB).
//!
//! **Why build it yourself?**
//! Writing a JSON parser is the "Hello World" of language implementation.
//! You learn about tokenization (handling whitespace, strings, numbers) and recursive structure parsing.
//! It forces you to deal with the flexibility of `enum`s in Rust to represent heterogeneous data.

use std::collections::HashMap;
use std::fmt;
use std::iter::Peekable;
use std::str::Chars;

// =========================================================================================
// Architecture
// =========================================================================================
//
// Data Structure:
//
//      JsonValue (Enum)
//      ├── Null
//      ├── Bool(bool)
//      ├── Number(f64)
//      ├── String(String)
//      ├── Array(Vec<JsonValue>)
//      └── Object(HashMap<String, JsonValue>)
//
// Parsing Flow:
//      input -> Chars -> Peekable -> parse_value() -> (match char)
//                                      ├── '{' -> parse_object()
//                                      ├── '[' -> parse_array()
//                                      ├── '"' -> parse_string()
//                                      ├── 't'/'f' -> parse_bool()
//                                      ├── 'n' -> parse_null()
//                                      └── digit/- -> parse_number()
//
// Complexity:
// ┌───────────────┬────────┬────────┐
// │ Operation     │ Time   │ Space  │
// ├───────────────┼────────┼────────┤
// │ Parse         │ O(N)   │ O(D)   │
// │ Serialize     │ O(N)   │ O(N)   │
// └───────────────┴────────┴────────┘
// N = input size, D = depth of nesting (stack space).
//
// Design Decisions:
// - **Number Representation**: `f64`.
//   - *Tradeoff*: Precision loss for large integers (beyond 2^53).
//   - *Alternative*: `serde_json` stores a custom Number type that can be u64, i64, or f64.
// - **Object Storage**: `HashMap`.
//   - *Tradeoff*: O(1) lookup but random iteration order.
//   - *Alternative*: `BTreeMap` for sorted keys (deterministic output). We use HashMap for speed.
// - **Error Handling**: `Result<JsonValue, String>`. Simple string errors.
//   - *Alternative*: Custom Error enum with line/column info.

/// Represents a JSON value.
#[derive(Debug, Clone, PartialEq)]
pub enum JsonValue {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Array(Vec<JsonValue>),
    Object(HashMap<String, JsonValue>),
}

/// Parses a JSON string into a JsonValue.
pub fn parse(input: &str) -> Result<JsonValue, String> {
    let mut parser = Parser::new(input);
    let value = parser.parse_value()?;
    parser.skip_whitespace();
    if parser.peek().is_some() {
        return Err("Trailing characters".to_string());
    }
    Ok(value)
}

struct Parser<'a> {
    chars: Peekable<Chars<'a>>,
}

impl<'a> Parser<'a> {
    fn new(input: &'a str) -> Self {
        Self {
            chars: input.chars().peekable(),
        }
    }

    fn peek(&mut self) -> Option<&char> {
        self.chars.peek()
    }

    fn next(&mut self) -> Option<char> {
        self.chars.next()
    }

    fn skip_whitespace(&mut self) {
        while let Some(&c) = self.peek() {
            if c.is_whitespace() {
                self.next();
            } else {
                break;
            }
        }
    }

    fn parse_value(&mut self) -> Result<JsonValue, String> {
        self.skip_whitespace();
        let c = self.peek().ok_or("Unexpected EOF")?;

        match c {
            '{' => self.parse_object(),
            '[' => self.parse_array(),
            '"' => self.parse_string().map(JsonValue::String),
            't' | 'f' => self.parse_bool().map(JsonValue::Bool),
            'n' => self.parse_null().map(|_| JsonValue::Null),
            '-' | '0'..='9' => self.parse_number().map(JsonValue::Number),
            _ => Err(format!("Unexpected character: {}", c)),
        }
    }

    fn parse_object(&mut self) -> Result<JsonValue, String> {
        self.next(); // consume '{'
        let mut map = HashMap::new();

        self.skip_whitespace();
        if let Some(&'}') = self.peek() {
            self.next();
            return Ok(JsonValue::Object(map));
        }

        loop {
            self.skip_whitespace();
            let key = self.parse_string()?;

            self.skip_whitespace();
            if self.next() != Some(':') {
                return Err("Expected ':' after key in object".to_string());
            }

            let value = self.parse_value()?;
            map.insert(key, value);

            self.skip_whitespace();
            match self.next() {
                Some('}') => break,
                Some(',') => continue,
                _ => return Err("Expected '}' or ',' in object".to_string()),
            }
        }

        Ok(JsonValue::Object(map))
    }

    fn parse_array(&mut self) -> Result<JsonValue, String> {
        self.next(); // consume '['
        let mut vec = Vec::new();

        self.skip_whitespace();
        if let Some(&']') = self.peek() {
            self.next();
            return Ok(JsonValue::Array(vec));
        }

        loop {
            let value = self.parse_value()?;
            vec.push(value);

            self.skip_whitespace();
            match self.next() {
                Some(']') => break,
                Some(',') => continue,
                _ => return Err("Expected ']' or ',' in array".to_string()),
            }
        }

        Ok(JsonValue::Array(vec))
    }

    fn parse_string(&mut self) -> Result<String, String> {
        if self.next() != Some('"') {
            return Err("Expected '\"' to start string".to_string());
        }

        let mut s = String::new();
        loop {
            match self.next() {
                Some('"') => break,
                Some('\\') => {
                    match self.next() {
                        Some('"') => s.push('"'),
                        Some('\\') => s.push('\\'),
                        Some('/') => s.push('/'),
                        Some('b') => s.push('\x08'),
                        Some('f') => s.push('\x0c'),
                        Some('n') => s.push('\n'),
                        Some('r') => s.push('\r'),
                        Some('t') => s.push('\t'),
                        Some('u') => {
                            // Basic unicode support
                            let mut hex = String::new();
                            for _ in 0..4 {
                                hex.push(self.next().ok_or("Unexpected EOF in unicode escape")?);
                            }
                            let code = u32::from_str_radix(&hex, 16)
                                .map_err(|_| "Invalid unicode escape".to_string())?;
                            s.push(std::char::from_u32(code).ok_or("Invalid unicode char")?);
                        }
                        Some(c) => return Err(format!("Invalid escape sequence: \\{}", c)),
                        None => return Err("Unexpected EOF in string escape".to_string()),
                    }
                }
                Some(c) => s.push(c),
                None => return Err("Unexpected EOF in string".to_string()),
            }
        }
        Ok(s)
    }

    fn parse_bool(&mut self) -> Result<bool, String> {
        if self.peek() == Some(&'t') {
            self.consume("true")?;
            Ok(true)
        } else {
            self.consume("false")?;
            Ok(false)
        }
    }

    fn parse_null(&mut self) -> Result<(), String> {
        self.consume("null")
    }

    fn consume(&mut self, expected: &str) -> Result<(), String> {
        for c in expected.chars() {
            if self.next() != Some(c) {
                return Err(format!("Expected '{}'", expected));
            }
        }
        Ok(())
    }

    fn parse_number(&mut self) -> Result<f64, String> {
        let mut num_str = String::new();

        // Handle negative sign
        if let Some(&'-') = self.peek() {
            num_str.push(self.next().unwrap());
        }

        // Handle integer part
        let mut has_digit = false;
        while let Some(&c) = self.peek() {
            if c.is_ascii_digit() {
                num_str.push(self.next().unwrap());
                has_digit = true;
            } else {
                break;
            }
        }

        if !has_digit {
             return Err("Invalid number: no digits".to_string());
        }

        // Handle fraction
        if let Some(&'.') = self.peek() {
            num_str.push(self.next().unwrap());
            while let Some(&c) = self.peek() {
                if c.is_ascii_digit() {
                    num_str.push(self.next().unwrap());
                } else {
                    break;
                }
            }
        }

        // Handle exponent
        if let Some(&'e') | Some(&'E') = self.peek() {
            num_str.push(self.next().unwrap());
            if let Some(&'+') | Some(&'-') = self.peek() {
                num_str.push(self.next().unwrap());
            }
            while let Some(&c) = self.peek() {
                if c.is_ascii_digit() {
                    num_str.push(self.next().unwrap());
                } else {
                    break;
                }
            }
        }

        num_str.parse::<f64>().map_err(|_| "Invalid float".to_string())
    }
}

impl fmt::Display for JsonValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            JsonValue::Null => write!(f, "null"),
            JsonValue::Bool(b) => write!(f, "{}", b),
            JsonValue::Number(n) => write!(f, "{}", n),
            JsonValue::String(s) => write!(f, "{:?}", s), // Use Debug to handle escaping
            JsonValue::Array(arr) => {
                write!(f, "[")?;
                for (i, v) in arr.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", v)?;
                }
                write!(f, "]")
            }
            JsonValue::Object(obj) => {
                write!(f, "{{")?;
                // Sort keys for deterministic output in display
                let mut keys: Vec<&String> = obj.keys().collect();
                keys.sort();
                for (i, k) in keys.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{:?}: {}", k, obj.get(*k).unwrap())?;
                }
                write!(f, "}}")
            }
        }
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `serde_json`: The gold standard. Uses a Visitor pattern for ultra-efficient serialization/deserialization directly
//   into Rust structs (via `serde` trait). Handles every edge case and is heavily optimized.
// - `simd-json`: Uses SIMD instructions to parse JSON extremely fast.
//
// Missing vs. Production:
// - **Zero-copy**: We allocate `String` for every string value. `serde_json` can borrow from input (`Cow<str>`).
// - **Struct Mapping**: We only return a `JsonValue` DOM. We can't automatically map to a Rust struct (`#[derive(Deserialize)]`).
// - **Precision**: `f64` loses precision for large integers.
// - **Error Reporting**: We just return strings. Real parsers give line/col numbers.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_basics() {
        assert_eq!(parse("null"), Ok(JsonValue::Null));
        assert_eq!(parse("true"), Ok(JsonValue::Bool(true)));
        assert_eq!(parse("false"), Ok(JsonValue::Bool(false)));
        assert_eq!(parse("123"), Ok(JsonValue::Number(123.0)));
        assert_eq!(parse("12.34"), Ok(JsonValue::Number(12.34)));
        assert_eq!(parse("\"hello\""), Ok(JsonValue::String("hello".to_string())));
    }

    #[test]
    fn test_parse_array() {
        let json = "[1, 2, 3]";
        let res = parse(json).unwrap();
        match res {
            JsonValue::Array(vec) => {
                assert_eq!(vec.len(), 3);
                assert_eq!(vec[0], JsonValue::Number(1.0));
            }
            _ => panic!("Expected Array"),
        }
    }

    #[test]
    fn test_parse_object() {
        let json = "{\"key\": \"value\", \"num\": 42}";
        let res = parse(json).unwrap();
        match res {
            JsonValue::Object(map) => {
                assert_eq!(map.len(), 2);
                assert_eq!(map["key"], JsonValue::String("value".to_string()));
                assert_eq!(map["num"], JsonValue::Number(42.0));
            }
            _ => panic!("Expected Object"),
        }
    }

    #[test]
    fn test_parse_nested() {
        let json = "{\"list\": [null, true, {\"a\": 1}]}";
        let res = parse(json).unwrap();
        // Just checking it parses successfully
        assert!(matches!(res, JsonValue::Object(_)));
    }

    #[test]
    fn test_parse_errors() {
        assert!(parse("{").is_err());
        assert!(parse("{\"a\": 1").is_err());
        assert!(parse("[1, 2").is_err());
        assert!(parse("tru").is_err());
    }

    #[test]
    fn test_trailing() {
        assert!(parse("null ").is_ok()); // trailing whitespace ok
        assert!(parse("null x").is_err()); // trailing chars not ok
    }

    #[test]
    fn test_to_string() {
        let json = "{\"a\": 1, \"b\": [2, 3]}";
        let parsed = parse(json).unwrap();
        let s = parsed.to_string();
        // Output format is deterministic due to key sorting in Display impl
        assert_eq!(s, "{\"a\": 1, \"b\": [2, 3]}");
    }
}
