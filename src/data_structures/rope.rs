//! # Rope (Text Editor Data Structure)
//!
//! A foundational data structure for efficiently storing and manipulating very long strings.
//!
//! **Replaces Crates:** `ropey`, `xi-rope`
//!
//! **Real-world Usage:**
//! - Core text buffer for modern text editors (Zed, Alacritty, VSCode, Sublime Text).
//! - Large JSON or log file viewers where loading the entire string into contiguous memory is impossible or inefficient.
//!
//! **Why build it yourself?**
//! A standard `String` in Rust is a single contiguous block of memory. Inserting a character at the
//! beginning of a 10MB `String` requires copying all 10MB in `O(N)` time. A Rope is a binary tree of string slices,
//! allowing `O(log N)` inserts and deletes. Building a Rope teaches you how to balance recursive tree structures,
//! track weights (character counts) across nodes, and handle string slice boundaries cleanly without fighting the borrow checker.

// =========================================================================================
// Architecture
// =========================================================================================
//
// Data Structure: Binary Tree of Strings
//
//               [Weight: 5]  <-- Internal Node. Weight = sum of lengths in LEFT subtree.
//              /           \
//    [Leaf: "Hello"]      [Weight: 1]
//                        /           \
//               [Leaf: " "]         [Leaf: "World!"]
//
// Invariants:
// 1. **Weight**: Every internal node stores the total length of the string represented by its left child.
// 2. **Leaves**: Only leaf nodes store actual string data. Internal nodes just route queries.
// 3. **Immutability (Optional but common)**: To support undo/redo trees easily, Ropes are often implemented
//    as persistent data structures using `Arc`. Here, we implement a mutable version for simplicity.
//
// Complexity:
// ┌───────────────┬─────────────┬─────────────┐
// │ Operation     │ Time        │ Space       │
// ├───────────────┼─────────────┼─────────────┤
// │ Insert        │ O(log N)    │ O(log N)    │
// │ Delete        │ O(log N)    │ O(log N)    │
// │ Index/Char    │ O(log N)    │ O(1)        │
// │ Concat        │ O(1)        │ O(1)        │
// └───────────────┴─────────────┴─────────────┘
// Note: Time complexity assumes the tree is reasonably balanced.
//
// Design Decisions:
// - **Leaf Size Limit**: Real ropes chunk strings into small arrays (e.g. 512 bytes) to avoid deep trees
//   for every single character insertion. We'll use a `LEAF_MAX` threshold to split strings.
// - **Weights in Bytes vs Chars**: We store lengths in **bytes** to match Rust's native `String` indexing,
//   but a production text editor rope tracks both byte lengths and char/grapheme counts to support `O(log N)`
//   line and column lookups.

use std::cmp;

/// Maximum length of a string in a leaf node before it should be split.
const LEAF_MAX: usize = 32;

/// A node in the Rope binary tree.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Node {
    /// A leaf node containing actual string data.
    Leaf(String),
    /// An internal node containing the weight of its left child, and pointers to left/right children.
    Internal {
        weight: usize,
        left: Box<Node>,
        right: Box<Node>,
    },
}

impl Node {
    /// Returns the total byte length of the string represented by this node.
    fn len(&self) -> usize {
        match self {
            Node::Leaf(s) => s.len(),
            Node::Internal { weight, right, .. } => weight + right.len(),
        }
    }

    /// Concatenates two nodes into a new internal node.
    fn concat(left: Node, right: Node) -> Node {
        // If either is empty, return the other.
        if left.len() == 0 {
            return right;
        }
        if right.len() == 0 {
            return left;
        }

        // RUST INSIGHT: We take ownership of `left` and `right` and box them.
        // This avoids deep copies of the underlying strings, achieving O(1) concatenation.
        let weight = left.len();
        Node::Internal {
            weight,
            left: Box::new(left),
            right: Box::new(right),
        }
    }

    /// Splits the node into two nodes at the given byte index.
    /// Returns a tuple of (Left Node, Right Node).
    fn split(self, index: usize) -> (Node, Node) {
        if index == 0 {
            return (Node::Leaf(String::new()), self);
        }
        if index == self.len() {
            return (self, Node::Leaf(String::new()));
        }

        match self {
            Node::Leaf(s) => {
                // GOTCHA: Splitting a string slice must happen at a valid UTF-8 character boundary.
                // Rust will panic if we split `s` at an invalid byte index.
                // A production rope would enforce character boundaries during insertion/deletion.
                assert!(
                    s.is_char_boundary(index),
                    "Split index is not a char boundary"
                );
                let (left_str, right_str) = s.split_at(index);
                (
                    Node::Leaf(left_str.to_string()),
                    Node::Leaf(right_str.to_string()),
                )
            }
            Node::Internal {
                weight,
                left,
                right,
            } => {
                if index < weight {
                    // Split point is in the left child.
                    let (l_left, l_right) = left.split(index);
                    (l_left, Node::concat(l_right, *right))
                } else if index > weight {
                    // Split point is in the right child.
                    let (r_left, r_right) = right.split(index - weight);
                    (Node::concat(*left, r_left), r_right)
                } else {
                    // Split point is exactly between left and right children.
                    (*left, *right)
                }
            }
        }
    }

    /// Recursively collects the strings from all leaves into a single String.
    fn collect_into(&self, buffer: &mut String) {
        match self {
            Node::Leaf(s) => buffer.push_str(s),
            Node::Internal { left, right, .. } => {
                left.collect_into(buffer);
                right.collect_into(buffer);
            }
        }
    }
}

/// A Rope data structure for efficient string manipulation.
#[derive(Clone, Debug)]
pub struct Rope {
    root: Node,
}

