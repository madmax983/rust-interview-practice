//! # 1046. Last Stone Weight
//!
//! Link: <https://leetcode.com/problems/last-stone-weight/>
//! Difficulty: Easy
//!
//! You are given an array of integers `stones` where `stones[i]` is the weight of the `i`th stone.
//! We are playing a game with the stones. On each turn, we choose the heaviest two stones and smash them together.
//! Suppose the heaviest two stones have weights `x` and `y` with `x <= y`.
//! If `x == y`, both stones are destroyed.
//! If `x != y`, the stone of weight `x` is destroyed, and the stone of weight `y` has new weight `y - x`.
//! At the end of the game, there is at most one stone left. Return the weight of the last remaining stone.
//! If there are no stones left, return 0.
//!
//! This problem is a natural fit for Rust's `std::collections::BinaryHeap`. It perfectly demonstrates
//! how to model processes that repeatedly require the "largest" element, and how ownership/borrowing
//! interacts with collection mutation.
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::heaps::last_stone_weight::last_stone_weight;
//!
//! assert_eq!(last_stone_weight(vec![2, 7, 4, 1, 8, 1]), 1);
//! assert_eq!(last_stone_weight(vec![1]), 1);
//! ```
//!
//! ## Constraints
//!
//! - `1 <= stones.length <= 30`
//! - `1 <= stones[i] <= 1000`

use std::collections::BinaryHeap;

/// Brute force approach: Sort and Process
///
/// In this approach, we sort the array in ascending order to find the two largest elements
/// at the end. We process them, and if there's a remainder, we push it back and re-sort.
/// This modifies the collection heavily and requires repeated O(n log n) sorts.
///
/// Time: O(n^2 log n) - We might do up to n sorts, each taking O(n log n).
/// Space: O(1) auxiliary space (or O(n) depending on sort stability/allocation).
///
/// # Gotcha
/// `.sort()` is overkill when we only need the maximum elements. Repeated sorting
/// is inefficient and should trigger you to think "Priority Queue / Heap".
#[must_use]
#[allow(clippy::needless_pass_by_value)] // LeetCode signature
pub fn last_stone_weight_brute_force(mut stones: Vec<i32>) -> i32 {
    while stones.len() > 1 {
        // Sort to get largest elements at the end
        stones.sort_unstable();

        let y = stones.pop().unwrap(); // Largest
        let x = stones.pop().unwrap(); // Second largest

        if x != y {
            stones.push(y - x);
        }
    }

    // RUST INSIGHT: `pop()` safely returns an Option. We can unwrap_or(0) to handle
    // both the "1 stone left" and "0 stones left" cases concisely.
    stones.pop().unwrap_or(0)
}

/// Optimal approach: Max-Heap (BinaryHeap)
///
/// Rust's `BinaryHeap` is a max-heap by default, making it the perfect data structure
/// for this problem. We can populate the heap in O(n) time and extract/insert in O(log n).
///
/// Time: O(n log n) - Building the heap is O(n), and we do at most n extractions/insertions,
/// each taking O(log n).
/// Space: O(n) - To store the elements in the heap.
///
/// # Idiomatic Comparison
/// In Java, you'd use `PriorityQueue<Integer> pq = new PriorityQueue<>((a, b) -> b - a);`
/// In Rust, `BinaryHeap::from(vec)` does this inherently because it's a max-heap, and
/// it builds in O(n) time without repeatedly calling `.push()`.
///
/// # Rust Insight
/// We convert the `Vec` directly into a `BinaryHeap` using `From`. This takes ownership
/// of the underlying allocation, avoiding any new heap allocations. This is a zero-cost
/// abstraction in action.
#[must_use]
#[allow(clippy::needless_pass_by_value)] // LeetCode signature
pub fn last_stone_weight_optimal(stones: Vec<i32>) -> i32 {
    // OWNERSHIP INSIGHT: We take ownership of `stones` and reuse its allocation.
    // Building a BinaryHeap from a Vec takes O(n) time.
    let mut max_heap = BinaryHeap::from(stones);

    // We need at least 2 stones to play the game
    while max_heap.len() > 1 {
        // Safe to unwrap because we checked len > 1
        let y = max_heap.pop().unwrap(); // Heaviest
        let x = max_heap.pop().unwrap(); // Second heaviest

        if y > x {
            // Push the remainder back into the heap
            max_heap.push(y - x);
        }
    }

    // If the heap is empty, return 0, otherwise return the remaining element
    max_heap.pop().unwrap_or(0)
}

/// Main entry point - uses optimal solution
#[must_use]
pub fn last_stone_weight(stones: Vec<i32>) -> i32 {
    last_stone_weight_optimal(stones)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_brute_force_example_1() {
        assert_eq!(last_stone_weight_brute_force(vec![2, 7, 4, 1, 8, 1]), 1);
    }

    #[test]
    fn test_brute_force_example_2() {
        assert_eq!(last_stone_weight_brute_force(vec![1]), 1);
    }

    #[test]
    fn test_optimal_example_1() {
        assert_eq!(last_stone_weight_optimal(vec![2, 7, 4, 1, 8, 1]), 1);
    }

    #[test]
    fn test_optimal_example_2() {
        assert_eq!(last_stone_weight_optimal(vec![1]), 1);
    }

    #[test]
    fn test_all_approaches_edge_cases() {
        // All stones destroyed
        let input1 = vec![2, 2];
        assert_eq!(last_stone_weight_brute_force(input1.clone()), 0);
        assert_eq!(last_stone_weight_optimal(input1), 0);

        // Large differences
        let input2 = vec![100, 10, 1];
        // 100 and 10 smash -> 90. 90 and 1 smash -> 89.
        assert_eq!(last_stone_weight_brute_force(input2.clone()), 89);
        assert_eq!(last_stone_weight_optimal(input2), 89);

        // Many identical stones
        let input3 = vec![10, 10, 10, 10];
        assert_eq!(last_stone_weight_brute_force(input3.clone()), 0);
        assert_eq!(last_stone_weight_optimal(input3), 0);

        // Odd number of identical stones
        let input4 = vec![10, 10, 10];
        assert_eq!(last_stone_weight_brute_force(input4.clone()), 10);
        assert_eq!(last_stone_weight_optimal(input4), 10);
    }
}
