//! # Directed Graph (Arena + Indices Pattern)
//!
//! Implements a generic directed graph using the Arena allocator pattern, where nodes and
//! edges are stored in contiguous `Vec`s and referenced by integer indices instead of pointers.
//!
//! **Replaces Crates:** `petgraph`
//!
//! **Real-world Usage:**
//! - Package managers (resolving dependency trees).
//! - Routing algorithms (GPS navigation, network routing).
//! - Compilers (control flow graphs, abstract syntax trees).
//! - Build systems (task dependency resolution like `make` or `cargo`).
//!
//! **Why build it yourself?**
//! Classic pointer-based graphs (like in C++ or Java) are fundamentally at odds with Rust's
//! ownership and borrowing rules. A node cannot simultaneously be owned by the graph and
//! mutably referenced by multiple neighboring nodes without overhead (`Rc<RefCell<T>>`).
//! Building an index-based graph teaches you the idiomatic "Arena" pattern: centralize
//! ownership in vectors and use indices (`usize`) as weak pointers. This achieves better
//! cache locality, circumvents the borrow checker, and uses less memory.
//!
//! # Architecture
//!
//! ```text
//! Graph {
//!     nodes: Vec<NodeData>
//!     edges: Vec<EdgeData>
//! }
//!
//! NodeData [0] "A"           EdgeData [0] "weight 5"
//! first_edge: Some(0) ──────▶ source: 0, target: 1
//!                             next_edge: Some(1)
//!                                   │
//! NodeData [1] "B"                  ▼
//! first_edge: None            EdgeData [1] "weight 2"
//!                             source: 0, target: 2
//! NodeData [2] "C"            next_edge: None
//! first_edge: None
//! ```
//!
//! **Invariants:**
//! - All `NodeIndex` and `EdgeIndex` values must be valid indices into the `nodes` and `edges` vectors.
//! - Edges originating from a node form a linked list via the `next_edge` field, terminating in `None`.
//!
//! **Complexity:**
//! - `add_node`: O(1) amortized
//! - `add_edge`: O(1) amortized
//! - `node_weight(node)`: O(1)
//! - `edge_weight(edge)`: O(1)
//! - `neighbors(node)`: O(E) where E is the out-degree of the node
//!
//! **Tradeoffs:**
//! - **Removals:** Removing a node or edge shifts indices in the `Vec`, invalidating all subsequent indices.
//!   Production crates like `petgraph` handle this by swapping with the last element or using generational arenas,
//!   but this basic implementation omits robust removals to maintain focus on the core indexing pattern.

use std::fmt::Debug;

/// Represents an index to a node in the graph.
// RUST INSIGHT:
// Wrapping `usize` in a tuple struct creates a strongly typed index.
// This prevents bugs where an edge index is accidentally used as a node index,
// enforcing correctness at compile-time with zero runtime cost.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeIndex(pub usize);

/// Represents an index to an edge in the graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EdgeIndex(pub usize);

/// Internal storage for a node.
#[derive(Debug, Clone)]
struct Node<N> {
    /// The actual data stored in the node.
    weight: N,
    /// The index of the first edge originating from this node.
    /// Acts as the head of a linked list of outgoing edges.
    first_outgoing_edge: Option<EdgeIndex>,
}

/// Internal storage for an edge.
#[derive(Debug, Clone)]
struct Edge<E> {
    /// The node index where this edge terminates.
    target: NodeIndex,
    /// The actual data stored in the edge.
    weight: E,
    /// The index of the next edge originating from the same source node.
    /// Acts as the next pointer in the linked list of outgoing edges.
    next_outgoing: Option<EdgeIndex>,
}

