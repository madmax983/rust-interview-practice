//! # Segment Tree (Iterative)
//!
//! A generic Segment Tree implementation supporting efficient point updates and range queries.
//!
//! **Real-world Usage:**
//! - Computational geometry (range sums, minimums).
//! - Database query optimization (histogram maintenance).
//! - Competitive programming (standard tool for range queries).
//!
//! **Why build it yourself?**
//! While crates like `segment-tree` exist, implementing one from scratch teaches you:
//! 1.  **Array-based Tree Representation**: How to map a binary tree to a flat array (`2*i`, `2*i+1`) without pointers.
//! 2.  **Generics & Closures**: How to design a data structure that works for any monoid (associative operation + identity).
//! 3.  **Zero-Cost Abstractions**: The iterative implementation avoids recursion overhead and bounds checking in hot loops (if careful).
//!
//! ## Why This Matters in Rust
//!
//! This implementation highlights Rust's ability to handle **generic closures** efficiently. By storing the operation `F`,
//! we can inline the closure at compile time, resulting in code as fast as a hand-written `sum` or `min` loop, but
//! completely generic. It also demonstrates **ownership** in data structures: the tree owns the data, and the closure is
//! typically stateless (or owns its own state).

use std::fmt::Debug;

// =========================================================================================
// Architecture
// =========================================================================================
//
// Storage: `Vec<T>` of size `2 * n`.
// - Indices `1` to `n-1`: Internal nodes (aggregates).
// - Indices `n` to `2*n - 1`: Leaf nodes (original data).
// - Index `0`: Unused (simplified 1-based indexing for children calculation).
//
// Navigation:
// - Parent: `i / 2`
// - Left Child: `2 * i`
// - Right Child: `2 * i + 1`
//
// Invariants:
// 1. `tree[i] = op(tree[2*i], tree[2*i+1])` for all `1 <= i < n`.
// 2. The operation `op` must be associative: `op(a, op(b, c)) = op(op(a, b), c)`.
// 3. `identity` must be the neutral element: `op(a, identity) = a`.
//
// Complexity:
// ┌───────────────┬─────────────┬─────────────┐
// │ Operation     │ Time        │ Space       │
// ├───────────────┼─────────────┼─────────────┤
// │ Build         │ O(n)        │ O(n)        │
// │ Update        │ O(log n)    │ O(1)        │
// │ Query         │ O(log n)    │ O(1)        │
// └───────────────┴─────────────┴─────────────┘

#[derive(Debug, Clone)]
pub struct SegmentTree<T, F> {
    tree: Vec<T>,
    n: usize,
    op: F,
    identity: T,
}

