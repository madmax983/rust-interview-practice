//! # 20. Valid Parentheses
//!
//! Difficulty: Easy
//! Link: <https://leetcode.com/problems/valid-parentheses/>
//!
//! Given a string `s` containing just the characters `'('`, `')'`, `'{'`, `'}'`, `'['` and `']'`,
//! determine if the input string is valid.
//!
//! An input string is valid if:
//! 1. Open brackets must be closed by the same type of brackets.
//! 2. Open brackets must be closed in the correct order.
//! 3. Every close bracket has a corresponding open bracket of the same type.
//!
//! ## Why This Matters in Rust
//!
//! This problem perfectly illustrates Rust's `Vec` as an efficient stack, the use of `match` statements
//! for clean and exhaustive control flow, and how to avoid unnecessary UTF-8 validation by working directly
//! with byte slices (`&[u8]`). It also highlights how Rust's `Option` methods like `pop()` combined
//! with pattern matching (`Some`) allow for safe and expressive error handling when reading from a stack.
//!
//! ## Approach
//!
//! The standard approach is to iterate through each character of the string.
//! If it's an opening bracket, push it onto the stack.
//! If it's a closing bracket, pop the top of the stack and check if it matches the expected opening bracket.
//! If there's a mismatch or the stack is empty when encountering a closing bracket, the string is invalid.
//! At the end, if the stack is empty, the string is valid.
//!
//! We provide three implementations:
//! 1. **Iterative (`is_valid_iterative`)**: A standard loop over `chars()`, useful for understanding the core logic.
//! 2. **Optimal (`is_valid_optimal`)**: Working on `as_bytes()` to skip UTF-8 overhead, giving C-like performance with Rust safety.
//! 3. **Functional (`is_valid_functional`)**: Using `try_fold` to solve the problem immutably using iterator adapters.

/// Iterative Approach
///
/// Time: O(N) - Iterates through each character in the string once.
/// Space: O(N) - In the worst case (e.g., all opening brackets), the stack will hold all characters.
#[must_use]
pub fn is_valid_iterative(s: String) -> bool {
    let mut stack = Vec::new();

    // RUST INSIGHT: `chars()` handles proper UTF-8 decoding, which isn't strictly
    // necessary given the problem constraints (only ASCII brackets).
    for c in s.chars() {
        match c {
            '(' | '{' | '[' => stack.push(c),
            ')' => {
                // RUST INSIGHT: `pop()` returns an `Option<T>`. We use `if let` to safely check
                // if there's an element, and if it's the matching open bracket.
                if stack.pop() != Some('(') {
                    return false;
                }
            }
            '}' => {
                if stack.pop() != Some('{') {
                    return false;
                }
            }
            ']' => {
                if stack.pop() != Some('[') {
                    return false;
                }
            }
            // GOTCHA: Exhaustive matching is required in Rust, but LeetCode constraints guarantee
            // only bracket characters. A catch-all arm prevents compiler errors.
            _ => return false,
        }
    }

    // If the stack is empty, all opening brackets were matched.
    stack.is_empty()
}

/// Optimal Approach: Operating on Bytes
///
/// Time: O(N) - Iterates through each byte.
/// Space: O(N) - In the worst case, the stack stores all bytes.
///
/// Because LeetCode guarantees the input only contains ASCII bracket characters,
/// we can bypass UTF-8 decoding (`chars()`) and work directly on the underlying bytes (`as_bytes()`).
#[must_use]
pub fn is_valid_optimal(s: String) -> bool {
    // RUST INSIGHT: Working with `u8` bytes is significantly faster than `char` for ASCII strings,
    // as it avoids UTF-8 multi-byte boundary checks.
    let bytes = s.as_bytes();
    // Capacity hint: the stack can't exceed the length of the string.
    let mut stack = Vec::with_capacity(bytes.len());

    for &b in bytes {
        match b {
            // Push the *expected closing bracket* instead of the opening bracket.
            // This simplifies the matching logic later!
            b'(' => stack.push(b')'),
            b'{' => stack.push(b'}'),
            b'[' => stack.push(b']'),
            // For closing brackets, pop and check if it matches what we just read.
            // If the stack is empty, `pop()` returns `None`, which safely won't equal `Some(b)`.
            _ => {
                if stack.pop() != Some(b) {
                    return false;
                }
            }
        }
    }

    stack.is_empty()
}

