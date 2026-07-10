//! # 8. String to Integer (atoi)
//!
//! Difficulty: Medium
//! Link: <https://leetcode.com/problems/string-to-integer-atoi/>
//!
//! Implement the `myAtoi(string s)` function, which converts a string to a 32-bit signed integer.
//!
//! The algorithm for `myAtoi(string s)` is as follows:
//! 1. Read in and ignore any leading whitespace.
//! 2. Check if the next character (if not already at the end of the string) is '-' or '+'. Read this character in if it is either. This determines if the final result is negative or positive respectively. Assume the result is positive if neither is present.
//! 3. Read in next the characters until the next non-digit character or the end of the input is reached. The rest of the string is ignored.
//! 4. Convert these digits into an integer (i.e. "123" -> 123, "0032" -> 32). If no digits were read, then the integer is 0. Change the sign as necessary (from step 2).
//! 5. If the integer is out of the 32-bit signed integer range `[-2^31, 2^31 - 1]`, then clamp the integer so that it remains in the range. Specifically, integers less than `-2^31` should be clamped to `-2^31`, and integers greater than `2^31 - 1` should be clamped to `2^31 - 1`.
//! 6. Return the integer as the final result.
//!
//! ## Why this matters in Rust
//! This problem perfectly illustrates Rust's safety guarantees when dealing with numbers. In C or C++,
//! signed integer overflow is Undefined Behavior (UB). In Rust, the compiler forces you to handle
//! potential overflows safely (e.g., using `saturating_mul` and `saturating_add`, or `checked_` variants).
//! Furthermore, parsing strings character-by-character showcases how Rust handles iterators and bytes
//! versus characters. It's also an excellent candidate for a State Machine implemented via an `enum`
//! and exhaustive `match` expressions.
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::strings::string_to_integer_atoi::my_atoi;
//!
//! assert_eq!(my_atoi("42".to_string()), 42);
//! assert_eq!(my_atoi("   -042".to_string()), -42);
//! assert_eq!(my_atoi("1337c0d3".to_string()), 1337);
//! assert_eq!(my_atoi("0-1".to_string()), 0);
//! assert_eq!(my_atoi("words and 987".to_string()), 0);
//! ```
//!
//! ## Constraints
//!
//! - `0 <= s.length <= 200`
//! - `s` consists of English letters (lower-case and upper-case), digits (`0-9`), `' '`, `'+'`, `'-'`, and `'.'`.

/// Brute Force Approach: Iterative Byte Traversal
/// Time: O(N) - We traverse the string at most once.
/// Space: O(1) - Constant extra space used for state tracking.
///
/// This approach processes the string as a sequence of bytes. Since the constraints state
/// the string consists of ASCII characters (English letters, digits, whitespace, etc.),
/// iterating over bytes (`as_bytes()`) avoids the slight overhead of full UTF-8 validation
/// on each step.
///
/// We track the state manually using mutable variables.
#[must_use]
#[allow(clippy::needless_pass_by_value)] // LeetCode signature
pub fn my_atoi_brute_force(s: String) -> i32 {
    // RUST INSIGHT: `as_bytes()` allows O(1) random access or fast sequential traversal
    // for ASCII-only strings, avoiding UTF-8 decoding overhead.
    let bytes = s.as_bytes();
    let mut i = 0;
    let n = bytes.len();

    // 1. Skip leading whitespace
    while i < n && bytes[i] == b' ' {
        i += 1;
    }

    if i == n {
        return 0;
    }

    // 2. Check sign
    let mut sign: i32 = 1;
    if bytes[i] == b'-' {
        sign = -1;
        i += 1;
    } else if bytes[i] == b'+' {
        i += 1;
    }

    // 3 & 4. Read digits and build number
    let mut result: i32 = 0;
    while i < n && bytes[i].is_ascii_digit() {
        let digit = i32::from(bytes[i] - b'0'); // Parse byte directly

        // RUST INSIGHT: `saturating_mul` and `saturating_add` are safe operations
        // that clamp at the numeric bounds (i32::MAX or i32::MIN) instead of panicking
        // or wrapping on overflow. This perfectly matches the problem's "clamp" requirement.

        // However, because we apply the sign at the end in this approach, we must be careful.
        // i32::MAX is 2147483647, and i32::MIN is -2147483648.
        // If the number is exactly 2147483648 and sign is -1, building it as a positive `i32`
        // first will overflow to `i32::MAX`, resulting in `-2147483647` instead of `-2147483648`.

        // Therefore, we must apply the sign during accumulation, or use standard checked ops.
        // Let's use `checked_mul` and `checked_add` with explicit sign handling.

        // Calculate the next step: result = result * 10 + sign * digit
        if let Some(step1) = result.checked_mul(10) {
            if let Some(step2) = step1.checked_add(sign * digit) {
                result = step2;
            } else {
                // Addition overflowed
                return if sign == 1 { i32::MAX } else { i32::MIN };
            }
        } else {
            // Multiplication overflowed
            return if sign == 1 { i32::MAX } else { i32::MIN };
        }

        i += 1;
    }

    result
}

