//! # 207. Course Schedule
//!
//! Link: <https://leetcode.com/problems/course-schedule/>
//!
//! There are a total of `numCourses` courses you have to take, labeled from `0` to `numCourses - 1`.
//! You are given an array `prerequisites` where `prerequisites[i] = [ai, bi]` indicates that you must take course `bi` first if you want to take course `ai`.
//!
//! Return `true` if you can finish all courses. Otherwise, return `false`.
//!
//! This problem is a classic **Topological Sort** / **Cycle Detection** problem.
//! In Rust, it's an excellent opportunity to explore `enum` for state management (Unvisited, Visiting, Visited)
//! and to compare recursion (DFS) vs iteration (Kahn's Algorithm).
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::graphs::course_schedule::can_finish;
//!
//! assert_eq!(can_finish(2, vec![vec![1, 0]]), true);
//! assert_eq!(can_finish(2, vec![vec![1, 0], vec![0, 1]]), false);
//! ```
//!
//! ## Constraints
//!
//! - `1 <= numCourses <= 2000`
//! - `0 <= prerequisites.length <= 5000`
//! - `prerequisites[i].length == 2`
//! - `0 <= ai, bi < numCourses`
//! - All the pairs `prerequisites[i]` are unique.

use std::collections::VecDeque;

// =========================================================================================
// Brute Force Approach
// =========================================================================================

/// Brute Force: Naive DFS with Path Tracking.
///
/// For each course, we start a DFS to see if it leads back to itself.
/// We maintain a `visiting` set for the *current* recursion stack.
/// However, we do NOT maintain a global `visited` set of "safe" nodes across different DFS starts.
/// This means we might re-traverse safe subgraphs multiple times.
///
/// Time: O(V * (V + E)) - In the worst case, we might traverse the entire graph for each node.
/// Space: O(V) - Recursion stack and current path tracking.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn can_finish_brute_force(num_courses: i32, prerequisites: Vec<Vec<i32>>) -> bool {
    let num_courses = num_courses as usize;
    let mut adj = vec![vec![]; num_courses];
    for pair in &prerequisites {
        // [course, pre] -> pre -> course (edge direction matters: pre points to course)
        // b -> a means b must happen before a.
        adj[pair[1] as usize].push(pair[0] as usize);
    }

    // Iterate all nodes to ensure we cover disconnected components
    for i in 0..num_courses {
        let mut visiting = vec![false; num_courses];
        if has_cycle_brute(i, &adj, &mut visiting) {
            return false;
        }
    }
    true
}

fn has_cycle_brute(curr: usize, adj: &[Vec<usize>], visiting: &mut Vec<bool>) -> bool {
    if visiting[curr] {
        return true; // Cycle detected
    }

    // Note: In a true brute force without global visited, we can't easily skip nodes
    // unless we know they are safe. But simply checking `visiting` only detects cycles
    // in the *current* path.
    // If we reach a node with no outgoing edges, it returns false.

    visiting[curr] = true;
    for &neighbor in &adj[curr] {
        if has_cycle_brute(neighbor, adj, visiting) {
            return true;
        }
    }
    visiting[curr] = false; // Backtrack

    false
}

// =========================================================================================
// Optimized Approach
// =========================================================================================

#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    Unvisited,
    Visiting,
    Visited,
}

/// Optimized: DFS with 3-Coloring.
///
/// We use a global state array with three states:
/// - `Unvisited`: Node has not been processed.
/// - `Visiting`: Node is currently in the recursion stack (cycle detected if hit).
/// - `Visited`: Node and all its descendants have been verified safe (memoization).
///
/// Time: O(V + E) - Each node and edge is processed once.
/// Space: O(V) - Recursion stack and state array.
///
/// # RUST INSIGHT
/// Using an `enum` for `State` is more idiomatic and safer than using integers (0, 1, 2).
/// It prevents invalid state values and makes the logic self-documenting.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn can_finish_optimized(num_courses: i32, prerequisites: Vec<Vec<i32>>) -> bool {
    let num_courses = num_courses as usize;
    let mut adj = vec![vec![]; num_courses];
    for pair in &prerequisites {
        adj[pair[1] as usize].push(pair[0] as usize);
    }

    let mut state = vec![State::Unvisited; num_courses];

    for i in 0..num_courses {
        if state[i] == State::Unvisited {
            if has_cycle_optimized(i, &adj, &mut state) {
                return false;
            }
        }
    }
    true
}

