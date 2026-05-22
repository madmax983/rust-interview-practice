//! # 692. Top K Frequent Words
//!
//! Given an array of strings `words` and an integer `k`, return the `k` most frequent strings.
//! Return the answer sorted by the frequency from highest to lowest. Sort the words with the same frequency by their lexicographical order.
//!
//! - Difficulty: Medium
//! - LeetCode: <https://leetcode.com/problems/top-k-frequent-words/>
//!
//! ## Why this matters in Rust
//! This problem perfectly demonstrates how to customize Rust's `std::collections::BinaryHeap` (a default Max-Heap) into a specialized Min-Heap with dual conditions. By implementing a custom struct and overriding the `Ord` and `PartialOrd` traits, we leverage Rust's type system to encapsulate complex sorting logic directly within the element's behavior, leading to safe, idiomatic, and robust code.
//!
//! ## Approach
//!
//! We explore two main implementations:
//! 1.  **Brute Force / Straightforward**: Count frequencies using a `HashMap`, collect the entries into a `Vec`, and sort them using a custom comparator.
//! 2.  **Optimized (Min-Heap)**: Count frequencies using a `HashMap`, then maintain a Min-Heap of size `k`. Since `BinaryHeap` is a Max-Heap by default, we invert the ordering logic in our custom struct's `Ord` implementation so the smallest element (by frequency, or largest lexicographically if tied) is at the top to be popped when the heap exceeds size `k`. Finally, we extract the elements, reverse them, and return the result.

use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};

/// Straightforward Approach: HashMap + Sort
///
/// We count the frequencies of each word using a HashMap, then collect the key-value pairs into a Vec
/// and sort them according to the problem requirements (frequency descending, then lexicographically ascending).
///
/// - **Time Complexity**: O(N log N), where N is the total number of words. The HashMap population takes O(N), but sorting takes O(N log N).
/// - **Space Complexity**: O(N) to store the HashMap and the intermediate vector for sorting.
pub fn top_k_frequent_straightforward(words: Vec<String>, k: i32) -> Vec<String> {
    let mut counts: HashMap<String, usize> = HashMap::new();
    for word in words {
        *counts.entry(word).or_insert(0) += 1;
    }

    let mut entries: Vec<(String, usize)> = counts.into_iter().collect();

    // RUST INSIGHT: We use `sort_unstable_by` to provide a custom comparator closure.
    // It is often faster than `sort_by` when stable sorting is not required.
    // We compare frequencies first (b.1.cmp(&a.1) for descending), and if they match,
    // we compare words lexicographically (a.0.cmp(&b.0) for ascending).
    entries.sort_unstable_by(|a, b| match b.1.cmp(&a.1) {
        Ordering::Equal => a.0.cmp(&b.0),
        other => other,
    });

    entries
        .into_iter()
        .take(k as usize)
        .map(|(word, _)| word)
        .collect()
}

/// A custom struct to store word frequency and enable a custom Min-Heap.
#[derive(Eq, PartialEq)]
struct WordFreq {
    word: String,
    freq: usize,
}

// RUST INSIGHT: We implement Ord to reverse the default behavior of BinaryHeap (Max-Heap)
// into a Min-Heap. The heap top should be the element to discard when size exceeds k.
// So, the top should have the *lowest* frequency. If frequencies are equal, the top
// should have the *largest* lexicographical string, so it gets discarded first.
impl Ord for WordFreq {
    fn cmp(&self, other: &Self) -> Ordering {
        // We want BinaryHeap (a Max-Heap) to pop the elements we DON'T want to keep.
        // We want to evict elements with the LOWEST frequency.
        // Therefore, a LOWER frequency should be considered GREATER.
        match other.freq.cmp(&self.freq) {
            Ordering::Equal => {
                // If frequencies are equal, we want to evict the LARGEST lexicographical string.
                // Therefore, a LARGER string should be considered GREATER.
                self.word.cmp(&other.word)
            }
            other_ord => other_ord,
        }
    }
}

