//! # 684. Redundant Connection
//!
//! Link: <https://leetcode.com/problems/redundant-connection/>
//!
//! In this problem, a tree is an undirected graph that is connected and has no cycles.
//! You are given a graph that started as a tree with `n` nodes labeled from `1` to `n`, with one
//! additional edge added. The added edge has two different vertices chosen from `1` to `n`, and was
//! not an edge that already existed. The graph is represented as an array `edges` of length `n`
//! where `edges[i] = [ai, bi]` indicates that there is an edge between nodes `ai` and `bi` in the graph.
//!
//! Return an edge that can be removed so that the resulting graph is a tree of `n` nodes. If there are
//! multiple answers, return the answer that occurs last in the input.
//!
//! ## Why this matters in Rust
//! This problem provides a perfect opportunity to explore the **Union-Find (Disjoint Set)** data structure.
//! Graph representation in Rust is notoriously challenging due to the borrow checker's strict aliasing rules,
//! which makes pointer-heavy representations (like heavily interlinked structs) cumbersome.
//! Union-Find demonstrates how we can use a flat, contiguous `Vec` of indices to elegantly and efficiently
//! model connectivity without running afoul of ownership and borrowing constraints. It’s an example of
//! "Data-Oriented Design," a very idiomatic pattern in Rust.
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::graphs::redundant_connection::find_redundant_connection;
//!
//! let edges = vec![vec![1, 2], vec![1, 3], vec![2, 3]];
//! let result = find_redundant_connection(edges);
//! assert_eq!(result, vec![2, 3]);
//! ```
//!
//! ## Constraints
//!
//! - `n == edges.length`
//! - `3 <= n <= 1000`
//! - `edges[i].length == 2`
//! - `1 <= ai < bi <= edges.length`
//! - `ai != bi`
//! - There are no repeated edges.
//! - The given graph is connected.

// =========================================================================================
// Approach 1: Brute Force (DFS Cycle Detection)
// =========================================================================================

/// Brute force approach: DFS Cycle Detection
///
/// For each edge `(u, v)`, we can run a Depth-First Search (DFS) on the graph built from all
/// the edges processed so far. If there's already a path between `u` and `v`, then adding
/// the edge `(u, v)` would create a cycle, meaning it's the redundant connection.
///
/// Time: O(V^2) - For each edge, we might traverse up to V nodes.
/// Space: O(V) - For the adjacency list and DFS visit stack.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
#[allow(clippy::cast_possible_truncation)]
#[allow(clippy::cast_possible_wrap)]
pub fn find_redundant_connection_brute_force(edges: Vec<Vec<i32>>) -> Vec<i32> {
    let n = edges.len();
    // Using an adjacency list. Vertices are 1-indexed.
    let mut adj: Vec<Vec<usize>> = vec![vec![]; n + 1];

    for edge in edges {
        let u = edge[0] as usize;
        let v = edge[1] as usize;

        // Run DFS to see if a path already exists from u to v
        let mut visited = vec![false; n + 1];
        if dfs(&adj, u, v, &mut visited) {
            return edge; // This edge creates a cycle
        }

        // Otherwise, add the edge to the graph
        adj[u].push(v);
        adj[v].push(u);
    }

    vec![]
}

fn dfs(adj: &[Vec<usize>], source: usize, target: usize, visited: &mut [bool]) -> bool {
    if source == target {
        return true;
    }
    visited[source] = true;

    for &neighbor in &adj[source] {
        if !visited[neighbor]
            && dfs(adj, neighbor, target, visited) {
                return true;
            }
    }
    false
}

// =========================================================================================
// Approach 2: Optimized (Union-Find without Rank / Path Compression)
// =========================================================================================

/// Optimized approach: Basic Union-Find
///
/// We use a parent array to keep track of disjoint sets.
/// Before adding an edge between `u` and `v`, we find their respective roots in the tree.
/// If they have the same root, adding the edge creates a cycle.
///
/// Time: O(V^2) - In the worst case (a degenerate line graph), `find` takes O(V).
/// Space: O(V) - For the parent array.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
#[allow(clippy::cast_possible_truncation)]
#[allow(clippy::cast_possible_wrap)]
pub fn find_redundant_connection_optimized(edges: Vec<Vec<i32>>) -> Vec<i32> {
    let n = edges.len();
    let mut parent: Vec<usize> = (0..=n).collect(); // 1-indexed

    // Helper for finding root
    fn find(parent: &[usize], mut i: usize) -> usize {
        while parent[i] != i {
            i = parent[i];
        }
        i
    }

    for edge in edges {
        let u = edge[0] as usize;
        let v = edge[1] as usize;

        let root_u = find(&parent, u);
        let root_v = find(&parent, v);

        if root_u == root_v {
            return edge;
        }

        // Union: simply attach one root to the other
        parent[root_u] = root_v;
    }

    vec![]
}

// =========================================================================================
// Approach 3: Optimal (Union-Find with Path Compression and Union by Rank)
// =========================================================================================

/// A custom struct demonstrating encapsulation of the Union-Find logic.
///
/// RUST INSIGHT: By encapsulating `parent` and `rank` vectors, we prevent external
/// manipulation that could break our tree invariants. This highlights Rust's strong
/// emphasis on utilizing types to guarantee safety and correctness.
struct UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
}

