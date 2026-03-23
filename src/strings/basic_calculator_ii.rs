//! # 227. Basic Calculator II
//!
//! Given a string `s` which represents an expression, evaluate this expression and return its value.
//!
//! The integer division should truncate toward zero.
//!
//! You may assume that the given expression is always valid. All intermediate results will be in the range of `[-2^31, 2^31 - 1]`.
//!
//! - Difficulty: Medium
//! - LeetCode: <https://leetcode.com/problems/basic-calculator-ii/>
//!
//! ## Why this matters in Rust
//! This problem perfectly illustrates the power of Rust's iterators, enums, and pattern matching.
//! Parsing strings and evaluating expressions is a common task, and Rust allows us to build a robust,
//! zero-allocation parser by representing tokens as enums and evaluating them in a single pass.
//! It showcases how to move from a string-heavy, allocating brute-force approach to a clean,
//! state-machine-like optimal solution using `Iterator`.
//!
//! ## Approach
//!
//! We explore three implementations:
//! 1.  **Brute Force**: Involves cleaning the string, splitting it, and evaluating using multiple passes and heap allocations. O(n) space and time, but high constant factors.
//! 2.  **Optimized**: Uses a `Vec` as a stack to keep track of numbers to be added later. We iterate through the string's bytes, processing numbers and operators on the fly. O(n) space and time.
//! 3.  **Optimal**: Evaluates the expression in a single pass with O(1) space. We maintain the `current_number`, `last_number`, and `result` to compute everything without a stack. We also build a custom `Lexer` iterator returning an `enum Token` to cleanly separate parsing from evaluation.

/// Brute Force Approach: String Allocations and Multiple Passes
///
/// This approach cleans up the string (removes spaces), then iterates through
/// keeping track of the current number and the last seen operator.
/// It uses a stack (`Vec<i32>`) to store numbers before finally summing them.
///
/// Time: O(n) - Iterates through the string multiple times (replace, iterate).
/// Space: O(n) - For the stack and the intermediate cleaned string.
///
/// # GOTCHA
/// Using `.replace(" ", "")` creates a whole new `String` allocation. While it makes
/// parsing easier, it's inefficient. Idiomatic Rust avoids unnecessary allocations
/// by parsing strings in-place (often using byte slices `&[u8]` for ASCII).
#[must_use]
#[allow(clippy::needless_pass_by_value)]
#[allow(clippy::cast_possible_truncation)]
#[allow(clippy::cast_possible_wrap)]
pub fn calculate_brute_force(s: String) -> i32 {
    let clean_s = s.replace(' ', "");
    let mut stack: Vec<i32> = Vec::new();
    let mut current_num = 0;
    let mut operator = '+';

    for (i, c) in clean_s.chars().enumerate() {
        if c.is_ascii_digit() {
            current_num = current_num * 10 + c.to_digit(10).unwrap() as i32;
        }

        if !c.is_ascii_digit() || i == clean_s.len() - 1 {
            match operator {
                '+' => stack.push(current_num),
                '-' => stack.push(-current_num),
                '*' => {
                    if let Some(last) = stack.last_mut() {
                        *last *= current_num;
                    }
                }
                '/' => {
                    if let Some(last) = stack.last_mut() {
                        *last /= current_num;
                    }
                }
                _ => {}
            }
            operator = c;
            current_num = 0;
        }
    }

    stack.iter().sum()
}

/// Optimized Approach: Single Pass with Stack
///
/// We iterate over the bytes of the string. We build the `current_num` digit by digit.
/// When we hit an operator (or the end of the string), we evaluate the *previous*
/// operator with the *current* number and update the stack.
///
/// Time: O(n) - Single pass through the string.
/// Space: O(n) - For the stack (`Vec<i32>`).
///
/// # RUST INSIGHT
/// `s.bytes()` is an O(1) iterator over the raw bytes of the string. Since the problem
/// guarantees the string only contains digits, operators, and spaces (all valid ASCII),
/// byte-level iteration is perfectly safe and much faster than `chars()` which decodes UTF-8.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn calculate_optimized(s: String) -> i32 {
    let mut stack = Vec::new();
    let mut current_num = 0;
    let mut operator = b'+';
    let bytes = s.as_bytes();
    let n = bytes.len();

    for i in 0..n {
        let b = bytes[i];

        if b.is_ascii_digit() {
            current_num = current_num * 10 + i32::from(b - b'0');
        }

        // RUST INSIGHT: We check `!b.is_ascii_whitespace()` to skip spaces,
        // and we must also trigger evaluation on the very last byte `i == n - 1`.
        if (!b.is_ascii_whitespace() && !b.is_ascii_digit()) || i == n - 1 {
            match operator {
                b'+' => stack.push(current_num),
                b'-' => stack.push(-current_num),
                b'*' => {
                    if let Some(last) = stack.last_mut() {
                        *last *= current_num;
                    }
                }
                b'/' => {
                    if let Some(last) = stack.last_mut() {
                        *last /= current_num;
                    }
                }
                _ => unreachable!(),
            }
            operator = b;
            current_num = 0;
        }
    }

    stack.into_iter().sum()
}