/// Optimal Approach: State Machine (FSM)
/// Time: O(N) - We consume each character exactly once.
/// Space: O(1) - Constant extra space used for the State enum.
///
/// This approach models the parsing process as a Finite State Machine (FSM).
/// Using Rust's `enum` and exhaustive `match` ensures we handle every possible state transition,
/// making the code highly robust and preventing invalid logic paths.
#[derive(Debug)]
enum State {
    /// Initial state, consuming leading whitespace
    Start,
    /// Sign parsed (or implicit), consuming digits
    ReadingDigits,
    /// Invalid character encountered, stop processing
    Done,
}

#[must_use]
#[allow(clippy::needless_pass_by_value)] // LeetCode signature
pub fn my_atoi_optimal(s: String) -> i32 {
    let mut state = State::Start;
    let mut sign: i32 = 1;
    let mut result: i32 = 0;

    for byte in s.bytes() {
        // RUST INSIGHT: Exhaustive match. The compiler guarantees we cover all `State` variants.
        match state {
            State::Start => {
                match byte {
                    b' ' => { /* Remain in Start state */ }
                    b'-' => {
                        sign = -1;
                        state = State::ReadingDigits;
                    }
                    b'+' => {
                        state = State::ReadingDigits;
                    }
                    b'0'..=b'9' => {
                        let digit = i32::from(byte - b'0');
                        result = sign * digit;
                        state = State::ReadingDigits;
                    }
                    _ => {
                        // Any other character in Start state immediately ends parsing
                        state = State::Done;
                    }
                }
            }
            State::ReadingDigits => {
                match byte {
                    b'0'..=b'9' => {
                        let digit = i32::from(byte - b'0');

                        // GOTCHA: Overflow logic. We must accumulate carefully.
                        // Same overflow logic as Iterative approach.
                        if let Some(step1) = result.checked_mul(10) {
                            if let Some(step2) = step1.checked_add(sign * digit) {
                                result = step2;
                            } else {
                                result = if sign == 1 { i32::MAX } else { i32::MIN };
                                state = State::Done; // Stop processing further once saturated
                            }
                        } else {
                            result = if sign == 1 { i32::MAX } else { i32::MIN };
                            state = State::Done; // Stop processing further
                        }
                    }
                    _ => {
                        // Any non-digit character while reading digits ends parsing
                        state = State::Done;
                    }
                }
            }
            State::Done => {
                break; // Exit the loop entirely
            }
        }
    }

    result
}

/// Main entry point - uses the optimal State Machine approach for its idiomatic Rust safety
/// and explicitness.
#[must_use]
pub fn my_atoi(s: String) -> i32 {
    my_atoi_optimal(s)
}