impl<T, F> SegmentTree<T, F>
where
    T: Clone + Debug,
    F: Fn(&T, &T) -> T,
{
    /// Builds a new Segment Tree from the given data.
    ///
    /// # Arguments
    ///
    /// * `data` - The initial values.
    /// * `op` - The associative binary operation (e.g., `|a, b| a + b`).
    /// * `identity` - The neutral element for the operation (e.g., `0` for sum, `i32::MAX` for min).
    pub fn new(data: &[T], op: F, identity: T) -> Self {
        let n = data.len();
        if n == 0 {
            return Self {
                tree: vec![],
                n: 0,
                op,
                identity,
            };
        }

        // Allocate 2*n space.
        // We use 2*n to store n leaves and n-1 internal nodes.
        // Index 0 is unused for 1-based convenience.
        let mut tree = Vec::with_capacity(2 * n);

        // RUST INSIGHT: We initialize with the identity element or dummy values.
        // Since we fill everything, `vec![identity; 2 * n]` is safest.
        tree.resize(2 * n, identity.clone());

        // Fill leaves (indices n to 2n-1)
        for (i, val) in data.iter().enumerate() {
            tree[n + i] = val.clone();
        }

        // Build internal nodes (indices n-1 down to 1)
        for i in (1..n).rev() {
            // RUST INSIGHT: Accessing `2*i` and `2*i+1` is safe because `i < n`, so `2*i < 2*n`.
            tree[i] = op(&tree[2 * i], &tree[2 * i + 1]);
        }

        Self {
            tree,
            n,
            op,
            identity,
        }
    }

    /// Updates the value at the given index `idx` to `val`.
    ///
    /// Time Complexity: O(log n)
    pub fn update(&mut self, mut idx: usize, val: T) {
        assert!(idx < self.n, "Index out of bounds");

        // Move to leaf position
        idx += self.n;

        // Update leaf
        self.tree[idx] = val;

        // Propagate changes up to root
        while idx > 1 {
            idx /= 2;
            // GOTCHA: We must strictly follow left/right child order for non-commutative operations.
            // Even if `op` is commutative (like +), treating it as ordered is safer generic practice.
            self.tree[idx] = (self.op)(&self.tree[2 * idx], &self.tree[2 * idx + 1]);
        }
    }

    /// Queries the aggregated value in the range `[l, r)`.
    ///
    /// # Arguments
    ///
    /// * `l` - Inclusive lower bound.
    /// * `r` - Exclusive upper bound.
    ///
    /// Time Complexity: O(log n)
    pub fn query(&self, mut l: usize, mut r: usize) -> T {
        if l >= self.n || r > self.n || l > r {
            // Alternatively, return identity. But bounds checks are usually strict in Rust.
            if l == r {
                return self.identity.clone();
            }
            panic!("Invalid range [{}, {}) for size {}", l, r, self.n);
        }

        // RUST INSIGHT: We use `identity` as the accumulator.
        // `res_l` accumulates values from the left side of the range.
        // `res_r` accumulates values from the right side of the range.
        let mut res_l = self.identity.clone();
        let mut res_r = self.identity.clone();

        // Move to leaf positions
        l += self.n;
        r += self.n;

        while l < r {
            if l % 2 == 1 {
                // l is a right child, so its parent is NOT included.
                // Include l, then move to right neighbor.
                res_l = (self.op)(&res_l, &self.tree[l]);
                l += 1;
            }
            if r % 2 == 1 {
                // r is a right child (exclusive), so r-1 is included.
                r -= 1;
                res_r = (self.op)(&self.tree[r], &res_r);
            }
            // Move up
            l /= 2;
            r /= 2;
        }

        (self.op)(&res_l, &res_r)
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Alternative approaches:
// 1. **Fenwick Tree (Binary Indexed Tree)**:
//    - Faster to code, uses less memory (N vs 2N/4N).
//    - Only supports invertible operations (like sum) easily; min/max are hard/impossible.
// 2. **Recursive Segment Tree**:
//    - Easier to implement generic "Range Update" (Lazy Propagation).
//    - Risk of stack overflow for very deep trees (though log n is usually fine).
//    - Slightly slower due to function call overhead.
//
// When to use this:
// - You need range queries on a mutable array.
// - The operation is associative (Monoid).
// - You don't need range updates (or this iterative version is sufficient for point updates).

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sum_query() {
        let data = vec![1, 2, 3, 4, 5];
        let st = SegmentTree::new(&data, |a, b| a + b, 0);

        // Sum of [0, 5) -> 1+2+3+4+5 = 15
        assert_eq!(st.query(0, 5), 15);
        // Sum of [1, 4) -> 2+3+4 = 9
        assert_eq!(st.query(1, 4), 9);
        // Sum of [2, 3) -> 3
        assert_eq!(st.query(2, 3), 3);
        // Sum of [0, 0) -> 0
        assert_eq!(st.query(0, 0), 0);
    }

    #[test]
    fn test_min_query() {
        let data = vec![5, 2, 9, 1, 7];
        let st = SegmentTree::new(&data, |a, b| *a.min(b), i32::MAX);

        assert_eq!(st.query(0, 5), 1);
        assert_eq!(st.query(0, 2), 2); // min(5, 2)
        assert_eq!(st.query(2, 5), 1); // min(9, 1, 7)
    }

    #[test]
    fn test_update() {
        let data = vec![1, 2, 3, 4, 5];
        let mut st = SegmentTree::new(&data, |a, b| a + b, 0);

        assert_eq!(st.query(0, 5), 15);

        // Update index 2 (value 3) to 10.
        st.update(2, 10);
        // New array: [1, 2, 10, 4, 5]
        // Sum: 22
        assert_eq!(st.query(0, 5), 22);
        // Range including update: [1, 3) -> 2 + 10 = 12
        assert_eq!(st.query(1, 3), 12);
    }

    #[test]
    #[should_panic(expected = "Index out of bounds")]
    fn test_out_of_bounds_update() {
        let data = vec![1, 2, 3];
        let mut st = SegmentTree::new(&data, |a, b| a + b, 0);
        st.update(3, 5);
    }

    #[test]
    #[should_panic(expected = "Invalid range")]
    fn test_invalid_range_query() {
        let data = vec![1, 2, 3];
        let st = SegmentTree::new(&data, |a, b| a + b, 0);
        st.query(2, 1);
    }

    #[test]
    fn test_non_commutative_op() {
        // String concatenation is non-commutative.
        // "a" + "b" != "b" + "a"
        let data = vec![
            "a".to_string(),
            "b".to_string(),
            "c".to_string(),
            "d".to_string(),
        ];
        let op = |a: &String, b: &String| format!("{}{}", a, b);
        let st = SegmentTree::new(&data, op, String::new());

        assert_eq!(st.query(0, 4), "abcd");
        assert_eq!(st.query(1, 3), "bc");
        assert_eq!(st.query(2, 4), "cd");
    }
}
