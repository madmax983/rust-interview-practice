//! # 211. Design Add and Search Words Data Structure
//!
//! Difficulty: Medium
//! Link: <https://leetcode.com/problems/design-add-and-search-words-data-structure/>
//!
//! Design a data structure that supports adding new words and finding if a string matches any previously added string.
//! The search string can contain the dot character `'.'` to represent any one letter.
//!
//! This problem matters in Rust because it demonstrates how to combine recursive data structures (a Trie)
//! with recursive algorithms (Depth-First Search) while safely managing references and borrowing rules.
//! It also highlights the trade-offs between different string matching approaches.
//!
//! Note: data-structure design problem; a single canonical implementation (or the shown
//! design variants) is the sensible form, so the brute/optimized/optimal progression does
//! not apply. The three types below (`WordDictionaryBruteForce` via `Vec`,
//! `WordDictionaryOptimized` via length-bucketed `HashMap`, `WordDictionaryOptimal` via a
//! Trie + DFS) are genuinely distinct *designs* with different trade-offs, not three tiers
//! of the same algorithm.
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::trees::design_add_and_search_words_data_structure::WordDictionary;
//!
//! let mut word_dictionary = WordDictionary::new();
//! word_dictionary.add_word("bad".to_string());
//! word_dictionary.add_word("dad".to_string());
//! word_dictionary.add_word("mad".to_string());
//! assert!(!word_dictionary.search("pad".to_string())); // return False
//! assert!(word_dictionary.search("bad".to_string()));  // return True
//! assert!(word_dictionary.search(".ad".to_string()));  // return True
//! assert!(word_dictionary.search("b..".to_string()));  // return True
//! ```
//!
//! ## Constraints
//!
//! - `1 <= word.length <= 25`
//! - `word` in `add_word` consists of lowercase English letters.
//! - `word` in `search` consist of `'.'` or lowercase English letters.
//! - There will be at most `2` dots in `word` for `search` queries.
//! - At most `10^4` calls will be made to `add_word` and `search`.

use std::collections::{HashMap, HashSet};

// =========================================================================================
// Brute Force Approach
// =========================================================================================

/// Brute Force: Store all words in a `Vec<String>`.
///
/// This approach is very simple to implement but extremely slow for search.
/// For each search, it scans through all added words and compares them character by character.
///
/// Time:
/// - `add_word`: O(1) - Just pushing to a vector.
/// - `search`: O(N * L) where N is the number of words and L is the length of the word.
/// Space: O(N * L) to store all words.
#[derive(Default)]
pub struct WordDictionaryBruteForce {
    words: Vec<String>,
}

impl WordDictionaryBruteForce {
    #[must_use]
    pub fn new() -> Self {
        Self { words: Vec::new() }
    }

    pub fn add_word(&mut self, word: String) {
        self.words.push(word);
    }

    pub fn search(&self, word: String) -> bool {
        let search_bytes = word.as_bytes();

        for w in &self.words {
            if w.len() != search_bytes.len() {
                continue;
            }

            let w_bytes = w.as_bytes();
            let mut matches = true;

            for i in 0..search_bytes.len() {
                if search_bytes[i] != b'.' && search_bytes[i] != w_bytes[i] {
                    matches = false;
                    break;
                }
            }

            if matches {
                return true;
            }
        }
        false
    }
}

// =========================================================================================
// Optimized Approach
// =========================================================================================

/// Optimized: Group words by length using `HashMap<usize, HashSet<String>>`.
///
/// This is a common practical optimization when building a full Trie feels like overkill.
/// It dramatically reduces the search space by only comparing against words of the exact same length.
///
/// Time:
/// - `add_word`: O(L) to compute hash and insert into HashSet.
/// - `search`: O(M * L) where M is the number of words with the *same length*.
/// Space: O(N * L) to store all words in the hash map.
#[derive(Default)]
pub struct WordDictionaryOptimized {
    length_map: HashMap<usize, HashSet<String>>,
}

impl WordDictionaryOptimized {
    #[must_use]
    pub fn new() -> Self {
        Self {
            length_map: HashMap::new(),
        }
    }

    pub fn add_word(&mut self, word: String) {
        self.length_map.entry(word.len()).or_default().insert(word);
    }

    pub fn search(&self, word: String) -> bool {
        // If there are no words of this length, return early.
        let Some(words) = self.length_map.get(&word.len()) else {
            return false;
        };

        let search_bytes = word.as_bytes();

        for w in words {
            let w_bytes = w.as_bytes();
            let mut matches = true;

            for i in 0..search_bytes.len() {
                if search_bytes[i] != b'.' && search_bytes[i] != w_bytes[i] {
                    matches = false;
                    break;
                }
            }

            if matches {
                return true;
            }
        }

        false
    }
}

// =========================================================================================
// Optimal Approach
// =========================================================================================

/// Trie Node representation using a fixed array.
#[derive(Default)]
struct TrieNode {
    children: [Option<Box<TrieNode>>; 26],
    is_end: bool,
}

impl TrieNode {
    fn new() -> Self {
        Self {
            children: Default::default(),
            is_end: false,
        }
    }
}

