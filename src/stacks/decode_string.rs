//! # 394. Decode String
//!
//! Difficulty: Medium
//! Link: <https://leetcode.com/problems/decode-string/>
//!
//! Given an encoded string, return its decoded string.
//!
//! The encoding rule is: `k[encoded_string]`, where the `encoded_string` inside the square
//! brackets is being repeated exactly `k` times. Note that `k` is guaranteed to be a positive integer.
//!
//! You may assume that the input string is always valid; there are no extra white spaces,
//! square brackets are well-formed, etc. Furthermore, you may assume that the original data
//! does not contain any digits and that digits are only for those repeat numbers, `k`. For example,
//! there will not be input like `3a` or `2[4]`.
//!
//! This problem is a natural fit for building parsers in Rust. It provides a fantastic opportunity
//! to demonstrate Rust's iterators (especially `Peekable`), standard `Vec` as a stack, and how
//! enums (Algebraic Data Types) can cleanly model hierarchical text structures instead of juggling
//! untyped loops and nested strings.
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::stacks::decode_string::decode_string;
//!
//! assert_eq!(decode_string("3[a]2[bc]".to_string()), "aaabcbc");
//! assert_eq!(decode_string("3[a2[c]]".to_string()), "accaccacc");
//! assert_eq!(decode_string("2[abc]3[cd]ef".to_string()), "abcabccdcdcdef");
//! ```
//!
//! ## Constraints
//!
//! - `1 <= s.length <= 30`
//! - `s` consists of lowercase English letters, digits, and square brackets `'[]'`.
//! - `s` is guaranteed to be a valid input.
//! - All the integers in `s` are in the range `[1, 300]`.

use std::iter::Peekable;
use std::str::Chars;

/// Brute force approach: String Replacement using `rfind`.
/// Time: O(N * max(K)^D) - where N is length, K is max multiplier, D is depth.
/// Space: O(N * max(K)^D) - intermediate strings get very large.
///
/// This approach iteratively finds the innermost bracket pairs by searching for the
/// last `[` and the first `]` after it. It extracts the number, repeats the string,
/// and replaces that slice of the original string.
///
/// RUST INSIGHT: String manipulation in Rust is safe but requires explicit byte index
/// handling. `.replace_range()` modifies a `String` in-place, shifting elements.
/// Because Rust Strings are UTF-8, random index access isn't allowed (since characters
/// can span multiple bytes). We must operate on byte slices `&s[start..end]` carefully.
/// # Panics
/// Panics if the input string is malformed (e.g., missing closing brackets or unparseable numbers),
/// though `LeetCode` guarantees valid inputs.
#[must_use]
#[allow(clippy::needless_pass_by_value)] // LeetCode signature
pub fn decode_string_brute_force(mut s: String) -> String {
    // Keep looping as long as there's a '['
    while let Some(open_idx) = s.rfind('[') {
        // Find the matching ']' for this innermost '['
        // Since we used `rfind`, this '[' has no other '[' inside it.
        let close_idx = s[open_idx..].find(']').unwrap() + open_idx;

        // Find where the number starts (walk backwards from `open_idx`)
        let mut num_start = open_idx;
        while num_start > 0 {
            let prev_char = s[num_start - 1..num_start].chars().next().unwrap();
            if prev_char.is_ascii_digit() {
                num_start -= 1;
            } else {
                break;
            }
        }

        // Parse the multiplier
        let k: usize = s[num_start..open_idx].parse().unwrap();
        // Extract the inner string
        let inner = &s[open_idx + 1..close_idx];

        // Repeat the inner string `k` times
        let repeated = inner.repeat(k);

        // Replace the entire chunk `k[inner]` with the repeated string
        s.replace_range(num_start..=close_idx, &repeated);
    }

    s
}

