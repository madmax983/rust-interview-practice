//! # 283. Move Zeroes
//!
//! Difficulty: Easy
//! Link: <https://leetcode.com/problems/move-zeroes>/
//!
//! Given an integer array `nums`, move all `0`s to the end of it while maintaining the relative order of the non-zero elements.
//!
//! Note that you must do this in-place without making a copy of the array.
//!
//! This problem matters in Rust because it teaches you how to perform in-place array
//! modifications efficiently. It's a great introduction to mutable slice manipulation,
//! the `swap` method, and utilizing Rust's powerful standard library `Vec` methods
//! to achieve idiomatic, zero-cost abstractions.
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::arrays::move_zeroes::move_zeroes_optimal;
//!
//! let mut nums = vec![0, 1, 0, 3, 12];
//! move_zeroes_optimal(&mut nums);
//! assert_eq!(nums, vec![1, 3, 12, 0, 0]);
//! ```

/// Brute force approach: Allocate a new vector
/// Time: O(N) - Single pass to collect non-zeros
/// Space: O(N) - Allocates a new vector
///
/// This approach demonstrates what happens if you ignore the "in-place" requirement.
/// While functional, it allocates a new `Vec`, which incurs O(N) space and memory
/// copying overhead.
pub fn move_zeroes_brute_force(nums: &mut Vec<i32>) {
    let mut non_zeroes = Vec::with_capacity(nums.len());
    let mut zero_count = 0;

    for &num in nums.iter() {
        if num != 0 {
            non_zeroes.push(num);
        } else {
            zero_count += 1;
        }
    }

    // Append the zeroes
    non_zeroes.resize(non_zeroes.len() + zero_count, 0);

    // Replace the original contents
    // GOTCHA: `*nums = non_zeroes` clones the entire vector if it wasn't moved.
    // Since we own `non_zeroes`, this is an O(N) copy into the reference.
    *nums = non_zeroes;
}

/// Optimized approach: Two-pointer manual swap
/// Time: O(N) - Single pass
/// Space: O(1) - Modifies array in-place
///
/// This uses a slow and fast pointer approach. The `write_idx` keeps track of where
/// the next non-zero element should go.
///
/// RUST INSIGHT: Mutable slice operations like `.swap()` make in-place modifications
/// safe and bounds-checked.
// Signature kept as `&mut Vec<i32>` to match the sibling implementations' LeetCode signature.
#[allow(clippy::ptr_arg)]
pub fn move_zeroes_optimized(nums: &mut Vec<i32>) {
    let mut write_idx = 0;

    for read_idx in 0..nums.len() {
        if nums[read_idx] != 0 {
            nums.swap(read_idx, write_idx);
            write_idx += 1;
        }
    }
}

/// Optimal approach: Idiomatic standard library methods
/// Time: O(N) - Internally filters and resizes
/// Space: O(1) - Operates entirely in-place
///
/// This is the most idiomatic Rust approach. `Vec::retain` preserves elements matching
/// the predicate in-place in O(N) time. We then `resize` to restore the original length,
/// padding with zeroes.
///
/// RUST INSIGHT: `retain` combined with `resize` is often faster than manual loops because
/// standard library methods are heavily optimized with unsafe code under the hood (e.g., using `ptr::copy`).
pub fn move_zeroes_optimal(nums: &mut Vec<i32>) {
    let original_len = nums.len();

    // Retain only non-zero elements, shifting them to the front
    nums.retain(|&x| x != 0);

    // Pad the rest of the vector with zeroes
    nums.resize(original_len, 0);
}

/// Main entry point - uses optimal solution
pub fn move_zeroes(nums: &mut Vec<i32>) {
    move_zeroes_optimal(nums);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_brute_force() {
        let mut nums = vec![0, 1, 0, 3, 12];
        move_zeroes_brute_force(&mut nums);
        assert_eq!(nums, vec![1, 3, 12, 0, 0]);

        let mut nums = vec![0];
        move_zeroes_brute_force(&mut nums);
        assert_eq!(nums, vec![0]);
    }

    #[test]
    fn test_optimized() {
        let mut nums = vec![0, 1, 0, 3, 12];
        move_zeroes_optimized(&mut nums);
        assert_eq!(nums, vec![1, 3, 12, 0, 0]);

        let mut nums = vec![0];
        move_zeroes_optimized(&mut nums);
        assert_eq!(nums, vec![0]);
    }

    #[test]
    fn test_optimal() {
        let mut nums = vec![0, 1, 0, 3, 12];
        move_zeroes_optimal(&mut nums);
        assert_eq!(nums, vec![1, 3, 12, 0, 0]);

        let mut nums = vec![0];
        move_zeroes_optimal(&mut nums);
        assert_eq!(nums, vec![0]);
    }

    #[test]
    fn test_all_zeros() {
        let mut nums1 = vec![0, 0, 0, 0];
        let mut nums2 = vec![0, 0, 0, 0];
        let mut nums3 = vec![0, 0, 0, 0];

        move_zeroes_brute_force(&mut nums1);
        move_zeroes_optimized(&mut nums2);
        move_zeroes_optimal(&mut nums3);

        assert_eq!(nums1, vec![0, 0, 0, 0]);
        assert_eq!(nums2, vec![0, 0, 0, 0]);
        assert_eq!(nums3, vec![0, 0, 0, 0]);
    }

    #[test]
    fn test_no_zeros() {
        let mut nums1 = vec![1, 2, 3, 4];
        let mut nums2 = vec![1, 2, 3, 4];
        let mut nums3 = vec![1, 2, 3, 4];

        move_zeroes_brute_force(&mut nums1);
        move_zeroes_optimized(&mut nums2);
        move_zeroes_optimal(&mut nums3);

        assert_eq!(nums1, vec![1, 2, 3, 4]);
        assert_eq!(nums2, vec![1, 2, 3, 4]);
        assert_eq!(nums3, vec![1, 2, 3, 4]);
    }
}

// # Alternative Approaches
//
// 1. **Count Zeroes First**: You could count zeroes in one pass, then iterate backwards
//    moving non-zeroes to the end of the non-zero region. This is generally more complex
//    than the two-pointer approach shown in `move_zeroes_optimized`.
// 2. **Stable Sort (Overkill)**: You could conceptually use a stable sort with a custom comparator
//    that pushes zeroes to the end. This is O(N log N) time and would be terrible performance-wise,
//    but it conceptually satisfies the requirement if built-in methods are the only tool you know.
