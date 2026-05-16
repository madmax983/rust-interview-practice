//! # 692. Top K Frequent Words
//!
//! Difficulty: Medium
//! Link: <https://leetcode.com/problems/top-k-frequent-words/>
//!
//! Given an array of strings `words` and an integer `k`, return the `k` most frequent strings.
//! Return the answer sorted by the frequency from highest to lowest. Sort the words with the same
//! frequency by their lexicographical order.
//!
//! ## Why This Matters in Rust
//!
//! This problem is a classic demonstration of how to manage complex sorting requirements in Rust
//! using custom implementations of the `Ord` and `PartialOrd` traits. By defining a custom struct
//! and implementing these traits, you completely control the behavior of `std::collections::BinaryHeap`,
//! allowing you to implicitly turn Rust's default Max-Heap into a specialized Min-Heap that perfectly
//! matches the problem's dual-condition requirements (frequency descending, lexicographical ascending).
//!
//! ## Approach
//!
//! First, we count the frequency of each word using a `HashMap`.
//!
//! **Straightforward Approach:** Collect the hash map entries into a `Vec`, sort the `Vec` using
//! a custom comparator (or `sort_unstable_by`), and take the first `k` elements.
//! Time Complexity: O(N log N) - where N is the number of unique words.
//! Space Complexity: O(N) - to store the elements.
//!
//! **Optimal Approach:** Use a `BinaryHeap` to maintain the top `k` elements. We define a custom
//! struct `WordCount` that flips the standard ordering so that the "smallest" elements (lowest frequency,
//! largest lexicographically) are considered the "greatest" by the max-heap and get popped when the
//! heap exceeds size `k`.
//! Time Complexity: O(N log K) - processing N elements, heap operations take log K.
//! Space Complexity: O(N) - for the hash map, plus O(K) for the heap.
//!
//! ## Alternative Approaches
//!
//! - **Trie + Bucket Sort:** For a strictly O(N) time complexity, you can bucket words by frequency,
//!   and use a Trie within each bucket to extract words in lexicographical order. This is highly optimized
//!   for time but uses significantly more space and complex code.

use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};

/// Represents a word and its frequency count.
#[derive(Debug, Eq, PartialEq)]
struct WordCount {
    word: String,
    count: usize,
}

// RUST INSIGHT: To use a custom struct in a `BinaryHeap`, we must implement `Ord`.
// The heap acts as a max-heap, meaning the element that evaluates as "Greater" is popped first.
// We want to discard elements with lower counts, or larger alphabetical strings when counts match.
// Therefore, we invert the count comparison, but keep the word comparison standard!
impl Ord for WordCount {
    fn cmp(&self, other: &Self) -> Ordering {
        match other.count.cmp(&self.count) {
            Ordering::Equal => self.word.cmp(&other.word), // Discard lexicographically larger words
            other_order => other_order,                    // Discard lower frequencies
        }
    }
}

// `Ord` requires `PartialOrd`. We can just defer to our `Ord` implementation.
impl PartialOrd for WordCount {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Straightforward Approach: Sort all elements
/// Time: O(N log N)
/// Space: O(N)
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn top_k_frequent_straightforward(words: Vec<String>, k: i32) -> Vec<String> {
    // GOTCHA: `HashMap::with_capacity` is generally good if we know the size, but here the number
    // of unique words might be much smaller than `words.len()`. Using `words.len()` avoids reallocations
    // at the cost of potential over-allocation.
    let mut counts = HashMap::with_capacity(words.len());

    // Count frequencies using the Entry API
    for word in words {
        *counts.entry(word).or_insert(0) += 1;
    }

    let mut vec: Vec<(String, usize)> = counts.into_iter().collect();

    // Sort by count descending, then word ascending
    vec.sort_unstable_by(|a, b| match b.1.cmp(&a.1) {
        Ordering::Equal => a.0.cmp(&b.0),
        other => other,
    });

    vec.into_iter()
        .take(k as usize)
        .map(|(word, _)| word)
        .collect()
}

/// Optimal Approach: Min-Heap of size K
/// Time: O(N log K)
/// Space: O(N)
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn top_k_frequent_optimal(words: Vec<String>, k: i32) -> Vec<String> {
    let mut counts = HashMap::new();

    // RUST INSIGHT: `or_insert` on the Entry API returns a mutable reference to the value.
    // This allows us to increment the counter cleanly without double-lookups.
    for word in words {
        *counts.entry(word).or_insert(0) += 1;
    }

    let mut heap = BinaryHeap::with_capacity((k + 1) as usize);

    for (word, count) in counts {
        heap.push(WordCount { word, count });

        // Maintain the heap size at `k`
        if heap.len() > k as usize {
            // Because of our custom `Ord`, this pops the element with the lowest frequency,
            // or the lexicographically largest word among ties.
            heap.pop();
        }
    }

    // Extract the remaining elements. Since the heap pops the "greatest" (i.e., lowest freq),
    // extracting all elements will give us the top K in reverse order.
    let mut result = Vec::with_capacity(k as usize);
    while let Some(wc) = heap.pop() {
        result.push(wc.word);
    }

    // Reverse to get highest frequency first
    result.reverse();
    result
}

/// Main entry point - uses the optimal solution
#[must_use]
pub fn top_k_frequent(words: Vec<String>, k: i32) -> Vec<String> {
    top_k_frequent_optimal(words, k)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vec_str(s: &[&str]) -> Vec<String> {
        s.iter().map(|&w| w.to_string()).collect()
    }

    #[test]
    fn test_happy_path() {
        let words = vec_str(&["i", "love", "leetcode", "i", "love", "coding"]);
        let k = 2;
        let expected = vec_str(&["i", "love"]);

        assert_eq!(top_k_frequent_straightforward(words.clone(), k), expected);
        assert_eq!(top_k_frequent_optimal(words.clone(), k), expected);
        assert_eq!(top_k_frequent(words, k), expected);
    }

    #[test]
    fn test_alphabetical_tie_edge_case() {
        let words = vec_str(&[
            "the", "day", "is", "sunny", "the", "the", "the", "sunny", "is", "is",
        ]);
        let k = 4;
        let expected = vec_str(&["the", "is", "sunny", "day"]);

        assert_eq!(top_k_frequent_straightforward(words.clone(), k), expected);
        assert_eq!(top_k_frequent_optimal(words.clone(), k), expected);
        assert_eq!(top_k_frequent(words, k), expected);
    }

    #[test]
    fn test_single_element() {
        let words = vec_str(&["rust"]);
        let k = 1;
        let expected = vec_str(&["rust"]);

        assert_eq!(top_k_frequent_straightforward(words.clone(), k), expected);
        assert_eq!(top_k_frequent_optimal(words.clone(), k), expected);
        assert_eq!(top_k_frequent(words, k), expected);
    }

    #[test]
    fn test_all_same_frequency() {
        let words = vec_str(&["z", "y", "x"]);
        let k = 2;
        // Lexicographical order resolves ties
        let expected = vec_str(&["x", "y"]);

        assert_eq!(top_k_frequent_straightforward(words.clone(), k), expected);
        assert_eq!(top_k_frequent_optimal(words.clone(), k), expected);
        assert_eq!(top_k_frequent(words, k), expected);
    }
}