/// Optimized approach: Single Pass using a Stack.
/// Time: O(N * max(K)^D) - length of the final decoded string.
/// Space: O(N * max(K)^D) - space for the stack and final string.
///
/// Instead of repeatedly scanning and replacing substrings, we process the string in
/// one pass. When we encounter a '[', we push the current accumulated string and
/// the multiplier onto a stack. When we see a ']', we pop them off and append the
/// repeated new segment.
///
/// GOTCHA: It is tempting to write `stack.push((count, current_str))` and reinitialize
/// `current_str = String::new()`. Using `std::mem::take` allows us to swap in an empty
/// string while moving the existing one, which is very idiomatic and avoids unnecessary allocations.
#[must_use]
#[allow(clippy::needless_pass_by_value)] // LeetCode signature
pub fn decode_string_optimized(s: String) -> String {
    // Stack stores: (multiplier, previous_string_context)
    let mut stack: Vec<(usize, String)> = Vec::new();

    let mut current_num = 0;
    let mut current_str = String::new();

    for ch in s.chars() {
        if ch.is_ascii_digit() {
            current_num = current_num * 10 + (ch as usize - '0' as usize);
        } else if ch == '[' {
            // Push the current state context to the stack and reset
            stack.push((current_num, std::mem::take(&mut current_str)));
            current_num = 0;
        } else if ch == ']' {
            // Pop the previous context and append current_str repeated `k` times
            if let Some((k, prev_str)) = stack.pop() {
                // RUST INSIGHT: `std::mem::take` leaves `current_str` empty,
                // and we assign the new built string back to `current_str`.
                current_str = prev_str + &current_str.repeat(k);
            }
        } else {
            // Normal character
            current_str.push(ch);
        }
    }

    current_str
}

// =========================================================================================
// Optimal Approach: Recursive Descent Parser with AST
// =========================================================================================

/// An Abstract Syntax Tree (AST) node for our encoded string.
///
/// RUST INSIGHT: Enums with associated data are perfect for representing hierarchical
/// structures like expressions. This allows us to separate the *parsing* of the string
/// from the *evaluation* (decoding) of the string.
#[derive(Debug, PartialEq)]
enum Expr {
    /// A raw string literal (e.g., "abc")
    Literal(String),
    /// A repeated block of expressions (e.g., `3[a]`)
    Repeat(usize, Vec<Self>),
}

impl Expr {
    /// Evaluates the AST into a final decoded String
    /// ⚡ BOLT OPTIMIZATION: Takes a mutable buffer `&mut String` to avoid allocating
    /// intermediate strings during recursive evaluation, representing a zero-cost abstraction
    /// that completely removes `O(max(K)^D)` allocations.
    fn evaluate(&self, out: &mut String) {
        match self {
            Self::Literal(s) => out.push_str(s),
            Self::Repeat(k, exprs) => {
                // Evaluate the inner block into a temporary buffer
                // This minimizes repeated allocations by only allocating once per depth
                let mut inner = String::new();
                for expr in exprs {
                    expr.evaluate(&mut inner);
                }

                // Append the repeated block to the output
                for _ in 0..*k {
                    out.push_str(&inner);
                }
            }
        }
    }
}

/// Parses a sequence of expressions from the iterator until it hits a ']' or end of string.
fn parse_expressions(chars: &mut Peekable<Chars>) -> Vec<Expr> {
    let mut exprs = Vec::new();

    while let Some(&ch) = chars.peek() {
        if ch == ']' {
            // End of current block, let the caller consume the ']'
            break;
        }

        if ch.is_ascii_digit() {
            // 1. Parse number
            let mut k = 0;
            while let Some(&digit_char) = chars.peek() {
                if digit_char.is_ascii_digit() {
                    k = k * 10 + (digit_char as usize - '0' as usize);
                    chars.next(); // Consume digit
                } else {
                    break;
                }
            }

            // 2. Expect and consume '['
            assert_eq!(chars.next(), Some('['));

            // 3. Parse inner expressions recursively
            let inner_exprs = parse_expressions(chars);

            // 4. Expect and consume ']'
            assert_eq!(chars.next(), Some(']'));

            exprs.push(Expr::Repeat(k, inner_exprs));
        } else {
            // Parse contiguous literal string
            let mut literal = String::new();
            while let Some(&c) = chars.peek() {
                if c.is_ascii_lowercase() || c.is_ascii_uppercase() {
                    literal.push(c);
                    chars.next(); // Consume letter
                } else {
                    break;
                }
            }
            exprs.push(Expr::Literal(literal));
        }
    }

    exprs
}

