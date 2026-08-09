//! # 269. Alien Dictionary
//!
//! Difficulty: Hard
//! Link: <https://leetcode.com/problems/alien-dictionary/>
//!
//! There is a new alien language that uses the English alphabet. However, the order among the letters is unknown to you.
//! You are given a list of strings `words` from the alien language's dictionary, where the strings in `words` are
//! sorted lexicographically by the rules of this new language.
//! Return a string of the unique letters in the new alien language sorted in lexicographically increasing order by the new language's rules.
//! If there is no solution, return `""`. If there are multiple solutions, return any of them.
//!
//! ## Why This Matters in Rust
//!
//! This problem perfectly demonstrates graph modeling and Kahn's Algorithm (Topological Sort) in Rust.
//! By using `std::collections::HashMap` for both the adjacency list and the in-degree count, we can elegantly
//! express the graph relationships without running into borrow checker issues. It also showcases Rust's robust
//! `Option` and `Result` handling (though we use Option implicitly here via early returns) and iterator chaining
//! when processing string bytes.
//!
//! ## Approach
//!
//! We use Topological Sort (Kahn's Algorithm).
//!
//! 1. **Graph Construction**: Iterate through adjacent pairs of words. For the first differing character between `word1` and `word2`,
//!    add a directed edge from `word1[i]` to `word2[i]` in our adjacency list, and increment the in-degree of `word2[i]`.
//!    - **GOTCHA**: If `word1` is a prefix of `word2`, it is valid. However, if `word2` is a prefix of `word1` (e.g., `["abc", "ab"]`),
//!      the dictionary is invalid and we must return an empty string immediately.
//! 2. **Initialization**: We collect all unique characters present in the `words`. Any character without an in-degree edge gets an in-degree of 0.
//! 3. **Queue Processing**: Push all characters with an in-degree of 0 into a `VecDeque`. Pop them one by one, append to our result string,
//!    and decrement the in-degree of their neighbors. If a neighbor's in-degree reaches 0, push it to the queue.
//! 4. **Cycle Detection**: If the resulting string length doesn't match the total number of unique characters, a cycle exists, and we return `""`.
//!
//! **Time Complexity:** O(C), where C is the total length of all words in the input array. We examine each character at most a few times.
//! **Space Complexity:** O(U + E) = O(1) in this case since U (unique characters) is at most 26, and E (edges) is at most 26^2.
//!
//! ## Idiomatic Rust
//!
//! Instead of using nested arrays for graphs, we use `HashMap<u8, Vec<u8>>` and `HashMap<u8, usize>`, leveraging byte slices `&[u8]`
//! since the characters are guaranteed to be valid ASCII English lowercase letters. This is much faster and more idiomatic
//! than calling `.chars()` which yields Unicode `char`s.
//!
//! ---
//!
//! ## Alternative Approaches
//!
//! - **DFS with cycle detection (3-color state)**: Also valid and sometimes faster by avoiding queue allocations, but Kahn's algorithm
//!   is often easier to read and debug for graph sorting problems.

use std::collections::{HashMap, HashSet, VecDeque};

/// Finds the alien dictionary ordering of characters.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn alien_order(words: Vec<String>) -> String {
    // RUST INSIGHT: We work with byte representations `&[u8]` of Strings.
    // Since the problem constraints usually guarantee English lowercase letters,
    // bytes are safe, avoid UTF-8 overhead, and let us use quick byte-level indexing.
    let words_bytes: Vec<&[u8]> = words.iter().map(String::as_bytes).collect();

    // Step 1: Initialize graph and in-degree maps.
    let mut adj: HashMap<u8, HashSet<u8>> = HashMap::new();
    let mut in_degree: HashMap<u8, usize> = HashMap::new();

    // Ensure every unique character in the dictionary is present in our in-degree map with at least 0.
    for word in &words_bytes {
        for &b in *word {
            in_degree.entry(b).or_insert(0);
        }
    }

    // Step 2: Build the graph
    for w in words_bytes.windows(2) {
        let (w1, w2) = (w[0], w[1]);

        let mut found_diff = false;
        let min_len = w1.len().min(w2.len());

        for i in 0..min_len {
            if w1[i] != w2[i] {
                let u = w1[i];
                let v = w2[i];

                // GOTCHA: We use a HashSet for adjacency to prevent duplicate edges from falsely incrementing in-degree.
                let neighbors = adj.entry(u).or_default();
                if neighbors.insert(v) {
                    *in_degree.entry(v).or_insert(0) += 1;
                }

                found_diff = true;
                break;
            }
        }

        // If no difference was found, but the first word is longer than the second,
        // it means the dictionary is sorted incorrectly (e.g., ["abc", "ab"]).
        if !found_diff && w1.len() > w2.len() {
            return String::new();
        }
    }

    // Step 3: Topological sort using Kahn's algorithm
    let mut queue = VecDeque::new();

    // RUST INSIGHT: Iterating over key-value pairs where we only need the key?
    // We could use `for (k, v) in &in_degree`, but iterating over `.iter()` and destructuring is common.
    for (&node, &degree) in &in_degree {
        if degree == 0 {
            queue.push_back(node);
        }
    }

    let mut result = Vec::with_capacity(in_degree.len());

    while let Some(node) = queue.pop_front() {
        result.push(node);

        if let Some(neighbors) = adj.get(&node) {
            for &next_node in neighbors {
                if let Some(deg) = in_degree.get_mut(&next_node) {
                    *deg -= 1;
                    if *deg == 0 {
                        queue.push_back(next_node);
                    }
                }
            }
        }
    }

    // Step 4: Cycle detection check
    // If the result doesn't contain all unique characters, there was a cycle.
    if result.len() != in_degree.len() {
        return String::new();
    }

    // RUST INSIGHT: String::from_utf8 safely converts our vector of bytes back into a String.
    // Since we only ever processed ASCII lowercase characters, this will always succeed.
    String::from_utf8(result).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_happy_path() {
        let words = vec![
            "wrt".to_string(),
            "wrf".to_string(),
            "er".to_string(),
            "ett".to_string(),
            "rftt".to_string(),
        ];
        let result = alien_order(words);
        assert_eq!(result, "wertf");
    }

    #[test]
    fn test_edge_case_invalid_prefix() {
        let words = vec!["abc".to_string(), "ab".to_string()];
        let result = alien_order(words);
        assert_eq!(result, "");
    }

    #[test]
    fn test_stress_boundary_cycle() {
        let words = vec!["z".to_string(), "x".to_string(), "z".to_string()];
        let result = alien_order(words);
        assert_eq!(result, "");
    }

    #[test]
    fn test_single_word() {
        let words = vec!["z".to_string()];
        let result = alien_order(words);
        assert_eq!(result, "z");
    }
}
