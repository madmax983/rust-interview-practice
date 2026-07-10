//! # 133. Clone Graph
//!
//! Link: <https://leetcode.com/problems/clone-graph/>
//!
//! Given a reference of a node in a connected undirected graph, return a deep copy (clone) of the graph.
//! Each node in the graph contains a value (`int`) and a list (`List[Node]`) of its neighbors.
//!
//! This problem is a quintessential graph traversal exercise that forces you to grapple with **cycles** and **shared ownership**.
//! In Rust, it is particularly educational because it demonstrates why `Rc<RefCell<T>>` is the standard pattern
//! for graph nodes when you need shared, mutable ownership in a recursive structure.
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::graphs::clone_graph::{Node, clone_graph};
//! use std::rc::Rc;
//! use std::cell::RefCell;
//!
//! // Create a simple graph: 1 -- 2
//! let node1 = Node::new(1);
//! let node2 = Node::new(2);
//!
//! Node::connect(&node1, &node2);
//!
//! let cloned_node1 = clone_graph(Some(Rc::clone(&node1)));
//!
//! assert_eq!(cloned_node1.unwrap().borrow().val, 1);
//! ```
//!
//! ## Constraints
//!
//! - The number of nodes in the graph is in the range `[0, 100]`.
//! - `1 <= Node.val <= 100`
//! - `Node.val` is unique for each node.
//! - There are no repeated edges and no self-loops in the graph.
//! - The graph is connected and all nodes can be visited starting from the given node.

use std::cell::RefCell;
use std::collections::HashMap;
use std::collections::VecDeque;
use std::rc::Rc;

// =========================================================================================
// Data Structures
// =========================================================================================

/// Definition for a Node.
#[derive(Debug, PartialEq, Eq)]
pub struct Node {
    pub val: i32,
    pub neighbors: Vec<Rc<RefCell<Self>>>,
}

impl Node {
    #[inline]
    #[must_use]
    pub fn new(val: i32) -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(Self {
            val,
            neighbors: Vec::new(),
        }))
    }

    /// Helper to connect two nodes (undirected edge).
    pub fn connect(node1: &Rc<RefCell<Self>>, node2: &Rc<RefCell<Self>>) {
        node1.borrow_mut().neighbors.push(Rc::clone(node2));
        node2.borrow_mut().neighbors.push(Rc::clone(node1));
    }
}

// =========================================================================================
// Brute Force Approach
// =========================================================================================

/// Brute Force: DFS with Linear Scan for Visited Nodes.
///
/// Instead of a HashMap, we use a simple `Vec` to track visited nodes.
/// To check if a node has been visited, we iterate through the `Vec`.
///
/// Time: O(V * V + E) - For each node, we scan the visited list (O(V)). Total O(V^2).
/// Space: O(V) - Recursion stack and tracking list.
///
/// # Why implement this?
/// To demonstrate *why* `HashMap` is essential. For small N (100), this is acceptable,
/// but for large N, the O(V) lookup kills performance.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn clone_graph_brute_force(node: Option<Rc<RefCell<Node>>>) -> Option<Rc<RefCell<Node>>> {
    node.map(|start_node| {
        // Track visited: (original_val, cloned_node)
        // We use `val` as ID since it's unique.
        let mut visited: Vec<(i32, Rc<RefCell<Node>>)> = Vec::new();
        clone_dfs_brute(&start_node, &mut visited)
    })
}

fn clone_dfs_brute(
    node: &Rc<RefCell<Node>>,
    visited: &mut Vec<(i32, Rc<RefCell<Node>>)>,
) -> Rc<RefCell<Node>> {
    let val = node.borrow().val;

    // Linear scan to find if visited
    for (v, cloned) in visited.iter() {
        if *v == val {
            return Rc::clone(cloned);
        }
    }

    // Clone the node (without neighbors initially)
    let new_node = Node::new(val);
    visited.push((val, Rc::clone(&new_node)));

    // RUST INSIGHT: We must drop the borrow of `node` before recursing if we were holding it.
    // However, here we only accessed `val` (Copy), so we don't hold a borrow across the loop.
    // Iterating `neighbors` requires borrowing `node`.
    // BOLT OPTIMIZATION: Avoid cloning the entire neighbors vector

    for neighbor in &node.borrow().neighbors {
        let new_neighbor = clone_dfs_brute(neighbor, visited);
        new_node.borrow_mut().neighbors.push(new_neighbor);
    }

    new_node
}

// =========================================================================================
// Optimized Approach
// =========================================================================================

