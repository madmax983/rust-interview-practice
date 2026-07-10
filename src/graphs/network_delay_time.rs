//! # 743. Network Delay Time
//!
//! Difficulty: Medium
//! Link: <https://leetcode.com/problems/network-delay-time>/
//!
//! You are given a network of `n` nodes, labeled from 1 to `n`. You are also given `times`, a list of
//! travel times as directed edges `times[i] = (u, v, w)`, where `u` is the source node, `v` is the
//! target node, and `w` is the time it takes for a signal to travel from source to target.
//!
//! We will send a signal from a given node `k`. Return the minimum time it takes for all the
//! `n` nodes to receive the signal. If it is impossible for all the `n` nodes to receive the signal, return -1.
//!
//! This problem is a classic application of Dijkstra's Algorithm for finding the shortest path in a
//! weighted graph with non-negative edge weights. In Rust, it demonstrates the effective use of
//! `std::collections::BinaryHeap` as a priority queue and how to implement custom ordering traits (`Ord`, `PartialOrd`)
//! to achieve Min-Heap behavior from a standard Max-Heap.

use std::cmp::Ordering;
use std::collections::BinaryHeap;

/// Represents a state in the priority queue.
/// We store the `cost` (time) to reach a `position` (node).
#[derive(Copy, Clone, Eq, PartialEq)]
struct State {
    cost: i32,
    position: usize,
}

// RUST INSIGHT: The standard library `BinaryHeap` is a Max-Heap.
// To use it as a Min-Heap (required for Dijkstra's), we must reverse the ordering.
// When comparing two `State` instances, we compare their `cost` fields in reverse.
impl Ord for State {
    fn cmp(&self, other: &Self) -> Ordering {
        // Notice the order of arguments: `other.cost.cmp(&self.cost)`
        // This makes the element with the SMALLER cost appear "greater" to the Max-Heap,
        // so it will be popped first.
        other
            .cost
            .cmp(&self.cost)
            // If costs are equal, use position as a tie-breaker (arbitrary but deterministic)
            .then_with(|| self.position.cmp(&other.position))
    }
}

// `PartialOrd` must be consistent with `Ord`.
impl PartialOrd for State {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Brute force approach: Bellman-Ford
///
/// Relax every edge `V - 1` times. After `V - 1` full passes, all shortest paths from the
/// source are finalized (a simple path visits at most `V` nodes / `V - 1` edges). This does not
/// need a priority queue and, unlike Dijkstra, also tolerates negative edge weights (not required
/// here, but it is why Bellman-Ford is the natural "simpler but slower" baseline).
///
/// Time: O(V * E) - `V - 1` passes, each relaxing all `E` edges.
/// Space: O(V) - just the distance vector (edges are read directly from `times`).
#[must_use]
pub fn network_delay_time_brute_force(times: Vec<Vec<i32>>, n: i32, k: i32) -> i32 {
    let n = n as usize;
    let mut dist = vec![i32::MAX; n];
    let start_node = (k - 1) as usize;
    dist[start_node] = 0;

    // Relax all edges up to V - 1 times.
    for _ in 0..n.saturating_sub(1) {
        let mut updated = false;
        for edge in &times {
            let u = (edge[0] - 1) as usize;
            let v = (edge[1] - 1) as usize;
            let w = edge[2];

            // Only relax from a reachable node; guard against i32::MAX + w overflow.
            if dist[u] != i32::MAX
                && let Some(next_cost) = dist[u].checked_add(w)
                && next_cost < dist[v]
            {
                dist[v] = next_cost;
                updated = true;
            }
        }
        // Early exit: a pass with no relaxation means we've converged.
        if !updated {
            break;
        }
    }

    let max_dist = *dist.iter().max().unwrap();
    if max_dist == i32::MAX { -1 } else { max_dist }
}

/// Optimal approach: Dijkstra's Algorithm with a binary-heap priority queue
///
/// Time: O(E log V) - each edge can push one entry onto the heap; heap ops are O(log V).
/// Space: O(N + E) for the adjacency list and distance vector.
///
/// Steps:
/// 1. Build an adjacency list from the `times` vector.
/// 2. Initialize a `dist` vector with `i32::MAX` to represent infinity.
/// 3. Push the starting node `k` into the priority queue with cost 0.
/// 4. While the priority queue is not empty:
///    a. Pop the node with the smallest cost.
///    b. If we found a shorter path to this node before, skip.
///    c. Iterate through neighbors. If `new_cost < old_cost`, update `dist` and push to queue.
/// 5. After the loop, find the maximum value in `dist`. If it's `i32::MAX`, return -1 (unreachable nodes exist).
#[must_use]
pub fn network_delay_time_optimal(times: Vec<Vec<i32>>, n: i32, k: i32) -> i32 {
    let n = n as usize;
    // GOTCHA: The problem uses 1-based indexing for nodes (1 to n).
    // Idiomatic Rust uses 0-based indexing. We'll adjust indices when accessing the graph.
    // We convert `k` to 0-based at the start.

    // Build Adjacency List
    // graph[u] -> vec![(v, w)]
    let mut graph = vec![vec![]; n];
    for time in times {
        let u = (time[0] - 1) as usize;
        let v = (time[1] - 1) as usize;
        let w = time[2];
        graph[u].push((v, w));
    }

    // Distances array. `dist[i]` is the minimum time to reach node `i`.
    // Initialize with "infinity".
    let mut dist = vec![i32::MAX; n];

    // Priority Queue for Dijkstra
    let mut pq = BinaryHeap::new();

    // Start with node `k` (converted to 0-based index)
    let start_node = (k - 1) as usize;
    dist[start_node] = 0;
    pq.push(State {
        cost: 0,
        position: start_node,
    });

    while let Some(State { cost, position }) = pq.pop() {
        // RUST INSIGHT: We might have multiple entries for the same node in the heap
        // (lazy deletion). If we pop a state with a cost higher than what we've already found,
        // it means we found a shorter path to `position` previously, so we ignore this one.
        if cost > dist[position] {
            continue;
        }

        // Explore neighbors
        for &(neighbor, weight) in &graph[position] {
            // Check for overflow before adding
            // Although constraints usually prevent this, it's good practice.
            if let Some(next_cost) = cost.checked_add(weight) {
                // If we found a shorter path to the neighbor
                if next_cost < dist[neighbor] {
                    dist[neighbor] = next_cost;
                    pq.push(State {
                        cost: next_cost,
                        position: neighbor,
                    });
                }
            }
        }
    }

    // The answer is the maximum time in the `dist` array, because the signal must reach *all* nodes.
    // If any node is unreachable (`i32::MAX`), return -1.
    // RUST INSIGHT: `iter().max()` returns an Option<&T>. We unwrap safely because `dist` is non-empty (n >= 1).
    let max_dist = *dist.iter().max().unwrap();

    if max_dist == i32::MAX { -1 } else { max_dist }
}

/// Main entry point - uses the optimal (Dijkstra) solution.
#[must_use]
pub fn network_delay_time(times: Vec<Vec<i32>>, n: i32, k: i32) -> i32 {
    network_delay_time_optimal(times, n, k)
}

// =========================================================================================
// Alternative approaches
// =========================================================================================
//
// 1. Bellman-Ford Algorithm (implemented above as `network_delay_time_brute_force`):
//    - Also handles negative weight edges (which Dijkstra cannot).
//    - Time Complexity: O(V * E).
//
// 2. SPFA (Shortest Path Faster Algorithm):
//    - An optimization of Bellman-Ford using a queue.
//    - Average case O(E), worst case O(V * E).
//
// 3. Floyd-Warshall Algorithm:
//    - Computes all-pairs shortest paths.
//    - Time Complexity: O(V^3).
//    - Useful if N is very small (e.g., N <= 100).

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_example_1() {
        // Input: times = [[2,1,1],[2,3,1],[3,4,1]], n = 4, k = 2
        // Output: 2
        let times = vec![vec![2, 1, 1], vec![2, 3, 1], vec![3, 4, 1]];
        assert_eq!(network_delay_time(times, 4, 2), 2);
    }

