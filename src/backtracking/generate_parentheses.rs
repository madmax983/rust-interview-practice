//! # 22. Generate Parentheses
//!
//! Difficulty: Medium
//! Link: <https://leetcode.com/problems/generate-parentheses/>
//!
//! Given `n` pairs of parentheses, write a function to generate all combinations of well-formed parentheses.
//!
//! ## Why This Matters in Rust
//!
//! This problem is an excellent exercise in **Backtracking** and **State Modification**. In Rust, passing around
//! a mutable `String` buffer (`&mut String`) and a mutable vector of results (`&mut Vec<String>`) during
//! recursive calls perfectly demonstrates safe mutable aliasing rules. It teaches how to avoid allocating
//! thousands of intermediate `String`s by using `push()` and `pop()` to manipulate a single, contiguous
//! block of memory on the heap.
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::backtracking::generate_parentheses::generate_parenthesis;
//!
//! let result = generate_parenthesis(3);
//! assert_eq!(
//!     result,
//!     vec![
//!         "((()))".to_string(),
//!         "(()())".to_string(),
//!         "(())()".to_string(),
//!         "()(())".to_string(),
//!         "()()()".to_string()
//!     ]
//! );
//! ```
//!
//! ## Constraints
//!
//! - `1 <= n <= 8`

// =========================================================================================
// Brute Force Approach
// =========================================================================================

/// Brute force approach: Generate all 2^(2n) sequences and filter valid ones.
///
/// Time: O(2^(2n) * n) - We generate `2^(2n)` sequences. Verifying each sequence takes O(n) time.
/// Space: O(2n) - For the recursion stack and the current sequence buffer.
///
/// This approach explores every possible combination of `(` and `)` of length `2n`.
/// It completely ignores the rules of well-formed parentheses until the very end,
/// resulting in massive amounts of wasted work.
#[must_use]
// LeetCode constraints (1 <= n <= 8) guarantee this cast fits.
#[allow(clippy::cast_sign_loss)]
pub fn generate_parenthesis_brute_force(n: i32) -> Vec<String> {
    let mut result = Vec::new();
    // RUST INSIGHT: Pre-allocating capacity avoids reallocations as the string grows.
    let mut current = String::with_capacity((n * 2) as usize);
    generate_all(&mut current, n * 2, &mut result);
    result
}

// LeetCode constraints (length <= 16) guarantee this cast fits.
#[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
fn generate_all(current: &mut String, length: i32, result: &mut Vec<String>) {
    if current.len() as i32 == length {
        if is_valid(current) {
            // GOTCHA: `current.clone()` allocates a new String on the heap.
            // We must clone because `current` is mutated in subsequent recursive calls.
            result.push(current.clone());
        }
        return;
    }

    current.push('(');
    generate_all(current, length, result);
    current.pop();

    current.push(')');
    generate_all(current, length, result);
    current.pop();
}

fn is_valid(s: &str) -> bool {
    let mut balance = 0;
    for c in s.chars() {
        if c == '(' {
            balance += 1;
        } else {
            balance -= 1;
        }
        if balance < 0 {
            return false;
        }
    }
    balance == 0
}

// =========================================================================================
// Optimized Approach
// =========================================================================================

/// Optimized approach: Standard Backtracking with State Validation.
///
/// Time: O(4^n / sqrt(n)) - The n-th Catalan number, which bounds the number of valid combinations.
/// Space: O(n) - The depth of the recursion stack is at most `2n`.
///
/// Instead of generating all sequences blindly, we only add `(` or `)` if we know
/// it will lead to a valid sequence. We track the count of `open` and `close` parentheses.
#[must_use]
// LeetCode constraints (1 <= n <= 8) guarantee this cast fits.
#[allow(clippy::cast_sign_loss)]
pub fn generate_parenthesis_optimized(n: i32) -> Vec<String> {
    let mut result = Vec::new();
    let mut current = String::with_capacity((n * 2) as usize);
    backtrack_optimized(&mut current, 0, 0, n, &mut result);
    result
}

// LeetCode constraints (max * 2 <= 16) guarantee this cast fits.
#[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
fn backtrack_optimized(
    current: &mut String,
    open: i32,
    close: i32,
    max: i32,
    result: &mut Vec<String>,
) {
    if current.len() as i32 == max * 2 {
        result.push(current.clone());
        return;
    }

    // RUST INSIGHT: We pass `current` as `&mut String` to mutate it in place.
    // This entirely avoids creating new `String` instances at each recursion step,
    // which is a common performance pitfall in languages like Java or Python (where strings are immutable).
    if open < max {
        current.push('(');
        backtrack_optimized(current, open + 1, close, max, result);
        current.pop(); // Backtrack
    }

    if close < open {
        current.push(')');
        backtrack_optimized(current, open, close + 1, max, result);
        current.pop(); // Backtrack
    }
}

