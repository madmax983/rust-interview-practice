//! # 189. Rotate Array
//!
//! Difficulty: Medium
//! Link: <https://leetcode.com/problems/rotate-array/>
//!
//! Given an integer array `nums`, rotate the array to the right by `k` steps,
//! where `k` is non-negative.
//!
//! ## Why This Matters in Rust
//!
//! This problem perfectly illustrates the difference between working with high-level
//! standard library methods and implementing low-level slice manipulation. It teaches
//! crucial Rust concepts regarding mutable borrowing, slice boundaries, and how functions
//! like `split_at_mut` safely bypass the borrow checker's rule against multiple mutable
//! references by proving to the compiler that the slices are disjoint.
//!
//! ## Approach
//!
//! We provide three implementations:
//! 1. **Brute Force (`rotate_brute_force`)**: Creates a new vector to hold the rotated elements and copies them back. `O(N)` time, `O(N)` space.
//! 2. **Optimized (`rotate_optimized`)**: The classic three-step array reversal algorithm. Reverses the whole array, then the first `k` elements, then the remaining `N-k` elements. `O(N)` time, `O(1)` space.
//! 3. **Optimal (`rotate_optimal`)**: Uses Rust's highly optimized standard library `rotate_right` which uses SIMD/memmove under the hood when possible. `O(N)` time, `O(1)` space.

/// Brute Force Approach: Extra Array Allocation
///
/// Time: O(N) - iterates through the array to build a new one.
/// Space: O(N) - allocates a completely new vector.
///
/// This approach simply calculates the new index for each element,
/// writes to a temporary vector, and then clones the result back.
#[allow(clippy::ptr_arg)]
pub fn rotate_brute_force(nums: &mut Vec<i32>, k: i32) {
    if nums.is_empty() {
        return;
    }
    let n = nums.len();

    // GOTCHA: `k` can be larger than `nums.len()`. We must take the modulo to prevent
    // unnecessary full rotations and out-of-bounds indexing.
    let k = (k as usize) % n;
    if k == 0 {
        return;
    }

    let mut temp = vec![0; n];
    for (i, &val) in nums.iter().enumerate() {
        temp[(i + k) % n] = val;
    }

    // Copy elements back
    nums.copy_from_slice(&temp);
}

/// Optimized Approach: Three-Step Reversal
///
/// Time: O(N) - three passes through sections of the array.
/// Space: O(1) - reverses the elements in-place.
///
/// 1. Reverse the entire array.
/// 2. Reverse the first `k` elements.
/// 3. Reverse the rest of the array `n-k` elements.
#[allow(clippy::ptr_arg)]
pub fn rotate_optimized(nums: &mut Vec<i32>, k: i32) {
    if nums.is_empty() {
        return;
    }
    let n = nums.len();
    let k = (k as usize) % n;
    if k == 0 {
        return;
    }

    // Step 1: Reverse everything
    nums.reverse();

    // RUST INSIGHT: `split_at_mut` is a safe wrapper around `unsafe` code that proves
    // to the compiler we are creating two disjoint mutable slices. Without this, trying
    // to mutably borrow the first `k` elements and the rest simultaneously would fail to compile!
    let (left, right) = nums.split_at_mut(k);

    // Step 2: Reverse first k elements
    left.reverse();

    // Step 3: Reverse the remaining n-k elements
    right.reverse();
}

/// Optimal Approach: Standard Library Built-in
///
/// Time: O(N) - shifts elements in memory.
/// Space: O(1) - in-place swap/memmove.
///
/// Rust's standard library provides a `rotate_right` (and `rotate_left`) method on slices.
/// Under the hood, this uses highly optimized algorithms (often GCD-based or block swaps)
/// depending on the size of the slice and architecture.
#[allow(clippy::ptr_arg)]
pub fn rotate_optimal(nums: &mut Vec<i32>, k: i32) {
    if nums.is_empty() {
        return;
    }
    let k = (k as usize) % nums.len();

    // RUST INSIGHT: Zero-cost abstractions at work. The standard library provides an
    // explicitly named, optimal, in-place slice rotation that prevents reinvention of the wheel.
    nums.rotate_right(k);
}

