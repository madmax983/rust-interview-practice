// Suppress pedantic and nursery lints for design pattern examples.
#![allow(clippy::pedantic, clippy::nursery, unused)]
//! # Composite Pattern
//!
//! Replaces: **Composite Pattern** (OOP), **Abstract Base Classes for Nodes/Leaves**
//!
//! Real Rust usage: `serde_json::Value`, `html5ever::rcdom::Node`, Abstract Syntax Trees (ASTs) in `syn`
//!
//! ## Why this pattern exists in Rust
//! In OOP, the Composite pattern relies on a shared interface (or abstract base class) implemented by both "Leaf" (individual)
//! and "Composite" (container) nodes, usually resulting in heap allocation and dynamic dispatch (`Box<dyn Component>`).
//!
//! Rust handles hierarchical data structures (trees) in two primary ways:
//! 1. **Enum Dispatch (Sum Types - Closed Set):** The most idiomatic Rust approach. A single Enum defines both Leaf and Composite variants.
//!    This is fast, cache-friendly, requires no dynamic dispatch, and allows the compiler to enforce exhaustive matching.
//! 2. **Trait Objects (Open Set):** Similar to classic OOP. Used when users of the library need to add new node types without recompiling
//!    the base library (e.g., UI frameworks with custom widgets).
//!
//! ## Architecture
//!
//! **Approach 1: Enum Dispatch (Idiomatic)**
//! ```text
//! enum FileSystem {
//!     File(String, u64),                // Leaf
//!     Directory(String, Vec<FileSystem>) // Composite
//! }
//! ```
//!
//! **Approach 2: Trait Objects (Extensible)**
//! ```text
//! trait Component { fn size(&self) -> u64; }
//! struct File { ... }                   // Leaf
//! struct Directory { children: Vec<Box<dyn Component>> } // Composite
//! ```
//!
//! **Invariants:**
//! - A Composite node acts as a container for 0 or more Components (which can be Leaves or other Composites).
//! - Operations on a Composite recursively delegate to their children.
//!
//! ## When to use
//! - **Enum Dispatch:** When modeling a domain where the types of nodes are known and finite (ASTs, JSON, UI layouts).
//! - **Trait Objects:** When building an extensible framework where third parties need to inject custom node types.

// ============================================================================
// Approach 1: Enum Dispatch (Idiomatic Rust)
// ============================================================================

/// Represents a node in a file system using an Enum.
///
/// **COMPILE-TIME WIN:** The compiler guarantees exhaustive pattern matching.
/// If we add a `Symlink` variant later, compilation will fail everywhere we forgot to handle it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileSystemNode {
    /// A leaf node (no children).
    File { name: String, size: u64 },
    /// A composite node (contains children).
    ///
    /// **OWNERSHIP INSIGHT:** We use `Vec<FileSystemNode>` to own the children.
    /// Because the size of `Vec` is known at compile time, we don't need to `Box` the Enum itself.
    Directory {
        name: String,
        children: Vec<FileSystemNode>,
    },
}

impl FileSystemNode {
    /// Recursively calculates the total size of the node.
    #[must_use]
    pub fn total_size(&self) -> u64 {
        match self {
            // Leaf: Just return the size
            Self::File { size, .. } => *size,
            // Composite: Sum the sizes of all children
            Self::Directory { children, .. } => children.iter().map(Self::total_size).sum(),
        }
    }

    /// Recursively searches for a file by name.
    #[must_use]
    pub fn find(&self, target_name: &str) -> Option<&Self> {
        match self {
            Self::File { name, .. } if name == target_name => Some(self),
            Self::File { .. } => None,
            Self::Directory { name, children } => {
                if name == target_name {
                    return Some(self);
                }
                // TRADEOFF: Iterating and recursively searching can be slow for deep trees.
                // An iterative approach or a separate HashMap index might be better for huge file systems.
                children.iter().find_map(|child| child.find(target_name))
            }
        }
    }
}

// ============================================================================
// Approach 2: Trait Objects (Extensible / Open Set)
// ============================================================================

/// The common trait for all UI components.
pub trait UiComponent {
    fn render(&self) -> String;
}

/// A Leaf component.
pub struct TextComponent {
    text: String,
}

impl TextComponent {
    #[must_use]
    pub const fn new(text: String) -> Self {
        Self { text }
    }
}

impl UiComponent for TextComponent {
    fn render(&self) -> String {
        self.text.clone()
    }
}

/// A Composite component.
pub struct WindowComponent {
    title: String,
    // TRADEOFF: We must use `Box<dyn UiComponent>` because we don't know the exact types
    // or sizes of the children at compile time. This requires heap allocation and dynamic dispatch.
    children: Vec<Box<dyn UiComponent>>,
}

