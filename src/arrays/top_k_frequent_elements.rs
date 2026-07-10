//! # 347. Top K Frequent Elements
//!
//! Link: <https://leetcode.com/problems/top-k-frequent-elements/>
//! Difficulty: Medium
//!
//! Given an integer array `nums` and an integer `k`, return the `k` most frequent elements.
//! You may return the answer in any order.
//!
//! This problem is a natural fit for exploring Rust's `BinaryHeap` and custom traits (`Ord`, `PartialOrd`),
//! as well as demonstrating how iterator combinators make the bucket sort approach incredibly elegant.
//! It highlights the use of Rust's `HashMap` entry API for frequency counting and the power of `.flatten()`.
//!
//! ## Examples
//!
//! ```rust
//! use rust_interview_practice::arrays::top_k_frequent_elements::top_k_frequent;
//!
//! let mut result = top_k_frequent(vec![1, 1, 1, 2, 2, 3], 2);
//! result.sort_unstable(); // Order doesn't matter, sorting for assertion
//! assert_eq!(result, vec![1, 2]);
//! ```
//!
//! ## Constraints
//!
//! - 1 <= nums.length <= 10^5
//! - -10^4 <= nums[i] <= 10^4
//! - `k` is in the range `[1, the number of unique elements in the array]`.
//! - It is guaranteed that the answer is unique.

use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};

/// Brute Force approach: Hash Map and Sorting
///
/// We count frequencies using a `HashMap`, then convert it to a `Vec` and sort it
/// in descending order by frequency. Finally, we take the top `k` elements.
///
/// Time: O(N \log N) - Sorting the unique elements dominates.
/// Space: O(N) - Storing frequencies in the map and intermediate vector.
///
/// # Why this works
/// It directly implements the requirement: count, sort by count, take top K.
///
/// # Rust Insight
/// We use `into_iter()` to consume the map, `.sort_unstable_by()` which is faster
/// than stable sort when equal elements' relative order doesn't matter, and
/// `take(k)` followed by `map()` and `collect()` for a zero-cost abstraction pipeline.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
#[allow(clippy::cast_possible_truncation)]
#[allow(clippy::cast_sign_loss)]
pub fn top_k_frequent_brute_force(nums: Vec<i32>, k: i32) -> Vec<i32> {
    let mut counts = HashMap::with_capacity(nums.len());
    // RUST INSIGHT: `.entry()` API is idiomatic for frequency maps,
    // avoiding double lookups (contains_key + insert).
    for num in nums {
        *counts.entry(num).or_insert(0) += 1;
    }

    let mut freq_vec: Vec<(i32, usize)> = counts.into_iter().collect();
    // Sort descending by frequency (the `.1` element of the tuple)
    freq_vec.sort_unstable_by(|a, b| b.1.cmp(&a.1));

    freq_vec
        .into_iter()
        .take(k as usize)
        .map(|(val, _)| val)
        .collect()
}

/// Custom node to model our Min-Heap behavior explicitly.
///
/// **OWNERSHIP & TYPE MODELING INSIGHT:**
/// By default, `BinaryHeap` in Rust is a Max-Heap. We could use `std::cmp::Reverse`,
/// but creating a custom struct and implementing `Ord` is a powerful way to
/// explicitly model domain constraints. We want a Min-Heap based on `freq`.
#[derive(Eq, PartialEq)]
struct FreqNode {
    val: i32,
    freq: usize,
}

// RUST INSIGHT: To put a custom type in `BinaryHeap`, it must implement `Ord`.
// We invert the comparison to turn the default Max-Heap into a Min-Heap.
impl Ord for FreqNode {
    fn cmp(&self, other: &Self) -> Ordering {
        // Reverse comparison: comparing `other` to `self` makes the smallest `freq`
        // the "greatest" element, thus it sits at the top of the Max-Heap.
        other.freq.cmp(&self.freq)
    }
}

