//! # 692. Top K Frequent Words
//!
//! Difficulty: Medium
//! Link: <https://leetcode.com/problems/top-k-frequent-words/>
//!
//! Given an array of strings `words` and an integer `k`, return the `k` most frequent strings.
//! Return the answer sorted by the frequency from highest to lowest. Sort the words with the same
//! frequency by their lexicographical order.
//!
//! This problem is a natural fit for Rust's `std::collections::BinaryHeap` and demonstrates how to
//! customize its behavior. By default, `BinaryHeap` is a max-heap. To efficiently solve this problem,
//! we need a min-heap that bounds its size to `k`, avoiding the `O(N log N)` cost of sorting all elements.
//! It teaches how to implement `Ord` and `PartialOrd` for custom structs to define dual-condition sorting.

use std::cmp::Ordering;
use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};

/// Brute force / Straightforward approach: Hash Map + Full Sort
/// Time: O(N log N) - where N is the number of words. Building the frequency map is O(N),
/// and sorting the unique words takes O(U log U) where U <= N.
/// Space: O(N) - to store the hash map and the vector of unique words.
///
/// We count the frequencies, put them into a vector, sort the entire vector with a custom
/// comparator, and then take the top `k` elements.
#[must_use]
#[allow(clippy::needless_pass_by_value)] // LeetCode signature
pub fn top_k_frequent_straightforward(words: Vec<String>, k: i32) -> Vec<String> {
    let mut counts = HashMap::new();

    // RUST INSIGHT: `entry` API is idiomatic and efficient. It avoids a double lookup
    // (one to check if it exists, one to insert/update).
    for word in words {
        *counts.entry(word).or_insert(0) += 1;
    }

    let mut count_vec: Vec<(String, i32)> = counts.into_iter().collect();

    // GOTCHA: We must sort by frequency descending, then lexicographically ascending.
    // Notice `b.1.cmp(&a.1)` for descending, and `a.0.cmp(&b.0)` for ascending.
    count_vec.sort_unstable_by(|a, b| match b.1.cmp(&a.1) {
        Ordering::Equal => a.0.cmp(&b.0),
        other => other,
    });

    count_vec
        .into_iter()
        .take(k as usize)
        .map(|(w, _)| w)
        .collect()
}

/// A custom struct to represent a word and its frequency, allowing us to implement custom
/// ordering traits for the `BinaryHeap`.
#[derive(Eq, PartialEq)]
struct WordCount {
    word: String,
    count: i32,
}

/// RUST INSIGHT: When we implement `Ord`, we also must implement `PartialOrd`, `Eq`, and `PartialEq`.
/// The desirability of a `WordCount` is defined as: higher frequency is greater. If frequencies
/// are equal, a lexicographically smaller word is greater (e.g., "apple" > "zebra").
impl PartialOrd for WordCount {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for WordCount {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.count.cmp(&other.count) {
            Ordering::Equal => {
                // Secondary sort: alphabetical (lower is greater/more desirable)
                // "apple" > "zebra", so we reverse the standard string comparison.
                other.word.cmp(&self.word)
            }
            other_order => other_order,
        }
    }
}

/// Optimal approach: Hash Map + Bounded Min-Heap
/// Time: O(N log K) - where N is the number of words. We insert up to N elements into a heap
/// of max size K.
/// Space: O(N) - for the hash map to store frequencies. The heap takes O(K) space.
///
/// By using a bounded min-heap, we maintain only the top `k` most desirable elements.
/// When the heap size exceeds `k`, we pop the minimum element (the least desirable one).
#[must_use]
#[allow(clippy::needless_pass_by_value)] // LeetCode signature
#[allow(clippy::cast_sign_loss)] // `k` is guaranteed to be positive in this context
pub fn top_k_frequent_optimal(words: Vec<String>, k: i32) -> Vec<String> {
    let mut counts = HashMap::new();
    for word in words {
        *counts.entry(word).or_insert(0) += 1;
    }

    // RUST INSIGHT: `BinaryHeap` is a max-heap. To make it a min-heap (so `.pop()` evicts
    // the least desirable element), we wrap our `WordCount` in `std::cmp::Reverse`.
    let mut heap = BinaryHeap::with_capacity(k as usize + 1);

    for (word, count) in counts {
        heap.push(Reverse(WordCount { word, count }));

        // Keep the heap size bounded to `k`. If it exceeds `k`, pop the least desirable element.
        if heap.len() > k as usize {
            heap.pop();
        }
    }

    let mut result = Vec::with_capacity(heap.len());

    // GOTCHA: `into_iter` for `BinaryHeap` does not guarantee sorted order!
    // We must `.pop()` repeatedly to extract elements in order.
    while let Some(Reverse(wc)) = heap.pop() {
        result.push(wc.word);
    }

    // They are popped from least desirable to most desirable.
    // Reversing gives most desirable to least desirable.
    result.reverse();

    result
}

/// Main entry point - uses the optimal heap approach.
#[must_use]
pub fn top_k_frequent(words: Vec<String>, k: i32) -> Vec<String> {
    top_k_frequent_optimal(words, k)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vec_str(strings: &[&str]) -> Vec<String> {
        strings.iter().map(|&s| s.to_string()).collect()
    }

    // Happy Path tests
    #[test]
    fn test_top_k_frequent_straightforward() {
        let words = vec_str(&["i", "love", "leetcode", "i", "love", "coding"]);
        assert_eq!(
            top_k_frequent_straightforward(words, 2),
            vec_str(&["i", "love"])
        );
    }

    #[test]
    fn test_top_k_frequent_optimal() {
        let words = vec_str(&["i", "love", "leetcode", "i", "love", "coding"]);
        assert_eq!(
            top_k_frequent_optimal(words, 2),
            vec_str(&["i", "love"])
        );
    }

    // Edge Case tests
    #[test]
    fn test_top_k_frequent_same_frequency() {
        // All words appear exactly once, should be sorted alphabetically
        let words = vec_str(&["the", "day", "is", "sunny"]);
        let expected = vec_str(&["day", "is", "sunny", "the"]);
        assert_eq!(top_k_frequent_optimal(words.clone(), 4), expected);
        assert_eq!(top_k_frequent_straightforward(words, 4), expected);
    }

    // Stress/Boundary tests
    #[test]
    fn test_top_k_frequent_single_element() {
        let words = vec_str(&["rust"]);
        assert_eq!(top_k_frequent_optimal(words.clone(), 1), vec_str(&["rust"]));
        assert_eq!(top_k_frequent_straightforward(words, 1), vec_str(&["rust"]));
    }
}