/// Main entry point - uses the optimal standard library method
pub fn rotate(nums: &mut Vec<i32>, k: i32) {
    rotate_optimal(nums, k);
}

/// ## Alternative Approaches
///
/// - **Cyclic Replacements**: An algorithm that computes the GCD of `n` and `k` and swaps
///   elements in cycles. This is conceptually complex and prone to off-by-one errors but achieves
///   `O(N)` time and `O(1)` space without reversing. Rust's built-in `rotate_right` often falls
///   back to this or similar block-swapping strategies under the hood.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rotate_brute_force_example1() {
        let mut nums = vec![1, 2, 3, 4, 5, 6, 7];
        rotate_brute_force(&mut nums, 3);
        assert_eq!(nums, vec![5, 6, 7, 1, 2, 3, 4]);
    }

    #[test]
    fn test_rotate_optimized_example1() {
        let mut nums = vec![1, 2, 3, 4, 5, 6, 7];
        rotate_optimized(&mut nums, 3);
        assert_eq!(nums, vec![5, 6, 7, 1, 2, 3, 4]);
    }

    #[test]
    fn test_rotate_optimal_example2() {
        let mut nums = vec![-1, -100, 3, 99];
        rotate_optimal(&mut nums, 2);
        assert_eq!(nums, vec![3, 99, -1, -100]);
    }

    // Edge Case: Empty Array
    #[test]
    fn test_empty_array() {
        let mut nums: Vec<i32> = vec![];
        rotate(&mut nums, 5);
        assert_eq!(nums, Vec::<i32>::new());
    }

    // Edge Case: Single Element
    #[test]
    fn test_single_element() {
        let mut nums = vec![42];
        rotate(&mut nums, 100);
        assert_eq!(nums, vec![42]);
    }

    // Edge Case: k is 0
    #[test]
    fn test_k_is_zero() {
        let mut nums = vec![1, 2, 3];
        rotate(&mut nums, 0);
        assert_eq!(nums, vec![1, 2, 3]);
    }

    // Edge Case: k is exactly the length of the array
    #[test]
    fn test_k_is_length() {
        let mut nums = vec![1, 2, 3];
        rotate(&mut nums, 3);
        assert_eq!(nums, vec![1, 2, 3]);
    }

    // Edge Case: k > length
    #[test]
    fn test_k_greater_than_length() {
        let mut nums = vec![1, 2];
        rotate(&mut nums, 3);
        assert_eq!(nums, vec![2, 1]); // Equivalent to k = 1
    }

    // Stress Test: Large array, k > length
    #[test]
    fn test_stress() {
        let mut nums: Vec<i32> = (1..=10_000).collect();
        let k = 10_005; // Equivalent to k = 5
        let mut expected = nums.clone();

        rotate(&mut nums, k);
        expected.rotate_right(5);

        assert_eq!(nums, expected);
    }

    // Cross-implementation agreement test
    #[test]
    fn test_all_approaches_agreement() {
        let cases: Vec<(Vec<i32>, i32)> = vec![
            (vec![1, 2, 3, 4, 5, 6, 7], 3),
            (vec![-1, -100, 3, 99], 2),
            (vec![1, 2, 3], 0),
            (vec![1, 2, 3], 3),
            (vec![1, 2], 3),
            (vec![42], 100),
            (vec![], 5),
            (vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10], 25),
        ];

        for (nums, k) in cases {
            let mut a = nums.clone();
            let mut b = nums.clone();
            let mut c = nums.clone();
            rotate_brute_force(&mut a, k);
            rotate_optimized(&mut b, k);
            rotate_optimal(&mut c, k);
            assert_eq!(a, b, "brute vs optimized mismatch for {nums:?}, k={k}");
            assert_eq!(b, c, "optimized vs optimal mismatch for {nums:?}, k={k}");
        }
    }
}
