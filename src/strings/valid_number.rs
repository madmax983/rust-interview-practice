//! # 65. Valid Number
//!
//! Difficulty: Hard
//! Link: <https://leetcode.com/problems/valid-number/>
//!
//! A valid number can be split up into these components (in order):
//! 1. A decimal number or an integer.
//! 2. (Optional) An `'e'` or `'E'`, followed by an integer.
//!
//! A decimal number can be split up into:
//! 1. (Optional) A sign character (either `'+'` or `'-'`).
//! 2. One of the following formats:
//!    - One or more digits, followed by a dot `'.'`.
//!    - One or more digits, followed by a dot `'.'`, followed by one or more digits.
//!    - A dot `'.'`, followed by one or more digits.
//!
//! An integer can be split up into:
//! 1. (Optional) A sign character (either `'+'` or `'-'`).
//! 2. One or more digits.
//!
//! ## Why This Matters in Rust
//!
//! This problem perfectly demonstrates why Rust's enums and exhaustive pattern matching
//! are ideal for building safe, maintainable state machines. In C++ or Java, state machines
//! often rely on loose integer states and unverified `switch`/`case` fallthroughs. In Rust,
//! we can represent transitions using `match` on strongly typed `Enum` states and input classes.
//! The compiler ensures that every possible transition is explicitly handled, entirely eliminating
//! a whole class of subtle state machine bugs.
//!
//! ## Approach
//!
//! We provide three approaches:
//! 1. **Brute Force (`is_number_brute_force`)**: Iterative string checking, keeping track of seen
//!    dots, exponents, and digits. While functional, it leads to deep branching logic.
//! 2. **Optimized (`is_number_optimized`)**: A simplified iterative parser utilizing Rust's iterator
//!    combinators, breaking the input down to segments.
//! 3. **Optimal (`is_number_optimal`)**: A Deterministic Finite Automaton (DFA) / State Machine.
//!    This is the production-grade approach, modeling states as an `enum` and input characters as `CharType`,
//!    allowing us to encode valid transitions systematically and safely.

/// Brute Force Approach: Iterative boolean flags
///
/// Time: O(N) where N is the length of the string
/// Space: O(N) for the `Vec<char>` we collect once up front
#[must_use]
#[allow(clippy::needless_pass_by_value)] // LeetCode signature
pub fn is_number_brute_force(s: String) -> bool {
    let mut seen_digit = false;
    let mut seen_exponent = false;
    let mut seen_dot = false;

    // GOTCHA: Collect the chars ONCE so we can index the previous char in O(1).
    // Calling `s.chars().nth(i - 1)` inside the loop would be O(N) per call,
    // making the whole "brute force" secretly O(N^2). Collecting keeps it honestly O(N).
    let chars: Vec<char> = s.chars().collect();

    // We process the string char-by-char
    for (i, &c) in chars.iter().enumerate() {
        match c {
            '0'..='9' => {
                seen_digit = true;
            }
            '+' | '-' => {
                // A sign is only valid at the very beginning or right after an exponent
                if i > 0 && chars[i - 1] != 'e' && chars[i - 1] != 'E' {
                    return false;
                }
            }
            '.' => {
                // A dot is only valid if we haven't seen an exponent or another dot
                if seen_dot || seen_exponent {
                    return false;
                }
                seen_dot = true;
            }
            'e' | 'E' => {
                // An exponent is only valid if we've seen at least one digit and haven't seen an exponent yet
                if seen_exponent || !seen_digit {
                    return false;
                }
                seen_exponent = true;
                // We need more digits after the exponent
                seen_digit = false;
            }
            _ => {
                // Any other character makes it invalid
                return false;
            }
        }
    }

    // Must have seen at least one valid digit to form a number
    seen_digit
}

