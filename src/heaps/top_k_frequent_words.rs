//! # 692. Top K Frequent Words
//!
//! Given an array of strings `words` and an integer `k`, return the `k` most frequent strings.
//! Return the answer sorted by the frequency from highest to lowest. Sort the words with the same frequency by their lexicographical order.
//!
//! - Difficulty: Medium
//! - LeetCode: <https://leetcode.com/problems/top-k-frequent-words/>
//!
//! ## Why this matters in Rust
//! This problem perfectly demonstrates how to customize Rust's `std::collections::BinaryHeap` (a default Max-Heap) into a specialized Min-Heap with dual conditions. By implementing a custom struct and overriding the `Ord` and `PartialOrd` traits, we elegantly encapsulate the sorting logic (frequency descending, lexicographical ascending) directly into the type system, rather than passing anonymous comparator closures.
//!
//! ## Approach
//!
//! We explore two implementations:
//! 1.  **Brute Force**: Count frequencies using a `HashMap`, collect the entries into a `Vec`, and sort the entire vector using our dual conditions, finally taking the first `k` elements.
//! 2.  **Optimized (Min-Heap)**: Count frequencies, then maintain a heap of size `k`. To do this efficiently, we design our `Ord` implementation such that the elements we want to *discard* (lower frequency, or lexicographically larger strings) are considered "greater" by Rust's Max-Heap. This turns our Max-Heap into a Min-Heap of the top `k` elements.

use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};

/// A custom struct to model a word and its frequency, with custom ordering.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct WordCount {
    pub word: String,
    pub count: usize,
}

// RUST INSIGHT: We implement `Ord` to make `BinaryHeap` act as a Min-Heap for our specific needs.
// By default, `BinaryHeap` pops the "greatest" element. We want to discard words that have a
// lower frequency, or if frequencies are equal, words that are lexicographically larger.
// Therefore, we define "greater" as having a lower count or a larger string.
impl Ord for WordCount {
    fn cmp(&self, other: &Self) -> Ordering {
        // First, compare by frequency. We want smaller counts to be "greater".
        match other.count.cmp(&self.count) {
            Ordering::Equal => {
                // If counts are equal, we want lexicographically larger words to be "greater".
                self.word.cmp(&other.word)
            }
            ord => ord,
        }
    }
}

impl PartialOrd for WordCount {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Brute Force Approach: HashMap + Sort
///
/// Count the frequencies using a HashMap, then sort all unique words.
///
/// - **Time Complexity**: O(N log N) where N is the number of words. The sorting step dominates.
/// - **Space Complexity**: O(N) to store the HashMap and the intermediate Vector.
#[must_use]
pub fn top_k_frequent_brute_force(words: Vec<String>, k: i32) -> Vec<String> {
    let mut counts = HashMap::new();
    for word in words {
        // RUST INSIGHT: `entry().or_insert()` is the idiomatic way to update a map counter.
        *counts.entry(word).or_insert(0) += 1;
    }

    let mut word_counts: Vec<_> = counts.into_iter().collect();

    // GOTCHA: We must sort by frequency descending, then by word ascending.
    // Notice how we use a tuple for the sort key. The first element is reversed `Reverse(count)`
    // and the second element is just the `word`.
    word_counts.sort_unstable_by(|(word_a, count_a), (word_b, count_b)| {
        match count_b.cmp(count_a) {
            Ordering::Equal => word_a.cmp(word_b),
            ord => ord,
        }
    });

    word_counts
        .into_iter()
        .take(k as usize)
        .map(|(word, _)| word)
        .collect()
}

/// Optimized Approach: HashMap + Min-Heap (Custom Ord)
///
/// Maintain a Heap of size `k`. Because of our custom `Ord` implementation, the `BinaryHeap`
/// will keep the "least desirable" of our top `k` elements at the root, making it easy to pop.
///
/// - **Time Complexity**: O(N log K), where N is the number of words. We insert up to N elements into a heap of size K.
/// - **Space Complexity**: O(N) for the HashMap, and O(K) for the heap.
#[must_use]
pub fn top_k_frequent_optimized(words: Vec<String>, k: i32) -> Vec<String> {
    let k_usize = k as usize;
    let mut counts = HashMap::new();
    for word in words {
        *counts.entry(word).or_insert(0) += 1;
    }

    // We pre-allocate k + 1 because we push before we pop.
    let mut heap: BinaryHeap<WordCount> = BinaryHeap::with_capacity(k_usize + 1);

    for (word, count) in counts {
        heap.push(WordCount { word, count });
        if heap.len() > k_usize {
            heap.pop(); // Removes the element with lowest frequency / highest lexicographical order
        }
    }

    // GOTCHA: The elements in the heap are currently the top K, but popping them
    // yields the "least" of the top K first. We need them in descending order of frequency.
    let mut result = Vec::with_capacity(k_usize);
    while let Some(wc) = heap.pop() {
        result.push(wc.word);
    }
    // Reverse because the heap gave us the elements from smallest frequency to highest.
    result.reverse();

    result
}

/// Main entry point
#[must_use]
pub fn top_k_frequent(words: Vec<String>, k: i32) -> Vec<String> {
    top_k_frequent_optimized(words, k)
}

// Alternative Approaches:
// 1. **BTreeMap + BTreeSet**: You could group words by frequency using `BTreeMap<usize, BTreeSet<String>>`, but this carries higher memory overhead and tree balancing costs than a simple BinaryHeap.
// 2. **Trie + Heap**: For extremely large datasets with many common prefixes, a Trie could save space when storing strings, but would complicate frequency counting.

#[cfg(test)]
mod tests {
    use super::*;

    fn vec_string(words: &[&str]) -> Vec<String> {
        words.iter().map(|&s| s.to_string()).collect()
    }

    #[test]
    fn test_brute_force() {
        let words = vec_string(&["i", "love", "leetcode", "i", "love", "coding"]);
        let k = 2;
        let expected = vec_string(&["i", "love"]);
        assert_eq!(top_k_frequent_brute_force(words, k), expected);
    }

    #[test]
    fn test_optimized() {
        let words = vec_string(&["i", "love", "leetcode", "i", "love", "coding"]);
        let k = 2;
        let expected = vec_string(&["i", "love"]);
        assert_eq!(top_k_frequent_optimized(words, k), expected);
    }

    #[test]
    fn test_edge_case_alphabetical_fallback() {
        // "the" has count 4, "is" has count 3, "sunny" has count 2, "day" has count 1.
        // Alphabetical order: "day", "is", "sunny", "the"
        let words = vec_string(&[
            "the", "day", "is", "sunny", "the", "the", "the", "sunny", "is", "is",
        ]);
        let k = 4;
        let expected = vec_string(&["the", "is", "sunny", "day"]); // 'the': 4, 'is': 3, 'sunny': 2, 'day': 1
        assert_eq!(top_k_frequent_optimized(words, k), expected);
    }

    #[test]
    fn test_stress_boundary_case() {
        let words = vec_string(&["a", "a", "b", "b", "c", "c"]);
        // Frequencies are equal. We want the first 2 lexicographically: 'a' and 'b'
        let expected = vec_string(&["a", "b"]);
        assert_eq!(top_k_frequent_optimized(words.clone(), 2), expected);
        assert_eq!(top_k_frequent_brute_force(words, 2), expected);
    }
}
