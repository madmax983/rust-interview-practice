//! # Myers Diff Algorithm
//!
//! Implements Eugene W. Myers' O(ND) Difference Algorithm for comparing two sequences.
//!
//! **Replaces Crates:** `similar`, `diff`, `im-rc` (diffing internals).
//!
//! **Real-world Usage:**
//! - Version control systems (`git diff`).
//! - Text editors and IDEs (showing inline changes).
//! - Virtual DOM reconciliation in UI frameworks (diffing trees/lists).
//!
//! **Why build it yourself?**
//! Implementing Myers diff teaches you how to map a sequence comparison problem into
//! a shortest-path graph problem. You'll understand why diffs sometimes look "weird"
//! (it prioritizes deletions before insertions to find the theoretically shortest edit script).
//! It also highlights the power of dynamic programming and how to optimize space complexity
//! by storing only the "frontier" of the search space.
//!
//! # Architecture
//!
//! **Data Structure:**
//! We conceptualize the comparison of sequence `A` (length N) and sequence `B` (length M)
//! as a 2D grid.
//! - Moving right (x+1) means deleting an element from `A`.
//! - Moving down (y+1) means inserting an element from `B`.
//! - Moving diagonally (x+1, y+1) means elements are equal (zero cost).
//!
//! The algorithm explores diagonals, defined as `k = x - y`.
//! For each "depth" `d` (number of edits), we track the maximum `x` reached on each diagonal `k`.
//!
//! **Invariants:**
//! 1. The maximum length of the edit script is `N + M`.
//! 2. The depth `D` being searched never exceeds `N + M`.
//! 3. The `V` array size is at least `2 * (N + M) + 1` to accommodate diagonals from `-(N+M)` to `+(N+M)`.
//!
//! ```text
//!       B
//!     0 1 2 3
//!   0 + - - -
//! A 1 | \
//!   2 |   \
//! ```
//!
//! **Complexity:**
//! - **Time**: O(N * D) where N is the sum of lengths and D is the number of differences.
//! - **Space**: O(N + M) to store the frontier (V array) and the trace (history of V arrays).
//!   Generating the actual diff script requires storing the history of the frontier at each depth.
//!
//! **Design Decisions & Tradeoffs:**
//! - This is the base O(ND) algorithm. It is very fast for small differences, but degrades
//!   to O(N^2) if the sequences are entirely different.
//! - Production diff tools (like Git) use Myers but with various heuristics:
//!   - Running it forwards and backwards simultaneously (Linear Space Refinement).
//!   - Short-circuiting for common prefixes and suffixes.
//!   - Bounding the maximum depth (e.g., falling back to a simpler algorithm if D > threshold).
//! - We use a generic interface `&[T]` allowing diffing of chars, strings, or custom structs.
//!
//! Note: single canonical implementation; the brute/optimized/optimal progression does not apply.

use std::fmt;

/// Represents an operation in the edit script.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffOp<'a, T> {
    /// Element exists in both sequences.
    Equal(&'a T),
    /// Element was deleted from the original sequence.
    Delete(&'a T),
    /// Element was inserted into the new sequence.
    Insert(&'a T),
}

impl<'a, T: fmt::Display> fmt::Display for DiffOp<'a, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DiffOp::Equal(val) => write!(f, "  {}", val),
            DiffOp::Delete(val) => write!(f, "- {}", val),
            DiffOp::Insert(val) => write!(f, "+ {}", val),
        }
    }
}

/// A trait defining the interface for computing differences between sequences.
/// Shows how Rust traits enable swappable strategies (e.g., Myers vs. Patience diff algorithms).
pub trait DiffAlgorithm {
    /// Computes the difference between `old` and `new`, returning a list of operations.
    fn diff<'a, T: PartialEq>(&self, old: &'a [T], new: &'a [T]) -> Vec<DiffOp<'a, T>>;
}

/// The O(ND) Myers diff algorithm implementation.
pub struct MyersDiff;

