//! # 208. Implement Trie (Prefix Tree)
//!
//! Link: <https://leetcode.com/problems/implement-trie-prefix-tree/>
//!
//! A Trie (pronounced "try") or prefix tree is a tree data structure used to efficiently store and retrieve keys in a dataset of strings.
//! There are various applications of this data structure, such as autocomplete and spellchecker.
//!
//! This problem demonstrates:
//! 1.  **Recursive Data Structures**: Similar to trees but with N children.
//! 2.  **Trade-offs**: Memory vs Speed vs Flexibility (Array vs HashMap).
//! 3.  **Ownership**: Managing recursive `Box<Node>` structures.
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::trees::implement_trie::Trie;
//!
//! let mut trie = Trie::new();
//! trie.insert("apple".to_string());
//! assert!(trie.search("apple".to_string()));   // return True
//! assert!(!trie.search("app".to_string()));    // return False
//! assert!(trie.starts_with("app".to_string())); // return True
//! trie.insert("app".to_string());
//! assert!(trie.search("app".to_string()));     // return True
//! ```
//!
//! ## Constraints
//!
//! - `1 <= word.length, prefix.length <= 2000`
//! - `word` and `prefix` consist only of lowercase English letters.
//! - At most `3 * 10^4` calls in total will be made to `insert`, `search`, and `starts_with`.

use std::collections::{HashMap, HashSet};

// =========================================================================================
// Brute Force Approach
// =========================================================================================

/// Brute Force: Store words in a `HashSet`.
///
/// This is arguably "optimized" for `search` (O(L) average), but fails the spirit of a Trie
/// because `starts_with` becomes O(N * L) where N is the number of words, as we must scan all words.
/// Alternatively, a `Vec<String>` would be O(N * L) for both search and starts_with.
/// Here we use `HashSet` to be slightly less naive, but the prefix search is the killer.
///
/// Time:
/// - insert: O(L)
/// - search: O(L)
/// - starts_with: O(N * L) - Must iterate all words to check prefix.
/// Space: O(N * L) - Store every character of every word.
#[derive(Default)]
pub struct TrieBruteForce {
    words: HashSet<String>,
}

impl TrieBruteForce {
    #[must_use]
    pub fn new() -> Self {
        Self {
            words: HashSet::new(),
        }
    }

    pub fn insert(&mut self, word: String) {
        self.words.insert(word);
    }

    pub fn search(&self, word: String) -> bool {
        self.words.contains(&word)
    }

    pub fn starts_with(&self, prefix: String) -> bool {
        // Linear scan required for prefix check in a hash set
        self.words.iter().any(|w| w.starts_with(&prefix))
    }
}

// =========================================================================================
// Optimized Approach
// =========================================================================================

/// Optimized: Trie using `HashMap`.
///
/// This is the idiomatic Rust approach for a general-purpose Trie.
/// It supports full Unicode characters and sparse data well.
///
/// Time: O(L) for all operations.
/// Space: O(N * L) - potentially more overhead per node (HashMap structure) than array.
///
/// # Rust Insight
/// Using `HashMap` avoids the fixed-size array limitation and handles any valid `char`.
/// However, `HashMap` has memory overhead and hashing cost.
#[derive(Default)]
pub struct TrieOptimized {
    children: HashMap<char, Box<TrieOptimized>>,
    is_end_of_word: bool,
}

impl TrieOptimized {
    #[must_use]
    pub fn new() -> Self {
        Self {
            children: HashMap::new(),
            is_end_of_word: false,
        }
    }

    pub fn insert(&mut self, word: String) {
        let mut current = self;
        for c in word.chars() {
            current = current.children.entry(c).or_insert_with(|| Box::new(TrieOptimized::new()));
        }
        current.is_end_of_word = true;
    }

    pub fn search(&self, word: String) -> bool {
        let mut current = self;
        for c in word.chars() {
            match current.children.get(&c) {
                Some(node) => current = node,
                None => return false,
            }
        }
        current.is_end_of_word
    }

    pub fn starts_with(&self, prefix: String) -> bool {
        let mut current = self;
        for c in prefix.chars() {
            match current.children.get(&c) {
                Some(node) => current = node,
                None => return false,
            }
        }
        true
    }
}

// =========================================================================================
// Optimal Approach
// =========================================================================================

