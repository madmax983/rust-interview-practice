//! # 151. Reverse Words in a String
//!
//! Difficulty: Medium
//! Link: <https://leetcode.com/problems/reverse-words-in-a-string/>
//!
//! Given an input string `s`, reverse the order of the **words**.
//!
//! A **word** is defined as a sequence of non-space characters. The words in `s`
//! will be separated by at least one space.
//!
//! Return a string of the words in reverse order concatenated by a single space.
//!
//! **Note** that `s` may contain leading or trailing spaces or multiple spaces between two words.
//! The returned string should only have a single space separating the words.
//! Do not include any extra spaces.
//!
//! ## Why this matters in Rust
//! This problem showcases the sheer power of Rust's standard library `str` methods and iterators.
//! Instead of manually managing loops and tracking start/end indices for words—which is common in
//! C++ or Java to achieve in-place modifications—Rust encourages zero-cost abstractions.
//! The `split_whitespace` iterator handles leading, trailing, and consecutive spaces seamlessly.
//!
//! ## Approach
//!
//! In languages like C/C++, a typical approach might be to reverse the entire string in-place,
//! then reverse each individual word in-place, and finally shift characters to remove extra spaces.
//! While possible in Rust using `into_bytes()` and mutable slices, working with UTF-8 strings
//! means you shouldn't blindly swap bytes unless you're guaranteed ASCII.
//!
//! The idiomatic Rust approach leans on its powerful, zero-cost Iterator trait.
//!
//! ### 1. Idiomatic (Iterator Chain)
//! The `split_whitespace()` iterator does the heavy lifting of parsing the words and
//! ignoring any extra spaces. By reversing the iterator and joining, we get an extremely readable
//! one-liner that compiles down to very efficient code.
//!
//! Time Complexity: O(N) where N is the length of `s`. We traverse the string.
//! Space Complexity: O(N) since we return a new String. Intermediate collections (like `Vec<&str>`)
//! take O(W) where W is the number of words.
//!
//! ### 2. Optimized (Pre-allocated String Builder)
//! While the first approach is highly readable, it might allocate an intermediate `Vec`
//! (if `.collect::<Vec<_>>().join(" ")` is used). We can optimize this by allocating
//! the `String` directly with the known maximum capacity (length of `s`) and using `std::fmt::Write`
//! or `push_str`. This eliminates intermediate allocations and handles everything in a single pass.
//!
//! Time Complexity: O(N)
//! Space Complexity: O(N) to hold the resulting string, but zero intermediate allocations.
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::strings::reverse_words_in_a_string::reverse_words_idiomatic;
//!
//! let result = reverse_words_idiomatic("the sky is blue".to_string());
//! assert_eq!(result, "blue is sky the");
//!
//! let result2 = reverse_words_idiomatic("  hello world  ".to_string());
//! assert_eq!(result2, "world hello");
//! ```

/// Reverses the words in a string using idiomatic iterator combinators.
///
/// This is the recommended approach for 99% of use cases in Rust.
/// It creates an intermediate `Vec<&str>` before joining, which is usually fine
/// unless you are in an extremely hot path.
#[must_use]
pub fn reverse_words_idiomatic(s: String) -> String {
    // RUST INSIGHT: `split_whitespace()` is incredibly powerful.
    // It automatically handles leading, trailing, and multiple internal spaces.
    // Unlike `.split(' ')`, it yields substrings of non-whitespace characters.
    s.split_whitespace()
        .rev()
        // GOTCHA: `join` requires an intermediate slice or `Vec` to work on,
        // so `collect::<Vec<_>>()` is necessary here.
        .collect::<Vec<_>>()
        .join(" ")
}

/// An optimized approach that avoids the intermediate `Vec<&str>` allocation.
///
/// It pre-allocates a `String` with the maximum possible required capacity,
/// then iterates over the words in reverse and appends them.
#[must_use]
pub fn reverse_words_optimal(s: String) -> String {
    // We know the result cannot be longer than the original string.
    // By pre-allocating, we avoid dynamic re-allocations during `push_str`.
    let mut result = String::with_capacity(s.len());

    // RUST INSIGHT: Iterating without collecting directly streams the references.
    let mut words = s.split_whitespace().rev().peekable();

    while let Some(word) = words.next() {
        result.push_str(word);

        // RUST INSIGHT: We use `peek` to determine if we need to add a space.
        // If there's another word coming, add a space.
        // This avoids having an extra trailing space that we'd have to `pop()` later.
        if words.peek().is_some() {
            result.push(' ');
        }
    }

    // The result might be shorter than `s.len()` because we removed extra spaces,
    // but the capacity was pre-allocated efficiently.
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_happy_path() {
        let input = "the sky is blue".to_string();
        assert_eq!(reverse_words_idiomatic(input.clone()), "blue is sky the");
        assert_eq!(reverse_words_optimal(input), "blue is sky the");
    }

    #[test]
    fn test_multiple_spaces_and_trimming() {
        // Edge case: Leading, trailing, and multiple spaces
        let input = "  hello world  ".to_string();
        assert_eq!(reverse_words_idiomatic(input.clone()), "world hello");
        assert_eq!(reverse_words_optimal(input), "world hello");

        let input2 = "a good   example".to_string();
        assert_eq!(reverse_words_idiomatic(input2.clone()), "example good a");
        assert_eq!(reverse_words_optimal(input2), "example good a");
    }

    #[test]
    fn test_single_word_and_empty() {
        // Edge cases: single words or just spaces
        let input = "single".to_string();
        assert_eq!(reverse_words_idiomatic(input.clone()), "single");
        assert_eq!(reverse_words_optimal(input), "single");

        let input2 = "   ".to_string();
        assert_eq!(reverse_words_idiomatic(input2.clone()), "");
        assert_eq!(reverse_words_optimal(input2), "");
    }

    #[test]
    fn test_stress_long_string() {
        // Stress case: long string
        let mut input = String::new();
        for _ in 0..1000 {
            input.push_str("word  ");
        }
        let expected = vec!["word"; 1000].join(" ");

        assert_eq!(reverse_words_idiomatic(input.clone()), expected);
        assert_eq!(reverse_words_optimal(input), expected);
    }
}
