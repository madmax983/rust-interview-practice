//! # 385. Mini Parser
//!
//! Difficulty: Medium
//! Link: <https://leetcode.com/problems/mini-parser/>
//!
//! Given a string `s` represents the serialization of a nested list, implement a parser to
//! deserialize it and return the deserialized `NestedInteger`.
//!
//! This problem is the quintessential example of Rust's powerful recursive `enum`s. It
//! demonstrates how to parse a flat string into a tree-like recursive data structure,
//! utilizing `Peekable` iterators and pattern matching to safely handle nested state
//! without manual index juggling or confusing out-of-bounds errors.
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::strings::mini_parser::{deserialize, NestedInteger};
//!
//! // Example 1: Single Integer
//! let res = deserialize("324".to_string());
//! assert_eq!(res, NestedInteger::Int(324));
//!
//! // Example 2: Nested List
//! let res = deserialize("[123,[456,[789]]]".to_string());
//! // Represents: [123, [456, [789]]]
//! ```
//!
//! ## Constraints
//!
//! - `1 <= s.length <= 5 * 10^4`
//! - `s` consists of digits, square brackets `"[]"`, negative sign `'-'`, and commas `','`.
//! - `s` is the serialization of valid `NestedInteger`.
//! - All the values in the input are in the range `[-10^6, 10^6]`.

use std::iter::Peekable;
use std::str::Chars;

// =========================================================================================
// Data Structures
// =========================================================================================

/// Represents a nested integer, which can either be a single integer or a list of nested integers.
///
/// **Rust Insight:**
/// In C++ or Java, recursive data structures usually require manual pointer management or
/// object references. In Rust, an enum cannot contain itself directly because its size
/// wouldn't be known at compile time. However, `Vec<T>` allocates its contents on the heap,
/// providing the necessary indirection to make `NestedInteger` a valid, safely managed
/// recursive type. (If it wasn't a list, we'd use `Box<NestedInteger>`).
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum NestedInteger {
    Int(i32),
    List(Vec<NestedInteger>),
}

impl NestedInteger {
    /// Helper method to add an element to the list variant.
    /// Panics if called on an `Int` variant.
    pub fn add(&mut self, elem: NestedInteger) {
        if let NestedInteger::List(list) = self {
            list.push(elem);
        } else {
            panic!("Called add on Int variant");
        }
    }
}

// =========================================================================================
// Approach 1: Iterative Stack-Based Parser
// =========================================================================================

/// Iterative Approach: Stack
///
/// We iterate through the characters. If we see a `[`, we push a new `NestedInteger::List`
/// onto our stack. When we parse a number, we push it into the list at the top of the stack.
/// When we see a `]`, we pop the current list and push it into the *new* top of the stack.
///
/// Time: O(N) - Single pass through the string.
/// Space: O(D) - Stack depth corresponds to the maximum nesting depth of the string.
///
/// **Gotcha:**
/// Parsing negative numbers and multi-digit numbers manually requires careful tracking of
/// whether we are currently "building" a number. We must also handle the edge case where
/// the string is just a single integer without brackets.
#[must_use]
pub fn deserialize_iterative(s: String) -> NestedInteger {
    if !s.starts_with('[') {
        return NestedInteger::Int(s.parse().unwrap());
    }

    let mut stack: Vec<NestedInteger> = Vec::new();
    let mut num_start: Option<usize> = None;
    let bytes = s.as_bytes();

    for (i, &byte) in bytes.iter().enumerate() {
        match byte {
            b'[' => {
                stack.push(NestedInteger::List(Vec::new()));
            }
            b'-' | b'0'..=b'9' => {
                if num_start.is_none() {
                    num_start = Some(i);
                }
            }
            b',' | b']' => {
                if let Some(start) = num_start {
                    // We finished reading a number, parse it
                    let num_str = std::str::from_utf8(&bytes[start..i]).unwrap();
                    let val: i32 = num_str.parse().unwrap();
                    let len = stack.len();
                    stack[len - 1].add(NestedInteger::Int(val));
                    num_start = None;
                }

                if byte == b']' && stack.len() > 1 {
                    // Finished parsing a nested list. Pop it and add to parent.
                    let finished_list = stack.pop().unwrap();
                    let len = stack.len();
                    stack[len - 1].add(finished_list);
                }
            }
            _ => {} // Ignore whitespace or unexpected chars if any
        }
    }

    stack.pop().unwrap()
}

// =========================================================================================
// Approach 2: Optimal Recursive Descent Parser
// =========================================================================================