/// Functional Approach: Using `try_fold`
///
/// Time: O(N)
/// Space: O(N)
///
/// A functional style that avoids manual looping. `try_fold` allows short-circuiting
/// on the first mismatched bracket by returning `Err`.
#[must_use]
pub fn is_valid_functional(s: String) -> bool {
    // We fold over the bytes. State is our `Vec<u8>` stack.
    // ⚡ BOLT OPTIMIZATION: Pre-allocate stack capacity to avoid reallocations.
    // The maximum depth of the stack is the length of the string.
    let result = s
        .as_bytes()
        .iter()
        .try_fold(Vec::with_capacity(s.len()), |mut stack, &b| {
            match b {
                b'(' => stack.push(b')'),
                b'{' => stack.push(b'}'),
                b'[' => stack.push(b']'),
                _ => {
                    // If a mismatch occurs, we return Err to short-circuit the fold.
                    if stack.pop() != Some(b) {
                        return Err(());
                    }
                }
            }
            // Return Ok with the updated stack to continue folding.
            Ok(stack)
        });

    // Valid if the iteration completed successfully (`Ok`) AND the final stack is empty.
    matches!(result, Ok(stack) if stack.is_empty())
}

/// Main entry point - uses the optimal byte-level solution.
#[must_use]
pub fn is_valid(s: String) -> bool {
    is_valid_optimal(s)
}

/// ## Alternative Approaches
///
/// - **Early Exit on Odd Lengths**: Since each opening bracket needs a closing bracket,
///   strings with odd lengths can be immediately returned as `false`.
/// - **Stackless Validation (Counter approach)**: If there is only ONE type of bracket
///   (e.g., only `(` and `)`), you can simply use an integer counter instead of a stack,
///   incrementing on open and decrementing on close (returning false if it goes negative).
///   This drops space complexity to O(1). However, since there are three types that can
///   be nested, a stack is strictly required for this problem.

#[cfg(test)]
mod tests {
    use super::*;

    // Happy Path Tests
    #[test]
    fn test_valid_parentheses_simple() {
        assert!(is_valid_iterative("()".to_string()));
        assert!(is_valid_optimal("()".to_string()));
        assert!(is_valid_functional("()".to_string()));
    }

    #[test]
    fn test_valid_parentheses_multiple() {
        assert!(is_valid_iterative("()[]{}".to_string()));
        assert!(is_valid_optimal("()[]{}".to_string()));
        assert!(is_valid_functional("()[]{}".to_string()));
    }

    #[test]
    fn test_valid_parentheses_nested() {
        assert!(is_valid_iterative("({[]})".to_string()));
        assert!(is_valid_optimal("({[]})".to_string()));
        assert!(is_valid_functional("({[]})".to_string()));
    }

    // Edge Case Tests
    #[test]
    fn test_invalid_mismatched() {
        assert!(!is_valid_iterative("(]".to_string()));
        assert!(!is_valid_optimal("(]".to_string()));
        assert!(!is_valid_functional("(]".to_string()));
    }

    #[test]
    fn test_invalid_wrong_order() {
        assert!(!is_valid_iterative("([)]".to_string()));
        assert!(!is_valid_optimal("([)]".to_string()));
        assert!(!is_valid_functional("([)]".to_string()));
    }

    #[test]
    fn test_invalid_only_opening() {
        assert!(!is_valid_iterative("(((".to_string()));
        assert!(!is_valid_optimal("(((".to_string()));
        assert!(!is_valid_functional("(((".to_string()));
    }

    #[test]
    fn test_invalid_only_closing() {
        assert!(!is_valid_iterative(")))".to_string()));
        assert!(!is_valid_optimal(")))".to_string()));
        assert!(!is_valid_functional(")))".to_string()));
    }

    #[test]
    fn test_empty_string() {
        assert!(is_valid_iterative("".to_string()));
        assert!(is_valid_optimal("".to_string()));
        assert!(is_valid_functional("".to_string()));
    }

    // Stress/Boundary Tests
    #[test]
    fn test_large_nested_string() {
        let mut s = String::new();
        let depth = 10_000;
        for _ in 0..depth {
            s.push_str("([{");
        }
        for _ in 0..depth {
            s.push_str("}])");
        }
        assert!(is_valid_iterative(s.clone()));
        assert!(is_valid_optimal(s.clone()));
        assert!(is_valid_functional(s));
    }
}
