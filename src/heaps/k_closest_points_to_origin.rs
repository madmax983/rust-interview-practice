//! # 973. K Closest Points to Origin
//!
//! Given an array of `points` where `points[i] = [xi, yi]` represents a point on the X-Y plane and an integer `k`, return the `k` closest points to the origin `(0, 0)`.
//! The distance between two points on the X-Y plane is the Euclidean distance (i.e., `√(x1 - x2)^2 + (y1 - y2)^2`).
//! You may return the answer in any order. The answer is guaranteed to be unique (except for the order that it is in).
//!
//! - Difficulty: Medium
//! - `LeetCode`: <https://leetcode.com/problems/k-closest-points-to-origin/>
//!
//! ## Why this matters in Rust
//! This problem is a natural fit for Rust's `std::collections::BinaryHeap` and demonstrates how to leverage Rust's type system by implementing the `Ord` and `PartialOrd` traits for custom behavior. Instead of passing anonymous comparator closures like in Python or C++, Rust encourages modeling the problem domain with specific types and embedding the sorting logic directly into the type's behavior, leading to robust and reusable code. It also highlights efficient in-place slice mutation for the `QuickSelect` approach.
//!
//! ## Approach
//!
//! We explore three implementations:
//! 1.  **Brute Force**: Sort the entire array of points by their distance to the origin and take the first `k` elements.
//! 2.  **Optimized (Max-Heap)**: Maintain a Max-Heap of size `k`. As we iterate through the points, we add them to the heap and pop the maximum element if the heap size exceeds `k`. The remaining elements are the `k` closest points.
//! 3.  **Optimal (`QuickSelect`)**: Use the `QuickSelect` algorithm to partially sort the array in-place, partitioning elements such that the first `k` elements are the closest points.

use std::cmp::Ordering;
use std::collections::BinaryHeap;

/// A custom struct to model a point in the 2D plane and compute its distance.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

impl Point {
    /// RUST INSIGHT: We compute the squared distance to avoid floating-point operations
    /// (square root) and potential precision issues. Since we only care about relative
    /// ordering, `a^2 < b^2` is equivalent to `a < b` for non-negative distances.
    #[must_use]
    pub const fn distance_squared(&self) -> i32 {
        self.x * self.x + self.y * self.y
    }
}

// To use Point in a Max-Heap based on its distance, we implement Ord.
// RUST INSIGHT: BinaryHeap in Rust is a Max-Heap by default. We want the heap to
// prioritize the *furthest* point among the k closest, so we can pop it when we
// find a closer point. Therefore, our Ord implementation compares distances normally.
impl Ord for Point {
    fn cmp(&self, other: &Self) -> Ordering {
        self.distance_squared().cmp(&other.distance_squared())
    }
}

