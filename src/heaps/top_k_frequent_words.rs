//! # 692. Top K Frequent Words
//!
//! Difficulty: Medium
//! Link: <https://leetcode.com/problems/top-k-frequent-words/>
//!
//! Given an array of strings `words` and an integer `k`, return the `k` most frequent strings.
//!
//! Return the answer sorted by the frequency from highest to lowest. Sort the words with the same frequency by their lexicographical order.
//!
//! This problem demonstrates how to customize Rust's `std::collections::BinaryHeap` (which is a Max-Heap by default)
//! into a specialized bounded Min-Heap. By implementing a custom struct and carefully overriding the `Ord` and `PartialOrd`
//! traits, we can maintain the top `k` elements in O(N log K) time, rather than fully sorting in O(N log N).
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::heaps::top_k_frequent_words::top_k_frequent;
//!
//! let words = vec![
//!     "i".to_string(), "love".to_string(), "leetcode".to_string(),
//!     "i".to_string(), "love".to_string(), "coding".to_string()
//! ];
//! let k = 2;
//! assert_eq!(top_k_frequent(words, k), vec!["i", "love"]);
//! ```

use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};

/// Custom struct to hold a word and its frequency.
/// We implement `Ord` such that this acts as a MIN-heap for our specific needs.
/// Wait! If it's a MIN-heap, the elements we want to *discard* evaluate as `Ordering::Greater`
/// so they sit at the top of the heap and get popped.
/// What do we want to keep?
/// 1. High frequency.
/// 2. If frequencies tie, we want lexicographically smaller strings.
/// Therefore, the elements we want to DISCARD (which should go to the top of the Max-Heap to be popped) are:
/// 1. Low frequency.
/// 2. If frequencies tie, lexicographically LARGER strings.
#[derive(Eq, PartialEq)]
struct WordFreq {
    word: String,
    freq: i32,
}

impl Ord for WordFreq {
    fn cmp(&self, other: &Self) -> Ordering {
        // RUST INSIGHT: BinaryHeap is a Max-Heap. The element that is "Greater" goes to the top.
        // We want to POP the lowest frequency words when the heap exceeds size K.
        // Therefore, smaller frequency should be considered "Greater" for the heap.
        let freq_cmp = other.freq.cmp(&self.freq); // Reverse order

        if freq_cmp == Ordering::Equal {
            // If frequencies are equal, we want to keep the lexicographically SMALLER word.
            // So we want to POP the lexicographically LARGER word.
            // Therefore, standard lexicographical ordering makes the larger word "Greater".
            self.word.cmp(&other.word)
        } else {
            freq_cmp
        }
    }
}

impl PartialOrd for WordFreq {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Bounded Min-Heap approach
///
/// Time: O(N + N log K) - We count frequencies in O(N). Then we process N unique words,
/// pushing to a heap of max size K, which takes O(log K) per word. Total O(N log K).
/// Space: O(N) - For the HashMap storing frequencies. The Heap takes O(K).
#[must_use]
pub fn top_k_frequent_optimal(words: Vec<String>, k: i32) -> Vec<String> {
    let k = k as usize;
    let mut counts = HashMap::new();

    // Count frequencies
    for word in words {
        *counts.entry(word).or_insert(0) += 1;
    }

    // GOTCHA: By default BinaryHeap is a Max-Heap. We customized `Ord` on `WordFreq`
    // so that the "worst" elements (lowest freq, largest string) sit at the top.
    let mut min_heap = BinaryHeap::with_capacity(k + 1);

    for (word, freq) in counts {
        min_heap.push(WordFreq { word, freq });
        if min_heap.len() > k {
            // Pop the element that is "Greatest" according to our custom Ord,
            // which is the lowest frequency or the lexicographically largest string among ties.
            min_heap.pop();
        }
    }

    // RUST INSIGHT: The remaining elements are the Top K.
    // However, they are in the min-heap, so popping them gives them in ascending order
    // of our "keep" priority (i.e., the worst of the best comes out first).
    // So we pop them, which gives us ascending order, and then we reverse the vector.
    let mut result = Vec::with_capacity(k);
    while let Some(wf) = min_heap.pop() {
        result.push(wf.word);
    }

    result.reverse();
    result
}

/// Brute Force / Full Sorting approach
///
/// Time: O(N log N) - Count frequencies in O(N), then sort all unique words in O(U log U) where U <= N.
/// Space: O(N) - For the HashMap and the vector to sort.
#[must_use]
pub fn top_k_frequent_brute_force(words: Vec<String>, k: i32) -> Vec<String> {
    let mut counts = HashMap::new();
    for word in words {
        *counts.entry(word).or_insert(0) += 1;
    }

    let mut unique_words: Vec<(String, i32)> = counts.into_iter().collect();

    // Sort by frequency descending, then lexicographically ascending
    unique_words.sort_unstable_by(|a, b| {
        let freq_cmp = b.1.cmp(&a.1);
        if freq_cmp == Ordering::Equal {
            a.0.cmp(&b.0)
        } else {
            freq_cmp
        }
    });

    unique_words
        .into_iter()
        .take(k as usize)
        .map(|(word, _)| word)
        .collect()
}

/// Main entry point
#[must_use]
pub fn top_k_frequent(words: Vec<String>, k: i32) -> Vec<String> {
    top_k_frequent_optimal(words, k)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_happy_path() {
        let words = vec![
            "i".to_string(),
            "love".to_string(),
            "leetcode".to_string(),
            "i".to_string(),
            "love".to_string(),
            "coding".to_string(),
        ];
        let expected = vec!["i".to_string(), "love".to_string()];

        assert_eq!(top_k_frequent_brute_force(words.clone(), 2), expected);
        assert_eq!(top_k_frequent_optimal(words, 2), expected);
    }

    #[test]
    fn test_frequency_ties_alphabetical_order() {
        let words = vec![
            "the".to_string(),
            "day".to_string(),
            "is".to_string(),
            "sunny".to_string(),
            "the".to_string(),
            "the".to_string(),
            "the".to_string(),
            "sunny".to_string(),
            "is".to_string(),
            "is".to_string(),
        ];
        // the: 4, is: 3, sunny: 2, day: 1
        let expected = vec![
            "the".to_string(),
            "is".to_string(),
            "sunny".to_string(),
            "day".to_string(),
        ];

        assert_eq!(top_k_frequent_brute_force(words.clone(), 4), expected);
        assert_eq!(top_k_frequent_optimal(words, 4), expected);
    }

    #[test]
    fn test_stress_boundary() {
        // all same frequency, should return alphabetically first K
        let words = vec![
            "z".to_string(),
            "y".to_string(),
            "x".to_string(),
            "w".to_string(),
            "v".to_string(),
        ];
        let expected = vec!["v".to_string(), "w".to_string(), "x".to_string()];

        assert_eq!(top_k_frequent_brute_force(words.clone(), 3), expected);
        assert_eq!(top_k_frequent_optimal(words, 3), expected);
    }
}