impl DiffAlgorithm for MyersDiff {
    fn diff<'a, T: PartialEq>(&self, old: &'a [T], new: &'a [T]) -> Vec<DiffOp<'a, T>> {
        let n = old.len();
        let m = new.len();
        let max_d = n + m;

        if max_d == 0 {
            return Vec::new();
        }

        // V array stores the maximum x value for each diagonal k.
        // The range of k is from -max_d to +max_d.
        // To use a 0-indexed array, we offset k by `max_d`.
        // Size needed: 2 * max_d + 1.
        let offset = max_d;
        let mut v = vec![0; 2 * max_d + 1];

        // Track the history of V for backtracing.
        // We only need this to reconstruct the actual diff, not just calculate the distance.
        let mut trace: Vec<Vec<usize>> = Vec::new();

        // Special case: initializing the v array so the first step works.
        v[offset + 1] = 0;

        let mut found = false;
        let mut depth = 0;

        // Forward search
        for d in 0..=max_d {
            depth = d;
            let mut next_v = v.clone();

            // RUST INSIGHT:
            // We use `isize` for `k` because it can be negative.
            // We must be careful casting `isize` to `usize` for array indexing,
            // which is why we add `offset`.
            let mut k = -(d as isize);
            while k <= d as isize {
                let k_idx = (k + offset as isize) as usize;

                // Determine if we are moving down (Insert) or right (Delete).
                // We move down if k == -d (at the left edge), or if we are not at the right edge
                // and the x value above us is greater than the x value to our left.
                let move_down = k == -(d as isize)
                    || (k != d as isize && v[k_idx - 1] < v[k_idx + 1]);

                let mut x = if move_down {
                    v[k_idx + 1] // Move down (x stays the same, y increases)
                } else {
                    v[k_idx - 1] + 1 // Move right (x increases)
                };

                let mut y = (x as isize - k) as usize;

                // Follow snake (diagonal edges where elements are equal)
                while x < n && y < m && old[x] == new[y] {
                    x += 1;
                    y += 1;
                }

                next_v[k_idx] = x;

                if x >= n && y >= m {
                    found = true;
                    break;
                }

                // Step by 2 because diagonals parity matches depth parity.
                k += 2;
            }

            v = next_v;
            trace.push(v.clone());

            if found {
                break;
            }
        }

        // Backtrack to find the shortest edit script.
        let mut script = Vec::new();
        let mut x = n;
        let mut y = m;

        let mut current_d = depth;
        while current_d > 0 || x > 0 || y > 0 {
            let d = current_d;
            let v = &trace[d];
            let k = x as isize - y as isize;
            let k_idx = (k + offset as isize) as usize;

            let move_down = if d == 0 {
                false // At depth 0 we only move diagonal
            } else {
                k == -(d as isize) || (k != d as isize && v[k_idx - 1] < v[k_idx + 1])
            };

            let (prev_x, prev_y) = if d == 0 {
                (0, 0)
            } else if move_down {
                let px = v[k_idx + 1];
                (px, (px as isize - (k + 1)) as usize)
            } else {
                let px = v[k_idx - 1];
                (px, (px as isize - (k - 1)) as usize)
            };

            // Trace the snake backwards
            while x > prev_x && y > prev_y {
                x -= 1;
                y -= 1;
                // Push equal elements
                script.push(DiffOp::Equal(&old[x]));
            }

            if d > 0 {
                if move_down {
                    y -= 1;
                    script.push(DiffOp::Insert(&new[y]));
                } else {
                    x -= 1;
                    script.push(DiffOp::Delete(&old[x]));
                }
                current_d -= 1;
            } else {
                break;
            }
        }

        // The script is built backwards, so reverse it.
        script.reverse();
        script
    }
}

// Missing vs. Production:
// - **Linear Space Refinement**: Production crates like `similar` use Myers' Linear Space algorithm (running forward and backward simultaneously) to reduce space complexity from O(N+M) to O(1) during the search.
// - **Pre-filtering**: Standard crates will immediately identify and strip common prefixes and suffixes from `old` and `new` to vastly shrink the search grid size.
// - **Diff Semantic Tuning**: Many crates will perform cleanup passes on the final edit script to clump insertions and deletions together, as sometimes mathematically optimal shortest paths produce confusing visual outputs for humans (e.g. interleaving too many single characters).
//
// Suggested next steps / extensions:
// 1. Add prefix/suffix trimming before running the main O(ND) algorithm.
// 2. Implement the backward search and merge it with the forward search for the Linear Space optimization.
// 3. Add a "Patience Diff" struct implementing the `DiffAlgorithm` trait to compare outputs.
//
// Benchmarking Note:
// To benchmark, construct two large strings (e.g. 5,000 characters) with a moderate number of edits (e.g., 50 inserts and deletes).
// Use `std::hint::black_box()` around the strings to ensure the diff calculation is fully executed.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diff_equal() {
        let algo = MyersDiff;
        let a = vec![1, 2, 3];
        let b = vec![1, 2, 3];
        let d = algo.diff(&a, &b);
        assert_eq!(
            d,
            vec![
                DiffOp::Equal(&1),
                DiffOp::Equal(&2),
                DiffOp::Equal(&3),
            ]
        );
    }

    #[test]
    fn test_diff_all_insert() {
        let algo = MyersDiff;
        let a: Vec<i32> = vec![];
        let b = vec![1, 2];
        let d = algo.diff(&a, &b);
        assert_eq!(
            d,
            vec![
                DiffOp::Insert(&1),
                DiffOp::Insert(&2),
            ]
        );
    }

    #[test]
    fn test_diff_all_delete() {
        let algo = MyersDiff;
        let a = vec![1, 2];
        let b: Vec<i32> = vec![];
        let d = algo.diff(&a, &b);
        assert_eq!(
            d,
            vec![
                DiffOp::Delete(&1),
                DiffOp::Delete(&2),
            ]
        );
    }

    #[test]
    fn test_diff_mixed() {
        let algo = MyersDiff;
        // "ABCABBA" -> "CBABAC" (Myers original paper example)
        let a: Vec<char> = "ABCABBA".chars().collect();
        let b: Vec<char> = "CBABAC".chars().collect();
        let d = algo.diff(&a, &b);

        let mut actual = String::new();
        for op in d {
            match op {
                DiffOp::Equal(c) => actual.push(*c),
                DiffOp::Delete(c) => { actual.push('-'); actual.push(*c); },
                DiffOp::Insert(c) => { actual.push('+'); actual.push(*c); },
            }
        }

        // Multiple valid shortest paths exist, this is one of them.
        // It's possible the test fails if a different, but equally valid path is taken.
        // Both "-A-BC+BAB-BA+C" and "-A-BC+B+AB-BA+C" have the same edit distance (D=5)
        // We accept the one that the algorithm produces.
        assert_eq!(actual, "-A-BC+BAB-BA+C");
    }

    #[test]
    fn test_diff_words() {
        let algo = MyersDiff;
        let a = vec!["the", "quick", "brown", "fox"];
        let b = vec!["the", "fast", "brown", "fox", "jumps"];
        let d = algo.diff(&a, &b);

        assert_eq!(
            d,
            vec![
                DiffOp::Equal(&"the"),
                DiffOp::Delete(&"quick"),
                DiffOp::Insert(&"fast"),
                DiffOp::Equal(&"brown"),
                DiffOp::Equal(&"fox"),
                DiffOp::Insert(&"jumps"),
            ]
        );
    }
}