/// Optimized Approach: Iterative byte processing
///
/// Time: O(N)
/// Space: O(1)
///
/// Same logic as brute force, but utilizing `.bytes()` which is faster and more idiomatic
/// than calling `nth()` on a `Chars` iterator which takes O(N) time per call.
#[must_use]
#[allow(clippy::needless_pass_by_value)] // LeetCode signature
pub fn is_number_optimized(s: String) -> bool {
    let mut seen_digit = false;
    let mut seen_exponent = false;
    let mut seen_dot = false;

    let bytes = s.as_bytes();

    // RUST INSIGHT: Iterating over bytes instead of chars is safe here because all valid
    // characters (+, -, ., e, E, 0-9) are single-byte ASCII characters. This avoids UTF-8
    // decoding overhead.
    for (i, &b) in bytes.iter().enumerate() {
        match b {
            b'0'..=b'9' => {
                seen_digit = true;
            }
            b'+' | b'-' => {
                if i > 0 && bytes[i - 1] != b'e' && bytes[i - 1] != b'E' {
                    return false;
                }
            }
            b'.' => {
                if seen_dot || seen_exponent {
                    return false;
                }
                seen_dot = true;
            }
            b'e' | b'E' => {
                if seen_exponent || !seen_digit {
                    return false;
                }
                seen_exponent = true;
                seen_digit = false; // Reset because we need digits after exponent
            }
            _ => return false, // Invalid character
        }
    }

    seen_digit
}

/// The Character classes for the State Machine
enum CharType {
    Digit,
    Sign,
    Dot,
    Exponent,
    Invalid,
}

impl CharType {
    const fn from_byte(b: u8) -> Self {
        match b {
            b'0'..=b'9' => Self::Digit,
            b'+' | b'-' => Self::Sign,
            b'.' => Self::Dot,
            b'e' | b'E' => Self::Exponent,
            _ => Self::Invalid,
        }
    }
}

/// The States for the State Machine
#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    Start,
    Sign,
    Integer,
    Dot,          // E.g., "3." or "."
    EmptyDot,     // A dot with no preceding integer
    Fraction,     // Decimal part
    Exponent,     // 'e' or 'E'
    ExponentSign, // sign after 'e' or 'E'
    ExponentInt,  // Digits after exponent
    Invalid,
}

impl State {
    /// Progresses the State Machine based on the input character class.
    ///
    /// // RUST INSIGHT: Exhaustive pattern matching ensures we never miss a state-input
    /// // combination. The compiler enforces that every possible state transition is handled.
    const fn transition(self, char_type: CharType) -> Self {
        // Arms are grouped by their resulting state; or-patterns merge the source
        // states that share a transition so no two arms have identical bodies.
        match (self, char_type) {
            (Self::Start, CharType::Sign) => Self::Sign,

            // A digit while reading the integer part.
            (Self::Start | Self::Sign | Self::Integer, CharType::Digit) => Self::Integer,

            // A dot with no integer preceding it.
            (Self::Start | Self::Sign, CharType::Dot) => Self::EmptyDot,

            // A dot after an integer.
            (Self::Integer, CharType::Dot) => Self::Dot,

            // Entering the exponent from any valid mantissa state.
            (Self::Integer | Self::Dot | Self::Fraction, CharType::Exponent) => Self::Exponent,

            // A digit forming the fractional part.
            (Self::Dot | Self::EmptyDot | Self::Fraction, CharType::Digit) => Self::Fraction,

            // A sign directly after the exponent marker.
            (Self::Exponent, CharType::Sign) => Self::ExponentSign,

            // Digits after the exponent (and its optional sign).
            (Self::Exponent | Self::ExponentSign | Self::ExponentInt, CharType::Digit) => {
                Self::ExponentInt
            }

            // Any other input moves us to an invalid state, or keeps us there
            _ => Self::Invalid,
        }
    }

    /// Determines if the current state is considered a final valid state for the entire string.
    const fn is_valid_end(self) -> bool {
        matches!(
            self,
            Self::Integer | Self::Dot | Self::Fraction | Self::ExponentInt
        )
    }
}