/// Token enum representing lexical elements of our expression.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Token {
    Number(i32),
    Add,
    Subtract,
    Multiply,
    Divide,
}

/// A custom Lexer that implements `Iterator`.
/// This converts a raw string into a stream of `Token`s,
/// ignoring whitespace and properly grouping digits into numbers.
pub struct Lexer<'a> {
    bytes: std::slice::Iter<'a, u8>,
    peeked: Option<u8>,
}

impl<'a> Lexer<'a> {
    pub fn new(s: &'a str) -> Self {
        let mut bytes = s.as_bytes().iter();
        let peeked = bytes.next().copied();
        Self { bytes, peeked }
    }

    fn advance(&mut self) {
        self.peeked = self.bytes.next().copied();
    }
}

impl<'a> Iterator for Lexer<'a> {
    type Item = Token;

    fn next(&mut self) -> Option<Self::Item> {
        // Skip whitespace
        while let Some(b) = self.peeked {
            if b.is_ascii_whitespace() {
                self.advance();
            } else {
                break;
            }
        }

        let b = self.peeked?;

        if b.is_ascii_digit() {
            let mut num = 0;
            while let Some(d) = self.peeked {
                if d.is_ascii_digit() {
                    num = num * 10 + i32::from(d - b'0');
                    self.advance();
                } else {
                    break;
                }
            }
            return Some(Token::Number(num));
        }

        let token = match b {
            b'+' => Token::Add,
            b'-' => Token::Subtract,
            b'*' => Token::Multiply,
            b'/' => Token::Divide,
            _ => return None, // Invalid token
        };

        self.advance();
        Some(token)
    }
}

/// Optimal Approach: O(1) Space with Custom Lexer Iterator
///
/// We eliminate the stack entirely. We only need to keep track of the `result` computed so far,
/// and the `last_number` (the result of the most recent * or / operation, or just the number itself for +/-).
/// When we see a new +/- operator, we can add `last_number` to `result`.
/// We use our custom `Lexer` to cleanly separate parsing logic from evaluation logic.
///
/// Time: O(n) - Single pass through the string via the lexer.
/// Space: O(1) - No stack allocation, constant space state tracking.
///
/// # RUST INSIGHT
/// Building a custom `Iterator` (like `Lexer`) is a very idiomatic Rust pattern.
/// It encapsulates the complex string parsing state machine into a clean, testable stream of tokens,
/// making the actual evaluation loop extremely readable.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn calculate_optimal(s: String) -> i32 {
    let lexer = Lexer::new(&s);

    let mut result = 0;
    let mut last_number = 0;
    let mut current_op = Token::Add; // Default starting operation is addition

    for token in lexer {
        match token {
            Token::Number(num) => {
                match current_op {
                    Token::Add => {
                        result += last_number;
                        last_number = num;
                    }
                    Token::Subtract => {
                        result += last_number;
                        last_number = -num;
                    }
                    Token::Multiply => {
                        last_number *= num;
                    }
                    Token::Divide => {
                        last_number /= num;
                    }
                    _ => unreachable!(),
                }
            }
            op => current_op = op,
        }
    }

    result + last_number
}

/// Main entry point
#[must_use]
pub fn calculate(s: String) -> i32 {
    calculate_optimal(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_brute_force_examples() {
        assert_eq!(calculate_brute_force("3+2*2".to_string()), 7);
        assert_eq!(calculate_brute_force(" 3/2 ".to_string()), 1);
        assert_eq!(calculate_brute_force(" 3+5 / 2 ".to_string()), 5);
    }

    #[test]
    fn test_optimized_examples() {
        assert_eq!(calculate_optimized("3+2*2".to_string()), 7);
        assert_eq!(calculate_optimized(" 3/2 ".to_string()), 1);
        assert_eq!(calculate_optimized(" 3+5 / 2 ".to_string()), 5);
    }

    #[test]
    fn test_optimal_examples() {
        assert_eq!(calculate_optimal("3+2*2".to_string()), 7);
        assert_eq!(calculate_optimal(" 3/2 ".to_string()), 1);
        assert_eq!(calculate_optimal(" 3+5 / 2 ".to_string()), 5);
    }

    #[test]
    fn test_lexer() {
        let mut lexer = Lexer::new(" 12 + 34 * 56 ");
        assert_eq!(lexer.next(), Some(Token::Number(12)));
        assert_eq!(lexer.next(), Some(Token::Add));
        assert_eq!(lexer.next(), Some(Token::Number(34)));
        assert_eq!(lexer.next(), Some(Token::Multiply));
        assert_eq!(lexer.next(), Some(Token::Number(56)));
        assert_eq!(lexer.next(), None);
    }

    #[test]
    fn test_all_approaches_edge_cases() {
        let cases = vec![
            ("42", 42),
            ("1-1+1", 1),
            ("2*3*4", 24),
            ("14-3/2", 13),
            ("0", 0),
        ];

        for (s, expected) in cases {
            assert_eq!(calculate_brute_force(s.to_string()), expected, "Brute force failed for {}", s);
            assert_eq!(calculate_optimized(s.to_string()), expected, "Optimized failed for {}", s);
            assert_eq!(calculate_optimal(s.to_string()), expected, "Optimal failed for {}", s);
        }
    }
}