// =========================================================================================
// Alternative Approaches
// =========================================================================================
// 1. **Peekable Iterator**: Using `s.chars().peekable()` allows looking ahead before consuming.
//    While elegant, it is slightly slower than direct byte traversal and overkill for this
//    linear parsing problem.
// 2. **Regex**: A regex like `^\s*([+-]?\d+)` could extract the numeric portion. However,
//    Regex engines are significantly slower and don't natively handle the 32-bit clamping
//    requirement gracefully during parsing, requiring a secondary parsing step anyway.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_positive_number() {
        assert_eq!(my_atoi_brute_force("42".to_string()), 42);
        assert_eq!(my_atoi_optimal("42".to_string()), 42);
        assert_eq!(my_atoi("42".to_string()), 42);
    }

    #[test]
    fn test_negative_with_whitespace() {
        assert_eq!(my_atoi_brute_force("   -042".to_string()), -42);
        assert_eq!(my_atoi_optimal("   -042".to_string()), -42);
        assert_eq!(my_atoi("   -042".to_string()), -42);
    }

    #[test]
    fn test_trailing_characters() {
        assert_eq!(my_atoi_brute_force("1337c0d3".to_string()), 1337);
        assert_eq!(my_atoi_optimal("1337c0d3".to_string()), 1337);
        assert_eq!(my_atoi("1337c0d3".to_string()), 1337);
    }

    #[test]
    fn test_zero_minus_one() {
        assert_eq!(my_atoi_brute_force("0-1".to_string()), 0);
        assert_eq!(my_atoi_optimal("0-1".to_string()), 0);
        assert_eq!(my_atoi("0-1".to_string()), 0);
    }

    #[test]
    fn test_leading_words() {
        assert_eq!(my_atoi_brute_force("words and 987".to_string()), 0);
        assert_eq!(my_atoi_optimal("words and 987".to_string()), 0);
        assert_eq!(my_atoi("words and 987".to_string()), 0);
    }

    #[test]
    fn test_empty_string() {
        assert_eq!(my_atoi_brute_force("".to_string()), 0);
        assert_eq!(my_atoi_optimal("".to_string()), 0);
        assert_eq!(my_atoi("".to_string()), 0);
    }

    #[test]
    fn test_just_sign() {
        assert_eq!(my_atoi_brute_force("-".to_string()), 0);
        assert_eq!(my_atoi_optimal("-".to_string()), 0);
        assert_eq!(my_atoi("-".to_string()), 0);

        assert_eq!(my_atoi_brute_force("+".to_string()), 0);
        assert_eq!(my_atoi_optimal("+".to_string()), 0);
        assert_eq!(my_atoi("+".to_string()), 0);
    }

    #[test]
    fn test_overflow_positive() {
        // i32::MAX is 2147483647
        assert_eq!(my_atoi_brute_force("2147483648".to_string()), i32::MAX);
        assert_eq!(my_atoi_optimal("2147483648".to_string()), i32::MAX);
        assert_eq!(my_atoi("2147483648".to_string()), i32::MAX);

        assert_eq!(my_atoi_brute_force("9999999999".to_string()), i32::MAX);
        assert_eq!(my_atoi_optimal("9999999999".to_string()), i32::MAX);
        assert_eq!(my_atoi("9999999999".to_string()), i32::MAX);
    }

    #[test]
    fn test_overflow_negative() {
        // i32::MIN is -2147483648
        assert_eq!(my_atoi_brute_force("-2147483648".to_string()), i32::MIN);
        assert_eq!(my_atoi_optimal("-2147483648".to_string()), i32::MIN);
        assert_eq!(my_atoi("-2147483648".to_string()), i32::MIN);

        assert_eq!(my_atoi_brute_force("-2147483649".to_string()), i32::MIN);
        assert_eq!(my_atoi_optimal("-2147483649".to_string()), i32::MIN);
        assert_eq!(my_atoi("-2147483649".to_string()), i32::MIN);

        assert_eq!(my_atoi_brute_force("-9999999999".to_string()), i32::MIN);
        assert_eq!(my_atoi_optimal("-9999999999".to_string()), i32::MIN);
        assert_eq!(my_atoi("-9999999999".to_string()), i32::MIN);
    }

    #[test]
    fn test_multiple_signs() {
        assert_eq!(my_atoi_brute_force("+-12".to_string()), 0);
        assert_eq!(my_atoi_optimal("+-12".to_string()), 0);
        assert_eq!(my_atoi("+-12".to_string()), 0);

        assert_eq!(my_atoi_brute_force("-+12".to_string()), 0);
        assert_eq!(my_atoi_optimal("-+12".to_string()), 0);
        assert_eq!(my_atoi("-+12".to_string()), 0);
    }

    #[test]
    fn test_whitespace_between_sign_and_digits() {
        assert_eq!(my_atoi_brute_force("- 12".to_string()), 0);
        assert_eq!(my_atoi_optimal("- 12".to_string()), 0);
        assert_eq!(my_atoi("- 12".to_string()), 0);
    }
}
