//! # 17. Letter Combinations of a Phone Number
//!
//! Difficulty: Medium
//! Link: https://leetcode.com/problems/letter-combinations-of-a-phone-number/
//!
//! Given a string containing digits from `2-9` inclusive, return all possible letter
//! combinations that the number could represent. Return the answer in any order.
//!
//! A mapping of digits to letters (just like on the telephone buttons) is given below.
//! Note that 1 does not map to any letters.
//!
//! ## Why this matters in Rust
//! This problem elegantly contrasts two different Rust paradigms: Iterator-based combinatorics
//! and recursive backtracking with mutable buffers. It showcases how to use zero-cost
//! abstractions like static arrays of string slices (`&[&str]`) instead of heap-allocated
//! HashMaps for mappings. Furthermore, it demonstrates Rust's strict but powerful mutable
//! borrowing (`&mut String`), ensuring that a single path buffer can be manipulated safely
//! across recursive calls without triggering unnecessary heap allocations or garbage collection.
//!
//! ## Approach
//!
//! **1. Straightforward / Iterative (Fold)**
//! The problem can be solved by starting with an array containing an empty string, and for
//! each digit, multiplying the existing combinations by the new letters. In Rust, this can
//! be expressed beautifully using the `Iterator::fold` combinator. However, this creates
//! intermediate `Vec<String>` allocations for every digit processed.
//!
//! **2. Optimal / Recursive Backtracking**
//! To minimize allocations, we use depth-first search (DFS). We allocate a single `String`
//! buffer representing the current path and a `Vec<String>` for the final results. As we
//! traverse the digits, we push characters onto the buffer, recurse, and then pop the
//! character off (backtrack). This strictly bounds the memory footprint and avoids
//! intermediate array allocations.
//!
//! ## Time and Space Complexity
//!
//! - **Time Complexity**: `O(4^N * N)`, where `N` is the length of digits. The maximum number
//!   of letters mapped to a digit is 4 (for '7' and '9'). Building each string of length `N` takes `O(N)`.
//! - **Space Complexity**: `O(N)` for the recursion stack and the `&mut String` path buffer
//!   (excluding the output vector).

/// RUST INSIGHT:
/// Instead of using a `HashMap<char, &str>` which requires heap allocation and hashing overhead,
/// we use a static array of byte slices. Since digits '0' to '9' are contiguous in ASCII,
/// we can simply subtract `b'0'` from the character to get its index.
/// This is a zero-cost O(1) lookup.
const MAPPING: &[&[u8]] = &[
    b"",     // 0
    b"",     // 1
    b"abc",  // 2
    b"def",  // 3
    b"ghi",  // 4
    b"jkl",  // 5
    b"mno",  // 6
    b"pqrs", // 7
    b"tuv",  // 8
    b"wxyz", // 9
];

// ============================================================================
// Brute Force / Straightforward Approach
// ============================================================================

/// Brute force approach: Iterative Cartesian product using `Iterator::fold`
///
/// This approach is very concise and expressive, using Rust's iterator combinators.
/// However, it allocates a new `Vec<String>` at every step, which creates pressure
/// on the memory allocator. Same `O(4^N * N)` complexity as the optimal approach, but
/// with heavier intermediate allocation.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn letter_combinations_brute_force(digits: String) -> Vec<String> {
    if digits.is_empty() {
        return vec![];
    }

    // GOTCHA: Since we know the input consists of simple ASCII digits ('2'-'9'),
    // we use `.bytes()` instead of `.chars()`. `.chars()` has to validate and decode UTF-8
    // which is unnecessary overhead here.
    digits.bytes().fold(vec![String::new()], |acc, digit| {
        let index = (digit - b'0') as usize;
        let letters = MAPPING[index];

        let mut next_acc = Vec::with_capacity(acc.len() * letters.len());

        for prefix in &acc {
            for &letter in letters {
                // Creates a new String for every combination
                let mut new_str = prefix.clone();
                new_str.push(letter as char);
                next_acc.push(new_str);
            }
        }

        next_acc
    })
}

// ============================================================================
// Optimal Approach
// ============================================================================