/// Optimized: DFS with `HashMap`.
///
/// Uses a `HashMap` to store the mapping from `original_val` -> `cloned_node`.
/// This provides O(1) average time complexity for lookups.
///
/// Time: O(V + E) - Each node and edge is processed once.
/// Space: O(V) - Recursion stack and `HashMap`.
///
/// # Idiomatic Rust
/// - Uses `entry` API would be nice, but here we need to insert *before* processing neighbors to handle cycles.
/// - We use `i32` keys instead of `Rc` keys to avoid pointer hashing complexity.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn clone_graph_optimized(node: Option<Rc<RefCell<Node>>>) -> Option<Rc<RefCell<Node>>> {
    node.map(|start_node| {
        let mut visited = HashMap::new();
        clone_dfs_optimized(&start_node, &mut visited)
    })
}

fn clone_dfs_optimized(
    node: &Rc<RefCell<Node>>,
    visited: &mut HashMap<i32, Rc<RefCell<Node>>>,
) -> Rc<RefCell<Node>> {
    let val = node.borrow().val;

    // 1. Check if already visited
    if let Some(cloned) = visited.get(&val) {
        return Rc::clone(cloned);
    }

    // 2. Create clone
    let new_node = Node::new(val);
    visited.insert(val, Rc::clone(&new_node));

    // 3. Recurse on neighbors
    // GOTCHA: We cannot iterate `node.borrow().neighbors` directly while mutating `new_node`
    // inside the loop if we were somehow sharing state incorrectly. But here `new_node` is distinct.
    // However, to be safe and avoid holding the borrow of `node` during recursion (which might
    // circle back to `node`), we collect or clone the neighbor references first.
    //
    // Actually, `node.borrow()` creates a `Ref`. If recursion reaches `node` again,
    // it will try to `borrow()` it again. `RefCell` allows multiple immutable borrows.
    // So holding `node.borrow()` across recursion IS SAFE as long as we don't `borrow_mut()`.
    // But `neighbors` is a `Vec<Rc...>`, so we can just iterate.
    let neighbors = &node.borrow().neighbors;
    for neighbor in neighbors {
        let new_neighbor = clone_dfs_optimized(neighbor, visited);
        new_node.borrow_mut().neighbors.push(new_neighbor);
    }

    new_node
}

// =========================================================================================
// Optimal Approach
// =========================================================================================

/// Optimal: BFS with Direct Address Table (`Vec`).
///
/// Since the problem guarantees `1 <= Node.val <= 100`, we can use a fixed-size `Vec`
/// (or array) instead of a `HashMap`. This eliminates hashing overhead and provides
/// true O(1) access with better cache locality.
///
/// We also use BFS (Iterative) to avoid stack overflow risks for very deep graphs
/// (though N=100 is safe for recursion).
///
/// Time: O(V + E) - Linear traversal.
/// Space: O(V) - Queue and visited array.
///
/// # Panics
/// Panics if `Node.val` is out of bounds (1-100) or if the graph structure violates
/// invariants (e.g., node in queue but not in visited map), though logical construction guarantees safety.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
#[allow(clippy::cast_sign_loss)] // Node.val is positive (1..=100)
pub fn clone_graph_optimal(node: Option<Rc<RefCell<Node>>>) -> Option<Rc<RefCell<Node>>> {
    let start_node = node?;

    // Constraint: 1 <= val <= 100. We need index 100, so size 101.
    // Using `None` to represent unvisited.
    let mut visited: Vec<Option<Rc<RefCell<Node>>>> = vec![None; 101];

    let start_val = start_node.borrow().val;
    let new_start = Node::new(start_val);

    // Mark start as visited
    visited[start_val as usize] = Some(Rc::clone(&new_start));

    // BFS Queue
    let mut queue = VecDeque::new();
    queue.push_back(Rc::clone(&start_node));

    while let Some(current) = queue.pop_front() {
        let current_val = current.borrow().val;

        // Retrieve the clone of the current node
        // SAFETY: We put it in `visited` before pushing to queue.
        let current_clone = visited[current_val as usize].as_ref().unwrap().clone();

        // Iterate neighbors
        for neighbor in &current.borrow().neighbors {
            let neighbor_val = neighbor.borrow().val;

            // If neighbor not visited (not cloned yet)
            if visited[neighbor_val as usize].is_none() {
                // Create clone
                let new_neighbor = Node::new(neighbor_val);
                visited[neighbor_val as usize] = Some(Rc::clone(&new_neighbor));

                // Add to queue
                queue.push_back(Rc::clone(neighbor));
            }

            // Link the clone to the neighbor clone
            let neighbor_clone = visited[neighbor_val as usize].as_ref().unwrap();
            current_clone
                .borrow_mut()
                .neighbors
                .push(Rc::clone(neighbor_clone));
        }
    }

    // Return the clone of the start node
    visited[start_val as usize].clone()
}