// =========================================================================================
// Optimal Approach
// =========================================================================================

/// Optimal approach: Closure-based Backtracking (Zero-Allocation Buffer).
///
/// Time: O(4^n / sqrt(n)) - Same time complexity as the optimized approach.
/// Space: O(n) - Recursion stack space.
///
/// This approach uses the exact same algorithmic logic as the optimized one but leverages
/// Rust's capabilities to capture variables in closures, making the code much more concise.
/// Furthermore, we use a `Vec<u8>` (byte array) as the buffer instead of `String` since we
/// are exclusively working with ASCII characters `(` and `)`. This avoids UTF-8 boundary checks.
#[must_use]
// LeetCode constraints (1 <= n <= 8) guarantee this cast fits.
#[allow(clippy::cast_sign_loss)]
pub fn generate_parenthesis_optimal(n: i32) -> Vec<String> {
    // RUST INSIGHT: By defining a recursive closure (using a separate fn or capturing env),
    // we can avoid passing `max` and `result` around. However, Rust closures cannot
    // easily call themselves recursively if they capture mutable state due to borrowing rules.
    // Therefore, we still pass the mutable state explicitly, but hide it in a scoped helper.
    fn backtrack(current: &mut Vec<u8>, open: i32, close: i32, n: i32, result: &mut Vec<String>) {
        if open == n && close == n {
            // SAFETY: We only ever push b'(' and b')', which are valid UTF-8.
            // `from_utf8_unchecked` is a zero-cost abstraction to convert `Vec<u8>` to `String`.
            // UNSAFE JUSTIFICATION: We strictly control the contents of `current` to be ASCII brackets.
            let s = unsafe { String::from_utf8_unchecked(current.clone()) };
            result.push(s);
            return;
        }

        if open < n {
            current.push(b'(');
            backtrack(current, open + 1, close, n, result);
            current.pop();
        }

        if close < open {
            current.push(b')');
            backtrack(current, open, close + 1, n, result);
            current.pop();
        }
    }

    let mut result = Vec::new();
    // Use a byte buffer for zero-overhead ASCII manipulation.
    let mut current = Vec::with_capacity((n * 2) as usize);
    backtrack(&mut current, 0, 0, n, &mut result);
    result
}

/// Main entry point - uses optimal approach
#[must_use]
pub fn generate_parenthesis(n: i32) -> Vec<String> {
    generate_parenthesis_optimal(n)
}

// Alternative Approaches:
// 1. **Dynamic Programming**: We can build the combinations for `N` using combinations for `N-1`.
//    For example, `f(n) = "(" + f(i) + ")" + f(n - 1 - i)` for all `0 <= i < n`.
//    This is functionally elegant but often slower than direct backtracking in Rust due to
//    increased string allocations and concatenation overhead.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_n_1() {
        let expected = vec!["()".to_string()];
        assert_eq!(generate_parenthesis_brute_force(1), expected);
        assert_eq!(generate_parenthesis_optimized(1), expected);
        assert_eq!(generate_parenthesis_optimal(1), expected);
    }

    #[test]
    fn test_n_2() {
        let mut expected = vec!["(())".to_string(), "()()".to_string()];
        expected.sort();

        let mut res_brute = generate_parenthesis_brute_force(2);
        res_brute.sort();
        let mut res_opt = generate_parenthesis_optimized(2);
        res_opt.sort();
        let mut res_optimal = generate_parenthesis_optimal(2);
        res_optimal.sort();

        assert_eq!(res_brute, expected);
        assert_eq!(res_opt, expected);
        assert_eq!(res_optimal, expected);
    }

    #[test]
    fn test_n_3() {
        let mut expected = vec![
            "((()))".to_string(),
            "(()())".to_string(),
            "(())()".to_string(),
            "()(())".to_string(),
            "()()()".to_string(),
        ];
        expected.sort();

        let mut res_optimal = generate_parenthesis(3);
        res_optimal.sort();

        assert_eq!(res_optimal, expected);
    }

    #[test]
    fn test_zero_case() {
        // Technically constraints say 1 <= n <= 8, but it's good practice to handle 0.
        let expected: Vec<String> = vec!["".to_string()];
        assert_eq!(generate_parenthesis_optimal(0), expected);
    }
}