impl WindowComponent {
    #[must_use]
    pub const fn new(title: String) -> Self {
        Self {
            title,
            children: Vec::new(),
        }
    }

    pub fn add_child(&mut self, child: Box<dyn UiComponent>) {
        self.children.push(child);
    }
}

impl UiComponent for WindowComponent {
    fn render(&self) -> String {
        use std::fmt::Write;
        // ⚡ BOLT OPTIMIZATION: Use `String::with_capacity` and `write!` to avoid
        // intermediate `format!` allocations and reallocations when appending strings.
        let mut out = String::with_capacity(32 + self.children.len() * 32); // Heuristic
        let _ = writeln!(&mut out, "[Window: {}]", self.title);
        for child in &self.children {
            out.push_str("  ");
            out.push_str(&child.render());
            out.push('\n');
        }
        out
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_enum_composite() {
        let root = FileSystemNode::Directory {
            name: "root".to_string(),
            children: vec![
                FileSystemNode::File {
                    name: "config.toml".to_string(),
                    size: 100,
                },
                FileSystemNode::Directory {
                    name: "src".to_string(),
                    children: vec![
                        FileSystemNode::File {
                            name: "main.rs".to_string(),
                            size: 500,
                        },
                        FileSystemNode::File {
                            name: "lib.rs".to_string(),
                            size: 200,
                        },
                    ],
                },
            ],
        };

        // Test recursive size calculation
        assert_eq!(root.total_size(), 800);

        // Test recursive search
        let found = root.find("main.rs");
        assert!(found.is_some());
        if let Some(FileSystemNode::File { size, .. }) = found {
            assert_eq!(*size, 500);
        } else {
            panic!("Expected to find a file");
        }
    }

    #[test]
    fn test_trait_object_composite() {
        let mut window = WindowComponent::new("Main App".to_string());
        window.add_child(Box::new(TextComponent::new("Hello".to_string())));
        window.add_child(Box::new(TextComponent::new("World".to_string())));

        let mut sub_window = WindowComponent::new("Sidebar".to_string());
        sub_window.add_child(Box::new(TextComponent::new("Link 1".to_string())));
        window.add_child(Box::new(sub_window));

        let rendered = window.render();
        assert!(rendered.contains("[Window: Main App]"));
        assert!(rendered.contains("Hello"));
        assert!(rendered.contains("[Window: Sidebar]"));
        assert!(rendered.contains("Link 1"));
    }
}

// ============================================================================
// Footer
// ============================================================================
//
// How this pattern appears in std/crates:
// - `serde_json::Value`: An enum representing Null, Bool, Number, String, Array (Composite), Object (Composite).
// - HTML/XML parsers (like `html5ever`) use tree structures to represent the DOM.
//
// What the GoF/OOP equivalent is and why it doesn't translate directly:
// OOP uses interfaces and subclassing for everything. Rust prefers Enums (Sum Types) when the set of variants
// is closed. Enums are vastly superior for tree traversals (like ASTs) because you don't need downcasting
// (`instanceof` or `dynamic_cast`) to inspect the node type.
//
// When to reach for this vs. simpler alternatives:
// Use Enum Composites for data definition languages (JSON, ASTs). Use Trait Object Composites only when
// you are building a framework (like an ECS or GUI library) where users provide the concrete types.
//
// Suggested combinations with other patterns in this collection:
// - **Visitor Pattern**: Often used together to traverse and operate on the Enum Composite tree without polluting
//   the nodes with business logic.
//
// PRODUCTION NOTE:
// For extremely deep trees (e.g., parsing a 100MB JSON file), recursive calls can cause Stack Overflow.
// Production implementations often use an explicit `Vec` as a stack to perform iterative DFS traversal
// instead of relying on the call stack.
//
// GOTCHA:
// If an Enum variant holds a Box to itself (e.g., `enum Expr { Add(Box<Expr>, Box<Expr>) }`),
// be aware of the heap allocation overhead. Using `Vec<Expr>` as children avoids `Box` because `Vec`
// itself manages the heap allocation and its size is known.

// ANTI-PATTERN:
// Translating OOP literally into Rust:
// ```rust
// trait Node { fn size(&self) -> u64; }
// struct File { size: u64 }
// struct Dir { children: Vec<Box<dyn Node>> }
// ```
// If you use this for something like an AST, you'll find yourself needing to downcast `Box<dyn Node>`
// back into a `File` or `Dir` to access specific fields, which is slow and unidiomatic in Rust.
// Use Enums instead!
