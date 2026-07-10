//! Common string manipulation patterns for Rust coding interviews
//!
//! This module contains frequently-used patterns for String, &str,
//! char iteration, and string transformations.

#![allow(clippy::doc_markdown)] // Type names in docs are clear without backticks

/// Pattern: String vs &str - owned vs borrowed
#[must_use]
pub fn string_vs_str(s: String) -> String {
    let borrowed: &str = &s; // String to &str (borrow)
    borrowed.to_string() // &str to String (allocate new string)
}

/// Pattern: Iterating over characters
#[must_use]
pub fn iterate_chars(s: &str) -> Vec<char> {
    s.chars() // Returns iterator over Unicode scalar values
        .collect() // Collect into Vec<char>
}

/// Pattern: Iterating over bytes
#[must_use]
pub fn iterate_bytes(s: &str) -> Vec<u8> {
    s.bytes().collect()
}

/// Pattern: Char at index (careful - O(n) for UTF-8)
#[must_use]
pub fn char_at_index(s: &str, idx: usize) -> Option<char> {
    s.chars().nth(idx)
}

/// Pattern: Convert to Vec<char> for O(1) indexing
#[must_use]
pub fn string_to_vec_chars(s: &str) -> Vec<char> {
    s.chars().collect() // Important: &str indexing is O(n), Vec<char> is O(1)
}

/// Pattern: String building with push_str and push
#[must_use]
pub fn build_string(parts: Vec<&str>) -> String {
    let mut result = String::new(); // Create empty owned string
    for part in parts {
        result.push_str(part); // Append &str
        result.push(' '); // Append single char
    }
    result
}

/// Pattern: String building with format!
#[must_use]
pub fn format_string(name: &str, age: i32) -> String {
    format!("{name} is {age} years old")
}

/// Pattern: Joining with separator
#[must_use]
pub fn join_strings(parts: Vec<&str>) -> String {
    parts.join(", ")
}

/// Pattern: Splitting strings
#[must_use]
pub fn split_string(s: &str) -> Vec<&str> {
    s.split_whitespace() // Iterator over substrings (borrows from s)
        .collect() // Collect into Vec<&str>
}

/// Pattern: Split by delimiter
#[must_use]
pub fn split_by_delimiter(s: &str, delim: char) -> Vec<&str> {
    s.split(delim).collect()
}

/// Pattern: Checking prefixes/suffixes
#[must_use]
pub fn check_prefix_suffix(s: &str) -> (bool, bool) {
    let has_prefix = s.starts_with("hello");
    let has_suffix = s.ends_with("world");
    (has_prefix, has_suffix)
}

/// Pattern: Trimming whitespace
#[must_use]
pub fn trim_example(s: &str) -> &str {
    s.trim()
}

/// Pattern: Case conversion
#[must_use]
pub fn case_conversion(s: &str) -> (String, String) {
    (s.to_lowercase(), s.to_uppercase())
}

/// Pattern: Reversing a string
#[must_use]
pub fn reverse_string(s: &str) -> String {
    s.chars() // Iterate over chars
        .rev() // Reverse the iterator
        .collect() // Build String from reversed chars
}

/// Pattern: Checking if string contains substring
#[must_use]
pub fn contains_substring(s: &str, pattern: &str) -> bool {
    s.contains(pattern)
}

/// Pattern: Finding index of substring
#[must_use]
pub fn find_substring(s: &str, pattern: &str) -> Option<usize> {
    s.find(pattern)
}

/// Pattern: Replacing substrings
#[must_use]
pub fn replace_substring(s: &str, from: &str, to: &str) -> String {
    s.replace(from, to)
}

/// Pattern: Char classification
#[must_use]
pub fn classify_char(ch: char) -> &'static str {
    if ch.is_alphabetic() {
        // Built-in char methods for classification
        "letter"
    } else if ch.is_numeric() {
        "digit"
    } else if ch.is_whitespace() {
        "whitespace"
    } else {
        "other"
    }
}

/// Pattern: String to number parsing
///
/// # Errors
///
/// Returns a `ParseIntError` if `s` is not a valid `i32`.
pub fn parse_number(s: &str) -> Result<i32, std::num::ParseIntError> {
    s.parse::<i32>()
}

/// Pattern: Collecting chars into String
#[must_use]
pub fn chars_to_string(chars: Vec<char>) -> String {
    chars.iter().collect()
}