/// Core trait defining generic graph operations.
pub trait DirectedGraph<N, E> {
    /// Adds a node to the graph and returns its index.
    fn add_node(&mut self, weight: N) -> NodeIndex;
    /// Adds a directed edge between two existing nodes.
    fn add_edge(&mut self, source: NodeIndex, target: NodeIndex, weight: E) -> EdgeIndex;
    /// Retrieves a reference to a node's weight.
    fn node_weight(&self, id: NodeIndex) -> Option<&N>;
    /// Retrieves a reference to an edge's weight.
    fn edge_weight(&self, id: EdgeIndex) -> Option<&E>;
}

/// A generic directed graph using the arena pattern.
#[derive(Debug, Clone)]
pub struct Graph<N, E> {
    nodes: Vec<Node<N>>,
    edges: Vec<Edge<E>>,
}

impl<N, E> DirectedGraph<N, E> for Graph<N, E> {
    fn add_node(&mut self, weight: N) -> NodeIndex {
        let index = self.nodes.len();
        self.nodes.push(Node {
            weight,
            first_outgoing_edge: None,
        });
        NodeIndex(index)
    }

    fn add_edge(&mut self, source: NodeIndex, target: NodeIndex, weight: E) -> EdgeIndex {
        // GOTCHA:
        // Always validate indices at the API boundary to prevent logic bugs.
        // Panicking early is preferred over subtle logic failures later.
        assert!(
            source.0 < self.nodes.len(),
            "Source node index out of bounds"
        );
        assert!(
            target.0 < self.nodes.len(),
            "Target node index out of bounds"
        );

        let edge_idx = EdgeIndex(self.edges.len());

        let first_outgoing = self.nodes[source.0].first_outgoing_edge;

        self.edges.push(Edge {
            target,
            weight,
            next_outgoing: first_outgoing,
        });

        self.nodes[source.0].first_outgoing_edge = Some(edge_idx);

        edge_idx
    }

    fn node_weight(&self, id: NodeIndex) -> Option<&N> {
        self.nodes.get(id.0).map(|n| &n.weight)
    }

    fn edge_weight(&self, id: EdgeIndex) -> Option<&E> {
        self.edges.get(id.0).map(|e| &e.weight)
    }
}

impl<N, E> Graph<N, E> {
    /// Creates a new, empty graph.
    #[must_use] 
    pub const fn new() -> Self {
        Self {
            nodes: Vec::new(),
            edges: Vec::new(),
        }
    }

    /// Creates a new, empty graph with pre-allocated capacity.
    #[must_use] 
    pub fn with_capacity(nodes: usize, edges: usize) -> Self {
        Self {
            nodes: Vec::with_capacity(nodes),
            edges: Vec::with_capacity(edges),
        }
    }

    /// Returns a mutable reference to the weight of the given node.
    pub fn node_weight_mut(&mut self, id: NodeIndex) -> Option<&mut N> {
        self.nodes.get_mut(id.0).map(|n| &mut n.weight)
    }

    /// Returns an iterator over the neighbor nodes directed from the given source node.
    #[must_use] 
    pub fn neighbors(&self, source: NodeIndex) -> Neighbors<'_, N, E> {
        let current_edge = self.nodes.get(source.0).and_then(|n| n.first_outgoing_edge);
        Neighbors {
            graph: self,
            current_edge,
        }
    }
}

impl<N, E> Default for Graph<N, E> {
    fn default() -> Self {
        Self::new()
    }
}

/// An iterator over the neighbors of a node.
// PRODUCTION NOTE:
// Real implementations often return an iterator of `NodeIndex` directly to avoid
// lifetime coupling with the `Graph` if node weights aren't immediately needed,
// or provide both (`neighbors` and `edges`).
pub struct Neighbors<'a, N, E> {
    graph: &'a Graph<N, E>,
    current_edge: Option<EdgeIndex>,
}

