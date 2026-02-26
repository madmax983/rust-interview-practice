//! # 207. Course Schedule
//!
//! Link: <https://leetcode.com/problems/course-schedule/>
//!
//! There are a total of `numCourses` courses you have to take, labeled from `0` to `numCourses - 1`.
//! You are given an array `prerequisites` where `prerequisites[i] = [ai, bi]` indicates that you must take course `bi` first if you want to take course `ai`.
//! Return `true` if you can finish all courses. Otherwise, return `false`.
//!
//! This is a classic **Topological Sort** / **Cycle Detection** problem.
//! In Rust, it's a perfect opportunity to explore:
//! - **Graph Representation**: Adjacency list with `Vec<Vec<usize>>` (efficient, index-based) vs `HashMap`.
//! - **State Machines**: Using `enum` to represent node states (Unvisited, Visiting, Visited) in DFS.
//! - **Kahn's Algorithm**: Iterative approach using in-degrees, which is often more robust against stack overflow than recursion.
//!
//! ## Rust Insight: Enums for State
//! Instead of using magic integers (0, 1, 2), Rust's `enum` makes the state explicit and the compiler forces you to handle all cases in a `match`.
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::graphs::course_schedule::can_finish;
//!
//! let num_courses = 2;
//! let prerequisites = vec![vec![1, 0]]; // Take 0 before 1
//! assert_eq!(can_finish(num_courses, prerequisites), true);
//!
//! let num_courses = 2;
//! let prerequisites = vec![vec![1, 0], vec![0, 1]]; // Cycle!
//! assert_eq!(can_finish(num_courses, prerequisites), false);
//! ```

use std::collections::VecDeque;

// =========================================================================================
// DFS Approach (Recursive)
// =========================================================================================

/// DFS with 3-Coloring (State Machine)
///
/// We use three states for each node:
/// - `Unvisited`: Not yet processed.
/// - `Visiting`: Currently in the recursion stack (part of the current path). If we encounter a `Visiting` node, there is a cycle.
/// - `Visited`: Already processed and confirmed acyclic.
///
/// Time: O(V + E) - We visit every vertex and edge once.
/// Space: O(V + E) - Adjacency list O(V+E), Recursion stack O(V), State array O(V).
#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    Unvisited,
    Visiting,
    Visited,
}

#[must_use]
pub fn can_finish_dfs(num_courses: i32, prerequisites: Vec<Vec<i32>>) -> bool {
    // GOTCHA: Casting i32 to usize is generally safe for indices if verified positive,
    // but in competitive programming inputs are usually valid within constraints.
    // In production, you'd want `try_into()` or bounds checking.
    let num_courses = num_courses as usize;
    let mut adj = vec![vec![]; num_courses];

    // Build Adjacency List
    // Edge: [course, prereq] -> prereq points to course (prereq -> course)
    // Actually, for cycle detection, direction doesn't strictly matter if we just want "is there a cycle".
    // But topologically, if [a, b] means b -> a, then standard interpretation is:
    // b is a dependency of a.
    for pair in prerequisites {
        let course = pair[0] as usize;
        let prereq = pair[1] as usize;
        adj[prereq].push(course);
    }

    let mut state = vec![State::Unvisited; num_courses];

    for i in 0..num_courses {
        if state[i] == State::Unvisited && has_cycle_dfs(i, &adj, &mut state) {
            return false;
        }
    }

    true
}

fn has_cycle_dfs(node: usize, adj: &[Vec<usize>], state: &mut [State]) -> bool {
    // RUST INSIGHT: Match is exhaustive.
    match state[node] {
        State::Visiting => return true, // Cycle detected!
        State::Visited => return false, // Already checked, no cycle here.
        State::Unvisited => {}          // Continue processing
    }

    // Mark as Visiting (Gray)
    state[node] = State::Visiting;

    for &neighbor in &adj[node] {
        if has_cycle_dfs(neighbor, adj, state) {
            return true;
        }
    }

    // Mark as Visited (Black)
    state[node] = State::Visited;
    false
}

// =========================================================================================
// BFS Approach (Kahn's Algorithm)
// =========================================================================================

/// BFS (Kahn's Algorithm)
///
/// Uses in-degree counting. Nodes with in-degree 0 are "free" (dependencies met).
/// We process them, decrement neighbors' in-degrees, and add new 0-in-degree nodes to the queue.
/// If we process all nodes, there is no cycle.
///
/// Time: O(V + E)
/// Space: O(V + E)
///
/// # Why prefer this?
/// - Iterative (no recursion depth limit).
/// - Easier to extend to return the actual topological sort order.
#[must_use]
pub fn can_finish_bfs(num_courses: i32, prerequisites: Vec<Vec<i32>>) -> bool {
    let num_courses = num_courses as usize;
    let mut adj = vec![vec![]; num_courses];
    let mut in_degree = vec![0; num_courses];

    for pair in prerequisites {
        let course = pair[0] as usize;
        let prereq = pair[1] as usize;
        adj[prereq].push(course);
        in_degree[course] += 1;
    }

    let mut queue = VecDeque::new();
    for i in 0..num_courses {
        if in_degree[i] == 0 {
            queue.push_back(i);
        }
    }

    let mut processed_count = 0;
    while let Some(node) = queue.pop_front() {
        processed_count += 1;

        for &neighbor in &adj[node] {
            in_degree[neighbor] -= 1;
            if in_degree[neighbor] == 0 {
                queue.push_back(neighbor);
            }
        }
    }

    processed_count == num_courses
}

/// Main entry point - uses BFS (Kahn's) by default as it's generally more robust.
#[must_use]
pub fn can_finish(num_courses: i32, prerequisites: Vec<Vec<i32>>) -> bool {
    can_finish_bfs(num_courses, prerequisites)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_dependency() {
        let num_courses = 2;
        let prerequisites = vec![vec![1, 0]];
        assert!(can_finish(num_courses, prerequisites));
    }

    #[test]
    fn test_simple_cycle() {
        let num_courses = 2;
        let prerequisites = vec![vec![1, 0], vec![0, 1]];
        assert!(!can_finish(num_courses, prerequisites));
    }

    #[test]
    fn test_disconnected_graph() {
        // 0 -> 1, 2 -> 3 (No connection between 0-1 and 2-3)
        let num_courses = 4;
        let prerequisites = vec![vec![1, 0], vec![3, 2]];
        assert!(can_finish(num_courses, prerequisites));
    }

    #[test]
    fn test_complex_cycle() {
        // 0 -> 1 -> 2 -> 0
        let num_courses = 3;
        let prerequisites = vec![vec![1, 0], vec![2, 1], vec![0, 2]];
        assert!(!can_finish(num_courses, prerequisites));
    }

    #[test]
    fn test_dfs_implementation() {
        // Explicitly test DFS version
        let num_courses = 2;
        let prerequisites = vec![vec![1, 0], vec![0, 1]];
        assert!(!can_finish_dfs(num_courses, prerequisites));
    }
}
