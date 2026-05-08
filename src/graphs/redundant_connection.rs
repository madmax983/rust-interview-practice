//! # 684. Redundant Connection
//!
//! Difficulty: Medium
//! Link: <https://leetcode.com/problems/redundant-connection/>
//!
//! In this problem, a tree is an undirected graph that is connected and has no cycles.
//! You are given a graph that started as a tree with `n` nodes labeled from `1` to `n`,
//! with one additional edge added. Return an edge that can be removed so that the
//! resulting graph is a tree of `n` nodes. If there are multiple answers, return the
//! answer that occurs last in the input.
//!
//! This problem perfectly demonstrates graph representation and cycle detection in Rust.
//! Using a custom `UnionFind` struct showcases encapsulation and type system modeling,
//! avoiding raw arrays scattered across a function scope like one might write in C++.
//!
//! ## Approaches
//!
//! 1. **Brute Force DFS**: Before adding each edge `(u, v)`, perform a DFS to see if there is
//!    already a path between `u` and `v`. If there is, adding the edge creates a cycle.
//!    - Time: O(N^2)
//!    - Space: O(N)
//! 2. **Basic Union-Find**: Use a disjoint-set data structure to keep track of connected
//!    components. If two nodes of a new edge share the same parent, they are already connected.
//!    - Time: O(N^2) (worst case for skewed trees)
//!    - Space: O(N)
//! 3. **Optimal Union-Find**: Union-Find with *Path Compression* and *Union by Rank*.
//!    - Time: O(N * α(N)) ≈ O(N), where α is the inverse Ackermann function.
//!    - Space: O(N)
//!
//! In Rust, `UnionFind` is best modeled as a struct implementing `new`, `find`, and `union`
//! methods, allowing for proper state encapsulation. Mutable borrows are safely managed
//! within `find` for path compression.

use std::cmp::Ordering;

// =========================================================================================
// Brute Force Approach
// =========================================================================================

/// Brute force approach: DFS cycle detection.
/// Time: O(N^2) - For each edge, we might traverse up to O(N) existing edges.
/// Space: O(N) - For the adjacency list and visited array.
#[must_use]
#[allow(clippy::needless_pass_by_value)] // Required by LeetCode signature
pub fn find_redundant_connection_brute_force(edges: Vec<Vec<i32>>) -> Vec<i32> {
    let n = edges.len();
    // Use an adjacency list. Nodes are 1-indexed, so we allocate n + 1.
    let mut adj_list = vec![vec![]; n + 1];

    for edge in edges {
        // GOTCHA: It's important to cast to usize for indexing in Rust.
        // It prevents negative indexing bugs common in languages that allow it.
        let u = edge[0] as usize;
        let v = edge[1] as usize;

        let mut visited = vec![false; n + 1];

        // If there's already a path between u and v, this edge is redundant
        if dfs(&adj_list, u, v, &mut visited) {
            return vec![u as i32, v as i32];
        }

        // Otherwise, add the edge to the graph
        adj_list[u].push(v);
        adj_list[v].push(u);
    }

    vec![]
}

fn dfs(adj_list: &[Vec<usize>], current: usize, target: usize, visited: &mut [bool]) -> bool {
    if current == target {
        return true;
    }

    visited[current] = true;

    // RUST INSIGHT: `&adj_list[current]` safely creates an immutable borrow
    // of the neighbor list without copying it.
    for &neighbor in &adj_list[current] {
        if !visited[neighbor] {
            if dfs(adj_list, neighbor, target, visited) {
                return true;
            }
        }
    }

    false
}

// =========================================================================================
// Optimized Approach
// =========================================================================================

/// A basic implementation of a Disjoint Set (Union-Find) without optimizations.
struct BasicUnionFind {
    parent: Vec<usize>,
}

impl BasicUnionFind {
    fn new(size: usize) -> Self {
        Self {
            // Initially, each node is its own parent
            parent: (0..=size).collect(),
        }
    }

    /// Finds the representative of the set containing `node`.
    fn find(&self, mut node: usize) -> usize {
        while node != self.parent[node] {
            node = self.parent[node];
        }
        node
    }

    /// Unites the sets containing `u` and `v`.
    /// Returns `true` if they were in different sets, `false` if they were already in the same set.
    fn union(&mut self, u: usize, v: usize) -> bool {
        let root_u = self.find(u);
        let root_v = self.find(v);

        if root_u == root_v {
            false // A cycle is detected
        } else {
            self.parent[root_u] = root_v;
            true
        }
    }
}

/// Optimized approach: Basic Union-Find.
/// Time: O(N^2) - The tree can become a linked list, making `find` take O(N).
/// Space: O(N) - For the `parent` array.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn find_redundant_connection_optimized(edges: Vec<Vec<i32>>) -> Vec<i32> {
    let n = edges.len();
    let mut uf = BasicUnionFind::new(n);

    for edge in edges {
        let u = edge[0] as usize;
        let v = edge[1] as usize;

        if !uf.union(u, v) {
            return vec![u as i32, v as i32];
        }
    }

    vec![]
}

