//! # 297. Serialize and Deserialize Binary Tree
//!
//! Link: <https://leetcode.com/problems/serialize-and-deserialize-binary-tree/>
//!
//! Serialization is the process of converting a data structure or object into a sequence of bits
//! so that it can be stored in a file or memory buffer, or transmitted across a network connection link
//! to be reconstructed later in the same or another computer environment.
//!
//! This problem is a classic "Systems" interview question that tests your ability to design
//! a data format and implement parsing logic. In Rust, it's particularly instructive because
//! it involves:
//! 1.  **Ownership & Borrowing**: Traversing the tree (borrowing) vs reconstructing it (owning).
//! 2.  **Iterator State**: Consuming tokens from a stream during deserialization.
//! 3.  **Recursion**: Naturally fitting the recursive structure of the tree.
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::trees::serialize_and_deserialize_binary_tree::{Codec, TreeNode};
//!
//! let mut root = TreeNode::new(1);
//! root.left = Some(Box::new(TreeNode::new(2)));
//! root.right = Some(Box::new(TreeNode::new(3)));
//!
//! let codec = Codec::new();
//! let data = codec.serialize(Some(Box::new(root)));
//! let deserialized = codec.deserialize(data);
//!
//! // The deserialized tree should structurally match the original
//! if let Some(node) = deserialized {
//!     assert_eq!(node.val, 1);
//! }
//! ```
//!
//! # Gotchas
//!
//! - **Recursion Depth**: This recursive approach uses O(h) stack space. For very deep trees (skewed),
//!   this could cause a stack overflow. A production system might prefer an iterative approach.
//! - **Whitespace Handling**: We use `split_whitespace` which automatically handles multiple spaces and trims.
//!   If our format allowed empty values or significant whitespace, `split(' ')` would be safer but requires
//!   more careful parsing.
//!
//! ## Constraints
//!
//! - The number of nodes in the tree is in the range `[0, 10000]`.
//! - `-1000 <= Node.val <= 1000`

// Definition for a binary tree node.
#[derive(Debug, PartialEq, Eq)]
pub struct TreeNode {
    pub val: i32,
    pub left: Option<Box<Self>>,
    pub right: Option<Box<Self>>,
}

impl TreeNode {
    #[inline]
    #[must_use]
    pub const fn new(val: i32) -> Self {
        Self {
            val,
            left: None,
            right: None,
        }
    }
}

/// The Codec struct handles serialization and deserialization.
///
/// In a real system, this might hold configuration (e.g., compression settings)
/// or state (e.g., a buffer). Here it's stateless but serves as a namespace.
pub struct Codec;

impl Codec {
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Serializes a tree to a single string.
    ///
    /// Uses Pre-order Traversal (Root -> Left -> Right).
    /// Represents `None` as "N" and separates values with spaces.
    ///
    /// Time: O(n) - Visits every node once.
    /// Space: O(n) - The string grows linearly with the number of nodes.
    ///
    /// # Rust Insight
    /// We use a `String` buffer and append to it. This is more efficient than
    /// repeated concatenation (`format!`) which would allocate new strings constantly.
    pub fn serialize(&self, root: Option<Box<TreeNode>>) -> String {
        let mut out = String::new();
        self.serialize_helper(&root, &mut out);
        out
    }

    fn serialize_helper(&self, node: &Option<Box<TreeNode>>, out: &mut String) {
        match node {
            Some(n) => {
                // Pre-order: Process root, then left, then right
                out.push_str(&n.val.to_string());
                out.push(' ');
                self.serialize_helper(&n.left, out);
                self.serialize_helper(&n.right, out);
            }
            None => {
                out.push_str("N ");
            }
        }
    }