/// Optimal: Trie using Fixed Array `[Option<Box<Node>>; 26]`.
///
/// Tailored for the specific constraints (lowercase English letters).
/// This provides better cache locality and avoids hashing overhead.
///
/// Time: O(L) - Direct array access is very fast.
/// Space: O(N * L * 26) in worst sparse case, but practically compact for dense prefixes.
///
/// # Rust Insight
/// - `Box` is used for recursive definition `struct Node { children: [Option<Box<Node>>; 26] }`.
/// - We use `Default` to initialize the array cleanly.
/// - Indexing with `(c as u8 - b'a') as usize` is safe due to constraints.
#[derive(Default)]
pub struct TrieOptimal {
    children: [Option<Box<TrieOptimal>>; 26],
    is_end_of_word: bool,
}

impl TrieOptimal {
    #[must_use]
    pub fn new() -> Self {
        Self {
            children: Default::default(),
            is_end_of_word: false,
        }
    }

    pub fn insert(&mut self, word: String) {
        let mut current = self;
        for b in word.bytes() {
            let index = (b - b'a') as usize;
            // Get mutable reference to the option, inserting if None
            if current.children[index].is_none() {
                current.children[index] = Some(Box::new(TrieOptimal::new()));
            }
            // Move to the child
            // SAFETY: We just inserted it if it was None.
            current = current.children[index].as_mut().unwrap();
        }
        current.is_end_of_word = true;
    }

    pub fn search(&self, word: String) -> bool {
        let mut current = self;
        for b in word.bytes() {
            let index = (b - b'a') as usize;
            match &current.children[index] {
                Some(node) => current = node,
                None => return false,
            }
        }
        current.is_end_of_word
    }

    pub fn starts_with(&self, prefix: String) -> bool {
        let mut current = self;
        for b in prefix.bytes() {
            let index = (b - b'a') as usize;
            match &current.children[index] {
                Some(node) => current = node,
                None => return false,
            }
        }
        true
    }
}

// Main entry point - uses optimal solution
pub type Trie = TrieOptimal;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_brute_force_basic() {
        let mut trie = TrieBruteForce::new();
        trie.insert("apple".to_string());
        assert!(trie.search("apple".to_string()));
        assert!(!trie.search("app".to_string()));
        assert!(trie.starts_with("app".to_string()));
    }

    #[test]
    fn test_optimized_basic() {
        let mut trie = TrieOptimized::new();
        trie.insert("apple".to_string());
        assert!(trie.search("apple".to_string()));
        assert!(!trie.search("app".to_string()));
        assert!(trie.starts_with("app".to_string()));
    }

    #[test]
    fn test_optimal_basic() {
        let mut trie = TrieOptimal::new();
        trie.insert("apple".to_string());
        assert!(trie.search("apple".to_string()));
        assert!(!trie.search("app".to_string()));
        assert!(trie.starts_with("app".to_string()));
    }

    #[test]
    fn test_starts_with_logic() {
        let mut trie = Trie::new();
        trie.insert("hello".to_string());
        assert!(trie.starts_with("he".to_string()));
        assert!(trie.starts_with("hello".to_string()));
        assert!(!trie.starts_with("helloo".to_string()));
        assert!(!trie.starts_with("ol".to_string()));
    }

    #[test]
    fn test_consistency_all_approaches() {
        let mut t1 = TrieBruteForce::new();
        let mut t2 = TrieOptimized::new();
        let mut t3 = TrieOptimal::new();

        let words = vec!["apple", "app", "apricot", "banana"];
        for w in &words {
            t1.insert(w.to_string());
            t2.insert(w.to_string());
            t3.insert(w.to_string());
        }

        // Test Search
        for w in &words {
            assert!(t1.search(w.to_string()));
            assert!(t2.search(w.to_string()));
            assert!(t3.search(w.to_string()));
        }
        assert!(!t1.search("ap".to_string()));
        assert!(!t2.search("ap".to_string()));
        assert!(!t3.search("ap".to_string()));

        // Test Starts With
        assert!(t1.starts_with("ap".to_string()));
        assert!(t2.starts_with("ap".to_string()));
        assert!(t3.starts_with("ap".to_string()));

        assert!(!t1.starts_with("bananana".to_string()));
        assert!(!t2.starts_with("bananana".to_string()));
        assert!(!t3.starts_with("bananana".to_string()));
    }
}
