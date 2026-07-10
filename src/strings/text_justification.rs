//! # 68. Text Justification
//!
//! Difficulty: Hard
//! Link: <https://leetcode.com/problems/text-justification/>
//!
//! Given an array of strings `words` and a width `maxWidth`, format the text such that each line
//! has exactly `maxWidth` characters and is fully (left and right) justified.
//!
//! You should pack your words in a greedy approach; that is, pack as many words as you can in each line.
//! Pad extra spaces `' '` when necessary so that each line has exactly `maxWidth` characters.
//!
//! Extra spaces between words should be distributed as evenly as possible. If the number of spaces
//! on a line does not divide evenly between words, the empty slots on the left will be assigned more
//! spaces than the slots on the right.
//!
//! For the last line of text, it should be left-justified, and no extra space is inserted between words.
//!
//! This problem matters in Rust because it teaches you how to construct formatted output cleanly while avoiding
//! excessive `String` reallocations. It emphasizes the difference between owning a `String` versus operating
//! on string slices (`&str`), allowing for memory-efficient iteration and output building.
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::strings::text_justification::full_justify;
//!
//! let words = vec!["This".to_string(), "is".to_string(), "an".to_string(), "example".to_string(), "of".to_string(), "text".to_string(), "justification.".to_string()];
//! let max_width = 16;
//! let result = full_justify(words, max_width);
//! assert_eq!(result, vec![
//!     "This    is    an",
//!     "example  of text",
//!     "justification.  "
//! ]);
//! ```

/// Brute Force Approach: Simple Collection & String Interpolation
///
/// We iterate through words, keeping track of the current line's length. When a word exceeds the
/// limit, we justify the current line and add it to our output list.
///
/// Time: O(N) where N is the total characters across all words, as we iterate each word sequentially
///       and distribute spaces.
/// Space: O(N) to store the result as a `Vec<String>`.
///
/// How this differs from Java/Python/C++:
/// In Python, you'd likely use `"".join()` and list comprehensions aggressively. In Java, you'd use
/// a `StringBuilder`. In Rust, you have a direct representation of `String` as an owned buffer,
/// but using `.repeat()` for spaces requires dynamic allocations just like Java's `String.repeat()`.
///
/// Why this matters:
/// This explicitly uses standard collection mechanisms (a vector for the current line). It focuses
/// on algorithm correctness (how to divide the extra spaces) rather than memory optimal operations.
#[must_use] 
pub fn full_justify_brute_force(words: Vec<String>, max_width: i32) -> Vec<String> {
    let max_width = max_width as usize;
    let mut res = Vec::new();
    let mut current_line: Vec<String> = Vec::new();
    let mut current_len = 0;

    for word in words {
        // If adding the next word exceeds max width...
        if current_len + current_line.len() + word.len() > max_width {
            let mut line_str = String::with_capacity(max_width);
            let num_words = current_line.len();
            let total_spaces = max_width - current_len;

            // RUST INSIGHT: For string repetition, `str::repeat` is very convenient but
            // requires allocation. In brute-force, this is perfectly fine.
            if num_words == 1 {
                line_str.push_str(&current_line[0]);
                line_str.push_str(&" ".repeat(total_spaces));
            } else {
                let spaces_between = total_spaces / (num_words - 1);
                let mut extra_spaces = total_spaces % (num_words - 1);

                for (i, w) in current_line.iter().enumerate() {
                    line_str.push_str(w);
                    if i < num_words - 1 {
                        line_str.push_str(&" ".repeat(spaces_between));
                        if extra_spaces > 0 {
                            line_str.push(' ');
                            extra_spaces -= 1;
                        }
                    }
                }
            }
            res.push(line_str);

            current_line.clear();
            current_len = 0;
        }

        current_len += word.len();
        current_line.push(word);
    }

    // Handle last line
    let mut last_line = current_line.join(" ");
    let trailing_spaces = max_width - last_line.len();
    last_line.push_str(&" ".repeat(trailing_spaces));
    res.push(last_line);

    res
}

