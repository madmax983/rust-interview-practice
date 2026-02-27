//! # 239. Sliding Window Maximum
//!
//! Given an array of integers `nums` and a sliding window size `k`, return the maximum
//! value in the window as it moves from left to right.
//!
//! - Difficulty: Hard
//! - LeetCode: <https://leetcode.com/problems/sliding-window-maximum/>
//!
//! ## Why this matters in Rust
//! This problem is a perfect showcase for:
//! 1.  **std::collections::VecDeque**: Using a double-ended queue to implement a Monotonic Queue efficiently.
//! 2.  **Iterator Windows**: Demonstrating how `slice::windows` simplifies the naive approach but comes with a performance cost.
//! 3.  **Ownership & Indices**: Understanding why we store `usize` indices in the queue rather than values (to track window boundaries), avoiding ownership issues with non-Copy types.

use std::collections::VecDeque;

/// Brute Force Approach
///
/// Simply iterate through all windows of size `k` and find the maximum in each.
///
/// - **Time Complexity**: O(N * K), where N is the number of elements. For each window, we scan K elements.
/// - **Space Complexity**: O(1) auxiliary space (excluding the result).
///
/// # RUST INSIGHT
/// The `windows` iterator makes this implementation trivial one-liners. However, `max()` is O(K),
/// leading to quadratic behavior in the worst case (e.g., sorted array).
pub fn max_sliding_window_brute(nums: &[i32], k: i32) -> Vec<i32> {
    if nums.is_empty() || k == 0 {
        return vec![];
    }

    // GOTCHA: `k` is i32 from LeetCode signature, but slice indexing requires usize.
    // Always validate or cast carefully.
    let k = k as usize;

    nums.windows(k)
        .map(|w| *w.iter().max().unwrap_or(&0)) // unwrap is safe because k > 0 implies window is non-empty
        .collect()
}

/// Optimized Approach: Monotonic Queue
///
/// We use a `VecDeque` to store *indices* of elements. The deque maintains the invariant that
/// values corresponding to the indices are in **descending order**.
///
/// - The front of the deque always holds the index of the maximum element in the current window.
/// - As we slide the window:
///   1. Remove indices from the back that have values less than the current element (they can never be the max).
///   2. Add the current element's index to the back.
///   3. Remove the index from the front if it's out of the current window (i.e., `index <= current_index - k`).
///   4. The max for the window ending at `current_index` is `nums[deque.front()]`.
///
/// - **Time Complexity**: O(N). Each element is added to the deque once and removed at most once.
/// - **Space Complexity**: O(K). The deque stores at most `k` indices.
pub fn max_sliding_window_optimized(nums: &[i32], k: i32) -> Vec<i32> {
    if nums.is_empty() || k == 0 {
        return vec![];
    }

    let k = k as usize;
    let n = nums.len();
    let mut result = Vec::with_capacity(n - k + 1);
    let mut deque: VecDeque<usize> = VecDeque::with_capacity(k);

    for (i, &num) in nums.iter().enumerate() {
        // 1. Maintain Monotonic Property: Remove elements smaller than current from back
        // RUST INSIGHT: We access `nums` by index. If `nums` contained non-Copy types,
        // we might need references, but since it's `i32`, it's cheap.
        // `deque.back()` gives specific reference to the index, which we use to index `nums`.
        while let Some(&back_idx) = deque.back() {
            if nums[back_idx] < num {
                deque.pop_back();
            } else {
                break;
            }
        }

        // 2. Add current index
        deque.push_back(i);

        // 3. Remove out-of-bound indices from front
        // The window is [i - k + 1, i]. Any index < i - k + 1 is invalid.
        if let Some(&front_idx) = deque.front()
            && front_idx + k <= i
        {
            deque.pop_front();
        }

        // 4. Record result (only valid once we've processed at least k elements)
        if i >= k - 1
            && let Some(&max_idx) = deque.front()
        {
            result.push(nums[max_idx]);
        }
    }

    result
}

/// Entry point that defaults to the optimized solution.
pub fn max_sliding_window(nums: Vec<i32>, k: i32) -> Vec<i32> {
    max_sliding_window_optimized(&nums, k)
}

// Alternative Approaches:
// 1. **Max-Heap (Priority Queue)**: Store `(val, index)` in a Heap.
//    - Push new element.
//    - While heap top's index is out of window, pop it.
//    - Top is max.
//    - Time: O(N log N) worst case (if we don't remove elements eagerly, heap grows to N).
//    - Ideally O(N log K) with lazy removal.
// 2. **BST / BTreeMap**: similar to Heap but allows removing specific elements if we store counts. O(N log K).
//
// The Deque approach is preferred for O(N) time and O(K) space.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_brute_force_basic() {
        let nums = vec![1, 3, -1, -3, 5, 3, 6, 7];
        let k = 3;
        let expected = vec![3, 3, 5, 5, 6, 7];
        assert_eq!(max_sliding_window_brute(&nums, k), expected);
    }

    #[test]
    fn test_optimized_basic() {
        let nums = vec![1, 3, -1, -3, 5, 3, 6, 7];
        let k = 3;
        let expected = vec![3, 3, 5, 5, 6, 7];
        assert_eq!(max_sliding_window_optimized(&nums, k), expected);
    }

    #[test]
    fn test_k_equals_one() {
        let nums = vec![1, -1];
        let k = 1;
        // Window size 1 means max is the element itself
        assert_eq!(max_sliding_window_optimized(&nums, k), vec![1, -1]);
    }

    #[test]
    fn test_k_equals_len() {
        let nums = vec![1, 3, -1, -3, 5, 3, 6, 7];
        let k = 8;
        // Window size 8 covers entire array, max is 7
        assert_eq!(max_sliding_window_optimized(&nums, k), vec![7]);
    }

    #[test]
    fn test_descending_array() {
        // In a descending array, the max is always the first element of the window.
        // Deque front should just shift one by one.
        let nums = vec![9, 8, 7, 6, 5];
        let k = 3;
        // Windows: [9,8,7], [8,7,6], [7,6,5] -> Max: 9, 8, 7
        assert_eq!(max_sliding_window_optimized(&nums, k), vec![9, 8, 7]);
    }

    #[test]
    fn test_ascending_array() {
        // In an ascending array, the max is always the last element (newly added).
        // Deque should pop back everything before adding new.
        let nums = vec![1, 2, 3, 4, 5];
        let k = 3;
        // Windows: [1,2,3], [2,3,4], [3,4,5] -> Max: 3, 4, 5
        assert_eq!(max_sliding_window_optimized(&nums, k), vec![3, 4, 5]);
    }

    #[test]
    fn test_empty_input() {
        let nums: Vec<i32> = vec![];
        let k = 0;
        assert_eq!(max_sliding_window_optimized(&nums, k), vec![]);
    }
}
