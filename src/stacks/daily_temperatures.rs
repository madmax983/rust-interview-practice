//! # 739. Daily Temperatures
//!
//! Given an array of integers `temperatures` represents the daily temperatures, return an array `answer`
//! such that `answer[i]` is the number of days you have to wait after the `i`th day to get a warmer temperature.
//! If there is no future day for which this is possible, keep `answer[i] == 0`.
//!
//! Difficulty: Medium
//! Link: <https://leetcode.com/problems/daily-temperatures/>
//!
//! Why this matters in Rust:
//! This problem is a canonical example of the "Monotonic Stack" pattern. In Rust, utilizing `Vec`
//! as a stack is idiomatic and efficient. It also demonstrates how to handle indices and values
//! safely without the overhead of `Reference Counting` or `pointers`, relying on simple `usize`
//! indices which are `Copy`.

/// Brute Force Approach
///
/// For each day, we scan all future days to find the first warmer temperature.
///
/// Time Complexity: O(N^2) - In the worst case (decreasing temperatures), we scan the rest of the array for each element.
/// Space Complexity: O(1) - Aside from the output array, we use constant extra space.
pub fn daily_temperatures_brute_force(temperatures: Vec<i32>) -> Vec<i32> {
    let n = temperatures.len();
    let mut answer = vec![0; n];

    for i in 0..n {
        for j in (i + 1)..n {
            if temperatures[j] > temperatures[i] {
                // RUST INSIGHT: Explicit casting `as i32` is required because `j - i` is `usize`.
                // Rust is strict about numeric types to prevent overflow/underflow surprises.
                answer[i] = (j - i) as i32;
                break;
            }
        }
    }

    answer
}

/// Monotonic Stack Approach
///
/// We maintain a stack of indices representing days with temperatures that haven't found a warmer day yet.
/// The stack is "monotonic" because the temperatures corresponding to the indices in the stack are always in decreasing order.
///
/// As we iterate through the temperatures:
/// 1. If the current temperature is warmer than the temperature at the index on top of the stack,
///    it means we found the "next warmer day" for that index. We pop the index and calculate the difference.
/// 2. We repeat this check until the stack is empty or the top element is warmer than current.
/// 3. We push the current index onto the stack.
///
/// Time Complexity: O(N) - Each element is pushed onto the stack once and popped at most once.
/// Space Complexity: O(N) - The stack can hold up to N elements in the worst case (strictly decreasing order).
pub fn daily_temperatures_stack(temperatures: Vec<i32>) -> Vec<i32> {
    let n = temperatures.len();
    let mut answer = vec![0; n];
    // RUST INSIGHT: usage of `Vec` as a LIFO stack is standard.
    // We store `usize` indices because they are `Copy` and allow us to access the temperature values.
    // Storing references would complicate lifetimes significantly.
    let mut stack: Vec<usize> = Vec::new();

    for (current_day, &current_temp) in temperatures.iter().enumerate() {
        // RUST INSIGHT: `while let` pattern matching combined with `stack.last()` allows us to safely peek.
        // We use `last()` first to check the condition without popping.
        // Note: We access `temperatures` using the index.
        while let Some(&prev_day) = stack.last() {
            // GOTCHA: Ensure you're comparing temperatures, not indices!
            if temperatures[prev_day] < current_temp {
                stack.pop(); // Pop the index since we found a warmer day
                answer[prev_day] = (current_day - prev_day) as i32;
            } else {
                break;
            }
        }
        stack.push(current_day);
    }

    answer
}

/// Alternative implementation using `match` for clarity on stack operations.
/// Sometimes explicit matching is preferred over `while let` loops if the logic is complex.
pub fn daily_temperatures_match(temperatures: Vec<i32>) -> Vec<i32> {
    let mut result = vec![0; temperatures.len()];
    let mut stack: Vec<usize> = Vec::with_capacity(temperatures.len());

    for (i, &temp) in temperatures.iter().enumerate() {
        loop {
            match stack.last() {
                Some(&prev_index) if temperatures[prev_index] < temp => {
                    stack.pop();
                    result[prev_index] = (i - prev_index) as i32;
                }
                _ => break,
            }
        }
        stack.push(i);
    }
    result
}

// Default solution
pub fn daily_temperatures(temperatures: Vec<i32>) -> Vec<i32> {
    daily_temperatures_stack(temperatures)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_example_1() {
        let input = vec![73, 74, 75, 71, 69, 72, 76, 73];
        let expected = vec![1, 1, 4, 2, 1, 1, 0, 0];
        assert_eq!(daily_temperatures(input.clone()), expected);
        assert_eq!(daily_temperatures_brute_force(input.clone()), expected);
        assert_eq!(daily_temperatures_match(input), expected);
    }

    #[test]
    fn test_example_2() {
        let input = vec![30, 40, 50, 60];
        let expected = vec![1, 1, 1, 0];
        assert_eq!(daily_temperatures(input.clone()), expected);
        assert_eq!(daily_temperatures_brute_force(input), expected);
    }

    #[test]
    fn test_example_3() {
        let input = vec![30, 60, 90];
        let expected = vec![1, 1, 0];
        assert_eq!(daily_temperatures(input.clone()), expected);
        assert_eq!(daily_temperatures_brute_force(input), expected);
    }

    #[test]
    fn test_no_warmer_days() {
        let input = vec![90, 80, 70, 60];
        let expected = vec![0, 0, 0, 0];
        assert_eq!(daily_temperatures(input.clone()), expected);
        assert_eq!(daily_temperatures_brute_force(input), expected);
    }

    #[test]
    fn test_same_temperatures() {
        let input = vec![30, 30, 30];
        let expected = vec![0, 0, 0];
        assert_eq!(daily_temperatures(input.clone()), expected);
        assert_eq!(daily_temperatures_brute_force(input), expected);
    }

    #[test]
    fn test_mixed_temperatures() {
        let input = vec![89, 62, 70, 58, 47, 47, 46, 76, 100, 70];
        let expected = vec![8, 1, 5, 4, 3, 2, 1, 1, 0, 0];
        assert_eq!(daily_temperatures(input.clone()), expected);
        assert_eq!(daily_temperatures_brute_force(input), expected);
    }
}

// Alternative approaches:
// 1. Reverse Iteration: Iterate from right to left. Maintain the stack. This can sometimes be slightly cleaner
//    as you know the answer for the current element immediately upon popping.
// 2. Array as Stack: In languages with fixed-size arrays or performance-critical code, using a pre-allocated array
//    with a pointer/index can be faster than a dynamic `Vec`, though `Vec` is highly optimized in Rust.
