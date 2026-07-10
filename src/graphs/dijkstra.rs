//! # Dijkstra's Algorithm (Generic)
//!
//! # Header
//!
//! *   **Problem Name**: Dijkstra's Shortest Path Algorithm
//! *   **Difficulty**: Medium (Graph Algorithms)
//! *   **Link**: <https://en.wikipedia.org/wiki/Dijkstra%27s_algorithm>
//! *   **Why this matters in Rust**: It demonstrates generic programming with traits (`Eq`, `Hash`, `Ord`) and efficient usage of `BinaryHeap`.
//!
//! # Architecture
//!
//! Dijkstra's algorithm finds the shortest paths between nodes in a graph.
//! It uses a **Priority Queue** to explore the nearest unvisited node.
//!
//! **Invariants:**
//! 1.  Edge weights must be non-negative.
//! 2.  Once a node is popped from the PQ, its shortest distance is finalized.
//!
//! **Complexity:**
//!
//! | Operation | Time | Space |
//! | :--- | :--- | :--- |
//! | Find Path | O(E + V log V) | O(V) |
//!
//! where E is edges, V is vertices.
//!
//! ## Single-implementation note
//! Unlike the LeetCode problems in this crate, this file intentionally provides a *single*
//! implementation rather than the brute-force / optimized / optimal trio. The binary-heap
//! Dijkstra shown here is the canonical, textbook-optimal shortest-path algorithm for graphs
//! with non-negative edge weights: O(E + V log V). Plausible "alternatives" are either the same
//! algorithm with a different priority-queue (an O(V^2) array-based Dijkstra is strictly worse and
//! only wins on dense graphs) or solve a different problem (Bellman-Ford handles negative edges,
//! Floyd-Warshall computes all-pairs). None represents a meaningful brute→optimal *progression* of
//! this exact routine, so a single canonical implementation is the honest choice here.

use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};
use std::hash::Hash;

/// Represents a state in the priority queue.
#[derive(Copy, Clone, Eq, PartialEq)]
struct State<N: Eq + Ord, C: Ord> {
    cost: C,
    node: N,
}

// The priority queue depends on `Ord`.
// Explicitly implement the trait so the queue becomes a min-heap cost-wise.
// We want the *smallest* cost to be popped first.
impl<N: Eq + Ord, C: Ord> Ord for State<N, C> {
    fn cmp(&self, other: &Self) -> Ordering {
        // Notice that the we flip the ordering on costs.
        // In case of a tie we compare positions - this step is necessary
        // to make implementations of `PartialEq` and `Ord` consistent.
        other
            .cost
            .cmp(&self.cost)
            .then_with(|| self.node.cmp(&other.node))
    }
}

// `PartialOrd` needs to be consistent with `Ord`.
impl<N: Eq + Ord, C: Ord> PartialOrd for State<N, C> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Computes the shortest path from `start` to `goal`.
///
/// *   `start`: The starting node.
/// *   `goal`: The target node.
/// *   `successors`: A closure that returns a list of `(neighbor, cost)` for a given node.
pub fn dijkstra<N, C, FN, IN>(start: &N, goal: &N, mut successors: FN) -> Option<(Vec<N>, C)>
where
    N: Eq + Hash + Clone + Ord,
    C: Default + Copy + Ord + std::ops::Add<Output = C>,
    FN: FnMut(&N) -> IN,
    IN: IntoIterator<Item = (N, C)>,
{
    let mut dist: HashMap<N, C> = HashMap::new();
    let mut heap = BinaryHeap::new();
    let mut came_from: HashMap<N, N> = HashMap::new();

    // We can't use C::zero() without a Zero trait, so we rely on Default being 0 cost?
    // Or just start with the cost of the first step.
    // For generic C, `Default` usually implies zero/empty.
    dist.insert(start.clone(), C::default());
    heap.push(State {
        cost: C::default(),
        node: start.clone(),
    });

    while let Some(State { cost, node }) = heap.pop() {
        if &node == goal {
            // Reconstruct path
            let mut path = vec![goal.clone()];
            let mut current = goal.clone();
            while let Some(prev) = came_from.get(&current) {
                path.push(prev.clone());
                current = prev.clone();
            }
            path.reverse();
            return Some((path, cost));
        }

        // Important check: if we found a shorter way to `node` already, skip.
        if let Some(&d) = dist.get(&node)
            && cost > d
        {
            continue;
        }

        for (next_node, edge_cost) in successors(&node) {
            let new_cost = cost + edge_cost;
            let next_cost = dist.get(&next_node);

            if next_cost.is_none_or(|&c| new_cost < c) {
                heap.push(State {
                    cost: new_cost,
                    node: next_node.clone(),
                });
                dist.insert(next_node.clone(), new_cost);
                came_from.insert(next_node, node.clone());
            }
        }
    }

    None
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `pathfinding`: The go-to crate for A*, Dijkstra, BFS/DFS in Rust. It uses a very similar API.
// - `petgraph`: A comprehensive graph library. Includes algorithms but tied to its graph structure.
//
// Missing vs. Production:
// - **Zero Trait**: We assume `Default` is zero cost. In production, we'd use `num_traits::Zero`.
// - **Graph Structure**: We use an implicit graph (closure), which is flexible but less optimized than an adjacency list (CSR/CSC) for huge static graphs.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dijkstra_basic() {
        // Graph:
        // A -> B (1)
        // B -> C (2)
        // A -> C (10)
        // Shortest A->C is A->B->C (Cost 3)

        let successors = |node: &char| -> Vec<(char, i32)> {
            match node {
                'A' => vec![('B', 1), ('C', 10)],
                'B' => vec![('C', 2)],
                _ => vec![],
            }
        };

        let result = dijkstra(&'A', &'C', successors);
        assert!(result.is_some());
        let (path, cost) = result.unwrap();
        assert_eq!(cost, 3);
        assert_eq!(path, vec!['A', 'B', 'C']);
    }

    #[test]
    fn test_dijkstra_no_path() {
        let successors = |node: &char| -> Vec<(char, i32)> {
            match node {
                'A' => vec![('B', 1)],
                'C' => vec![],
                _ => vec![],
            }
        };

        let result = dijkstra(&'A', &'C', successors);
        assert!(result.is_none());
    }

    #[test]
    fn test_grid() {
        // 3x3 Grid, move right/down cost 1.
        // Start (0,0), Goal (2,2).
        // Path len should be 4 steps, cost 4.

        let successors = |&(x, y): &(i32, i32)| {
            let mut moves = Vec::new();
            if x < 2 {
                moves.push(((x + 1, y), 1));
            }
            if y < 2 {
                moves.push(((x, y + 1), 1));
            }
            moves
        };

        let result = dijkstra(&(0, 0), &(2, 2), successors);
        assert!(result.is_some());
        let (_, cost) = result.unwrap();
        assert_eq!(cost, 4);
    }
}