/// Main entry point - uses optimal solution.
#[must_use]
pub fn clone_graph(node: Option<Rc<RefCell<Node>>>) -> Option<Rc<RefCell<Node>>> {
    clone_graph_optimal(node)
}

// =========================================================================================
// Tests
// =========================================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // Helper to extract adjacency list from graph for easy comparison
    // Returns map: val -> vec[neighbor_vals]
    fn graph_to_adj(node: Option<Rc<RefCell<Node>>>) -> HashMap<i32, Vec<i32>> {
        let mut adj = HashMap::new();
        if let Some(n) = node {
            let mut visited = HashMap::new();
            let mut stack = vec![n];

            while let Some(curr) = stack.pop() {
                let val = curr.borrow().val;
                if visited.contains_key(&val) {
                    continue;
                }
                visited.insert(val, ());

                let neighbors: Vec<i32> = curr
                    .borrow()
                    .neighbors
                    .iter()
                    .map(|n| n.borrow().val)
                    .collect();
                adj.insert(val, neighbors);

                for neighbor in &curr.borrow().neighbors {
                    stack.push(Rc::clone(neighbor));
                }
            }
        }
        adj
    }

    // Helper to create a simple connected graph: 1 -- 2 -- 3 -- 1 (cycle)
    fn create_cycle_graph() -> Rc<RefCell<Node>> {
        let n1 = Node::new(1);
        let n2 = Node::new(2);
        let n3 = Node::new(3);

        Node::connect(&n1, &n2);
        Node::connect(&n2, &n3);
        Node::connect(&n3, &n1);

        n1
    }

    #[test]
    fn test_brute_force_cycle() {
        let original = create_cycle_graph();
        let cloned = clone_graph_brute_force(Some(Rc::clone(&original)));

        let adj_orig = graph_to_adj(Some(original));
        let adj_cloned = graph_to_adj(cloned);

        assert_eq!(adj_orig, adj_cloned);
    }

    #[test]
    fn test_optimized_cycle() {
        let original = create_cycle_graph();
        let cloned = clone_graph_optimized(Some(Rc::clone(&original)));

        let adj_orig = graph_to_adj(Some(original));
        let adj_cloned = graph_to_adj(cloned);

        assert_eq!(adj_orig, adj_cloned);
    }

    #[test]
    fn test_optimal_cycle() {
        let original = create_cycle_graph();
        let cloned = clone_graph_optimal(Some(Rc::clone(&original)));

        let adj_orig = graph_to_adj(Some(original));
        let adj_cloned = graph_to_adj(cloned);

        assert_eq!(adj_orig, adj_cloned);
    }

    #[test]
    fn test_empty_graph() {
        assert_eq!(clone_graph(None), None);
    }

    #[test]
    fn test_single_node() {
        let node = Node::new(1);
        let cloned = clone_graph(Some(Rc::clone(&node)));

        assert!(cloned.is_some());
        let c = cloned.unwrap();
        assert_eq!(c.borrow().val, 1);
        assert!(c.borrow().neighbors.is_empty());

        // Ensure deep copy (different addresses)
        assert!(!Rc::ptr_eq(&node, &c));
    }

    #[test]
    fn test_all_approaches_agree() {
        // Cross-implementation agreement: all three clones must yield the same adjacency structure.
        let original = create_cycle_graph();
        let expected = graph_to_adj(Some(Rc::clone(&original)));

        let bf = graph_to_adj(clone_graph_brute_force(Some(Rc::clone(&original))));
        let opt = graph_to_adj(clone_graph_optimized(Some(Rc::clone(&original))));
        let optimal = graph_to_adj(clone_graph_optimal(Some(Rc::clone(&original))));

        assert_eq!(bf, expected);
        assert_eq!(opt, expected);
        assert_eq!(optimal, expected);
    }

    #[test]
    fn test_deep_copy_independence() {
        let n1 = Node::new(1);
        let n2 = Node::new(2);
        Node::connect(&n1, &n2);

        let cloned = clone_graph(Some(Rc::clone(&n1))).unwrap();

        // Modify original
        n1.borrow_mut().val = 100;

        // Check clone is unchanged
        assert_eq!(cloned.borrow().val, 1);
        // Clone neighbor should be 2
        assert_eq!(cloned.borrow().neighbors[0].borrow().val, 2);
    }
}