impl<'a, N, E> Iterator for Neighbors<'a, N, E> {
    type Item = (NodeIndex, &'a N, &'a E);

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(edge_idx) = self.current_edge {
            // PRODUCTION NOTE:
            // Using `expect` here instead of `unsafe { get_unchecked() }` is idiomatic safe Rust.
            // Since we maintain the internal invariant that EdgeIndices and NodeIndices are valid,
            // this branch will not panic unless the structure is fundamentally corrupted.
            let edge = self
                .graph
                .edges
                .get(edge_idx.0)
                .expect("Corrupted edge index");
            self.current_edge = edge.next_outgoing;

            let target_node = self
                .graph
                .nodes
                .get(edge.target.0)
                .expect("Corrupted node index");

            Some((edge.target, &target_node.weight, &edge.weight))
        } else {
            None
        }
    }
}

// =========================================================================================
// Benchmarking
// =========================================================================================
// Benchmark graph operations using `criterion`.
// - Measure the performance of `add_node` and `add_edge` against a pointer-based equivalent
//   to show the benefits of cache locality.
// - Benchmark `neighbors` traversal by comparing the time taken to traverse nodes with dense
//   and sparse degrees.
//
// Example `criterion` benchmark for traversal:
// ```rust,ignore
// b.iter(|| {
//     for neighbor in graph.neighbors(black_box(NodeIndex(0))) {
//         black_box(neighbor);
//     }
// })
// ```

// =========================================================================================
// Footer
// =========================================================================================
//
// **Comparison to Canonical Crate (`petgraph`):**
// `petgraph::Graph` implements this exact Arena + Indices pattern but includes robust tracking
// for generational handles or index shifting for node removals (`remove_node`, `remove_edge`).
// `petgraph` also supports undirected graphs out of the box using different internal invariants,
// and it integrates with `FixedBitSet` to manage traversal state (visited nodes).
//
// **What's Missing vs. Production:**
// 1. **Removals:** This implementation only allows adding nodes and edges.
// 2. **Undirected edges:** We assume all edges are directional.
// 3. **Generational Indices:** Indices in this graph can be reused incorrectly if we ever
//    support removals, leading to ABA problems.
//
// **Suggested Next Steps:**
// - Implement a `remove_node` function using the `swap_remove` trick and update all edge indices.
// - Build DFS and BFS iterators around the graph.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_node_and_edge() {
        let mut graph = Graph::new();

        let a = graph.add_node("A");
        let b = graph.add_node("B");
        let c = graph.add_node("C");

        let e1 = graph.add_edge(a, b, 10);
        let e2 = graph.add_edge(a, c, 20);

        assert_eq!(graph.node_weight(a), Some(&"A"));
        assert_eq!(graph.node_weight(b), Some(&"B"));
        assert_eq!(graph.edge_weight(e1), Some(&10));
        assert_eq!(graph.edge_weight(e2), Some(&20));
    }

    #[test]
    fn test_neighbors_iteration() {
        let mut graph = Graph::new();
        let n1 = graph.add_node("Source");
        let n2 = graph.add_node("Target1");
        let n3 = graph.add_node("Target2");

        graph.add_edge(n1, n2, "E1");
        graph.add_edge(n1, n3, "E2");

        // Note: Edges are pushed to the front of the linked list,
        // so we expect them in reverse insertion order.
        let neighbors: Vec<_> = graph.neighbors(n1).collect();
        assert_eq!(neighbors.len(), 2);

        assert_eq!(neighbors[0].0, n3);
        assert_eq!(neighbors[0].1, &"Target2");
        assert_eq!(neighbors[0].2, &"E2");

        assert_eq!(neighbors[1].0, n2);
        assert_eq!(neighbors[1].1, &"Target1");
        assert_eq!(neighbors[1].2, &"E1");
    }

    #[test]
    #[should_panic(expected = "Source node index out of bounds")]
    fn test_invalid_source_edge() {
        let mut graph = Graph::<i32, i32>::new();
        graph.add_edge(NodeIndex(0), NodeIndex(1), 100);
    }

    #[test]
    #[should_panic(expected = "Target node index out of bounds")]
    fn test_invalid_target_edge() {
        let mut graph = Graph::<i32, i32>::new();
        let a = graph.add_node(1);
        graph.add_edge(a, NodeIndex(1), 100);
    }
}
