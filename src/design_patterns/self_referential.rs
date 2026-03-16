// Suppress pedantic and nursery lints for design pattern examples.
#![allow(clippy::pedantic, clippy::nursery, unused)]

//! # Self-Referential Data Structures (Arena Allocator / Indices)
//!
//! Replaces: **Self-Referential Structs**, **Graph Objects** (OOP)
//!
//! Real Rust usage: `petgraph`, `slotmap`, `id-arena`, `rustc` (the Rust compiler itself)
//!
//! ## Why this pattern exists in Rust
//! In OOP (Java, Python, C++), building graphs, trees with parent pointers, or doubly-linked lists
//! is trivial because objects are garbage-collected or manually managed via raw pointers.
//! In Rust, self-referential structs violate the core borrowing rule: you cannot have both
//! owned data and borrowed references to that data in the same struct (aliasing XOR mutation).
//!
//! The highest "I'm still thinking in Java" correction factor involves developers trying to use
//! `Rc<RefCell<Node>>` everywhere to build graphs. This is slow, leaks memory if there are cycles,
//! and panics at runtime. The Rust solution is to use an **Arena Allocator** with **Generational Indices**.
//!
//! ## Architecture
//!
//! **Approach 1: The Anti-Pattern (Rc/RefCell Soup)**
//! ```text
//! struct Node {
//!     value: i32,
//!     parent: Option<Weak<RefCell<Node>>>,
//!     children: Vec<Rc<RefCell<Node>>>
//! }
//! ```
//!
//! **Approach 2: Arena + Indices (Idiomatic Rust)**
//! ```text
//! [ Arena (Vec<Node>) ]
//!   │
//!   ├── Index 0: Node { data: A, next: 1 }
//!   ├── Index 1: Node { data: B, next: 2 }
//!   └── Index 2: Node { data: C, next: 0 }  <-- cycle without memory leaks!
//! ```
//!
//! **Invariants:**
//! - Data is owned by the Arena (`Vec`), not the individual nodes.
//! - References (`&'a Node`) are replaced with `usize` or typed `NodeId` structs.
//! - Borrow checking is deferred from compile-time lifetimes to runtime array bounds checks.
//!
//! ## When to use
//! - Graphs, Trees with parent pointers, GUI widget hierarchies, ECS (Entity Component Systems).
//!
//! ## Anti-patterns
//! - Using `Rc<RefCell<T>>` for simple graphs.
//! - Trying to store a struct and a `&'a T` reference to a field of that same struct.

use std::collections::HashMap;

// ============================================================================
// Approach: Arena Allocator with Typed Indices
// ============================================================================

// ANTI-PATTERN: `Rc<RefCell<Node>>` throughout the graph, causing runtime panics
// and memory leaks if there are cycles.

// COMPILE-TIME WIN: We use a newtype to prevent passing a NodeId to an Edge arena.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(usize);

/// A node in our graph/tree that only stores indices instead of references.
#[derive(Debug)]
pub struct Node {
    pub data: String,
    // OWNERSHIP INSIGHT: Instead of holding `&'a Node` or `Rc<Node>`, we hold indices.
    // The compiler doesn't care about `usize`, so we bypass the borrow checker's self-referential rules entirely.
    pub parent: Option<NodeId>,
    pub children: Vec<NodeId>,
}

/// The Arena owns all the nodes and hands out IDs.
#[derive(Debug, Default)]
pub struct Arena {
    nodes: Vec<Node>,
}

impl Arena {
    pub fn new() -> Self {
        Self { nodes: Vec::new() }
    }

    /// Adds a node to the arena and returns its stable ID.
    pub fn insert(&mut self, data: String) -> NodeId {
        // PRODUCTION NOTE: In real-world code (like `slotmap`), we would return a
        // Generational Index (index + generation counter) to prevent the ABA problem
        // if this index is later removed and reused.
        let id = NodeId(self.nodes.len());
        self.nodes.push(Node {
            data,
            parent: None,
            children: Vec::new(),
        });
        id
    }