impl PartialOrd for FreqNode {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Optimized approach: Min-Heap
///
/// We maintain a Min-Heap of size `k`. As we iterate through frequencies, we push
/// into the heap. If the size exceeds `k`, we pop the minimum. This guarantees
/// the heap retains the top `k` largest frequencies.
///
/// Time: O(N \log k) - Inserting into a size `k` heap.
/// Space: O(N) - Hash map stores up to N unique elements.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
#[allow(clippy::cast_possible_truncation)]
#[allow(clippy::cast_sign_loss)]
pub fn top_k_frequent_optimized(nums: Vec<i32>, k: i32) -> Vec<i32> {
    let mut counts = HashMap::with_capacity(nums.len());
    for num in nums {
        *counts.entry(num).or_insert(0) += 1;
    }

    let k_usize = k as usize;
    let mut heap = BinaryHeap::with_capacity(k_usize + 1);

    for (val, freq) in counts {
        heap.push(FreqNode { val, freq });
        // GOTCHA: We must push first, then pop, to ensure we don't accidentally
        // discard a high-frequency element if we checked before pushing.
        if heap.len() > k_usize {
            heap.pop();
        }
    }

    heap.into_iter().map(|node| node.val).collect()
}

/// Optimal approach: Bucket Sort
///
/// Since the maximum frequency an element can have is `N` (the length of the array),
/// we can create an array of "buckets" where the index represents the frequency,
/// and the value is a list of numbers with that frequency.
///
/// Time: O(N) - One pass to count, one pass to distribute into buckets,
///              and one pass to gather results.
/// Space: O(N) - For the hash map and the buckets array.
///
/// # Why this approach is idiomatic Rust
/// The collection phase perfectly leverages Rust's iterator combinators.
/// Instead of nested imperative `for` loops counting down and breaking when `k`
/// is reached, we use a declarative chain: `rev()`, `flatten()`, `take()`.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
#[allow(clippy::cast_possible_truncation)]
#[allow(clippy::cast_sign_loss)]
pub fn top_k_frequent_optimal(nums: Vec<i32>, k: i32) -> Vec<i32> {
    let n = nums.len();
    let mut counts = HashMap::with_capacity(nums.len());

    for num in nums {
        *counts.entry(num).or_insert(0) += 1;
    }

    // `buckets[i]` will store elements that appear exactly `i` times.
    // We need `n + 1` buckets because frequency can be `n`.
    let mut buckets: Vec<Vec<i32>> = vec![Vec::new(); n + 1];

    for (val, freq) in counts {
        buckets[freq].push(val);
    }

    // RUST INSIGHT: Declarative iterator chain.
    // 1. `into_iter().rev()`: Iterate backwards from highest frequency (index `n`).
    // 2. `.flatten()`: Flattens the `Vec<Vec<i32>>` into an iterator of `i32`. Empty buckets are cleanly skipped.
    // 3. `.take(k)`: Stop exactly when we have `k` elements.
    // 4. `.collect()`: Gather into the final result.
    buckets
        .into_iter()
        .rev()
        .flatten()
        .take(k as usize)
        .collect()
}

/// Main entry point - uses optimal solution
#[must_use]
pub fn top_k_frequent(nums: Vec<i32>, k: i32) -> Vec<i32> {
    top_k_frequent_optimal(nums, k)
}

// =========================================================================================
// Alternative Approaches
// =========================================================================================
//
// 1. **Quickselect (Hoare's Selection Algorithm)**:
//    - Time: O(N) average, O(N^2) worst case.
//    - Space: O(N) for unique elements map.
//    - This is theoretically optimal for time, but bucket sort is practically
//      faster for this specific problem and easier to implement bug-free.
//
// 2. **BTreeMap**:
//    - Instead of `HashMap`, we could use `BTreeMap` mapped by frequency.
//    - Time: O(N log U) where U is unique elements.
//    - Slower than HashMap + BinaryHeap due to tree rebalancing overhead.

#[cfg(test)]
mod tests {
    use super::*;