/// Pattern: Removing characters
#[must_use]
pub fn remove_char(s: &str, target: char) -> String {
    s.chars().filter(|&c| c != target).collect()
}

/// Pattern: Counting character occurrences
#[must_use]
pub fn count_char(s: &str, target: char) -> usize {
    s.chars().filter(|&c| c == target).count()
}

/// Pattern: String slicing (byte-based - careful with UTF-8)
#[must_use]
pub fn string_slice(s: &str) -> &str {
    if s.len() >= 5 { &s[0..5] } else { s }
}

/// Pattern: Checking if all chars satisfy condition
#[must_use]
pub fn all_digits(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_string_vs_str() {
        let result = string_vs_str("hello".to_string());
        assert_eq!(result, "hello");
    }

    #[test]
    fn test_iterate_chars() {
        assert_eq!(iterate_chars("abc"), vec!['a', 'b', 'c']);
    }

    #[test]
    fn test_iterate_bytes() {
        assert_eq!(iterate_bytes("abc"), vec![97, 98, 99]);
    }

    #[test]
    fn test_char_at_index() {
        assert_eq!(char_at_index("hello", 1), Some('e'));
        assert_eq!(char_at_index("hello", 10), None);
    }

    #[test]
    fn test_string_to_vec_chars() {
        assert_eq!(string_to_vec_chars("rust"), vec!['r', 'u', 's', 't']);
    }

    #[test]
    fn test_build_string() {
        let result = build_string(vec!["hello", "world"]);
        assert_eq!(result, "hello world ");
    }

    #[test]
    fn test_format_string() {
        assert_eq!(format_string("Alice", 30), "Alice is 30 years old");
    }

    #[test]
    fn test_join_strings() {
        assert_eq!(join_strings(vec!["a", "b", "c"]), "a, b, c");
    }

    #[test]
    fn test_split_string() {
        assert_eq!(
            split_string("hello world rust"),
            vec!["hello", "world", "rust"]
        );
    }

    #[test]
    fn test_split_by_delimiter() {
        assert_eq!(split_by_delimiter("a,b,c", ','), vec!["a", "b", "c"]);
    }

    #[test]
    fn test_check_prefix_suffix() {
        assert_eq!(check_prefix_suffix("hello world"), (true, true));
        assert_eq!(check_prefix_suffix("goodbye"), (false, false));
    }

    #[test]
    fn test_trim_example() {
        assert_eq!(trim_example("  hello  "), "hello");
    }

    #[test]
    fn test_case_conversion() {
        let (lower, upper) = case_conversion("Hello");
        assert_eq!(lower, "hello");
        assert_eq!(upper, "HELLO");
    }

    #[test]
    fn test_reverse_string() {
        assert_eq!(reverse_string("hello"), "olleh");
    }

    #[test]
    fn test_contains_substring() {
        assert!(contains_substring("hello world", "world"));
        assert!(!contains_substring("hello world", "rust"));
    }

    #[test]
    fn test_find_substring() {
        assert_eq!(find_substring("hello world", "world"), Some(6));
        assert_eq!(find_substring("hello world", "rust"), None);
    }

    #[test]
    fn test_replace_substring() {
        assert_eq!(
            replace_substring("hello world", "world", "rust"),
            "hello rust"
        );
    }

    #[test]
    fn test_classify_char() {
        assert_eq!(classify_char('a'), "letter");
        assert_eq!(classify_char('5'), "digit");
        assert_eq!(classify_char(' '), "whitespace");
        assert_eq!(classify_char('!'), "other");
    }

    #[test]
    fn test_parse_number() {
        assert_eq!(parse_number("42"), Ok(42));
        assert!(parse_number("abc").is_err());
    }

    #[test]
    fn test_chars_to_string() {
        assert_eq!(chars_to_string(vec!['h', 'i']), "hi");
    }

    #[test]
    fn test_remove_char() {
        assert_eq!(remove_char("hello", 'l'), "heo");
    }

    #[test]
    fn test_count_char() {
        assert_eq!(count_char("hello", 'l'), 2);
    }

    #[test]
    fn test_string_slice() {
        assert_eq!(string_slice("hello world"), "hello");
        assert_eq!(string_slice("hi"), "hi");
    }

    #[test]
    fn test_all_digits() {
        assert!(all_digits("12345"));
        assert!(!all_digits("123a5"));
        assert!(!all_digits(""));
    }
}
