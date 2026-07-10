//! # Union-Find (Disjoint Set Union) Implementation
//!
//! A data structure that tracks a set of elements partitioned into a number of disjoint (non-overlapping) subsets.
//! It provides near-constant time operations to add new sets, merge existing sets, and determine whether elements are in the same set.
//!
//! **Replaces Crates:** `petgraph::unionfind`, `disjoint-sets`
//!
//! **Real-world Usage:**
//! - Network connectivity (checking if two computers can reach each other).
//! - Image processing (connected component labeling).
//! - Kruskal's algorithm for Minimum Spanning Trees.
//! - Type inference compilers (unification).
//!
//! **Why build it yourself?**
//! It's the classic example of "Amortized Analysis". You'll learn how `Path Compression` and `Union by Rank`
//! turn O(log n) operations into effectively O(1) (Inverse Ackermann function).

// =========================================================================================
// Architecture
// =========================================================================================
//
// Data Structure:
// - `parent`: Array where parent[i] is the parent of node i.
// - `rank`: Array tracking the approximate height of the tree rooted at i.
//
// Invariants:
// 1. If parent[i] == i, then i is a root.
// 2. Rank is only relevant for roots.
//
// Complexity:
// ┌───────────────┬───────────────────┬─────────────┐
// │ Operation     │ Amortized Time    │ Space       │
// ├───────────────┼───────────────────┼─────────────┤
// │ Find          │ O(α(n)) ≈ O(1)    │ O(n)        │
// │ Union         │ O(α(n)) ≈ O(1)    │ O(n)        │
// │ Connected     │ O(α(n)) ≈ O(1)    │ O(n)        │
// └───────────────┴───────────────────┴─────────────┘
// α(n) is the inverse Ackermann function, which is <= 4 for all practical values of n.

// RUST INSIGHT:
// Amortized analysis proves that while individual operations might take O(log n),
// a sequence of m operations takes O(m * α(n)), making it practically constant time.

#[derive(Debug, Clone)]
pub struct UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    count: usize, // Number of disjoint sets
}

impl UnionFind {
    /// Creates a new Union-Find structure with `n` elements (0 to n-1).
    /// Initially, each element is in its own set.
    #[must_use] 
    pub fn new(n: usize) -> Self {
        let mut parent = Vec::with_capacity(n);
        for i in 0..n {
            parent.push(i);
        }

        Self {
            parent,
            rank: vec![0; n],
            count: n,
        }
    }

    /// Returns the representative (root) of the set containing element `i`.
    /// Performs path compression.
    pub fn find(&mut self, i: usize) -> usize {
        assert!(i < self.parent.len(), "Index out of bounds");

        let mut root = i;
        // Find root
        // RUST INSIGHT: We implement path compression iteratively to avoid stack overflow on deep trees,
        // although rank optimization makes extremely deep trees highly unlikely.
        while root != self.parent[root] {
            root = self.parent[root];
        }

        // Path compression: make all nodes on path point directly to root
        let mut curr = i;
        while curr != root {
            let next = self.parent[curr];
            self.parent[curr] = root;
            curr = next;
        }

        root
    }

    /// Merges the set containing `i` and the set containing `j`.
    /// Returns `true` if they were successfully merged (were in different sets).
    /// Returns `false` if they were already in the same set.
    pub fn union(&mut self, i: usize, j: usize) -> bool {
        let root_i = self.find(i);
        let root_j = self.find(j);

        if root_i == root_j {
            return false;
        }

        // Union by Rank
        // RUST INSIGHT: Merging the shorter tree into the taller one guarantees the tree height
        // grows only logarithmically (before path compression flattens it).
        if self.rank[root_i] < self.rank[root_j] {
            self.parent[root_i] = root_j;
        } else if self.rank[root_i] > self.rank[root_j] {
            self.parent[root_j] = root_i;
        } else {
            self.parent[root_j] = root_i;
            self.rank[root_i] += 1;
        }

        self.count -= 1;
        true
    }

    /// Checks if `i` and `j` are in the same set.
    pub fn connected(&mut self, i: usize, j: usize) -> bool {
        self.find(i) == self.find(j)
    }

    /// Returns the number of disjoint sets.
    #[must_use] 
    pub const fn count(&self) -> usize {
        self.count
    }

    /// Returns the total number of elements.
    #[must_use] 
    pub const fn len(&self) -> usize {
        self.parent.len()
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `petgraph`: Integrated into graph structure, optimized for graph algo usage.
//
// Missing vs. Production:
// - **Generic Keys**: This implementation uses `usize` indices. Production crates often support
//   arbitrary types `T: Hash + Eq` by mapping them to indices internally (e.g., via a HashMap).
// - **Component Size**: Often useful to know the size of the set containing `i` (by maintaining a `size` array).

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_union_find() {
        let mut uf = UnionFind::new(10);

        assert_eq!(uf.count(), 10);
        assert!(!uf.connected(0, 1));

        uf.union(0, 1);
        assert!(uf.connected(0, 1));
        assert_eq!(uf.count(), 9);

        uf.union(2, 3);
        uf.union(0, 2);

        // 0-1 and 2-3 are merged. Now {0, 1, 2, 3} are connected.
        assert!(uf.connected(1, 3));
        assert!(uf.connected(0, 3));
        assert_eq!(uf.count(), 7); // 1 merged set of 4, plus 6 singletons.

        // Redundant union
        assert!(!uf.union(1, 2));
    }

    #[test]
    fn test_path_compression_works() {
        let mut uf = UnionFind::new(5);
        // 0 -> 1 -> 2 -> 3 -> 4
        uf.union(3, 4);
        uf.union(2, 3);
        uf.union(1, 2);
        uf.union(0, 1);

        assert!(uf.connected(0, 4));

        // Internally, find(0) should now compress the path.
        // We can't inspect internal state easily without exposing it,
        // but the operations should remain correct.
        let root = uf.find(0);
        assert_eq!(root, uf.find(4));
    }

    #[test]
    #[should_panic]
    fn test_out_of_bounds() {
        let mut uf = UnionFind::new(5);
        uf.find(10);
    }
}