    #[test]
    fn test_example_2() {
        // Input: times = [[1,2,1]], n = 2, k = 1
        // Output: 1
        let times = vec![vec![1, 2, 1]];
        assert_eq!(network_delay_time(times, 2, 1), 1);
    }

    #[test]
    fn test_example_3() {
        // Input: times = [[1,2,1]], n = 2, k = 2
        // Output: -1 (Node 1 is unreachable from Node 2)
        let times = vec![vec![1, 2, 1]];
        assert_eq!(network_delay_time(times, 2, 2), -1);
    }

    #[test]
    fn test_disconnected_graph() {
        // 1 -> 2, 3 (isolated)
        let times = vec![vec![1, 2, 10]];
        assert_eq!(network_delay_time(times, 3, 1), -1);
    }

    #[test]
    fn test_complex_graph() {
        // 1 -> 2 (1)
        // 2 -> 3 (2)
        // 1 -> 3 (4)
        // Path 1->2->3 is cost 3. Path 1->3 is cost 4.
        // Shortest to 3 is 3. Max time to reach all (2 and 3) is 3.
        let times = vec![vec![1, 2, 1], vec![2, 3, 2], vec![1, 3, 4]];
        assert_eq!(network_delay_time(times, 3, 1), 3);
    }

    #[test]
    fn test_brute_force_bellman_ford() {
        // Same cases as the LeetCode examples, exercised via Bellman-Ford directly.
        let times = vec![vec![2, 1, 1], vec![2, 3, 1], vec![3, 4, 1]];
        assert_eq!(network_delay_time_brute_force(times, 4, 2), 2);

        let times = vec![vec![1, 2, 1]];
        assert_eq!(network_delay_time_brute_force(times, 2, 2), -1);
    }

    #[test]
    fn test_all_approaches_agree() {
        // Cross-implementation agreement between Bellman-Ford (brute force) and Dijkstra (optimal).
        let cases: Vec<(Vec<Vec<i32>>, i32, i32)> = vec![
            (vec![vec![2, 1, 1], vec![2, 3, 1], vec![3, 4, 1]], 4, 2),
            (vec![vec![1, 2, 1]], 2, 1),
            (vec![vec![1, 2, 1]], 2, 2),
            (vec![vec![1, 2, 1], vec![2, 3, 2], vec![1, 3, 4]], 3, 1),
            (vec![vec![1, 2, 10]], 3, 1),
        ];

        for (times, n, k) in cases {
            let bf = network_delay_time_brute_force(times.clone(), n, k);
            let optimal = network_delay_time_optimal(times, n, k);
            assert_eq!(bf, optimal);
        }
    }
}