/// Optimal Approach: String Slice Iterator and Minimal Allocation
///
/// Instead of pushing cloned/owned Strings into a temporary `Vec`, we maintain a slice window over
/// the `&[String]` elements and calculate line boundaries in place. We preallocate
/// the exact final String buffer using `String::with_capacity`.
///
/// Time: O(N) where N is the total characters in the result.
/// Space: O(1) auxiliary space (excluding the returned vector).
///
/// How this differs from Java/Python/C++:
/// Unlike Python or Java which lack native string slices or immutable views into large character arrays
/// without copying or heap allocations, Rust's `&str` and slicing (`&words[i..j]`) enable us to traverse
/// the data natively and copy bytes exactly once directly into our pre-allocated `String` buffers.
///
/// RUST INSIGHT:
/// - Taking `words: Vec<String>` means we own the data.
/// - We can just work on `&[String]` via `&words`.
/// - By passing a pre-allocated capacity to our `String`, we avoid the overhead of dynamically
///   resizing the buffer when appending the characters.
/// - Instead of using `" ".repeat(n)` which creates an intermediate string, we use `.extend(std::iter::repeat(' ').take(n))`
///   which just writes bytes directly into our buffer without any extra allocations.
///
/// GOTCHA:
/// Do not confuse byte length with character length, though `LeetCode` guarantees ASCII so
/// `.len()` is safe here. If there were Unicode text, `.chars().count()` might be required for `max_width`.
#[must_use] 
pub fn full_justify_optimal(words: Vec<String>, max_width: i32) -> Vec<String> {
    let max_width = max_width as usize;
    let mut res = Vec::new();
    let mut i = 0;

    while i < words.len() {
        let mut line_len = words[i].len();
        let mut j = i + 1;

        // Find how many words fit into the line
        while j < words.len() && line_len + 1 + words[j].len() <= max_width {
            line_len += 1 + words[j].len();
            j += 1;
        }

        let num_words = j - i;
        // Total chars in the words without spaces
        let words_len: usize = words[i..j].iter().map(std::string::String::len).sum();
        let total_spaces = max_width - words_len;

        let mut line = String::with_capacity(max_width);

        if num_words == 1 || j == words.len() {
            // Left justify for a single word or the very last line
            for k in i..j {
                line.push_str(&words[k]);
                if k < j - 1 {
                    line.push(' ');
                }
            }
            // Pad remaining spaces
            if line.len() < max_width {
                line.extend(std::iter::repeat_n(' ', max_width - line.len()));
            }
        } else {
            // Distribute spaces evenly
            let spaces_between = total_spaces / (num_words - 1);
            let mut extra_spaces = total_spaces % (num_words - 1);

            for k in i..j {
                line.push_str(&words[k]);
                if k < j - 1 {
                    line.extend(std::iter::repeat_n(' ', spaces_between));
                    if extra_spaces > 0 {
                        line.push(' ');
                        extra_spaces -= 1;
                    }
                }
            }
        }

        res.push(line);
        i = j;
    }

    res
}

/// Main Entry Point
#[must_use] 
pub fn full_justify(words: Vec<String>, max_width: i32) -> Vec<String> {
    full_justify_optimal(words, max_width)
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Alternative approaches:
// 1. `std::fmt::Write`: You can implement `std::fmt::Display` for a custom line-grouper struct
//    and write directly to a `String` using `write!`.
// 2. Stateful Iterators: You can wrap `words.into_iter()` inside a custom `Iterator` implementation
//    that yields `String` directly. While highly idiomatic, the standard imperative slice
//    approach is often easier to debug and more common for interview solutions.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_brute_force_example() {
        let words = vec![
            "This".to_string(),
            "is".to_string(),
            "an".to_string(),
            "example".to_string(),
            "of".to_string(),
            "text".to_string(),
            "justification.".to_string(),
        ];
        let expected = vec!["This    is    an", "example  of text", "justification.  "];
        assert_eq!(full_justify_brute_force(words, 16), expected);
    }

    #[test]
    fn test_optimal_example() {
        let words = vec![
            "This".to_string(),
            "is".to_string(),
            "an".to_string(),
            "example".to_string(),
            "of".to_string(),
            "text".to_string(),
            "justification.".to_string(),
        ];
        let expected = vec!["This    is    an", "example  of text", "justification.  "];
        assert_eq!(full_justify_optimal(words, 16), expected);
    }

    #[test]
    fn test_single_word_line() {
        let words = vec![
            "What".to_string(),
            "must".to_string(),
            "be".to_string(),
            "acknowledgment".to_string(),
            "shall".to_string(),
            "be".to_string(),
        ];
        let expected = vec!["What   must   be", "acknowledgment  ", "shall be        "];
        assert_eq!(full_justify(words, 16), expected);
    }

    #[test]
    fn test_long_spaces() {
        let words = vec![
            "Science".to_string(),
            "is".to_string(),
            "what".to_string(),
            "we".to_string(),
            "understand".to_string(),
            "well".to_string(),
            "enough".to_string(),
            "to".to_string(),
            "explain".to_string(),
            "to".to_string(),
            "a".to_string(),
            "computer.".to_string(),
            "Art".to_string(),
            "is".to_string(),
            "everything".to_string(),
            "else".to_string(),
            "we".to_string(),
            "do".to_string(),
        ];
        let expected = vec![
            "Science  is  what we",
            "understand      well",
            "enough to explain to",
            "a  computer.  Art is",
            "everything  else  we",
            "do                  ",
        ];
        assert_eq!(full_justify(words, 20), expected);
    }

    #[test]
    fn test_single_word_input() {
        let words = vec!["Hello".to_string()];
        let expected = vec!["Hello     "];
        assert_eq!(full_justify(words, 10), expected);
    }
}
