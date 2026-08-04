//! # 127. Word Ladder
//!
//! Difficulty: Hard
//! Link: <https://leetcode.com/problems/word-ladder/>
//!
//! A transformation sequence from word `beginWord` to word `endWord` using a dictionary `wordList` is a sequence of words `beginWord -> s_1 -> s_2 -> ... -> s_k` such that:
//! - Every adjacent pair of words differs by a single letter.
//! - Every `s_i` for `1 <= i <= k` is in `wordList`. Note that `beginWord` does not need to be in `wordList`.
//! - `s_k == endWord`
//!
//! Given two words, `beginWord` and `endWord`, and a dictionary `wordList`, return the number of words in the shortest transformation sequence from `beginWord` to `endWord`, or 0 if no such sequence exists.
//!
//! This problem matters in Rust because it perfectly illustrates the performance cliffs associated with string manipulation in systems programming. In languages like C++ or Java, characters in a string can be mutated in `O(1)` time via array indexing. In Rust, standard strings are UTF-8 encoded, making `O(1)` indexing impossible. This problem demonstrates the progression from naive `String` allocation to zero-cost ASCII manipulation using byte slices, and advanced state tracking with `HashSet` intersections.

use std::collections::{HashSet, VecDeque};

/// Brute force approach: Standard BFS with String Allocations.
/// Time: O(N * M^2) where N is the number of words and M is the length of each word.
/// Space: O(N * M) for the queue and visited set.
///
/// This approach represents the "naive" Rust way. It mutates the string by replacing each character
/// one by one, allocating a new `String` every time. It uses `.chars()` and `.collect()`, which
/// are safe but slow due to repeated heap allocations and UTF-8 validation overhead.
#[must_use]
// LeetCode signature: three implementations share (String, String, Vec<String>) by value.
#[allow(clippy::needless_pass_by_value)]
pub fn word_ladder_brute_force(
    begin_word: String,
    end_word: String,
    word_list: Vec<String>,
) -> i32 {
    if begin_word == end_word {
        return 1;
    }

    let mut word_set: HashSet<String> = word_list.into_iter().collect();
    if !word_set.contains(&end_word) {
        return 0;
    }

    // Remove the starting word so we don't cycle back to it
    word_set.remove(&begin_word);

    let mut queue = VecDeque::new();
    queue.push_back((begin_word, 1));

    while let Some((current_word, level)) = queue.pop_front() {
        if current_word == end_word {
            return level;
        }

        // GOTCHA: Mutating strings in Rust by replacing characters is cumbersome and slow
        // if we allocate new strings repeatedly. `chars().collect()` does an O(M) allocation.
        let chars: Vec<char> = current_word.chars().collect();
        for i in 0..chars.len() {
            let original_char = chars[i];
            for c in 'a'..='z' {
                if c == original_char {
                    continue;
                }

                let mut new_chars = chars.clone();
                new_chars[i] = c;
                let new_word: String = new_chars.into_iter().collect();

                if word_set.contains(&new_word) {
                    // RUST INSIGHT: Removing from the set serves two purposes:
                    // 1. It marks the word as visited.
                    // 2. It avoids the need for a separate `visited` HashSet, saving memory.
                    word_set.remove(&new_word);
                    queue.push_back((new_word, level + 1));
                }
            }
        }
    }

    0
}

/// Optimized approach: BFS with Byte Array Manipulation (`Vec<u8>`).
/// Time: O(N * M * 26) = O(N * M)
/// Space: O(N * M) for queue and visited set.
///
/// By recognizing that `LeetCode` inputs guarantee ASCII characters ('a'-'z'), we can safely cast
/// strings to `Vec<u8>` or `&[u8]`. This bypasses UTF-8 boundaries and allows us to perform
/// O(1) in-place byte manipulation, drastically reducing heap allocations per character mutation.
#[must_use]
pub fn word_ladder_optimized(begin_word: String, end_word: String, word_list: Vec<String>) -> i32 {
    let mut word_set: HashSet<Vec<u8>> = word_list
        .into_iter()
        .map(std::string::String::into_bytes)
        .collect();
    let end_word_bytes = end_word.into_bytes();

    if !word_set.contains(&end_word_bytes) {
        return 0;
    }

    let begin_word_bytes = begin_word.into_bytes();
    if begin_word_bytes == end_word_bytes {
        return 1;
    }

    word_set.remove(&begin_word_bytes);

    let mut queue = VecDeque::new();
    queue.push_back((begin_word_bytes, 1));

    while let Some((mut current_word_bytes, level)) = queue.pop_front() {
        if current_word_bytes == end_word_bytes {
            return level;
        }

        // RUST INSIGHT: Because we operate on bytes, we don't need intermediate string allocations
        // or `.chars()`. We just temporarily replace a byte, check the map, and put it back.
        for i in 0..current_word_bytes.len() {
            let original_byte = current_word_bytes[i];

            for b in b'a'..=b'z' {
                if b == original_byte {
                    continue;
                }

                // In-place mutation for zero-allocation checks
                current_word_bytes[i] = b;

                if let Some(next_word) = word_set.take(&current_word_bytes) {
                    queue.push_back((next_word, level + 1));
                }
            }

            // Restore the original byte for the next iteration
            current_word_bytes[i] = original_byte;
        }
    }

    0
}

