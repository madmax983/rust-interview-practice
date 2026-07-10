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

/// Optimal approach: Stack-based, process components sequentially
/// Time: O(n) - Single pass through the string
/// Space: O(n) - Stack stores path components
///
/// Technique: imperative stack. We split the input string by `/`, iterate over the components,
/// and manage a stack:
/// - `.` or empty string: Ignore (current directory or redundant slash).
/// - `..`: Pop from the stack (go up one level) if not empty.
/// - Any other name: Push onto the stack.
/// Finally, we join the stack with `/` and prepend a root `/`.
///
/// This approach is idiomatic Rust because it uses the iterator `split` combined with
/// pattern matching, avoiding C-style manual character scanning.
#[must_use] 
pub fn simplify_path_optimal(path: &str) -> String {
    // RUST INSIGHT: `Vec` is the idiomatic stack in Rust.
    // It has O(1) amortized push/pop and contiguous memory layout.
    // Here we store `&str` slices, which are references into the original `path` string.
    // This avoids allocating new `String` objects for each component.
    // ⚡ BOLT OPTIMIZATION: Pre-allocate capacity. The maximum number of components
    // is half the path length (e.g., "/a/b/c"). Clamping the denominator handles empty strings safely.
    let mut stack: Vec<&str> = Vec::with_capacity(path.len() / 2);

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

    // ⚡ BOLT OPTIMIZATION: Build the result string in-place with pre-allocated capacity.
    // This removes 2 heap allocations (`stack.join("/")` and `format!()`) and prevents
    // dynamic reallocations while building the final path.
    let mut result = String::with_capacity(path.len().max(1));
    if stack.is_empty() {
        result.push('/');
    } else {
        for dir in stack {
            result.push('/');
            result.push_str(dir);
        }
    }

    result
}

/// Brute force approach: Functional `fold`
/// Time: O(n)
/// Space: O(n)
///
/// Technique: functional `fold`. This implementation builds the stack in a single expression.
/// It is labelled `_brute_force` per the repo's naming convention as the alternative implementation;
/// its complexity is identical to the optimal loop. While more "functional", it can be slightly harder
/// to read for those new to combinators due to the `mut` accumulator.
#[must_use] 
pub fn simplify_path_brute_force(path: &str) -> String {
    let stack = path.split('/').fold(
        Vec::with_capacity(path.len() / 2),
        |mut stack, component| {
            match component {
                "" | "." => {}
                ".." => {
                    stack.pop();
                }
                dir => stack.push(dir),
            }
            stack
        },
    );

    let mut result = String::with_capacity(path.len().max(1));
    if stack.is_empty() {
        result.push('/');
    } else {
        for dir in stack {
            result.push('/');
            result.push_str(dir);
        }
    }

    result
}

/// Main entry point - uses optimal solution
#[must_use]
pub fn simplify_path(path: &str) -> String {
    simplify_path_optimal(path)
}

/*
    Alternative approaches:
    1. Functional `fold`: As shown in `simplify_path_brute_force`, this is more concise but
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
    fn test_brute_force_approach() {
        assert_eq!(simplify_path_brute_force("/a/./b/../../c/"), "/c");
    }

    #[test]
    fn test_all_approaches_agree() {
        let cases = [
            "/home/",
            "/home//foo/",
            "/../",
            "/home/..",
            "/",
            "/a/./b/../../c/",
            "/a//b////c/d//././/..",
        ];
        for path in cases {
            assert_eq!(simplify_path_brute_force(path), simplify_path_optimal(path));
            assert_eq!(simplify_path(path), simplify_path_optimal(path));
        }
    }

    #[test]
    fn test_capacity_optimization() {
        let path = "/a//b////c/d//././/..";
        let result = simplify_path(path);
        // The result should have pre-allocated capacity to avoid reallocations.
        // It should be at least as large as the original string to prevent reallocation.
        assert!(
            result.capacity() >= path.len(),
            "String capacity should be pre-allocated to at least path.len()"
        );
    }
}