    /// Deserializes your encoded data to tree.
    ///
    /// Parses the string back into a tree structure using the same Pre-order logic.
    ///
    /// Time: O(n) - Processes every token once.
    /// Space: O(h) - Recursion stack depth.
    ///
    /// # Rust Insight: Iterator as State
    /// We use `std::str::SplitWhitespace` as our token stream. By passing it
    /// as a mutable reference (`&mut Iterator`), we ensure that recursive calls
    /// consume tokens from the *same* iterator instance, effectively maintaining
    /// the "current position" in the stream without manual index management.
    #[must_use]
    pub fn deserialize(&self, data: String) -> Option<Box<TreeNode>> {
        let mut tokens = data.split_whitespace();
        self.deserialize_helper(&mut tokens)
    }

    fn deserialize_helper<'a, I>(&self, tokens: &mut I) -> Option<Box<TreeNode>>
    where
        I: Iterator<Item = &'a str>,
    {
        // Get the next token
        let token = tokens.next()?;

        // If it's our null marker "N", return None
        if token == "N" {
            return None;
        }

        // Otherwise, parse the value and recurse
        // RUST INSIGHT: parse() returns a Result. In a robust system we'd handle errors,
        // but here we assume valid input per problem constraints, so unwrap is "acceptable"
        // for LeetCode context, though expect() is better for debugging.
        let val = token
            .parse::<i32>()
            .expect("Invalid number in serialized data");

        let mut node = TreeNode::new(val);
        node.left = self.deserialize_helper(tokens);
        node.right = self.deserialize_helper(tokens);

        Some(Box::new(node))
    }
}

// Satisfy clippy::default_trait_access
impl Default for Codec {
    fn default() -> Self {
        Self::new()
    }
}

/// # Alternative Approaches
///
/// 1. **Breadth-First Search (Level Order)**:
///    Instead of DFS, use a Queue to serialize level by level. This is often
///    more intuitive for humans to read but requires slightly more complex
///    state management (Queue) during deserialization compared to the
///    implicit stack of recursion.
///
/// 2. **Binary Format**:
///    For production, serializing to a text string "1 2 3 N N" is inefficient.
///    A binary format (like Protocol Buffers or a custom byte stream) would be
///    much more compact and faster to parse.
///
/// 3. **Parentheses Notation**:
///    "1(2)(3)" - This is another common format (like Lisp s-expressions),
///    but parsing it often requires a more complex parser or stack-based approach.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_happy_path() {
        // Tree:
        //      1
        //     / \
        //    2   3
        //       / \
        //      4   5
        let mut root = TreeNode::new(1);
        root.left = Some(Box::new(TreeNode::new(2)));
        let mut right = TreeNode::new(3);
        right.left = Some(Box::new(TreeNode::new(4)));
        right.right = Some(Box::new(TreeNode::new(5)));
        root.right = Some(Box::new(right));

        let codec = Codec::new();
        let serialized = codec.serialize(Some(Box::new(root)));

        // Expected: "1 2 N N 3 4 N N 5 N N "

        let deserialized = codec.deserialize(serialized);

        // Verify structure
        let node = deserialized.unwrap();
        assert_eq!(node.val, 1);
        assert_eq!(node.left.unwrap().val, 2);

        let right = node.right.unwrap();
        assert_eq!(right.val, 3);
        assert_eq!(right.left.unwrap().val, 4);
        assert_eq!(right.right.unwrap().val, 5);
    }

    #[test]
    fn test_empty_tree() {
        let codec = Codec::new();
        let serialized = codec.serialize(None);
        let deserialized = codec.deserialize(serialized);
        assert_eq!(deserialized, None);
    }

    #[test]
    fn test_single_node() {
        let codec = Codec::new();
        let root = Some(Box::new(TreeNode::new(42)));
        let serialized = codec.serialize(root);
        let deserialized = codec.deserialize(serialized);
        assert_eq!(deserialized.unwrap().val, 42);
    }

    #[test]
    fn test_negative_values() {
        let codec = Codec::new();
        let root = Some(Box::new(TreeNode::new(-10)));
        let serialized = codec.serialize(root);
        let deserialized = codec.deserialize(serialized);
        assert_eq!(deserialized.unwrap().val, -10);
    }
}