/// Optimal Approach: Recursive Descent Parser with `Peekable` Iterator
///
/// We can elegantly translate the grammar of the nested list into mutually recursive functions.
/// We use a `Peekable<Chars>` iterator to look ahead and decide whether we are parsing a
/// list (starts with `[`) or an integer (starts with `-` or digit).
///
/// Time: O(N) - Every character is consumed exactly once.
/// Space: O(D) - Call stack depth is proportional to nesting level.
///
/// **Rust Insight:**
/// Passing `&mut Peekable<Chars>` allows recursive calls to consume characters from the
/// same underlying stream. `Peekable::peek()` lets us make decisions without advancing
/// the iterator, eliminating the need to track indices or backtrack.
#[must_use]
pub fn deserialize_optimal(s: String) -> NestedInteger {
    let mut iter = s.chars().peekable();
    parse_nested_integer(&mut iter)
}

fn parse_nested_integer(iter: &mut Peekable<Chars>) -> NestedInteger {
    if let Some(&c) = iter.peek() {
        if c == '[' {
            parse_list(iter)
        } else {
            parse_integer(iter)
        }
    } else {
        // Empty string case, shouldn't happen based on constraints but safe fallback
        NestedInteger::List(Vec::new())
    }
}

fn parse_list(iter: &mut Peekable<Chars>) -> NestedInteger {
    // Consume the '['
    iter.next();

    let mut list = Vec::new();

    // Check for empty list '[]'
    if let Some(&c) = iter.peek()
        && c == ']'
    {
        iter.next(); // Consume ']'
        return NestedInteger::List(list);
    }

    loop {
        // Parse the next element (either a number or a nested list)
        list.push(parse_nested_integer(iter));

        match iter.next() {
            Some(',') => continue, // More elements follow
            Some(']') => break,    // End of the list
            _ => panic!("Invalid serialization format"),
        }
    }

    NestedInteger::List(list)
}

fn parse_integer(iter: &mut Peekable<Chars>) -> NestedInteger {
    let mut num_str = String::new();

    // RUST INSIGHT: `while let` with a condition on the peeked value is
    // a very idiomatic way to consume tokens matching a certain criteria
    // without over-consuming.
    while let Some(&c) = iter.peek() {
        if c == '-' || c.is_ascii_digit() {
            num_str.push(c);
            iter.next(); // Actually consume the character
        } else {
            break;
        }
    }

    let val = num_str.parse::<i32>().expect("Failed to parse integer");
    NestedInteger::Int(val)
}

/// Main entry point - uses optimal recursive approach
#[must_use]
pub fn deserialize(s: String) -> NestedInteger {
    deserialize_optimal(s)
}

// =========================================================================================
// Alternative Approaches
// =========================================================================================
// 1. **Parser Combinator (nom crate):** In a real-world Rust project, parsing strings into
//    structured data is usually done with libraries like `nom` or `serde`. While writing
//    recursive descent parsers by hand is an excellent exercise, parser combinators are
//    generally more robust and easier to maintain for complex grammars.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_integer() {
        let input = "324".to_string();
        let expected = NestedInteger::Int(324);

        assert_eq!(deserialize_iterative(input.clone()), expected);
        assert_eq!(deserialize_optimal(input.clone()), expected);
        assert_eq!(deserialize(input), expected);
    }

    #[test]
    fn test_negative_single_integer() {
        let input = "-42".to_string();
        let expected = NestedInteger::Int(-42);

        assert_eq!(deserialize_iterative(input.clone()), expected);
        assert_eq!(deserialize_optimal(input.clone()), expected);
        assert_eq!(deserialize(input), expected);
    }

    #[test]
    fn test_nested_list() {
        // [123,[456,[789]]]
        let input = "[123,[456,[789]]]".to_string();
        let expected = NestedInteger::List(vec![
            NestedInteger::Int(123),
            NestedInteger::List(vec![
                NestedInteger::Int(456),
                NestedInteger::List(vec![NestedInteger::Int(789)]),
            ]),
        ]);

        assert_eq!(deserialize_iterative(input.clone()), expected);
        assert_eq!(deserialize_optimal(input.clone()), expected);
        assert_eq!(deserialize(input), expected);
    }

    #[test]
    fn test_empty_list() {
        let input = "[]".to_string();
        let expected = NestedInteger::List(vec![]);

        assert_eq!(deserialize_iterative(input.clone()), expected);
        assert_eq!(deserialize_optimal(input.clone()), expected);
        assert_eq!(deserialize(input), expected);
    }

    #[test]
    fn test_stress_mixed_list() {
        // [-1,[],[[]],[-2,3]]
        let input = "[-1,[],[[]],[-2,3]]".to_string();
        let expected = NestedInteger::List(vec![
            NestedInteger::Int(-1),
            NestedInteger::List(vec![]),
            NestedInteger::List(vec![NestedInteger::List(vec![])]),
            NestedInteger::List(vec![NestedInteger::Int(-2), NestedInteger::Int(3)]),
        ]);

        assert_eq!(deserialize_iterative(input.clone()), expected);
        assert_eq!(deserialize_optimal(input.clone()), expected);
        assert_eq!(deserialize(input), expected);
    }
}
