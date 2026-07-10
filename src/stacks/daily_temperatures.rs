//! # 739. Daily Temperatures
//!
//! Given an array of integers `temperatures` represents the daily temperatures, return an array `answer`
//! such that `answer[i]` is the number of days you have to wait after the `i`th day to get a warmer temperature.
//! If there is no future day for which this is possible, keep `answer[i] == 0`.
//!
//! [LeetCode Problem 739](https://leetcode.com/problems/daily-temperatures/)
//!
//! ## Why This Matters in Rust
//!
//! This problem is a perfect introduction to the **Monotonic Stack** pattern and highlights several key Rust concepts:
//!
//! 1.  **Vector as Stack**: Rust's `Vec` type is the idiomatic way to implement a stack (using `push` and `pop`).
//! 2.  **Ownership and Indices**: When implementing graph or stack algorithms, a common beginner mistake is to try
//!     to store references (`&i32`) in the stack. However, this often leads to borrow checker fights because the
//!     references borrow from the vector we are actively iterating over. Storing *indices* (`usize`) is the
//!     idiomatic, safe, and zero-cost solution.
//! 3.  **Option Handling**: accessing the top of the stack via `last()` returns an `Option`, forcing us to safely
//!     handle the case where the stack is empty.

/// Brute force approach: Nested scan
///
/// For each day `i`, scan forward at `j > i` until we find a strictly warmer temperature, then record
/// the distance `j - i`. If none is found, the answer stays `0`.
///
/// Time Complexity: O(N²) - For each of the N days we may scan up to N later days.
/// Space Complexity: O(1) - Ignoring the output vector, no extra space is used.
#[must_use]
#[allow(clippy::needless_pass_by_value)] // LeetCode signature
#[allow(clippy::cast_possible_truncation)] // LeetCode constraints guarantee it fits
#[allow(clippy::cast_possible_wrap)] // LeetCode constraints guarantee it fits
pub fn daily_temperatures_brute_force(temperatures: Vec<i32>) -> Vec<i32> {
    let n = temperatures.len();
    let mut result = vec![0; n];

    for i in 0..n {
        // Look ahead for the first day strictly warmer than day `i`.
        for j in (i + 1)..n {
            if temperatures[j] > temperatures[i] {
                result[i] = (j - i) as i32;
                break;
            }
        }
    }

    result
}

/// Optimal approach: Monotonic Stack
///
/// We iterate through the temperatures array once. We maintain a "monotonic decreasing stack" of indices.
/// This means the temperatures corresponding to the indices in the stack are always in decreasing order.
///
/// When we encounter a current temperature that is *warmer* than the temperature at the index stored
/// at the top of the stack, we know we've found the "next warmer day" for that index. We pop the index,
/// calculate the difference, and repeat until the stack property is restored.
///
/// Time Complexity: O(N) - Each element is pushed onto the stack once and popped at most once.
/// Space Complexity: O(N) - In the worst case (strictly decreasing temperatures), the stack holds all indices.
#[must_use]
#[allow(clippy::needless_pass_by_value)] // LeetCode signature
#[allow(clippy::cast_possible_truncation)] // LeetCode constraints guarantee it fits
#[allow(clippy::cast_possible_wrap)] // LeetCode constraints guarantee it fits
pub fn daily_temperatures_optimal(temperatures: Vec<i32>) -> Vec<i32> {
    let n = temperatures.len();
    // Initialize the result vector with 0s.
    // If we never find a warmer day for an index, it stays 0.
    let mut result = vec![0; n];

    // The stack stores *indices* (usize) rather than values.
    // RUST INSIGHT: Storing indices avoids self-referential borrowing issues.
    // If we stored &i32, we couldn't easily calculate the distance (i - prev_index).
    let mut stack: Vec<usize> = Vec::with_capacity(n);

    for (i, &curr_temp) in temperatures.iter().enumerate() {
        // Check if the current temperature is warmer than the temperature at the index
        // currently at the top of the stack.
        //
        // We use a while loop because one warmer day might resolve multiple previous colder days.
        // e.g., [30, 40, 50] -> 50 resolves 40, then resolves 30.
        //
        // RUST INSIGHT: `while let Some(&prev_index) = stack.last()` works here because `usize` is `Copy`.
        // The borrow of `stack` in `last()` ends after the condition because `prev_index` is a copy,
        // not a reference extending into the loop body. If `T` were not `Copy` (e.g., `String`),
        // this would cause a borrow checker error when we call `stack.pop()`.
        while let Some(&prev_index) = stack.last() {
            if curr_temp > temperatures[prev_index] {
                // Found a warmer day!
                // GOTCHA: We must `pop()` to remove the index from the stack so we don't check it again.
                stack.pop();

                // Calculate the wait time.
                // Since i > prev_index always, this subtraction is safe (no underflow).
                result[prev_index] = (i - prev_index) as i32;
            } else {
                // The stack is monotonic decreasing. If curr_temp <= top,
                // we can't pop anymore. Break to push the current index.
                break;
            }
        }

        // Push the current index onto the stack to find its next warmer day later.
        stack.push(i);
    }

    result
}

