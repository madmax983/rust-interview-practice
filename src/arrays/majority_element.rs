//! # 169. Majority Element
//!
//! Difficulty: Easy
//!
//! Link: <https://leetcode.com/problems/majority-element/>
//!
//! Given an array `nums` of size `n`, return the majority element.
//! The majority element is the element that appears more than `⌊n / 2⌋` times.
//! You may assume that the majority element always exists in the array.
//!
//! This problem perfectly demonstrates Rust's `Iterator::fold` for accumulating state concisely.
//! It also highlights the `HashMap` Entry API as a robust alternative to manual existence checks.
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::arrays::majority_element::majority_element;
//!
//! let nums = vec![3, 2, 3];
//! assert_eq!(majority_element(nums), 3);
//! ```
//!
//! ## Constraints
//!
//! - `n == nums.length`
//! - `1 <= n <= 5 * 10^4`
//! - `-10^9 <= nums[i] <= 10^9`

use std::collections::HashMap;

// =========================================================================================
// Approach 1: HashMap Counting (Straightforward)
// =========================================================================================

/// Straightforward Approach: HashMap Frequency Count
///
/// We count the occurrences of each element using a `HashMap`. The Entry API (`entry(x).or_insert(0)`)
/// simplifies inserting or updating counts in a single pass.
///
/// Time: O(N) - Single pass through the array.
/// Space: O(N) - In the worst case, we might store N/2 distinct elements.
///
/// **Rust Insight:**
/// The `Entry` API prevents double-lookups that are common in other languages when checking
/// if a key exists before incrementing.
pub fn majority_element_hashmap(nums: Vec<i32>) -> i32 {
    let mut counts = HashMap::new();
    let threshold = nums.len() / 2;

    for num in nums {
        // RUST INSIGHT: `.entry(num).or_insert(0)` returns a mutable reference to the value.
        // We dereference it `*count` to modify it directly. This avoids doing a `.contains_key()`
        // followed by an `.insert()`, making it a single operation.
        let count = counts.entry(num).or_insert(0);
        *count += 1;

        if *count > threshold {
            return num;
        }
    }

    // GOTCHA: The problem guarantees a majority element exists.
    // In Rust, the compiler doesn't know this algorithm guarantees a return inside the loop,
    // so we must provide a fallback return or unreachable!() macro.
    unreachable!("Problem statement guarantees a majority element exists")
}

// =========================================================================================
// Approach 2: Boyer-Moore Voting Algorithm (Optimized)
// =========================================================================================

/// Optimized Approach: Boyer-Moore Voting Algorithm
///
/// The Boyer-Moore algorithm finds the majority element in O(1) space. It works by maintaining
/// a `candidate` and a `count`. If `count` is 0, we assign the current element as the candidate.
/// We increment `count` if the element matches the candidate, and decrement otherwise.
///
/// Time: O(N) - Single pass.
/// Space: O(1) - Only two variables are used.
///
/// **Rust Insight:**
/// Using `Iterator::fold` allows us to implement this state machine in a purely functional way,
/// completely avoiding mutable variables (`mut`) outside the accumulator tuple.
pub fn majority_element(nums: Vec<i32>) -> i32 {
    // We fold over the elements, carrying a state tuple of (candidate, count)
    let (candidate, _count) = nums.into_iter().fold((0, 0), |(candidate, count), num| {
        if count == 0 {
            // Pick a new candidate
            (num, 1)
        } else if candidate == num {
            // Increment count for current candidate
            (candidate, count + 1)
        } else {
            // Decrement count for different number
            (candidate, count - 1)
        }
    });

    candidate
}

// =========================================================================================
// Alternative Approaches
// =========================================================================================
// 1. **Sorting**: Sort the array and return `nums[nums.len() / 2]`. Time: O(N log N), Space: O(1) or O(N) depending on the sort implementation.
//    Preferred if the array is already sorted or if modifying the input is allowed and memory is extremely constrained.
// 2. **Divide and Conquer**: Recursively find the majority in the left and right halves. Time: O(N log N), Space: O(log N).
//    More complex, mostly educational for understanding divide-and-conquer principles.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_majority_element_happy_path() {
        assert_eq!(majority_element(vec![3, 2, 3]), 3);
        assert_eq!(majority_element_hashmap(vec![3, 2, 3]), 3);

        assert_eq!(majority_element(vec![2, 2, 1, 1, 1, 2, 2]), 2);
        assert_eq!(majority_element_hashmap(vec![2, 2, 1, 1, 1, 2, 2]), 2);
    }

    #[test]
    fn test_majority_element_edge_case() {
        // Single element
        assert_eq!(majority_element(vec![1]), 1);
        assert_eq!(majority_element_hashmap(vec![1]), 1);
    }

    #[test]
    fn test_majority_element_stress() {
        let mut nums = vec![7; 50000];
        nums.extend(vec![2; 49999]);
        assert_eq!(majority_element(nums.clone()), 7);
        assert_eq!(majority_element_hashmap(nums), 7);
    }
}