impl PartialOrd for Point {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Brute Force Approach: Sort all points
///
/// We sort the entire array based on distance and take the first `k` elements.
///
/// - **Time Complexity**: O(N log N), where N is the number of points. Sorting takes O(N log N).
/// - **Space Complexity**: O(1) extra space (or O(N) if sorting requires an allocation like `slice::sort_by`).
#[must_use]
// LeetCode constraint: k is a positive count, so the cast to usize cannot lose the sign.
#[allow(clippy::cast_sign_loss)]
pub fn k_closest_brute_force(points: Vec<Vec<i32>>, k: i32) -> Vec<Vec<i32>> {
    let mut points = points;
    // RUST INSIGHT: `sort_by_key` is idiomatic and clean. Since `distance_squared`
    // doesn't borrow data extending beyond the element, we can use it directly.
    points.sort_by_key(|p| p[0] * p[0] + p[1] * p[1]);
    points.truncate(k as usize);
    points
}

/// Optimized Approach: Max-Heap
///
/// Maintain a Max-Heap of the `k` closest points seen so far. If a new point is closer
/// than the furthest point in our heap, we push the new point and pop the furthest one.
///
/// - **Time Complexity**: O(N log K), where N is the total number of points. Each push/pop takes O(log K), and we do it at most N times.
/// - **Space Complexity**: O(K) to store the elements in the heap.
#[must_use]
// LeetCode constraint: k is a positive count, so the cast to usize cannot lose the sign.
#[allow(clippy::cast_sign_loss)]
pub fn k_closest_optimized(points: Vec<Vec<i32>>, k: i32) -> Vec<Vec<i32>> {
    let k_usize = k as usize;
    // We use our custom `Point` struct to wrap the coordinates, leveraging its `Ord` impl.
    // GOTCHA: By default, `BinaryHeap` is a Max-Heap. Our `Ord` implementation sorts
    // points such that points with *larger* distances are considered "greater".
    // This perfectly aligns with our goal: the top of the heap is the point with the
    // largest distance among the `k` closest points.
    let mut heap: BinaryHeap<Point> = BinaryHeap::with_capacity(k_usize + 1);

    for p in points {
        let point = Point { x: p[0], y: p[1] };
        heap.push(point);
        if heap.len() > k_usize {
            heap.pop(); // Remove the point furthest from the origin
        }
    }

    // Extract the points from the heap and convert them back to Vec<i32>.
    heap.into_iter().map(|p| vec![p.x, p.y]).collect()
}

/// Optimal Approach: `QuickSelect`
///
/// We use the `QuickSelect` algorithm to partially sort the array such that the first
/// `k` elements are the closest points. This avoids sorting the entire array or
/// maintaining a heap.
///
/// - **Time Complexity**: O(N) on average, O(N^2) in the worst case (though random pivot mitigates this).
/// - **Space Complexity**: O(1) auxiliary space, as the partitioning happens in-place.
#[must_use]
// LeetCode constraint: k is a positive count, so the cast to usize cannot lose the sign.
#[allow(clippy::cast_sign_loss)]
pub fn k_closest_optimal(mut points: Vec<Vec<i32>>, k: i32) -> Vec<Vec<i32>> {
    let k_usize = k as usize;
    let len = points.len();
    if k_usize >= len {
        return points;
    }

    // Helper closure to compute distance
    let dist = |p: &[i32]| p[0] * p[0] + p[1] * p[1];

    let mut left = 0;
    let mut right = len - 1;

    while left < right {
        // Partition the subarray points[left..=right]
        // We use Lomuto partition scheme with a middle pivot to avoid worst-case on already sorted arrays.
        let pivot_idx_initial = left + (right - left) / 2;
        points.swap(pivot_idx_initial, right); // move pivot to end
        let pivot_dist = dist(&points[right]);

        let mut i = left;
        for j in left..right {
            if dist(&points[j]) <= pivot_dist {
                points.swap(i, j);
                i += 1;
            }
        }
        points.swap(i, right);

        let pivot_idx = i;

        match pivot_idx.cmp(&k_usize) {
            Ordering::Equal => break, // We found exactly K elements
            Ordering::Less => left = pivot_idx + 1, // Look in the right half
            Ordering::Greater => {
                // RUST INSIGHT: To avoid underflow when pivot_idx is 0
                if pivot_idx > 0 {
                    right = pivot_idx - 1; // Look in the left half
                } else {
                    break;
                }
            }
        }
    }

    // Truncate the vector to return only the first K points.
    points.truncate(k_usize);
    points
}

/// Main entry point
#[must_use]
pub fn k_closest(points: Vec<Vec<i32>>, k: i32) -> Vec<Vec<i32>> {
    k_closest_optimal(points, k)
}

// Alternative Approaches:
// 1. **BTreeMap**: You could use a `BTreeMap` mapped by distance to groups of points, but this incurs O(N log N) overhead without the ability to cleanly bound size like a Max-Heap.
// 2. **Binary Search over distance**: If the maximum possible distance is known, binary search on the answer (distance threshold) is possible, but less idiomatic than the generic partition approach.

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_same_points(mut result: Vec<Vec<i32>>, mut expected: Vec<Vec<i32>>) {
        // Sort both arrays to ensure order invariance before comparison
        result.sort();
        expected.sort();
        assert_eq!(result, expected);
    }

    #[test]
    fn test_brute_force() {
        let points = vec![vec![1, 3], vec![-2, 2]];
        let k = 1;
        let expected = vec![vec![-2, 2]];
        assert_same_points(k_closest_brute_force(points.clone(), k), expected.clone());
    }

    #[test]
    fn test_optimized() {
        let points = vec![vec![3, 3], vec![5, -1], vec![-2, 4]];
        let k = 2;
        let expected = vec![vec![3, 3], vec![-2, 4]];
        assert_same_points(k_closest_optimized(points.clone(), k), expected.clone());
    }

    #[test]
    fn test_optimal() {
        let points = vec![vec![3, 3], vec![5, -1], vec![-2, 4]];
        let k = 2;
        let expected = vec![vec![3, 3], vec![-2, 4]];
        assert_same_points(k_closest_optimal(points.clone(), k), expected.clone());
    }

    #[test]
    fn test_edge_case_k_equals_len() {
        let points = vec![vec![1, 3], vec![-2, 2], vec![2, -2]];
        let k = 3;
        let expected = vec![vec![1, 3], vec![-2, 2], vec![2, -2]];
        assert_same_points(k_closest_optimal(points.clone(), k), expected.clone());
        assert_same_points(k_closest_optimized(points.clone(), k), expected.clone());
        assert_same_points(k_closest_brute_force(points.clone(), k), expected.clone());
    }

    #[test]
    fn test_stress_duplicate_distances() {
        // All points have the same distance (squared = 2)
        let points = vec![vec![1, 1], vec![-1, 1], vec![1, -1], vec![-1, -1]];
        let k = 2;
        // Any 2 points are valid, but we need to verify length and that they are drawn from the original.
        let result = k_closest_optimal(points.clone(), k);
        assert_eq!(result.len(), 2);
        for p in result {
            assert_eq!(p[0] * p[0] + p[1] * p[1], 2);
        }
    }
}