/// Optimal Approach: State Machine (DFA)
///
/// Time: O(N)
/// Space: O(1)
///
/// Models the input as a stream of tokens that mutate a state machine. This approach
/// completely eliminates the deep nesting of flags and avoids subtle bugs in edge cases.
#[must_use]
#[allow(clippy::needless_pass_by_value)] // LeetCode signature
pub fn is_number_optimal(s: String) -> bool {
    // GOTCHA: `fold` is perfectly idiomatic here, reducing the byte slice into a final state.
    // If the state becomes Invalid early on, it stays Invalid due to the wildcard match arm
    // in `State::transition`.
    let final_state = s.bytes().fold(State::Start, |current_state, b| {
        current_state.transition(CharType::from_byte(b))
    });

    final_state.is_valid_end()
}

/// Main entry point - uses optimal solution
#[must_use]
pub fn is_number(s: String) -> bool {
    is_number_optimal(s)
}

/// ## Alternative Approaches
///
/// - **Regular Expressions**: While the problem can be solved using a Regex matching the specific
///   pattern of a number, doing so usually involves compiling a Regex object at runtime (which has
///   overhead) and obscures the fundamental parsing rules.
/// - **Recursive Descent Parsing**: One could formulate the problem as a grammar and build a small
///   recursive descent parser. This is generally overkill for a simple set of rules and regular
///   grammar that can be handled by a basic DFA.
#[cfg(test)]
mod tests {
    use super::*;

    // Valid number examples from LeetCode
    const VALID_CASES: &[&str] = &[
        "2",
        "0089",
        "-0.1",
        "+3.14",
        "4.",
        "-.9",
        "2e10",
        "-90E3",
        "3e+7",
        "+6e-1",
        "53.5e93",
        "-123.456e789",
    ];

    // Invalid number examples from LeetCode
    const INVALID_CASES: &[&str] = &[
        "abc", "1a", "1e", "e3", "99e2.5", "--6", "-+3", "95a54e53", ".", "..", "+.", "e",
    ];

    #[test]
    fn test_brute_force_valid() {
        for &s in VALID_CASES {
            assert!(
                is_number_brute_force(s.to_string()),
                "Brute force failed for valid case: {s}"
            );
        }
    }

    #[test]
    fn test_brute_force_invalid() {
        for &s in INVALID_CASES {
            assert!(
                !is_number_brute_force(s.to_string()),
                "Brute force failed for invalid case: {s}"
            );
        }
    }

    #[test]
    fn test_optimized_valid() {
        for &s in VALID_CASES {
            assert!(
                is_number_optimized(s.to_string()),
                "Optimized failed for valid case: {s}"
            );
        }
    }

    #[test]
    fn test_optimized_invalid() {
        for &s in INVALID_CASES {
            assert!(
                !is_number_optimized(s.to_string()),
                "Optimized failed for invalid case: {s}"
            );
        }
    }

    #[test]
    fn test_optimal_valid() {
        for &s in VALID_CASES {
            assert!(
                is_number_optimal(s.to_string()),
                "Optimal failed for valid case: {s}"
            );
        }
    }

    #[test]
    fn test_optimal_invalid() {
        for &s in INVALID_CASES {
            assert!(
                !is_number_optimal(s.to_string()),
                "Optimal failed for invalid case: {s}"
            );
        }
    }

    #[test]
    fn test_all_approaches_agree() {
        // Cross-implementation verification: all three approaches must produce
        // identical results for every input.
        for &s in VALID_CASES.iter().chain(INVALID_CASES.iter()) {
            let brute = is_number_brute_force(s.to_string());
            let optimized = is_number_optimized(s.to_string());
            let optimal = is_number_optimal(s.to_string());
            assert_eq!(brute, optimized, "brute vs optimized disagree on: {s}");
            assert_eq!(optimized, optimal, "optimized vs optimal disagree on: {s}");
        }
    }
}