impl UnionFind {
    fn new(size: usize) -> Self {
        Self {
            parent: (0..=size).collect(), // 1-indexed arrays
            rank: vec![1; size + 1],
        }
    }

    /// Finds the representative root of `i`, employing Path Compression.
    fn find(&mut self, i: usize) -> usize {
        let mut root = i;
        while root != self.parent[root] {
            // Path halving optimization: make the node point to its grandparent.
            // GOTCHA: We must mutate `parent` to achieve compression, which requires `&mut self`.
            self.parent[root] = self.parent[self.parent[root]];
            root = self.parent[root];
        }
        root
    }

    /// Unites the sets containing `i` and `j`. Returns `false` if they are already united.
    fn union(&mut self, i: usize, j: usize) -> bool {
        let root_i = self.find(i);
        let root_j = self.find(j);

        if root_i == root_j {
            return false; // Cycle detected
        }

        // Union by rank: attach the shorter tree under the root of the taller tree
        match self.rank[root_i].cmp(&self.rank[root_j]) {
            std::cmp::Ordering::Less => {
                self.parent[root_i] = root_j;
            }
            std::cmp::Ordering::Greater => {
                self.parent[root_j] = root_i;
            }
            std::cmp::Ordering::Equal => {
                self.parent[root_j] = root_i;
                self.rank[root_i] += 1;
            }
        }
        true
    }
}

/// Optimal approach: Union-Find with Path Compression and Union by Rank
///
/// Time: O(V * α(V)) ≈ O(V) - Where α is the inverse Ackermann function, which grows
/// extremely slowly (effectively constant).
/// Space: O(V) - For the disjoint set data structures.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
#[allow(clippy::cast_possible_truncation)]
#[allow(clippy::cast_possible_wrap)]
pub fn find_redundant_connection_optimal(edges: Vec<Vec<i32>>) -> Vec<i32> {
    let n = edges.len();
    let mut uf = UnionFind::new(n);

    for edge in edges {
        let u = edge[0] as usize;
        let v = edge[1] as usize;

        if !uf.union(u, v) {
            return edge;
        }
    }

    vec![]
}

/// Main entry point - uses the optimal solution
#[must_use]
pub fn find_redundant_connection(edges: Vec<Vec<i32>>) -> Vec<i32> {
    find_redundant_connection_optimal(edges)
}

// =========================================================================================
// Alternative Approaches Footer
// =========================================================================================
// 1. BFS for Cycle Detection: Similar to the brute force DFS, we could use a Queue to find
//    a path from U to V. The time complexity and space complexities are similar to DFS.
// 2. Disjoint-Set without Rank: We showed this in `find_redundant_connection_optimized`.
//    Sometimes the constant factor overhead of tracking ranks isn't worth it on small graphs,
//    and simply relying on Path Compression is enough in practice.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_brute_force_example_1() {
        let edges = vec![vec![1, 2], vec![1, 3], vec![2, 3]];
        assert_eq!(find_redundant_connection_brute_force(edges), vec![2, 3]);
    }

    #[test]
    fn test_optimized_example_1() {
        let edges = vec![vec![1, 2], vec![1, 3], vec![2, 3]];
        assert_eq!(find_redundant_connection_optimized(edges), vec![2, 3]);
    }

    #[test]
    fn test_optimal_example_1() {
        let edges = vec![vec![1, 2], vec![1, 3], vec![2, 3]];
        assert_eq!(find_redundant_connection_optimal(edges), vec![2, 3]);
    }

    #[test]
    fn test_example_2_longer_cycle() {
        let edges = vec![vec![1, 2], vec![2, 3], vec![3, 4], vec![1, 4], vec![1, 5]];
        let expected = vec![1, 4];
        assert_eq!(
            find_redundant_connection_brute_force(edges.clone()),
            expected
        );
        assert_eq!(find_redundant_connection_optimized(edges.clone()), expected);
        assert_eq!(find_redundant_connection_optimal(edges), expected);
    }

    #[test]
    fn test_all_approaches_edge_case_triangle_at_end() {
        // Line with triangle at the end: 1-2-3-4-5-3
        let edges = vec![vec![1, 2], vec![2, 3], vec![3, 4], vec![4, 5], vec![3, 5]];
        let expected = vec![3, 5];
        assert_eq!(
            find_redundant_connection_brute_force(edges.clone()),
            expected
        );
        assert_eq!(find_redundant_connection_optimized(edges.clone()), expected);
        assert_eq!(find_redundant_connection_optimal(edges), expected);
    }

    #[test]
    fn test_stress_disconnected_trees_joining() {
        // Two disjoint trees getting joined
        let edges = vec![
            vec![1, 2],
            vec![2, 3], // Tree 1
            vec![4, 5],
            vec![5, 6], // Tree 2
            vec![3, 6], // Join Tree 1 & Tree 2
            vec![1, 6], // Forms cycle 1-2-3-6-1
        ];
        let expected = vec![1, 6];
        assert_eq!(
            find_redundant_connection_brute_force(edges.clone()),
            expected
        );
        assert_eq!(find_redundant_connection_optimized(edges.clone()), expected);
        assert_eq!(find_redundant_connection_optimal(edges), expected);
    }
}