/// Optimal: Trie with Depth-First Search for wildcard matching.
///
/// A Trie is the ideal structure for prefix-based or character-by-character string matching.
/// When we encounter a `'.'`, we recursively search all non-null children.
///
/// Time:
/// - `add_word`: O(L) where L is the length of the word.
/// - `search`: O(26^dots * L) in the worst case (if everything matches the prefix).
/// Space: O(N * L * 26) in the worst case, but nodes are heavily shared.
///
/// # Rust Insight
/// We implement the recursive search logic using a separate helper function taking `&[u8]`
/// (a slice of bytes). This allows us to perform zero-cost slice operations (`&chars[1..]`)
/// during the recursive DFS without needing to allocate new strings.
#[derive(Default)]
pub struct WordDictionaryOptimal {
    root: TrieNode,
}

impl WordDictionaryOptimal {
    #[must_use]
    pub fn new() -> Self {
        Self {
            root: TrieNode::new(),
        }
    }

    pub fn add_word(&mut self, word: String) {
        let mut curr = &mut self.root;
        for b in word.bytes() {
            let idx = (b - b'a') as usize;
            // GOTCHA: `get_or_insert_with` on `Option` is extremely useful here
            // to simplify the initialization of child nodes!
            curr = curr.children[idx].get_or_insert_with(|| Box::new(TrieNode::new()));
        }
        curr.is_end = true;
    }

    pub fn search(&self, word: String) -> bool {
        // Delegate to the recursive helper function
        Self::search_dfs(&self.root, word.as_bytes())
    }

    fn search_dfs(node: &TrieNode, chars: &[u8]) -> bool {
        if chars.is_empty() {
            return node.is_end;
        }

        let ch = chars[0];
        if ch == b'.' {
            // RUST INSIGHT: `iter().flatten()` iterates only over the `Some` values
            // in the `[Option<Box<TrieNode>>; 26]` array. This cleanly skips `None` children.
            for child in node.children.iter().flatten() {
                if Self::search_dfs(child, &chars[1..]) {
                    return true;
                }
            }
            false
        } else {
            let idx = (ch - b'a') as usize;
            if let Some(child) = &node.children[idx] {
                Self::search_dfs(child, &chars[1..])
            } else {
                false
            }
        }
    }
}

// Main entry point - uses optimal solution
pub type WordDictionary = WordDictionaryOptimal;

// =========================================================================================
// Alternative Approaches
// =========================================================================================
//
// 1. Regular Expressions (Regex): Constructing a regex pattern per search query (e.g., `^b.d$`).
//    While concise, compiling regex engines on the fly per query is very slow compared to Trie traversal.
// 2. HashMap combined with Trie: In scenarios with a very long wildcard queries but rare words,
//    sometimes caching known queries in a HashMap can speed up exact or common matches.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_brute_force_basic() {
        let mut dict = WordDictionaryBruteForce::new();
        dict.add_word("bad".to_string());
        dict.add_word("dad".to_string());
        dict.add_word("mad".to_string());
        assert!(!dict.search("pad".to_string()));
        assert!(dict.search("bad".to_string()));
        assert!(dict.search(".ad".to_string()));
        assert!(dict.search("b..".to_string()));
    }

    #[test]
    fn test_optimized_basic() {
        let mut dict = WordDictionaryOptimized::new();
        dict.add_word("bad".to_string());
        dict.add_word("dad".to_string());
        dict.add_word("mad".to_string());
        assert!(!dict.search("pad".to_string()));
        assert!(dict.search("bad".to_string()));
        assert!(dict.search(".ad".to_string()));
        assert!(dict.search("b..".to_string()));
    }

    #[test]
    fn test_optimal_basic() {
        let mut dict = WordDictionaryOptimal::new();
        dict.add_word("bad".to_string());
        dict.add_word("dad".to_string());
        dict.add_word("mad".to_string());
        assert!(!dict.search("pad".to_string()));
        assert!(dict.search("bad".to_string()));
        assert!(dict.search(".ad".to_string()));
        assert!(dict.search("b..".to_string()));
    }

    #[test]
    fn test_optimal_edge_cases() {
        let mut dict = WordDictionary::new();

        // Empty dictionary search
        assert!(!dict.search("a".to_string()));
        assert!(!dict.search(".".to_string()));

        // Add single letter words
        dict.add_word("a".to_string());
        assert!(dict.search("a".to_string()));
        assert!(dict.search(".".to_string()));
        assert!(!dict.search("aa".to_string()));
        assert!(!dict.search(".a".to_string()));

        // All dots matching
        dict.add_word("xyz".to_string());
        assert!(dict.search("...".to_string()));
        assert!(!dict.search("....".to_string())); // Length mismatch
    }

    #[test]
    fn test_all_approaches_consistency() {
        let mut d1 = WordDictionaryBruteForce::new();
        let mut d2 = WordDictionaryOptimized::new();
        let mut d3 = WordDictionaryOptimal::new();

        let words = vec!["apple", "apply", "ape", "app"];
        for w in &words {
            d1.add_word(w.to_string());
            d2.add_word(w.to_string());
            d3.add_word(w.to_string());
        }

        let queries = vec![
            ("app", true),
            ("ape", true),
            ("ap.", true),
            ("a..", true),
            ("a..le", true),
            ("app..", true),
            ("a...", false), // Wrong length
            ("b..", false),
            ("apx", false),
        ];

        for (query, expected) in queries {
            assert_eq!(
                d1.search(query.to_string()),
                expected,
                "Failed BruteForce for {}",
                query
            );
            assert_eq!(
                d2.search(query.to_string()),
                expected,
                "Failed Optimized for {}",
                query
            );
            assert_eq!(
                d3.search(query.to_string()),
                expected,
                "Failed Optimal for {}",
                query
            );
        }
    }
}
