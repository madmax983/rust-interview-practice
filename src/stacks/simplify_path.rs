//! # 71. Simplify Path
//!
//! Difficulty: Medium
//! Link: <https://leetcode.com/problems/simplify-path/>
//!
//! Given an absolute path for a file (Unix-style), simplify it. Or in other words,
//! convert it to the canonical path.
//!
//! In a UNIX-style file system, a period `.` refers to the current directory.
//! Furthermore, a double period `..` moves the directory up a level.
//!
//! Note that the returned canonical path must always begin with a slash `/`, and
//! there must be only a single slash `/` between two directory names. The last
//! directory name (if it exists) must not end with a trailing `/`. Also, the
//! canonical path must be the shortest string representing the absolute path.
//!
//! Why this matters in Rust:
//! This problem is a perfect showcase for Rust's string handling capabilities,
//! specifically the `split` iterator and `Vec` as a stack. It demonstrates how to
//! manipulate path components efficiently without manual index tracking or
//! complicated state machines, leveraging zero-allocation slicing (`&str`) where possible.

/// Stack-based approach: Process components sequentially
/// Time: O(n) - Single pass through the string
/// Space: O(n) - Stack stores path components
///
/// We split the input string by `/`, iterate over the components, and manage a stack:
/// - `.` or empty string: Ignore (current directory or redundant slash).
/// - `..`: Pop from the stack (go up one level) if not empty.
/// - Any other name: Push onto the stack.
/// Finally, we join the stack with `/` and prepend a root `/`.
///
/// This approach is idiomatic Rust because it uses the iterator `split` combined with
/// pattern matching, avoiding C-style manual character scanning.
pub fn simplify_path(path: &str) -> String {
    // RUST INSIGHT: `Vec` is the idiomatic stack in Rust.
    // It has O(1) amortized push/pop and contiguous memory layout.
    // Here we store `&str` slices, which are references into the original `path` string.
    // This avoids allocating new `String` objects for each component.
    let mut stack: Vec<&str> = Vec::new();

    // GOTCHA: `split('/')` yields empty strings when multiple slashes are adjacent (e.g., "//").
    // It also yields an empty string at the start if the path starts with `/`.
    // We must handle these empty components correctly.
    for component in path.split('/') {
        match component {
            "" | "." => {
                // Ignore empty strings (redundant slashes) and current directory markers
            }
            ".." => {
                // Move up one directory level if possible
                stack.pop();
            }
            dir => {
                // Push valid directory name
                stack.push(dir);
            }
        }
    }

    // Join the components with `/` and ensure it starts with `/`
    // If the stack is empty, this results in just "/" which is correct for root.
    format!("/{}", stack.join("/"))
}

/// Alternative functional approach: `fold`
/// Time: O(n)
/// Space: O(n)
///
/// This implementation uses `fold` to build the stack in a single expression.
/// While more "functional", it can be slightly harder to read for those new to combinators
/// due to the `mut` accumulator.
#[allow(dead_code)]
pub fn simplify_path_functional(path: &str) -> String {
    let stack = path.split('/').fold(Vec::new(), |mut stack, component| {
        match component {
            "" | "." => {},
            ".." => { stack.pop(); },
            dir => stack.push(dir),
        }
        stack
    });

    format!("/{}", stack.join("/"))
}

/*
    Alternative approaches:
    1. Functional `fold`: As shown in `simplify_path_functional`, this is more concise but
       requires understanding `fold` with a mutable accumulator (or `reduce`).
    2. `std::path::PathBuf`: In real-world applications, always use the standard library's
       `Path` and `PathBuf` types. They handle platform-specific separators (`\` vs `/`).
       However, `PathBuf` does not automatically normalize `..` components (you would need
       to iterate `components()` and rebuild the path manually or use a crate like `path-clean`).
*/

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_happy_path() {
        assert_eq!(simplify_path("/home/"), "/home");
        assert_eq!(simplify_path("/home//foo/"), "/home/foo");
    }

    #[test]
    fn test_edge_cases() {
        assert_eq!(simplify_path("/../"), "/");
        assert_eq!(simplify_path("/home/.."), "/");
        assert_eq!(simplify_path("/"), "/");
    }

    #[test]
    fn test_complex_path() {
        assert_eq!(simplify_path("/a/./b/../../c/"), "/c");
        assert_eq!(simplify_path("/a//b////c/d//././/.."), "/a/b/c");
    }

    #[test]
    fn test_functional_approach() {
        assert_eq!(simplify_path_functional("/a/./b/../../c/"), "/c");
    }
}
