//! LeetCode #692: Top K Frequent Words
//! Difficulty: Medium
//! Link: https://leetcode.com/problems/top-k-frequent-words/
//!
//! This problem demonstrates how to customize Rust's `std::collections::BinaryHeap`
//! (which is a Max-Heap by default) into a specialized Min-Heap with dual conditions.
//! It showcases how building a custom struct and overriding the `Ord` and `PartialOrd`
//! traits can model complex priority rules securely within Rust's type system.

use std::collections::{BinaryHeap, HashMap};
use std::cmp::Ordering;

/// Approach: Straightforward Full Sort
///
/// We count the frequencies of each word using a HashMap, collect them into a Vec,
/// and then sort the Vec based on frequency (descending) and word (ascending).
/// Finally, we take the top `k` elements.
///
/// Time Complexity: O(N log N) where N is the number of unique words, due to sorting.
/// Space Complexity: O(N) for the HashMap and Vec.
///
/// Why this is idiomatic:
/// It leverages iterator adapters (`.into_iter()`, `.take()`, `.map()`, `.collect()`)
/// to perform the transformation pipeline without manual loops, making it highly readable.
pub fn top_k_frequent_straightforward(words: Vec<String>, k: i32) -> Vec<String> {
    let mut counts = HashMap::new();
    for word in words {
        // RUST INSIGHT: `entry().or_insert()` is the idiomatic way to count frequencies.
        // It avoids double-lookups present in languages like C++ or Java.
        *counts.entry(word).or_insert(0) += 1;
    }

    let mut word_counts: Vec<(String, i32)> = counts.into_iter().collect();

    // GOTCHA: We want descending frequency, but ascending alphabetical order.
    // `.then_with` cleanly chains these conditions without complex ternary logic.
    word_counts.sort_unstable_by(|a, b| {
        b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0))
    });

    word_counts
        .into_iter()
        .take(k as usize)
        .map(|(word, _)| word)
        .collect()
}

/// A custom struct to manage our priority queue ordering.
#[derive(Eq, PartialEq)]
struct WordCount {
    word: String,
    count: i32,
}

/// We implement a bounded Min-Heap using Rust's default Max-Heap.
/// To keep the 'Top K' elements, we want to discard elements with lower frequencies
/// or (if tied) lexicographically larger words.
/// Thus, our `cmp` must make those discarded elements evaluate as `Ordering::Greater`
/// so they bubble to the top of the Max-Heap and get popped off.
impl Ord for WordCount {
    fn cmp(&self, other: &Self) -> Ordering {
        // RUST INSIGHT:
        // 1. Reverse ordering for count: lower counts evaluate as Greater.
        // 2. Standard ordering for words: larger words evaluate as Greater.
        other.count.cmp(&self.count)
            .then_with(|| self.word.cmp(&other.word))
    }
}

impl PartialOrd for WordCount {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Approach: Optimized Bounded Min-Heap
///
/// We count frequencies, but instead of sorting everything, we maintain a BinaryHeap
/// of size `k`. When the heap exceeds `k`, we pop the top element (which, due to our
/// custom `Ord` implementation, is the element we want to discard).
///
/// Time Complexity: O(N log K), where N is unique words. Maintaining a heap of size K
/// is more efficient than a full sort when K << N.
/// Space Complexity: O(N) for the HashMap, and O(K) for the Heap.
pub fn top_k_frequent_optimized(words: Vec<String>, k: i32) -> Vec<String> {
    let mut counts = HashMap::new();
    for word in words {
        *counts.entry(word).or_insert(0) += 1;
    }

    // RUST INSIGHT: We pre-allocate capacity since we know the exact maximum size.
    let mut heap = BinaryHeap::with_capacity((k + 1) as usize);

    for (word, count) in counts {
        heap.push(WordCount { word, count });
        if heap.len() > k as usize {
            // Popping removes the "largest" element according to our custom Ord,
            // which is the lowest frequency / highest lexicographical word.
            heap.pop();
        }
    }

    // GOTCHA: The heap pops elements in descending order of our `Ord` (which means
    // it yields the lowest frequency first among the top K). We need to reverse it.
    let mut result = Vec::with_capacity(k as usize);
    while let Some(wc) = heap.pop() {
        result.push(wc.word);
    }

    result.reverse();
    result
}

/// Alternative Approaches:
/// 1. **Bucket Sort (with Trie)**: Since maximum frequency is bounded by N, we could use an
///    array of Tries (one Trie per frequency bucket). This gives O(N) time complexity but
///    has significant constant factor overhead and memory usage.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_happy_path() {
        let words = vec![
            "i".to_string(), "love".to_string(), "leetcode".to_string(),
            "i".to_string(), "love".to_string(), "coding".to_string()
        ];
        let k = 2;

        let expected = vec!["i".to_string(), "love".to_string()];

        assert_eq!(top_k_frequent_straightforward(words.clone(), k), expected);
        assert_eq!(top_k_frequent_optimized(words, k), expected);
    }

    #[test]
    fn test_edge_case_same_frequency() {
        let words = vec![
            "the".to_string(), "day".to_string(), "is".to_string(), "sunny".to_string(),
            "the".to_string(), "the".to_string(), "the".to_string(), "sunny".to_string(),
            "is".to_string(), "is".to_string()
        ];
        let k = 4;

        let expected = vec![
            "the".to_string(), "is".to_string(), "sunny".to_string(), "day".to_string()
        ];

        assert_eq!(top_k_frequent_straightforward(words.clone(), k), expected);
        assert_eq!(top_k_frequent_optimized(words, k), expected);
    }

    #[test]
    fn test_boundary_all_unique() {
        let words = vec![
            "z".to_string(), "y".to_string(), "x".to_string(), "w".to_string()
        ];
        let k = 2;

        // Since frequencies are all 1, they should sort alphabetically.
        let expected = vec!["w".to_string(), "x".to_string()];

        assert_eq!(top_k_frequent_straightforward(words.clone(), k), expected);
        assert_eq!(top_k_frequent_optimized(words, k), expected);
    }
}