/// Main entry point - uses optimal solution
#[must_use]
pub fn daily_temperatures(temperatures: Vec<i32>) -> Vec<i32> {
    daily_temperatures_optimal(temperatures)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_happy_path() {
        let temperatures = vec![73, 74, 75, 71, 69, 72, 76, 73];
        let expected = vec![1, 1, 4, 2, 1, 1, 0, 0];
        assert_eq!(daily_temperatures(temperatures), expected);
    }

    #[test]
    fn test_monotonic_increasing() {
        // Each day is warmer than the previous, so the wait is always 1 day (except the last).
        let temperatures = vec![30, 40, 50, 60];
        let expected = vec![1, 1, 1, 0];
        assert_eq!(daily_temperatures(temperatures), expected);
    }

    #[test]
    fn test_monotonic_decreasing() {
        // No future day is warmer, so all results should be 0.
        let temperatures = vec![90, 80, 70, 60];
        let expected = vec![0, 0, 0, 0];
        assert_eq!(daily_temperatures(temperatures), expected);
    }

    #[test]
    fn test_duplicates() {
        // Equal temperatures don't trigger a pop (must be strictly warmer).
        let temperatures = vec![30, 30, 30, 35];
        // Day 0 (30): waits for Day 3 (35) -> 3 days
        // Day 1 (30): waits for Day 3 (35) -> 2 days
        // Day 2 (30): waits for Day 3 (35) -> 1 day
        // Day 3 (35): no warmer day -> 0
        let expected = vec![3, 2, 1, 0];
        assert_eq!(daily_temperatures(temperatures), expected);
    }

    #[test]
    fn test_empty() {
        let temperatures = vec![];
        let expected: Vec<i32> = vec![];
        assert_eq!(daily_temperatures(temperatures), expected);
    }

    #[test]
    fn test_single_element() {
        let temperatures = vec![30];
        let expected = vec![0];
        assert_eq!(daily_temperatures(temperatures), expected);
    }

    #[test]
    fn test_brute_force() {
        assert_eq!(
            daily_temperatures_brute_force(vec![73, 74, 75, 71, 69, 72, 76, 73]),
            vec![1, 1, 4, 2, 1, 1, 0, 0]
        );
        assert_eq!(
            daily_temperatures_brute_force(vec![30, 40, 50, 60]),
            vec![1, 1, 1, 0]
        );
        assert_eq!(
            daily_temperatures_brute_force(vec![30, 30, 30, 35]),
            vec![3, 2, 1, 0]
        );
        let empty: Vec<i32> = vec![];
        assert_eq!(daily_temperatures_brute_force(empty), Vec::<i32>::new());
    }

    #[test]
    fn test_all_approaches_agree() {
        let cases = vec![
            vec![73, 74, 75, 71, 69, 72, 76, 73],
            vec![30, 40, 50, 60],
            vec![90, 80, 70, 60],
            vec![30, 30, 30, 35],
            vec![30],
            vec![],
        ];
        for case in cases {
            let brute = daily_temperatures_brute_force(case.clone());
            let optimal = daily_temperatures_optimal(case.clone());
            let entry = daily_temperatures(case);
            assert_eq!(brute, optimal);
            assert_eq!(entry, optimal);
        }
    }
}