/// Optimal approach: Recursive Descent Parser generating an AST.
///
/// Time: O(N * max(K)^D) - evaluating the final string still takes time proportional to output.
/// Space: O(N + `final_length`) - AST size is proportional to input length, plus output string.
///
/// This approach models the problem domain explicitly. By converting the string into
/// an `Expr` tree, we separate parsing from execution. This is how real compilers
/// and interpreters handle nested structures. It's much cleaner to test and extend
/// than the stack-based approach.
#[must_use]
#[allow(clippy::needless_pass_by_value)] // LeetCode signature
pub fn decode_string_optimal(s: String) -> String {
    let mut chars = s.chars().peekable();
    let ast = parse_expressions(&mut chars);

    // ⚡ BOLT OPTIMIZATION: We pre-allocate a capacity heuristically or just let it grow,
    // passing the buffer down to eliminate intermediate string allocations.
    let mut result = String::with_capacity(s.len());
    for expr in ast {
        expr.evaluate(&mut result);
    }
    result
}

/// Main entry point - uses optimal solution
#[must_use]
pub fn decode_string(s: String) -> String {
    decode_string_optimal(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_brute_force() {
        assert_eq!(
            decode_string_brute_force("3[a]2[bc]".to_string()),
            "aaabcbc"
        );
        assert_eq!(
            decode_string_brute_force("3[a2[c]]".to_string()),
            "accaccacc"
        );
        assert_eq!(
            decode_string_brute_force("2[abc]3[cd]ef".to_string()),
            "abcabccdcdcdef"
        );
    }

    #[test]
    fn test_optimized() {
        assert_eq!(decode_string_optimized("3[a]2[bc]".to_string()), "aaabcbc");
        assert_eq!(decode_string_optimized("3[a2[c]]".to_string()), "accaccacc");
        assert_eq!(
            decode_string_optimized("2[abc]3[cd]ef".to_string()),
            "abcabccdcdcdef"
        );
    }

    #[test]
    fn test_optimal() {
        assert_eq!(decode_string_optimal("3[a]2[bc]".to_string()), "aaabcbc");
        assert_eq!(decode_string_optimal("3[a2[c]]".to_string()), "accaccacc");
        assert_eq!(
            decode_string_optimal("2[abc]3[cd]ef".to_string()),
            "abcabccdcdcdef"
        );
    }

    #[test]
    fn test_edge_cases() {
        // No brackets
        let s1 = "abcdef".to_string();
        assert_eq!(decode_string_optimal(s1.clone()), "abcdef");
        assert_eq!(decode_string_optimized(s1.clone()), "abcdef");
        assert_eq!(decode_string_brute_force(s1), "abcdef");

        // Double digits
        let s2 = "10[a]".to_string();
        let expected = "aaaaaaaaaa";
        assert_eq!(decode_string_optimal(s2.clone()), expected);
        assert_eq!(decode_string_optimized(s2.clone()), expected);
        assert_eq!(decode_string_brute_force(s2), expected);

        // Empty string
        assert_eq!(decode_string_optimal(String::new()), "");
    }

    #[test]
    fn test_ast_parsing() {
        let mut chars = "3[a]2[bc]".chars().peekable();
        let ast = parse_expressions(&mut chars);

        let expected = vec![
            Expr::Repeat(3, vec![Expr::Literal("a".to_string())]),
            Expr::Repeat(2, vec![Expr::Literal("bc".to_string())]),
        ];

        assert_eq!(ast, expected);
    }
}