    /// Retrieves an immutable reference to a node.
    pub fn get(&self, id: NodeId) -> Option<&Node> {
        self.nodes.get(id.0)
    }

    /// Retrieves a mutable reference to a node.
    pub fn get_mut(&mut self, id: NodeId) -> Option<&mut Node> {
        self.nodes.get_mut(id.0)
    }

    /// Links two nodes together in a parent-child relationship.
    ///
    /// GOTCHA: We must be careful not to hold a reference to the parent while modifying the child,
    /// or we'd violate Rust's aliasing rules (even with indices, `get_mut` requires exclusive access to the `Arena`).
    pub fn add_child(&mut self, parent: NodeId, child: NodeId) {
        // We can't borrow `parent` and `child` mutably at the same time via `self.nodes.get_mut()`.
        // So we do it sequentially.

        if let Some(p) = self.nodes.get_mut(parent.0) {
            p.children.push(child);
        }

        if let Some(c) = self.nodes.get_mut(child.0) {
            c.parent = Some(parent);
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_arena_graph_building() {
        let mut arena = Arena::new();

        // Build a simple tree
        let root = arena.insert("Root".to_string());
        let child1 = arena.insert("Child 1".to_string());
        let child2 = arena.insert("Child 2".to_string());

        arena.add_child(root, child1);
        arena.add_child(root, child2);

        // Verify root
        let root_node = arena.get(root).unwrap();
        assert_eq!(root_node.data, "Root");
        assert_eq!(root_node.children.len(), 2);
        assert_eq!(root_node.children[0], child1);
        assert!(root_node.parent.is_none());

        // Verify child
        let child1_node = arena.get(child1).unwrap();
        assert_eq!(child1_node.data, "Child 1");
        assert_eq!(child1_node.parent, Some(root));

        // META-PATTERN: We represent a graph with back-pointers, yet all memory is contiguous
        // and we have zero memory leaks because dropping the `Arena` drops all `Vec` elements.
    }

    #[test]
    fn test_cyclic_graph() {
        let mut arena = Arena::new();

        let a = arena.insert("A".to_string());
        let b = arena.insert("B".to_string());

        // TRADEOFF: We can easily create cycles because they are just integers.
        // If we used Rc/RefCell, this would be a memory leak unless we carefully used Weak pointers.
        arena.add_child(a, b);
        arena.add_child(b, a);

        let node_a = arena.get(a).unwrap();
        assert_eq!(node_a.children[0], b);

        let node_b = arena.get(b).unwrap();
        assert_eq!(node_b.children[0], a);
    }
}

// ============================================================================
// Footer
// ============================================================================
//
// Comparison to Canonical Crates:
// - `slotmap` / `generational-arena`: This basic implementation suffers from the ABA problem if we
//   remove nodes and reuse their indices. Production crates use "Generational Indices" (combining an
//   index with a generation counter) to ensure deleted slots cannot be incorrectly accessed.
// - `petgraph`: Uses exactly this approach for generic graph structures, storing nodes and edges in `Vec`s.
//
// GoF Equivalent:
// Not a traditional GoF pattern, but entirely replaces the need for the Memento/Composite patterns where
// complex object interdependencies are modeled via references.
//
// When to reach for this vs. simpler alternatives:
// If your structure is strictly a DAG (Directed Acyclic Graph) or simple Tree going top-down, you can use
// nested `Box<Node>` or `Vec<Node>`. Once you need upward-traversal (`parent` pointers) or cycles,
// immediately reach for an Arena.
//
// Suggested combinations with other patterns:
// - **Typestate / Newtype Builder**: Combine with typestates or newtypes to create strictly-typed
//   IDs (e.g., `NodeId` vs `EdgeId` vs `FaceId`) so they cannot be mixed up when querying the arena.
// - **Observer**: If graph modifications need to propagate visually (like in a GUI), the Arena can emit
//   events using the Observer pattern (channels) whenever a node is added or modified.
