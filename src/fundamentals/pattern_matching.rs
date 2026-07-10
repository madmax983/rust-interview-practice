//! Common pattern matching idioms for Rust coding interviews
//!
//! This module contains frequently-used patterns for match, if let,
//! destructuring, and guards.

#![allow(clippy::doc_markdown)] // Type names in docs are clear without backticks

/// Pattern: Basic match with exhaustive cases
#[must_use]
pub fn basic_match(value: i32) -> &'static str {
    match value {
        0 => "zero",
        1 => "one",
        2 => "two",
        _ => "many",
    }
}

/// Pattern: Match with ranges
#[must_use]
pub fn match_ranges(value: i32) -> &'static str {
    match value {
        i32::MIN..=0 => "negative",
        1..=10 => "small",
        11..=100 => "medium",
        _ => "large",
    }
}

/// Pattern: Match with guards
#[must_use]
pub fn match_with_guard(value: i32) -> &'static str {
    match value {
        x if x < 0 => "negative",  // Guard: 'if' adds extra condition
        x if x % 2 == 0 => "even", // Guards checked in order
        _ => "odd",                // Wildcard catches everything else
    }
}

/// Pattern: Match on tuples
#[must_use]
pub fn match_tuple(pair: (i32, i32)) -> i32 {
    match pair {
        (0, 0) => 0,     // Match exact values
        (x, 0) => x,     // Bind first element to x, match 0 for second
        (0, y) => y,     // Match 0 for first, bind second to y
        (x, y) => x + y, // Bind both elements
    }
}

/// Pattern: Match with destructuring
#[must_use]
#[allow(clippy::match_wildcard_for_single_variants)]
pub fn match_enum(value: Option<i32>) -> i32 {
    match value {
        Some(x) => x * 2,
        None => 0,
    }
}

/// Pattern: if let for single variant
#[must_use]
pub fn if_let_single(value: Option<i32>) -> i32 {
    // if let: cleaner than match when you only care about one pattern
    if let Some(x) = value {
        x * 2 // Extract and use value
    } else {
        0 // Handle other cases
    }
}

/// Pattern: if let with multiple patterns
#[must_use]
#[allow(clippy::manual_unwrap_or_default, clippy::manual_unwrap_or)] // Demonstrating if let / else on Result
pub fn if_let_multiple(value: Result<i32, String>) -> i32 {
    // A Result is exhaustive: if the Ok arm does not match, it must be Err,
    // so the else branch covers every remaining case (no dead arm needed).
    if let Ok(x) = value { x } else { 0 }
}

/// Pattern: while let for iteration
#[must_use]
pub fn while_let_iteration(mut stack: Vec<i32>) -> Vec<i32> {
    let mut result = Vec::new();
    while let Some(value) = stack.pop() {
        result.push(value * 2);
    }
    result
}

/// Pattern: Destructuring in function parameters
#[must_use]
pub fn destructure_params((x, y): (i32, i32)) -> i32 {
    x + y
}

/// Pattern: Destructuring in let bindings
#[must_use]
pub fn destructure_let(pair: (i32, i32)) -> i32 {
    let (x, y) = pair;
    x * y
}

/// Pattern: Match on slices
#[must_use]
pub fn match_slice(slice: &[i32]) -> i32 {
    match slice {
        [] => 0,             // Empty slice
        [x] => *x,           // Exactly one element (dereference needed)
        [x, y] => x + y,     // Exactly two elements
        [x, .., y] => x + y, // Two or more: first + last (.. ignores middle)
    }
}

/// Pattern: Match with @ binding
#[must_use]
pub fn match_at_binding(value: i32) -> String {
    match value {
        x @ 1..=5 => format!("small: {x}"), // @ binds matched value to x
        x @ 6..=10 => format!("medium: {x}"), // Can use x in the arm
        x => format!("other: {x}"),         // Simple binding (no range)
    }
}

/// Pattern: Match on references
#[must_use]
pub fn match_reference(opt: &Option<i32>) -> i32 {
    match opt {
        Some(x) => *x,
        None => 0,
    }
}

/// Pattern: Nested match
#[must_use]
pub fn nested_match(outer: Option<Option<i32>>) -> i32 {
    match outer {
        Some(Some(x)) => x,
        Some(None) => -1,
        None => 0,
    }
}

/// Pattern: Match multiple values with or patterns
#[must_use]
pub fn match_or_pattern(ch: char) -> bool {
    matches!(ch, 'a' | 'e' | 'i' | 'o' | 'u')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_match() {
        assert_eq!(basic_match(0), "zero");
        assert_eq!(basic_match(1), "one");
        assert_eq!(basic_match(5), "many");
    }

    #[test]
    fn test_match_ranges() {
        assert_eq!(match_ranges(-5), "negative");
        assert_eq!(match_ranges(5), "small");
        assert_eq!(match_ranges(50), "medium");
        assert_eq!(match_ranges(200), "large");
    }

    #[test]
    fn test_match_with_guard() {
        assert_eq!(match_with_guard(-5), "negative");
        assert_eq!(match_with_guard(4), "even");
        assert_eq!(match_with_guard(5), "odd");
    }

    #[test]
    fn test_match_tuple() {
        assert_eq!(match_tuple((0, 0)), 0);
        assert_eq!(match_tuple((5, 0)), 5);
        assert_eq!(match_tuple((0, 3)), 3);
        assert_eq!(match_tuple((2, 3)), 5);
    }

    #[test]
    fn test_match_enum() {
        assert_eq!(match_enum(Some(5)), 10);
        assert_eq!(match_enum(None), 0);
    }

    #[test]
    fn test_if_let_single() {
        assert_eq!(if_let_single(Some(5)), 10);
        assert_eq!(if_let_single(None), 0);
    }

    #[test]
    fn test_if_let_multiple() {
        assert_eq!(if_let_multiple(Ok(5)), 5);
        assert_eq!(if_let_multiple(Err("error".to_string())), 0);
    }

    #[test]
    fn test_while_let_iteration() {
        assert_eq!(while_let_iteration(vec![1, 2, 3]), vec![6, 4, 2]);
    }

    #[test]
    fn test_destructure_params() {
        assert_eq!(destructure_params((3, 4)), 7);
    }

    #[test]
    fn test_destructure_let() {
        assert_eq!(destructure_let((3, 4)), 12);
    }

    #[test]
    fn test_match_slice() {
        assert_eq!(match_slice(&[]), 0);
        assert_eq!(match_slice(&[5]), 5);
        assert_eq!(match_slice(&[2, 3]), 5);
        assert_eq!(match_slice(&[1, 2, 3, 4]), 5);
    }

    #[test]
    fn test_match_at_binding() {
        assert_eq!(match_at_binding(3), "small: 3");
        assert_eq!(match_at_binding(7), "medium: 7");
        assert_eq!(match_at_binding(15), "other: 15");
    }

    #[test]
    fn test_match_reference() {
        assert_eq!(match_reference(&Some(5)), 5);
        assert_eq!(match_reference(&None), 0);
    }

    #[test]
    fn test_nested_match() {
        assert_eq!(nested_match(Some(Some(5))), 5);
        assert_eq!(nested_match(Some(None)), -1);
        assert_eq!(nested_match(None), 0);
    }

    #[test]
    fn test_match_or_pattern() {
        assert!(match_or_pattern('a'));
        assert!(match_or_pattern('e'));
        assert!(!match_or_pattern('b'));
    }
}