// =========================================================================================
// Optimal Approach
// =========================================================================================

/// An optimal implementation of a Disjoint Set (Union-Find) using
/// Path Compression and Union by Rank.
struct OptimalUnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
}

impl OptimalUnionFind {
    fn new(size: usize) -> Self {
        Self {
            parent: (0..=size).collect(),
            rank: vec![0; size + 1],
        }
    }

    /// Finds the representative of the set containing `node` with path compression.
    ///
    /// RUST INSIGHT: This requires `&mut self` because path compression mutates the
    /// internal state of `parent` to point directly to the root, speeding up future lookups.
    fn find(&mut self, node: usize) -> usize {
        if self.parent[node] != node {
            // Path compression: recursively find the root and attach the current node to it
            self.parent[node] = self.find(self.parent[node]);
        }
        self.parent[node]
    }

    /// Unites the sets containing `u` and `v` using Union by Rank.
    fn union(&mut self, u: usize, v: usize) -> bool {
        let root_u = self.find(u);
        let root_v = self.find(v);

        if root_u == root_v {
            return false; // Cycle detected
        }

        // Union by rank: attach the shorter tree under the root of the taller tree
        match self.rank[root_u].cmp(&self.rank[root_v]) {
            Ordering::Less => {
                self.parent[root_u] = root_v;
            }
            Ordering::Greater => {
                self.parent[root_v] = root_u;
            }
            Ordering::Equal => {
                self.parent[root_v] = root_u;
                self.rank[root_u] += 1;
            }
        }

        true
    }
}

/// Optimal approach: Union-Find with Path Compression and Union by Rank.
/// Time: O(N * α(N)) ≈ O(N) - Near constant time for each operation.
/// Space: O(N) - For the `parent` and `rank` arrays.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn find_redundant_connection_optimal(edges: Vec<Vec<i32>>) -> Vec<i32> {
    let n = edges.len();
    let mut uf = OptimalUnionFind::new(n);

    // Iterators over `edges` consume the collection without needing to clone.
    for edge in edges {
        let u = edge[0] as usize;
        let v = edge[1] as usize;

        if !uf.union(u, v) {
            return vec![u as i32, v as i32];
        }
    }

    vec![]
}

/// Main entry point - uses optimal solution
#[must_use]
pub fn find_redundant_connection(edges: Vec<Vec<i32>>) -> Vec<i32> {
    find_redundant_connection_optimal(edges)
}

// =========================================================================================
// Tests
// =========================================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // Happy path test: a simple graph with one redundant connection
    #[test]
    fn test_happy_path() {
        let edges = vec![vec![1, 2], vec![1, 3], vec![2, 3]];
        let expected = vec![2, 3];

        assert_eq!(
            find_redundant_connection_brute_force(edges.clone()),
            expected
        );
        assert_eq!(find_redundant_connection_optimized(edges.clone()), expected);
        assert_eq!(find_redundant_connection_optimal(edges), expected);
    }

    // Edge case test: a slightly larger graph
    #[test]
    fn test_edge_case() {
        let edges = vec![vec![1, 2], vec![2, 3], vec![3, 4], vec![1, 4], vec![1, 5]];
        let expected = vec![1, 4];

        assert_eq!(
            find_redundant_connection_brute_force(edges.clone()),
            expected
        );
        assert_eq!(find_redundant_connection_optimized(edges.clone()), expected);
        assert_eq!(find_redundant_connection_optimal(edges), expected);
    }

    // Stress/boundary case: graph as a simple line plus an edge connecting the ends
    #[test]
    fn test_stress_case() {
        let edges = vec![
            vec![1, 2],
            vec![2, 3],
            vec![3, 4],
            vec![4, 5],
            vec![5, 6],
            vec![6, 7],
            vec![7, 8],
            vec![8, 9],
            vec![9, 10],
            vec![1, 10], // This forms a massive cycle involving all nodes
        ];
        let expected = vec![1, 10];

        assert_eq!(
            find_redundant_connection_brute_force(edges.clone()),
            expected
        );
        assert_eq!(find_redundant_connection_optimized(edges.clone()), expected);
        assert_eq!(find_redundant_connection_optimal(edges), expected);
    }
}

// Alternative approaches footer:
// - Breadth-First Search (BFS): Similar to DFS, we can check for paths using a queue.
//   It has the same theoretical time complexity O(N^2) as DFS, but might fail early or
//   find shorter paths faster. However, Union-Find remains the most elegant and optimal
//   choice for dynamically building connectivity components in an undirected graph.
