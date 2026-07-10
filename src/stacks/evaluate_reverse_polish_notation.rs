//! # 150. Evaluate Reverse Polish Notation
//!
//! Evaluate the value of an arithmetic expression in Reverse Polish Notation (RPN).
//!
//! Valid operators are `+`, `-`, `*`, and `/`. Each operand may be an integer or another expression.
//! The division between two integers should truncate toward zero.
//!
//! It is guaranteed that the given RPN expression is always valid. That means the expression would
//! always evaluate to a result, and there will not be any division by zero operation.
//!
//! [LeetCode Problem 150](https://leetcode.com/problems/evaluate-reverse-polish-notation/)
//!
//! ## Why This Matters in Rust
//!
//! This problem is an excellent showcase for:
//!
//! 1.  **Enums and Pattern Matching**: Instead of working with raw strings, we can parse tokens into a
//!     strongly-typed `Token` enum. This makes the evaluation logic robust and compiler-checked.
//! 2.  **Stack Operations**: RPN is the canonical stack problem. Rust's `Vec` serves as an efficient stack.
//! 3.  **Iterators vs Loops**: We can contrast a traditional imperative loop with a functional `fold` approach.
//! 4.  **Error Handling**: By using `Result`, we can propagate parsing or stack underflow errors gracefully,
//!     rather than panicking (though LeetCode guarantees valid input, real-world parsers shouldn't).
//!
//! ## Approach
//!
//! RPN (Postfix Notation) eliminates the need for parentheses by placing the operator *after* its operands.
//! The algorithm is:
//! 1.  Iterate through tokens.
//! 2.  If operand (number): Push to stack.
//! 3.  If operator: Pop two numbers, apply operator, push result back.
//! 4.  Final result is the single item remaining on the stack.

use std::str::FromStr;

/// Represents a parsed token in the RPN expression.
///
/// Using an enum here allows us to separate the *parsing* logic from the *evaluation* logic.
/// It also leverages Rust's type system to ensure we only handle valid operations.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Token {
    Number(i32),
    Operator(Op),
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Op {
    Add,
    Sub,
    Mul,
    Div,
}

impl Op {
    /// Applies the operator to two operands.
    /// Note the order: `b` was popped first (top of stack), so it is the right-hand operand.
    fn apply(self, a: i32, b: i32) -> i32 {
        match self {
            Op::Add => a + b,
            Op::Sub => a - b,
            Op::Mul => a * b,
            // RUST INSIGHT: Integer division in Rust truncates toward zero,
            // matching the problem requirement and C/C++ behavior.
            Op::Div => a / b,
        }
    }
}

/// Parses a string slice into a Token.
///
/// This is idiomatic Rust: implement `FromStr` so we can use `s.parse::<Token>()`.
impl FromStr for Token {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "+" => Ok(Token::Operator(Op::Add)),
            "-" => Ok(Token::Operator(Op::Sub)),
            "*" => Ok(Token::Operator(Op::Mul)),
            "/" => Ok(Token::Operator(Op::Div)),
            _ => s
                .parse::<i32>()
                .map(Token::Number)
                .map_err(|_| format!("Invalid token: {}", s)),
        }
    }
}

/// Brute force approach: Functional `try_fold`
///
/// Technique: this solution treats the evaluation as a folding operation over the tokens, where the
/// state being folded is the stack of numbers. It is labelled `_brute_force` per the repo's
/// naming convention as the alternative implementation; its complexity is identical to the optimal
/// loop, and it additionally returns a `Result` so parsing/underflow errors propagate instead of panicking.
///
/// Time Complexity: O(N)
/// Space Complexity: O(N)
#[allow(clippy::needless_pass_by_value)]
pub fn eval_rpn_brute_force(tokens: Vec<String>) -> Result<i32, String> {
    // BOLT OPTIMIZATION: Pre-allocate capacity for the stack.
    // In valid RPN, the maximum number of elements on the stack at any time
    // is (N / 2) + 1, where N is the number of tokens. Pre-allocating this
    // prevents dynamic heap reallocations during evaluation.
    let capacity = (tokens.len() / 2) + 1;
    let final_stack = tokens
        .iter()
        .map(|s| s.parse::<Token>()) // Parse strings to Tokens lazily
        .try_fold(Vec::with_capacity(capacity), |mut stack, token_result| {
            // Propagate parsing errors immediately
            let token = token_result?;

            match token {
                Token::Number(n) => stack.push(n),
                Token::Operator(op) => {
                    // GOTCHA: We need two operands. If the stack is empty, the expression is invalid.
                    // Also, strict popping order matters for non-commutative ops like - and /.
                    let b = stack.pop().ok_or("Stack underflow")?;
                    let a = stack.pop().ok_or("Stack underflow")?;
                    stack.push(op.apply(a, b));
                }
            }
            Ok::<Vec<i32>, String>(stack)
        })?;

    // The result should be the only element left.
    final_stack
        .last()
        .copied()
        .ok_or_else(|| "Empty expression".to_string())
}

