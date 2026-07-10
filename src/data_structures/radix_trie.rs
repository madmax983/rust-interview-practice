//! # Radix Trie (Compressed Trie)
//!
//! # Header
//!
//! *   **Problem Name**: Radix Trie (Compact Prefix Tree)
//! *   **Difficulty**: Hard
//! *   **Link**: <https://en.wikipedia.org/wiki/Radix_tree>
//! *   **Why this matters in Rust**: Used in HTTP routers (match `/users/:id`), IP routing tables, and efficient string sets.
//!
//! # Architecture
//!
//! A Radix Trie optimizes a standard Trie by merging nodes that have only one child.
//! Each node stores a `prefix` (string segment) instead of a single character.
//!
//! **Structure:**
//!
//! ```text
//! Root
//!  |
//!  +-- "rom" -> Node(value=None)
//!        |
//!        +-- "ane" -> Node(value=1)  (matches "romane")
//!        |
//!        +-- "anus" -> Node(value=2) (matches "romanus")
//! ```
//!
//! **Invariants:**
//! *   Every node's `prefix` is non-empty (except root).
//! *   The key in `children` matches the first character of the child's `prefix`.
//! *   No two children start with the same character.
//!
//! **Complexity:**
//!
//! | Operation | Time | Space |
//! | :--- | :--- | :--- |
//! | Insert | O(K) | O(N * K) |
//! | Lookup | O(K) | O(N * K) |
//!
//! Where K is key length. It is faster than standard Trie for sparse datasets due to fewer pointer chases.
//!
//! # Rust Insight
//!
//! *   **Generics**: `RadixTrie<V>` allows storing any payload.
//! *   **Ownership**: Recursive `Box<Node<V>>` handles memory management automatically.
//! *   **`HashMap`**: We use `HashMap<char, Box<Node<V>>>` for children to allow O(1) branch selection.

use std::collections::HashMap;
use std::fmt::Debug;

#[derive(Debug, Clone)]
struct Node<V> {
    prefix: String,
    children: HashMap<char, Box<Self>>,
    value: Option<V>,
}

impl<V> Node<V> {
    fn new(prefix: String, value: Option<V>) -> Self {
        Self {
            prefix,
            children: HashMap::new(),
            value,
        }
    }
}

impl<V> Default for Node<V> {
    fn default() -> Self {
        Self::new(String::new(), None)
    }
}

#[derive(Debug, Clone, Default)]
pub struct RadixTrie<V> {
    root: Node<V>,
}

impl<V> RadixTrie<V> {
    /// Creates a new empty Radix Trie.
    #[must_use]
    pub fn new() -> Self {
        Self {
            root: Node::new(String::new(), None), // Root has empty prefix
        }
    }

    /// Inserts a key-value pair into the trie.
    pub fn insert(&mut self, key: &str, value: V) {
        Self::insert_recursive(&mut self.root, key, value);
    }

    fn insert_recursive(node: &mut Node<V>, key: &str, value: V) {
        // If key matches the node's prefix exactly (relative to parent), we update value.
        // But here 'key' is the remaining suffix relative to 'node'.
        // If 'key' is empty, it means the value belongs to 'node'.
        if key.is_empty() {
            node.value = Some(value);
            return;
        }

        let first_char = key.chars().next().unwrap();

        // Check if a child exists starting with the first character
        if let Some(mut child) = node.children.remove(&first_char) {
            // Find common prefix between key and child.prefix
            let common_len = Self::common_prefix_len(key, &child.prefix);

            if common_len == child.prefix.len() {
                // Case 1: Child prefix is a prefix of key (e.g., child="ap", key="apple")
                // Recurse down with the remaining part of key
                Self::insert_recursive(&mut child, &key[common_len..], value);
                node.children.insert(first_char, child);
            } else {
                // Case 2: Mismatch occurs (e.g., child="apple", key="app", common="app")
                // OR (child="apple", key="apricot", common="ap")
                // We need to split the child node.

                let common_prefix = child.prefix[..common_len].to_string();
                let child_suffix = child.prefix[common_len..].to_string();
                let key_suffix = key[common_len..].to_string();

                // 1. Modify the existing child to represent the suffix part
                child.prefix = child_suffix;

                // 2. Create a new intermediate node with the common prefix
                let mut split_node = Box::new(Node::new(common_prefix, None));

                // 3. Attach the modified child to the split node
                // The child's new prefix starts with the char at common_len of original prefix
                let child_first_char = child.prefix.chars().next().unwrap();
                split_node.children.insert(child_first_char, child);

                // 4. Handle the new value
                if key_suffix.is_empty() {
                    // The key ended at the split point (e.g., inserting "app" when "apple" exists)
                    split_node.value = Some(value);
                } else {
                    // The key continues (e.g., inserting "apricot" when "apple" exists)
                    let key_suffix_char = key_suffix.chars().next().unwrap();
                    split_node.children.insert(
                        key_suffix_char,
                        Box::new(Node::new(key_suffix, Some(value))),
                    );
                }

                // 5. Insert split node back to parent
                node.children.insert(first_char, split_node);
            }
        } else {
            // Case 3: No matching child. Create a new one.
            node.children.insert(
                first_char,
                Box::new(Node::new(key.to_string(), Some(value))),
            );
        }
    }