    // Helper to verify result regardless of order
    fn assert_unstable_eq(mut a: Vec<i32>, mut b: Vec<i32>) {
        a.sort_unstable();
        b.sort_unstable();
        assert_eq!(a, b);
    }

    #[test]
    fn test_brute_force_happy_path() {
        assert_unstable_eq(
            top_k_frequent_brute_force(vec![1, 1, 1, 2, 2, 3], 2),
            vec![1, 2],
        );
    }

    #[test]
    fn test_brute_force_single_element() {
        assert_unstable_eq(top_k_frequent_brute_force(vec![1], 1), vec![1]);
    }

    #[test]
    fn test_brute_force_stress_boundaries() {
        // Negative numbers and all unique elements
        assert_unstable_eq(
            top_k_frequent_brute_force(vec![-1, -1, 2, 3, 4, 4, 4], 2),
            vec![4, -1],
        );
    }

    #[test]
    fn test_optimized_happy_path() {
        assert_unstable_eq(
            top_k_frequent_optimized(vec![1, 1, 1, 2, 2, 3], 2),
            vec![1, 2],
        );
    }

    #[test]
    fn test_optimized_single_element() {
        assert_unstable_eq(top_k_frequent_optimized(vec![1], 1), vec![1]);
    }

    #[test]
    fn test_optimized_stress_boundaries() {
        assert_unstable_eq(
            top_k_frequent_optimized(vec![-1, -1, 2, 3, 4, 4, 4], 2),
            vec![4, -1],
        );
    }

    #[test]
    fn test_optimal_happy_path() {
        assert_unstable_eq(
            top_k_frequent_optimal(vec![1, 1, 1, 2, 2, 3], 2),
            vec![1, 2],
        );
    }

    #[test]
    fn test_optimal_single_element() {
        assert_unstable_eq(top_k_frequent_optimal(vec![1], 1), vec![1]);
    }

    #[test]
    fn test_optimal_stress_boundaries() {
        assert_unstable_eq(
            top_k_frequent_optimal(vec![-1, -1, 2, 3, 4, 4, 4], 2),
            vec![4, -1],
        );
    }

    #[test]
    fn test_main_function() {
        assert_unstable_eq(top_k_frequent(vec![1, 1, 1, 2, 2, 3], 2), vec![1, 2]);
        assert_unstable_eq(top_k_frequent(vec![1, 2, 3, 4, 5], 5), vec![1, 2, 3, 4, 5]);
    }

    // Cross-implementation agreement test.
    // Uses inputs where the top-k answer is unambiguous (unique frequencies at the
    // boundary), matching LeetCode's "answer is guaranteed unique" constraint, so the
    // three approaches must yield the same set regardless of internal ordering.
    #[test]
    fn test_all_approaches_agreement() {
        let cases: Vec<(Vec<i32>, i32)> = vec![
            (vec![1, 1, 1, 2, 2, 3], 2),
            (vec![1], 1),
            (vec![-1, -1, 2, 3, 4, 4, 4], 2),
            (vec![5, 5, 5, 5, 6, 6, 6, 7, 7, 8], 3),
            (vec![1, 2, 3, 4, 5], 5),
        ];

        for (nums, k) in cases {
            let brute = top_k_frequent_brute_force(nums.clone(), k);
            let optimized = top_k_frequent_optimized(nums.clone(), k);
            let optimal = top_k_frequent_optimal(nums.clone(), k);

            let mut brute_sorted = brute.clone();
            brute_sorted.sort_unstable();
            let mut optimized_sorted = optimized.clone();
            optimized_sorted.sort_unstable();
            let mut optimal_sorted = optimal.clone();
            optimal_sorted.sort_unstable();

            assert_eq!(
                brute_sorted, optimized_sorted,
                "brute vs optimized mismatch for {nums:?}, k={k}"
            );
            assert_eq!(
                optimized_sorted, optimal_sorted,
                "optimized vs optimal mismatch for {nums:?}, k={k}"
            );
        }
    }
}