/// Optimal approach: Iterative standard loop
///
/// Technique: a standard imperative loop. This is often more readable for stack manipulations because
/// `pop()` operations don't require passing the stack ownership in and out of a closure. This is the
/// canonical implementation the main entry point dispatches to.
///
/// Time Complexity: O(N)
/// Space Complexity: O(N)
#[allow(clippy::needless_pass_by_value)]
pub fn eval_rpn_optimal(tokens: Vec<String>) -> i32 {
    // BOLT OPTIMIZATION: Pre-allocate capacity for the stack.
    // In valid RPN, the maximum number of elements on the stack at any time
    // is (N / 2) + 1, where N is the number of tokens. Pre-allocating this
    // prevents dynamic heap reallocations during evaluation.
    let mut stack = Vec::with_capacity((tokens.len() / 2) + 1);

    for token_str in tokens {
        // We can parse directly here or use the Enum. using the Enum strictly is cleaner.
        let token = token_str.parse::<Token>().expect("Invalid token input");

        match token {
            Token::Number(n) => stack.push(n),
            Token::Operator(op) => {
                // We unwrap here because the problem guarantees valid RPN.
                // In production code, we would return Result (like above).
                let b = stack.pop().expect("Invalid RPN: Stack underflow");
                let a = stack.pop().expect("Invalid RPN: Stack underflow");
                stack.push(op.apply(a, b));
            }
        }
    }

    stack.pop().expect("Invalid RPN: Empty result")
}

/// Main entry point - uses optimal solution
#[must_use]
pub fn eval_rpn(tokens: Vec<String>) -> i32 {
    // We default to the iterative solution as it's the standard implementation for this problem,
    // but the functional one (`eval_rpn_brute_force`) is available for educational comparison.
    eval_rpn_optimal(tokens)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn to_string_vec(v: Vec<&str>) -> Vec<String> {
        v.into_iter().map(String::from).collect()
    }

    #[test]
    fn test_simple_addition() {
        let tokens = to_string_vec(vec!["2", "1", "+", "3", "*"]);
        // (2 + 1) * 3 = 9
        assert_eq!(eval_rpn(tokens.clone()), 9);
        assert_eq!(eval_rpn_brute_force(tokens).unwrap(), 9);
    }

    #[test]
    fn test_division_and_addition() {
        let tokens = to_string_vec(vec!["4", "13", "5", "/", "+"]);
        // 4 + (13 / 5) = 4 + 2 = 6
        assert_eq!(eval_rpn(tokens.clone()), 6);
        assert_eq!(eval_rpn_brute_force(tokens).unwrap(), 6);
    }

    #[test]
    fn test_complex_expression() {
        let tokens = to_string_vec(vec![
            "10", "6", "9", "3", "+", "-11", "*", "/", "*", "17", "+", "5", "+",
        ]);
        // ((10 * (6 / ((9 + 3) * -11))) + 17) + 5
        // = ((10 * (6 / (12 * -11))) + 17) + 5
        // = ((10 * (6 / -132)) + 17) + 5
        // = ((10 * 0) + 17) + 5  (6 / -132 truncates to 0)
        // = (0 + 17) + 5
        // = 22
        assert_eq!(eval_rpn(tokens.clone()), 22);
        assert_eq!(eval_rpn_brute_force(tokens).unwrap(), 22);
    }

    #[test]
    fn test_negative_numbers() {
        let tokens = to_string_vec(vec!["3", "-4", "+"]);
        assert_eq!(eval_rpn(tokens.clone()), -1);
    }

    #[test]
    fn test_single_element() {
        let tokens = to_string_vec(vec!["42"]);
        assert_eq!(eval_rpn(tokens), 42);
    }

    #[test]
    fn test_functional_error_handling() {
        // Invalid token
        let tokens = to_string_vec(vec!["2", "foo", "+"]);
        assert!(eval_rpn_brute_force(tokens).is_err());

        // Stack underflow
        let tokens_underflow = to_string_vec(vec!["1", "+"]);
        assert!(eval_rpn_brute_force(tokens_underflow).is_err());
    }

    #[test]
    fn test_all_approaches_agree() {
        let cases = vec![
            vec!["42"],
            vec!["2", "1", "+", "3", "*"],
            vec!["4", "13", "5", "/", "+"],
            vec!["3", "-4", "+"],
            vec![
                "10", "6", "9", "3", "+", "-11", "*", "/", "*", "17", "+", "5", "+",
            ],
        ];

        for case in cases {
            let tokens = to_string_vec(case);
            let optimal = eval_rpn_optimal(tokens.clone());
            let entry = eval_rpn(tokens.clone());
            let brute = eval_rpn_brute_force(tokens).unwrap();
            assert_eq!(optimal, brute);
            assert_eq!(entry, optimal);
        }
    }
}