    // Helper to calculate the byte length of the common prefix.
    // Returns a BYTE offset (not a char count) so it can be used directly
    // for string slicing without splitting a multi-byte UTF-8 char.
    fn common_prefix_len(s1: &str, s2: &str) -> usize {
        let mut common_len = 0;
        for ((i, c1), c2) in s1.char_indices().zip(s2.chars()) {
            if c1 != c2 {
                return i;
            }
            common_len = i + c1.len_utf8();
        }
        common_len
    }

    /// Get a value by key.
    pub fn get(&self, key: &str) -> Option<&V> {
        let mut current = &self.root;
        let mut remaining_key = key;

        while let Some(first_char) = remaining_key.chars().next() {
            // No child for this character means the key is absent -> `?` yields `None`.
            let child = current.children.get(&first_char)?;
            if remaining_key.starts_with(&child.prefix) {
                remaining_key = &remaining_key[child.prefix.len()..];
                current = child;
            } else {
                // Key diverges from prefix -> Not found
                return None;
            }
        }

        current.value.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_insert_get() {
        let mut trie = RadixTrie::new();
        trie.insert("apple", 1);
        assert_eq!(trie.get("apple"), Some(&1));
        assert_eq!(trie.get("app"), None);
    }

    #[test]
    fn test_prefix_split() {
        let mut trie = RadixTrie::new();
        trie.insert("apple", 1);
        trie.insert("apricot", 2);

        assert_eq!(trie.get("apple"), Some(&1));
        assert_eq!(trie.get("apricot"), Some(&2));

        // "apple" and "apricot" share "ap".
        // Structure should be:
        // root -> "ap" -> "ple" (1)
        //              -> "ricot" (2)
    }

    #[test]
    fn test_insert_prefix_of_existing() {
        let mut trie = RadixTrie::new();
        trie.insert("apple", 1);
        trie.insert("app", 2);

        assert_eq!(trie.get("apple"), Some(&1));
        assert_eq!(trie.get("app"), Some(&2));
    }

    #[test]
    fn test_insert_extension_of_existing() {
        let mut trie = RadixTrie::new();
        trie.insert("app", 1);
        trie.insert("apple", 2);

        assert_eq!(trie.get("app"), Some(&1));
        assert_eq!(trie.get("apple"), Some(&2));
    }

    #[test]
    fn test_multibyte_utf8_shared_prefix() {
        // Regression: multi-byte UTF-8 keys sharing a prefix must not panic
        // on a non-char-boundary byte index during the split.
        let mut trie = RadixTrie::new();
        trie.insert("é1", 1);
        trie.insert("é2", 2);

        assert_eq!(trie.get("é1"), Some(&1));
        assert_eq!(trie.get("é2"), Some(&2));

        // Also exercise a longer shared multi-byte prefix.
        trie.insert("日本語A", 10);
        trie.insert("日本語B", 20);
        assert_eq!(trie.get("日本語A"), Some(&10));
        assert_eq!(trie.get("日本語B"), Some(&20));
    }

    #[test]
    fn test_no_common_prefix() {
        let mut trie = RadixTrie::new();
        trie.insert("apple", 1);
        trie.insert("banana", 2);

        assert_eq!(trie.get("apple"), Some(&1));
        assert_eq!(trie.get("banana"), Some(&2));
    }
}