fn has_cycle_optimized(curr: usize, adj: &[Vec<usize>], state: &mut Vec<State>) -> bool {
    match state[curr] {
        State::Visiting => return true, // Cycle!
        State::Visited => return false, // Already checked, safe
        State::Unvisited => {}
    }

    state[curr] = State::Visiting;

    for &neighbor in &adj[curr] {
        if has_cycle_optimized(neighbor, adj, state) {
            return true;
        }
    }

    state[curr] = State::Visited;
    false
}

// =========================================================================================
// Optimal Approach
// =========================================================================================

/// Optimal: Kahn's Algorithm (BFS).
///
/// This is an iterative topological sort algorithm.
/// 1. Calculate in-degrees for all nodes.
/// 2. Initialize a queue with all nodes having 0 in-degree.
/// 3. Process the queue: for each node, decrement its neighbors' in-degrees.
/// 4. If a neighbor's in-degree becomes 0, add it to the queue.
/// 5. If we processed all nodes, there is no cycle.
///
/// Time: O(V + E) - We visit every node and edge once.
/// Space: O(V) - Queue and in-degree array.
///
/// # Why this is optimal
/// While the time complexity is the same as DFS, Kahn's algorithm is iterative,
/// avoiding recursion depth limits (stack overflow) for deep graphs.
/// It's also easier to reason about "processing order".
#[must_use]
#[allow(clippy::needless_pass_by_value)]
#[allow(clippy::cast_possible_truncation)] // num_courses fits in usize
#[allow(clippy::cast_sign_loss)] // num_courses is positive
pub fn can_finish_optimal(num_courses: i32, prerequisites: Vec<Vec<i32>>) -> bool {
    let num_courses = num_courses as usize;
    let mut in_degree = vec![0; num_courses];
    let mut adj = vec![vec![]; num_courses];

    for pair in &prerequisites {
        // pair[1] -> pair[0]
        let u = pair[1] as usize;
        let v = pair[0] as usize;
        adj[u].push(v);
        in_degree[v] += 1;
    }

    // GOTCHA: `VecDeque` is generally preferred over `Vec` for queues in Rust to avoid O(N) shifts.
    // Kahn's specifically needs a queue (FIFO) or stack (LIFO) - order doesn't matter for cycle detection,
    // only for topological sort order. We use VecDeque for semantic correctness of BFS.
    let mut queue = VecDeque::new();
    for (i, &d) in in_degree.iter().enumerate() {
        if d == 0 {
            queue.push_back(i);
        }
    }

    let mut count = 0;
    while let Some(u) = queue.pop_front() {
        count += 1;
        for &v in &adj[u] {
            in_degree[v] -= 1;
            if in_degree[v] == 0 {
                queue.push_back(v);
            }
        }
    }

    count == num_courses
}

/// Main entry point.
#[must_use]
pub fn can_finish(num_courses: i32, prerequisites: Vec<Vec<i32>>) -> bool {
    can_finish_optimal(num_courses, prerequisites)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_brute_force_simple() {
        assert!(can_finish_brute_force(2, vec![vec![1, 0]]));
        assert!(!can_finish_brute_force(2, vec![vec![1, 0], vec![0, 1]]));
    }

    #[test]
    fn test_optimized_simple() {
        assert!(can_finish_optimized(2, vec![vec![1, 0]]));
        assert!(!can_finish_optimized(2, vec![vec![1, 0], vec![0, 1]]));
    }

    #[test]
    fn test_optimal_simple() {
        assert!(can_finish_optimal(2, vec![vec![1, 0]]));
        assert!(!can_finish_optimal(2, vec![vec![1, 0], vec![0, 1]]));
    }

    #[test]
    fn test_disconnected_graph() {
        // 0->1, 2->3 (No cycles)
        let prereqs = vec![vec![1, 0], vec![3, 2]];
        assert!(can_finish(4, prereqs.clone()));
    }

    #[test]
    fn test_complex_cycle() {
        // 0->1->2->3->1 (Cycle 1-2-3)
        let prereqs = vec![vec![1, 0], vec![2, 1], vec![3, 2], vec![1, 3]];
        assert!(!can_finish_brute_force(4, prereqs.clone()));
        assert!(!can_finish_optimized(4, prereqs.clone()));
        assert!(!can_finish_optimal(4, prereqs.clone()));
    }

    #[test]
    fn test_empty_graph() {
        assert!(can_finish(5, vec![]));
    }
}
