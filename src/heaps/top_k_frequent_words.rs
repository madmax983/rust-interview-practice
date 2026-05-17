//! # 692. Top K Frequent Words
//!
//! Difficulty: Medium
//! Link: <https://leetcode.com/problems/top-k-frequent-words/>
//!
//! Given an array of strings `words` and an integer `k`, return the `k` most frequent strings.
//! Return the answer sorted by the frequency from highest to lowest. Sort the words with the same
//! frequency by their lexicographical order.
//!
//! This problem perfectly demonstrates Rust's `std::collections::BinaryHeap` customization. By
//! default, it acts as a Max-Heap. Here, we define a custom struct and implement `Ord`/`PartialOrd`
//! to create a specialized Min-Heap with dual conditions (frequency ascending, lexicographical
//! descending for heap eviction), ensuring we only keep the top `k` elements in O(N log k) time.
//!
//! ## Approach
//!
//! ### Brute Force
//! 1. Count frequencies using a `HashMap`.
//! 2. Collect all entries into a `Vec`.
//! 3. Sort the `Vec` with a custom comparator (frequency descending, string ascending).
//! 4. Take the first `k` elements.
//! **Time**: O(N log N) - due to sorting all N unique words.
//! **Space**: O(N) - storing all words in the hash map and vector.
//!
//! ### Optimal (Min-Heap)
//! 1. Count frequencies using a `HashMap`.
//! 2. Maintain a Min-Heap (size `k`) using a custom `WordCount` struct that implements `Ord`.
//! 3. The `Ord` implementation reverses the typical logic so the smallest/least frequent elements
//!    stay at the root of the heap and get popped out when the heap exceeds size `k`.
//! 4. Extract the remaining `k` elements and reverse them (since popping a Min-Heap yields smallest first).
//! **Time**: O(N log k) - pushing/popping from a heap of size k takes log k time.
//! **Space**: O(N) for the HashMap, O(k) for the Heap.
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::heaps::top_k_frequent_words::top_k_frequent;
//!
//! let words = vec!["i".to_string(), "love".to_string(), "leetcode".to_string(), "i".to_string(), "love".to_string(), "coding".to_string()];
//! let k = 2;
//! assert_eq!(top_k_frequent(words, k), vec!["i".to_string(), "love".to_string()]);
//! ```

use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};

/// Brute force approach: HashMap + Sort all elements
/// Time: O(N log N) where N is number of unique words
/// Space: O(N) for HashMap and Vec
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn top_k_frequent_words_brute_force(words: Vec<String>, k: i32) -> Vec<String> {
    let mut counts: HashMap<String, usize> = HashMap::with_capacity(words.len());

    // Count frequencies
    for word in words {
        *counts.entry(word).or_insert(0) += 1;
    }

    // Collect into Vec
    let mut sorted_words: Vec<(String, usize)> = counts.into_iter().collect();

    // RUST INSIGHT: sort_by allows complex tuple comparisons
    // We sort by frequency descending (b.1.cmp(&a.1)), then string ascending (a.0.cmp(&b.0))
    sorted_words.sort_by(|a, b| match b.1.cmp(&a.1) {
        Ordering::Equal => a.0.cmp(&b.0),
        other => other,
    });

    // Extract top k
    sorted_words
        .into_iter()
        .take(k as usize)
        .map(|(word, _)| word)
        .collect()
}

// Custom struct to control BinaryHeap ordering
#[derive(Eq, PartialEq)]
struct WordCount {
    word: String,
    count: usize,
}

// RUST INSIGHT: We implement PartialOrd and Ord to make BinaryHeap act as a Min-Heap.
// Rust's BinaryHeap is a Max-Heap. The element that is "Greater" bubbles to the top.
// We want the LEAST frequent (or lexicographically LARGEST) to bubble to the top
// so we can pop it off when the heap size exceeds k.
impl Ord for WordCount {
    fn cmp(&self, other: &Self) -> Ordering {
        // Compare frequencies. We want smallest frequency at the top,
        // so if self.count < other.count, we return Greater.
        match other.count.cmp(&self.count) {
            Ordering::Equal => {
                // If frequencies are equal, we want lexicographically LARGEST at the top
                // so we can pop it out.
                // Normally a < b means Less. We want a < b to mean Greater.
                self.word.cmp(&other.word)
            }
            other_cmp => other_cmp,
        }
    }
}

impl PartialOrd for WordCount {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Optimal approach: HashMap + Min-Heap of size K
/// Time: O(N log K)
/// Space: O(N) for HashMap, O(K) for Heap
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn top_k_frequent_words_optimal(words: Vec<String>, k: i32) -> Vec<String> {
    let mut counts: HashMap<String, usize> = HashMap::with_capacity(words.len());

    for word in words {
        *counts.entry(word).or_insert(0) += 1;
    }

    let k = k as usize;
    let mut heap = BinaryHeap::with_capacity(k + 1);

    for (word, count) in counts {
        heap.push(WordCount { word, count });
        // Maintain heap size k
        if heap.len() > k {
            heap.pop();
        }
    }

    // The heap contains the top K elements, but popping yields the "greatest"
    // (which we defined as lowest frequency / highest lex string).
    // So we pop them into a vec, which puts them in reverse order of the final answer.
    let mut result = Vec::with_capacity(k);
    while let Some(wc) = heap.pop() {
        result.push(wc.word);
    }

    // Reverse to get descending frequencies
    result.reverse();
    result
}

/// Main entry point - uses optimal solution
#[must_use]
pub fn top_k_frequent(words: Vec<String>, k: i32) -> Vec<String> {
    top_k_frequent_words_optimal(words, k)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn to_string_vec(words: &[&str]) -> Vec<String> {
        words.iter().map(|&s| s.to_string()).collect()
    }

    #[test]
    fn test_brute_force() {
        assert_eq!(
            top_k_frequent_words_brute_force(
                to_string_vec(&["i", "love", "leetcode", "i", "love", "coding"]),
                2
            ),
            to_string_vec(&["i", "love"])
        );
        assert_eq!(
            top_k_frequent_words_brute_force(
                to_string_vec(&[
                    "the", "day", "is", "sunny", "the", "the", "the", "sunny", "is", "is"
                ]),
                4
            ),
            to_string_vec(&["the", "is", "sunny", "day"])
        );
    }

    #[test]
    fn test_optimal() {
        assert_eq!(
            top_k_frequent_words_optimal(
                to_string_vec(&["i", "love", "leetcode", "i", "love", "coding"]),
                2
            ),
            to_string_vec(&["i", "love"])
        );
        assert_eq!(
            top_k_frequent_words_optimal(
                to_string_vec(&[
                    "the", "day", "is", "sunny", "the", "the", "the", "sunny", "is", "is"
                ]),
                4
            ),
            to_string_vec(&["the", "is", "sunny", "day"])
        );
    }

    #[test]
    fn test_edge_cases() {
        // All same frequency
        assert_eq!(
            top_k_frequent(to_string_vec(&["z", "y", "x"]), 2),
            to_string_vec(&["x", "y"])
        );

        // k = 1
        assert_eq!(
            top_k_frequent(to_string_vec(&["a", "a", "b"]), 1),
            to_string_vec(&["a"])
        );

        // k = length of unique words
        assert_eq!(
            top_k_frequent(to_string_vec(&["a", "b"]), 2),
            to_string_vec(&["a", "b"])
        );
    }
}