/// Optimal approach: Bidirectional BFS.
/// Time: O(N * M) but the search space branching factor is drastically reduced.
/// Space: O(N * M)
///
/// Instead of searching from `begin_word` to `end_word`, we search simultaneously from both
/// ends. We always expand the smaller perimeter (level boundary) to minimize branching. If the two
/// search perimeters ever intersect, we've found the shortest path.
#[must_use]
pub fn word_ladder_optimal(begin_word: String, end_word: String, word_list: Vec<String>) -> i32 {
    let mut word_set: HashSet<Vec<u8>> = word_list
        .into_iter()
        .map(std::string::String::into_bytes)
        .collect();
    let end_word_bytes = end_word.into_bytes();

    if !word_set.contains(&end_word_bytes) {
        return 0;
    }

    let begin_word_bytes = begin_word.into_bytes();
    if begin_word_bytes == end_word_bytes {
        return 1;
    }

    // Bidirectional BFS uses HashSets for levels instead of queues to allow fast intersection checks.
    let mut begin_set = HashSet::new();
    let mut end_set = HashSet::new();

    begin_set.insert(begin_word_bytes.clone());
    end_set.insert(end_word_bytes.clone());

    // Remove the starting and ending boundaries so we don't self-loop or trivially match
    // already visited perimeters in redundant ways.
    word_set.remove(&begin_word_bytes);
    word_set.remove(&end_word_bytes);

    let mut level = 1;

    while !begin_set.is_empty() && !end_set.is_empty() {
        // Always expand the smaller set to minimize the search space (branching factor).
        if begin_set.len() > end_set.len() {
            // RUST INSIGHT: `std::mem::swap` is a zero-cost O(1) operation that simply
            // swaps pointers to the underlying heap allocations. No data is copied.
            std::mem::swap(&mut begin_set, &mut end_set);
        }

        let mut next_set = HashSet::new();

        // Process all words in the current level boundary
        // ⚡ BOLT OPTIMIZATION: Consuming the `HashSet` directly yields owned values,
        // eliminating the need to clone the byte vector for each word in the level.
        for mut current_word_bytes in begin_set {
            for i in 0..current_word_bytes.len() {
                let original_byte = current_word_bytes[i];

                for b in b'a'..=b'z' {
                    if b == original_byte {
                        continue;
                    }

                    current_word_bytes[i] = b;

                    // If the other side's perimeter contains our new word, the paths have intersected!
                    if end_set.contains(&current_word_bytes) {
                        return level + 1;
                    }

                    if let Some(next_word) = word_set.take(&current_word_bytes) {
                        next_set.insert(next_word);
                    }
                }

                current_word_bytes[i] = original_byte;
            }
        }

        begin_set = next_set;
        level += 1;
    }

    0
}

/// Main entry point - uses the optimal bidirectional BFS approach.
#[must_use]
pub fn ladder_length(begin_word: String, end_word: String, word_list: Vec<String>) -> i32 {
    word_ladder_optimal(begin_word, end_word, word_list)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Helper function to easily convert arrays of &str to Vec<String>
    fn to_string_vec(words: &[&str]) -> Vec<String> {
        words.iter().map(|&s| s.to_string()).collect()
    }

    // Happy Path tests
    #[test]
    fn test_word_ladder_happy_path() {
        let begin_word = "hit".to_string();
        let end_word = "cog".to_string();
        let word_list = to_string_vec(&["hot", "dot", "dog", "lot", "log", "cog"]);

        assert_eq!(
            word_ladder_brute_force(begin_word.clone(), end_word.clone(), word_list.clone()),
            5
        );
        assert_eq!(
            word_ladder_optimized(begin_word.clone(), end_word.clone(), word_list.clone()),
            5
        );
        assert_eq!(
            word_ladder_optimal(begin_word.clone(), end_word.clone(), word_list.clone()),
            5
        );
        assert_eq!(ladder_length(begin_word, end_word, word_list), 5);
    }

    // Edge Case tests
    #[test]
    fn test_word_ladder_no_path() {
        let begin_word = "hit".to_string();
        let end_word = "cog".to_string();
        let word_list = to_string_vec(&["hot", "dot", "dog", "lot", "log"]); // "cog" missing

        assert_eq!(
            word_ladder_brute_force(begin_word.clone(), end_word.clone(), word_list.clone()),
            0
        );
        assert_eq!(
            word_ladder_optimized(begin_word.clone(), end_word.clone(), word_list.clone()),
            0
        );
        assert_eq!(word_ladder_optimal(begin_word, end_word, word_list), 0);
    }

    #[test]
    fn test_word_ladder_direct_connection() {
        let begin_word = "hit".to_string();
        let end_word = "hot".to_string();
        let word_list = to_string_vec(&["hot", "dot", "dog"]);

        assert_eq!(
            word_ladder_brute_force(begin_word.clone(), end_word.clone(), word_list.clone()),
            2
        );
        assert_eq!(
            word_ladder_optimized(begin_word.clone(), end_word.clone(), word_list.clone()),
            2
        );
        assert_eq!(word_ladder_optimal(begin_word, end_word, word_list), 2);
    }

    // Stress/Boundary tests
    #[test]
    fn test_word_ladder_longer_path() {
        let begin_word = "a".to_string();
        let end_word = "c".to_string();
        let word_list = to_string_vec(&["a", "b", "c"]);

        assert_eq!(
            word_ladder_brute_force(begin_word.clone(), end_word.clone(), word_list.clone()),
            2
        );
        assert_eq!(
            word_ladder_optimized(begin_word.clone(), end_word.clone(), word_list.clone()),
            2
        );
        assert_eq!(word_ladder_optimal(begin_word, end_word, word_list), 2);
    }
}