/// Optimal approach using recursive backtracking
///
/// This avoids intermediate `Vec<String>` allocations. We allocate exactly one
/// `String` buffer and pass a mutable reference down the recursive call stack.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn letter_combinations_optimal(digits: String) -> Vec<String> {
    if digits.is_empty() {
        return vec![];
    }

    let digits_bytes = digits.as_bytes();

    // We can perfectly size the output vector to avoid reallocation if we do a quick math pass.
    // However, a reasonable capacity or exact math is fine.
    // Let's approximate: average 3 letters per digit.
    let capacity = 3_usize.pow(digits.len() as u32);
    let mut results = Vec::with_capacity(capacity);

    // Pre-allocate the path string to exactly the depth of the recursion.
    // This prevents the string from reallocating as we push/pop characters.
    let mut path = String::with_capacity(digits.len());

    backtrack(digits_bytes, 0, &mut path, &mut results);

    results
}

fn backtrack(digits: &[u8], index: usize, path: &mut String, results: &mut Vec<String>) {
    if index == digits.len() {
        // Base case: we have formed a complete combination.
        results.push(path.clone());
        return;
    }

    let digit = digits[index];
    let map_idx = (digit - b'0') as usize;
    let letters = MAPPING[map_idx];

    for &letter in letters {
        // Choose
        path.push(letter as char);

        // Explore
        backtrack(digits, index + 1, path, results);

        // Un-choose (Backtrack)
        path.pop();
    }
}

/// Main entry point - uses the optimal solution
#[must_use]
pub fn letter_combinations(digits: String) -> Vec<String> {
    letter_combinations_optimal(digits)
}

// ============================================================================
// Alternative Approaches
// ============================================================================
// 1. BFS with Queue: Similar to iterative, using a `VecDeque`. Pop the front
//    string, append all mapped characters for the next digit, and push back.
//    Also suffers from high allocation overhead.
// 2. Exact pre-allocation: You can calculate the exact number of combinations
//    by multiplying the lengths of the mapped strings for each digit, and allocate
//    the `results` Vec with exactly that capacity.

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn assert_combinations_eq(actual: Vec<String>, expected: Vec<&str>) {
        let actual_set: HashSet<String> = actual.into_iter().collect();
        let expected_set: HashSet<String> = expected.into_iter().map(String::from).collect();
        assert_eq!(actual_set, expected_set);
    }

    #[test]
    fn test_brute_force_happy_path() {
        let digits = "23".to_string();
        let expected = vec!["ad", "ae", "af", "bd", "be", "bf", "cd", "ce", "cf"];
        assert_combinations_eq(letter_combinations_brute_force(digits), expected);
    }

    #[test]
    fn test_all_approaches_agree() {
        // Cross-implementation agreement: both approaches must produce the same set
        // of combinations (order-independent) for a multi-digit input.
        let digits = "79".to_string();
        let bf: HashSet<String> = letter_combinations_brute_force(digits.clone())
            .into_iter()
            .collect();
        let opt: HashSet<String> = letter_combinations_optimal(digits).into_iter().collect();
        assert_eq!(bf, opt);
    }

    #[test]
    fn test_optimal_happy_path() {
        let digits = "23".to_string();
        let expected = vec!["ad", "ae", "af", "bd", "be", "bf", "cd", "ce", "cf"];
        assert_combinations_eq(letter_combinations_optimal(digits), expected);
    }

    #[test]
    fn test_empty_input() {
        let digits = "".to_string();
        let expected: Vec<&str> = vec![];
        assert_combinations_eq(letter_combinations(digits), expected);
    }

    #[test]
    fn test_single_digit() {
        let digits = "7".to_string();
        let expected = vec!["p", "q", "r", "s"];
        assert_combinations_eq(letter_combinations(digits), expected);
    }

    #[test]
    fn test_stress_multiple_digits() {
        let digits = "234".to_string();
        let result = letter_combinations(digits);
        // 3 * 3 * 3 = 27 combinations
        assert_eq!(result.len(), 27);
        assert!(result.contains(&"adg".to_string()));
        assert!(result.contains(&"cfi".to_string()));
    }
}