impl PartialOrd for WordFreq {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Optimized Approach: HashMap + Min-Heap of size K
///
/// We count frequencies using a HashMap. Then, we use a custom `WordFreq` struct and a
/// `BinaryHeap` functioning as a Min-Heap of size `k`. This ensures that we only keep the top `k` elements.
///
/// - **Time Complexity**: O(N log K), where N is the number of words. The HashMap population takes O(N). Pushing into a heap of size K takes O(log K), and we do it at most N times.
/// - **Space Complexity**: O(N) to store the frequencies in the HashMap. The Heap takes O(K) space.
pub fn top_k_frequent_optimized(words: Vec<String>, k: i32) -> Vec<String> {
    let mut counts: HashMap<String, usize> = HashMap::new();
    for word in words {
        *counts.entry(word).or_insert(0) += 1;
    }

    let k_usize = k as usize;
    // GOTCHA: BinaryHeap is a Max-Heap by default. Our `WordFreq`'s `Ord` implementation
    // handles the logic inversion to make it act like a Min-Heap.
    let mut heap: BinaryHeap<WordFreq> = BinaryHeap::with_capacity(k_usize + 1);

    for (word, freq) in counts {
        heap.push(WordFreq { word, freq });
        if heap.len() > k_usize {
            heap.pop(); // Remove the element with the lowest frequency (or largest word alphabetically)
        }
    }

    // Extract elements from the heap. Since it's a Min-Heap, elements come out in ascending
    // frequency order (and reverse alphabetical for ties). We need them descending, so we
    // collect them into a Vec and reverse it.
    let mut res = Vec::with_capacity(k_usize);
    while let Some(wf) = heap.pop() {
        res.push(wf.word);
    }
    res.reverse();
    res
}

/// Main entry point
pub fn top_k_frequent(words: Vec<String>, k: i32) -> Vec<String> {
    top_k_frequent_optimized(words, k)
}

// Alternative Approaches:
// 1. **BTreeMap**: You can map frequencies to a BTreeSet of words. This is often cleaner
//    in code but might have higher overhead due to tree rebalancing and allocation per node.
// 2. **Bucket Sort**: Since max frequency is N, we can use an array of `Vec<String>` where
//    index is the frequency. Then we sort the inner `Vec`s alphabetically and collect the top K.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_straightforward_example_1() {
        let words = vec!["i", "love", "leetcode", "i", "love", "coding"]
            .into_iter()
            .map(String::from)
            .collect();
        let k = 2;
        let expected = vec!["i".to_string(), "love".to_string()];
        assert_eq!(top_k_frequent_straightforward(words, k), expected);
    }

    #[test]
    fn test_straightforward_example_2() {
        let words = vec![
            "the", "day", "is", "sunny", "the", "the", "the", "sunny", "is", "is",
        ]
        .into_iter()
        .map(String::from)
        .collect();
        let k = 4;
        let expected = vec![
            "the".to_string(),
            "is".to_string(),
            "sunny".to_string(),
            "day".to_string(),
        ];
        assert_eq!(top_k_frequent_straightforward(words, k), expected);
    }

    #[test]
    fn test_optimized_example_1() {
        let words = vec!["i", "love", "leetcode", "i", "love", "coding"]
            .into_iter()
            .map(String::from)
            .collect();
        let k = 2;
        let expected = vec!["i".to_string(), "love".to_string()];
        assert_eq!(top_k_frequent_optimized(words, k), expected);
    }

    #[test]
    fn test_optimized_example_2() {
        let words = vec![
            "the", "day", "is", "sunny", "the", "the", "the", "sunny", "is", "is",
        ]
        .into_iter()
        .map(String::from)
        .collect();
        let k = 4;
        let expected = vec![
            "the".to_string(),
            "is".to_string(),
            "sunny".to_string(),
            "day".to_string(),
        ];
        assert_eq!(top_k_frequent_optimized(words, k), expected);
    }

    #[test]
    fn test_edge_case_all_same_frequency() {
        let words = vec!["z", "y", "x", "w", "v"]
            .into_iter()
            .map(String::from)
            .collect();
        let k = 3;
        // With frequency 1 for all, it should be lexicographically sorted: v, w, x
        let expected = vec!["v".to_string(), "w".to_string(), "x".to_string()];
        assert_eq!(top_k_frequent(words, k), expected);
    }
}
