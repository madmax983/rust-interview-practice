//! # 692. Top K Frequent Words
//!
//! Link: <https://leetcode.com/problems/top-k-frequent-words/>
//! Difficulty: Medium
//!
//! Given an array of strings `words` and an integer `k`, return the `k` most frequent strings.
//! Return the answer sorted by the frequency from highest to lowest. Sort the words with the same frequency by their lexicographical order.
//!
//! ## Why this matters in Rust
//! This problem demonstrates how to customize Rust's `std::collections::BinaryHeap` (a default Max-Heap) into a specialized Min-Heap with dual conditions (frequency descending, lexicographical ascending).
//! By implementing a custom struct and overriding the `Ord` and `PartialOrd` traits, we encode the problem's complex sorting logic directly into the type's behavior rather than relying on anonymous closures. This showcases Rust's type system as a powerful modeling tool.
//!
//! ## Approach
//!
//! We explore two implementations:
//! 1. **Brute Force (Full Sort)**: Count frequencies using a `HashMap`, then sort the entire list of unique words using `sort_unstable_by`.
//! 2. **Optimized (Min-Heap)**: Maintain a Min-Heap of size `k`. As we process the unique words, we add them to the heap and pop the least desirable elements. `BinaryHeap::into_sorted_vec` is then used to cleanly extract the top `k` elements in the required order.

use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};

/// A custom struct to model a word and its frequency.
/// We implement `Ord` and `PartialOrd` to define exactly how elements should be ordered in our heap.
#[derive(Debug, Eq, PartialEq)]
struct WordFreq {
    word: String,
    count: i32,
}

impl PartialOrd for WordFreq {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

// RUST INSIGHT: `BinaryHeap` is a Max-Heap by default.
// To keep the *top* K elements, we want to pop the *least* desirable elements when the heap exceeds size K.
// Therefore, we must define "Greater" to mean "less desirable".
// A word is less desirable if it has a lower frequency, or if frequencies are equal, it is lexicographically larger.
impl Ord for WordFreq {
    fn cmp(&self, other: &Self) -> Ordering {
        // Reverse frequency comparison: lower count is "Greater"
        match other.count.cmp(&self.count) {
            Ordering::Equal => {
                // Forward string comparison: lexicographically larger word is "Greater"
                self.word.cmp(&other.word)
            }
            ord => ord,
        }
    }
}

/// Brute force approach: Full Sort
///
/// Count all frequencies, convert to a vector, and sort the entire vector.
///
/// Time: O(N log N) - Where N is the number of unique words.
/// Space: O(N) - To store the frequencies and the resulting vector.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn top_k_frequent_brute_force(words: Vec<String>, k: i32) -> Vec<String> {
    let mut counts = HashMap::new();
    for word in words {
        *counts.entry(word).or_insert(0) += 1;
    }

    let mut unique_words: Vec<_> = counts.into_iter().collect();

    // Sort descending by count, then ascending by word
    unique_words.sort_unstable_by(|a, b| match b.1.cmp(&a.1) {
        Ordering::Equal => a.0.cmp(&b.0),
        ord => ord,
    });

    unique_words
        .into_iter()
        .take(k as usize)
        .map(|(word, _)| word)
        .collect()
}

/// Optimized approach: Min-Heap of size K
///
/// Use a BinaryHeap tailored as a Min-Heap (via the `WordFreq` custom `Ord` implementation) to keep only the top K elements.
///
/// Time: O(N log K) - Inserting into a heap of size K takes log K time, done N times.
/// Space: O(N) - For the HashMap (Heap takes O(K)).
///
/// # Gotcha
/// `.into_sorted_vec()` consumes the heap and returns elements in ascending order according to `Ord`.
/// Since our `Ord` defines the most desirable elements as "Less", they conveniently appear first in the vector!
#[must_use]
#[allow(clippy::needless_pass_by_value)]
#[allow(clippy::cast_sign_loss)] // `k` is strictly positive per constraints
pub fn top_k_frequent_optimized(words: Vec<String>, k: i32) -> Vec<String> {
    let mut counts = HashMap::new();
    for word in words {
        *counts.entry(word).or_insert(0) += 1;
    }

    let k_usize = k as usize;
    let mut heap = BinaryHeap::with_capacity(k_usize + 1);

    for (word, count) in counts {
        heap.push(WordFreq { word, count });

        // Maintain heap size to be exactly k
        if heap.len() > k_usize {
            heap.pop();
        }
    }

    // RUST INSIGHT: into_sorted_vec() sorts in ascending order.
    // Because our "best" elements are considered "smallest" by our Ord implementation,
    // they naturally come out exactly in the order the problem requires!
    heap.into_sorted_vec()
        .into_iter()
        .map(|wf| wf.word)
        .collect()
}

/// Main entry point
#[must_use]
pub fn top_k_frequent(words: Vec<String>, k: i32) -> Vec<String> {
    top_k_frequent_optimized(words, k)
}

// ## Alternative Approaches
// - **Bucket Sort + Trie**: Since frequencies are bounded by `N`, we can use a bucket array where the index is the frequency.
//   Each bucket contains a Trie to maintain lexicographical order.
//   This achieves O(N) time complexity but is significantly more complex to implement and has higher constant factors.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_brute_force_example_1() {
        let words = vec!["i", "love", "leetcode", "i", "love", "coding"]
            .into_iter()
            .map(String::from)
            .collect();
        assert_eq!(top_k_frequent_brute_force(words, 2), vec!["i", "love"]);
    }

    #[test]
    fn test_brute_force_example_2() {
        let words = vec![
            "the", "day", "is", "sunny", "the", "the", "the", "sunny", "is", "is",
        ]
        .into_iter()
        .map(String::from)
        .collect();
        assert_eq!(
            top_k_frequent_brute_force(words, 4),
            vec!["the", "is", "sunny", "day"]
        );
    }

    #[test]
    fn test_optimized_example_1() {
        let words = vec!["i", "love", "leetcode", "i", "love", "coding"]
            .into_iter()
            .map(String::from)
            .collect();
        assert_eq!(top_k_frequent_optimized(words, 2), vec!["i", "love"]);
    }

    #[test]
    fn test_optimized_example_2() {
        let words = vec![
            "the", "day", "is", "sunny", "the", "the", "the", "sunny", "is", "is",
        ]
        .into_iter()
        .map(String::from)
        .collect();
        assert_eq!(
            top_k_frequent_optimized(words, 4),
            vec!["the", "is", "sunny", "day"]
        );
    }

    #[test]
    fn test_all_approaches_edge_cases() {
        // All same frequency
        let words1: Vec<String> = vec!["z", "y", "x"].into_iter().map(String::from).collect();
        assert_eq!(top_k_frequent_brute_force(words1.clone(), 2), vec!["x", "y"]);
        assert_eq!(top_k_frequent_optimized(words1.clone(), 2), vec!["x", "y"]);

        // K equals total words
        let words2: Vec<String> = vec!["a", "b", "c"].into_iter().map(String::from).collect();
        assert_eq!(
            top_k_frequent_brute_force(words2.clone(), 3),
            vec!["a", "b", "c"]
        );
        assert_eq!(
            top_k_frequent_optimized(words2.clone(), 3),
            vec!["a", "b", "c"]
        );
    }
}