impl Rope {
    /// Creates a new, empty Rope.
    #[must_use]
    pub fn new() -> Self {
        Self {
            root: Node::Leaf(String::new()),
        }
    }

    /// Creates a Rope from a given string.
    #[must_use]
    pub fn from_str(s: &str) -> Self {
        if s.len() <= LEAF_MAX {
            Self {
                root: Node::Leaf(s.to_string()),
            }
        } else {
            // Build a balanced tree by chunking the string.
            let mut rope = Self::new();
            let mut current_idx = 0;
            while current_idx < s.len() {
                // Ensure we chunk at character boundaries
                let mut end = cmp::min(current_idx + LEAF_MAX, s.len());
                while !s.is_char_boundary(end) {
                    end -= 1;
                }

                let chunk = Node::Leaf(s[current_idx..end].to_string());
                rope.root = Node::concat(rope.root, chunk);
                current_idx = end;
            }
            rope
        }
    }

    /// Returns the total byte length of the rope.
    #[must_use]
    pub fn len(&self) -> usize {
        self.root.len()
    }

    /// Returns `true` if the rope contains no characters.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Inserts a string at the given byte index.
    pub fn insert(&mut self, index: usize, text: &str) {
        assert!(index <= self.len(), "Index out of bounds");

        // 1. Split the current rope at the index into Left and Right.
        // RUST INSIGHT: `std::mem::replace` is a powerful tool to take ownership of a struct field
        // without violating the borrow checker. We swap the current root with an empty leaf,
        // split the original root, and then build the new root.
        let old_root = std::mem::replace(&mut self.root, Node::Leaf(String::new()));
        let (left, right) = old_root.split(index);

        // 2. Create a new Node for the inserted text.
        let new_text = Node::Leaf(text.to_string());

        // 3. Concat: Left + NewText + Right
        let temp = Node::concat(left, new_text);
        self.root = Node::concat(temp, right);

        // PRODUCTION NOTE: After many inserts, the tree can become unbalanced (like a linked list),
        // degrading performance to O(N). Production ropes use AVL or Red-Black balancing mechanisms,
        // or a background routine that occasionally flattens and rebuilds small deep subtrees.
    }

    /// Deletes a range of bytes from the rope.
    pub fn delete(&mut self, start: usize, end: usize) {
        assert!(start <= end, "Start index must be <= end index");
        assert!(end <= self.len(), "End index out of bounds");

        let old_root = std::mem::replace(&mut self.root, Node::Leaf(String::new()));

        // 1. Split to get everything before `start`.
        let (left, right_and_deleted) = old_root.split(start);

        // 2. Split the remainder to discard the deleted section.
        // We split at (end - start) because `right_and_deleted`'s indexing starts at 0 internally.
        let (_, right) = right_and_deleted.split(end - start);

        // 3. Concat the remaining parts.
        self.root = Node::concat(left, right);
    }

    /// Collects the rope into a single contiguous `String`.
    #[must_use]
    pub fn to_string(&self) -> String {
        // Pre-allocate the entire capacity to avoid reallocations.
        let mut buffer = String::with_capacity(self.len());
        self.root.collect_into(&mut buffer);
        buffer
    }
}

impl Default for Rope {
    fn default() -> Self {
        Self::new()
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `ropey`: Highly optimized text editor rope. It chunks strings into ~500 byte blocks, stores them
//   contiguously, uses a B-Tree structure instead of a binary tree for massive cache-locality gains,
//   and tracks line breaks/UTF-16 code units simultaneously alongside bytes.
// - `xi-rope`: A persistent (immutable) rope used by the Xi editor. It uses `Arc` heavily to allow
//   easy multi-threading and zero-cost undo trees (you just save an `Arc` to the old root).
//
// Missing vs. Production:
// - **Balancing**: This implementation doesn't automatically rebalance. Sequential inserts will
//   degenerate it into a linked list.
// - **Line/Column indexing**: Editors need to find line numbers. Production ropes track `\n` counts
//   in the internal nodes (alongside weight) to allow O(log N) line lookups.
// - **UTF-8 Safety**: We split strings and insert at raw byte indices. If an index falls inside a multi-byte
//   UTF-8 character, it will panic or corrupt text.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rope_basic_insert() {
        let mut rope = Rope::new();
        rope.insert(0, "World!");
        rope.insert(0, "Hello ");
        assert_eq!(rope.to_string(), "Hello World!");
    }

    #[test]
    fn test_rope_insert_middle() {
        let mut rope = Rope::from_str("Hello!");
        rope.insert(5, " World");
        assert_eq!(rope.to_string(), "Hello World!");
    }

    #[test]
    fn test_rope_delete() {
        let mut rope = Rope::from_str("Hello cruel World!");
        rope.delete(6, 12); // Remove "cruel "
        assert_eq!(rope.to_string(), "Hello World!");
    }

    #[test]
    fn test_rope_delete_start() {
        let mut rope = Rope::from_str("Hello World!");
        rope.delete(0, 6);
        assert_eq!(rope.to_string(), "World!");
    }

    #[test]
    fn test_rope_delete_end() {
        let mut rope = Rope::from_str("Hello World!");
        rope.delete(5, 12);
        assert_eq!(rope.to_string(), "Hello");
    }

    #[test]
    fn test_rope_large_string() {
        // Exceeds LEAF_MAX (32) to force chunking
        let long_str =
            "A quick brown fox jumps over the lazy dog repeatedly to test the chunking mechanism.";
        let mut rope = Rope::from_str(long_str);

        rope.insert(1, " very");
        assert_eq!(
            rope.to_string(),
            "A very quick brown fox jumps over the lazy dog repeatedly to test the chunking mechanism."
        );
    }
}
