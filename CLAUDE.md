# LeetCode Practice Repository

## Purpose

This repository contains LeetCode problem solutions in Rust, designed for use with **gittype** to build muscle memory for coding interviews. The goal is to internalize Rust syntax patterns through repeated typing practice, eliminating stumbles during interviews where autocomplete isn't available.

## Architecture

### Directory Structure

```
leetcode/
├── src/
│   ├── lib.rs              # Module exports
│   ├── fundamentals/       # Core Rust patterns (not LeetCode problems)
│   │   ├── mod.rs
│   │   ├── iterators.rs
│   │   ├── collections.rs
│   │   ├── error_handling.rs
│   │   ├── pattern_matching.rs
│   │   └── strings.rs
│   ├── arrays/             # Array-based problems
│   │   ├── mod.rs
│   │   └── *.rs
│   ├── strings/            # String manipulation problems
│   │   ├── mod.rs
│   │   └── longest_substring_without_repeating.rs
│   ├── linked_lists/       # Linked list problems
│   ├── trees/              # Tree problems (BST, binary tree, etc.)
│   ├── graphs/             # Graph algorithms (BFS, DFS, etc.)
│   ├── dynamic_programming/# DP problems
│   ├── sliding_window/     # Sliding window technique
│   ├── two_pointers/       # Two pointer technique
│   └── backtracking/       # Backtracking problems
├── Cargo.toml
└── CLAUDE.md
```

### Problem Organization

Problems are organized by **primary data structure** or **algorithmic technique**:
- **strings/** - String manipulation, substring problems
- **arrays/** - Array operations, searching, sorting
- **linked_lists/** - Singly/doubly linked lists
- **trees/** - Binary trees, BST, tries
- **graphs/** - Graph traversal, shortest paths
- **dynamic_programming/** - DP problems
- **sliding_window/** - Sliding window technique
- **two_pointers/** - Two pointer patterns

When a problem fits multiple categories, use the **primary data structure** as the category.

## Fundamentals Category

The `fundamentals/` directory contains **Rust idioms and patterns** that aren't LeetCode problems but are essential for fluent coding. These are patterns you'll type repeatedly in any interview:

### `borrowing.rs` - Borrow Checker Patterns
- **Borrowing rules:** Immutable vs mutable borrows, exclusive access
- **Lifetimes:** Basic `'a`, multiple lifetimes, lifetime elision
- **Common patterns:** Splitting borrows, reborrowing, NLL (Non-Lexical Lifetimes)
- **Ownership:** Move vs copy, taking ownership vs borrowing
- **Dereferencing:** `*` operator, `as_ref()`, entry API
- **Avoiding issues:** Clone to sidestep borrows, return owned data

### `iterators.rs` - Iterator Patterns
- `map`, `filter`, `filter_map`, `collect`
- `enumerate`, `zip`, `chain`
- `fold`, `sum`, `max`, `min`
- `any`, `all`, `take`, `skip`
- `windows`, `partition`

### `collections.rs` - Collection Operations
- **HashMap:** frequency maps, `entry().or_insert()`, `get_or_default()`
- **HashSet:** membership, duplicates, set operations
- **VecDeque:** queue (FIFO), stack (LIFO), sliding window
- **BinaryHeap:** priority queue operations

### `error_handling.rs` - Option/Result Patterns
- **Option:** `unwrap_or`, `unwrap_or_else`, `map`, `and_then`
- **Result:** `map`, `map_err`, `and_then`, `?` operator
- Pattern matching: `match`, `if let`, `while let`
- `transpose`, collecting Results

### `pattern_matching.rs` - Match Patterns
- Basic match, ranges, guards
- Tuple destructuring
- Enum destructuring
- `if let`, `while let`
- Slice patterns
- `@` bindings
- Or patterns (`|`)

### `strings.rs` - String Operations
- String vs &str conversions
- Char/byte iteration
- String building (`push_str`, `format!`, `join`)
- Splitting, trimming, case conversion
- Substring operations
- Char classification
- Parsing

**Purpose:** Use gittype to practice these fundamentals alongside LeetCode problems. When you can type `.iter().filter().map().collect()` without thinking, you'll write algorithms much faster.

## Three-Implementation Pattern (LeetCode Problems)

**Every problem includes three implementations** to demonstrate algorithmic progression:

### 1. Brute Force (`_brute_force` suffix)
- **Purpose:** Demonstrates understanding of the problem
- **Characteristics:**
  - Straightforward, naive approach
  - Often O(n²) or O(n³) time complexity
  - Easy to understand and explain
- **Interview value:** Shows you can solve the problem, even if not optimally

### 2. Optimized (`_optimized` suffix)
- **Purpose:** Shows you can improve on brute force
- **Characteristics:**
  - Better time/space complexity
  - May use basic data structures (HashSet, HashMap, Vec)
  - Still readable and explainable
- **Interview value:** Demonstrates optimization thinking

### 3. Optimal (`_optimal` suffix)
- **Purpose:** Best possible solution
- **Characteristics:**
  - Optimal time and space complexity
  - May use advanced techniques or clever insights
  - Production-ready code
- **Interview value:** Shows mastery and deep understanding

### Main Entry Point
Each problem also exports a main function (e.g., `length_of_longest_substring`) that calls the optimal solution.

## File Template

```rust
//! # [Problem Number]. [Problem Title]
//!
//! [Problem description from LeetCode]
//!
//! ## Examples
//!
//! ```
//! use leetcode::category::problem::function_name;
//!
//! assert_eq!(function_name(input), expected);
//! ```
//!
//! ## Constraints
//!
//! - [Constraints from LeetCode]

/// Brute force approach: [Brief description]
/// Time: O(?) - [explanation]
/// Space: O(?) - [explanation]
#[must_use]
#[allow(clippy::needless_pass_by_value)] // LeetCode signature
#[allow(clippy::cast_possible_truncation)] // LeetCode constraints guarantee it fits
#[allow(clippy::cast_possible_wrap)]
pub fn function_name_brute_force(input: Type) -> ReturnType {
    // Implementation
}

/// Optimized approach: [Brief description]
/// Time: O(?) - [explanation]
/// Space: O(?) - [explanation]
#[must_use]
#[allow(clippy::needless_pass_by_value)] // LeetCode signature
#[allow(clippy::cast_possible_truncation)] // LeetCode constraints guarantee it fits
#[allow(clippy::cast_possible_wrap)]
pub fn function_name_optimized(input: Type) -> ReturnType {
    // Implementation
}

/// Optimal approach: [Brief description]
///
/// Time: O(?) - [explanation]
/// Space: O(?) - [explanation]
#[must_use]
#[allow(clippy::needless_pass_by_value)] // LeetCode signature
#[allow(clippy::cast_possible_truncation)] // LeetCode constraints guarantee it fits
#[allow(clippy::cast_possible_wrap)]
pub fn function_name_optimal(input: Type) -> ReturnType {
    // Implementation
}

/// Main entry point - uses optimal solution
#[must_use]
pub fn function_name(input: Type) -> ReturnType {
    function_name_optimal(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Tests for brute force
    #[test]
    fn test_brute_force_example_1() { /* ... */ }

    // Tests for optimized
    #[test]
    fn test_optimized_example_1() { /* ... */ }

    // Tests for optimal
    #[test]
    fn test_optimal_example_1() { /* ... */ }

    // Cross-implementation verification tests
    #[test]
    fn test_all_approaches_edge_case() {
        let input = /* ... */;
        assert_eq!(function_name_brute_force(input.clone()), expected);
        assert_eq!(function_name_optimized(input.clone()), expected);
        assert_eq!(function_name_optimal(input.clone()), expected);
    }
}
```

## Adding New Problems

### 1. Choose Category
Determine which category (e.g., `arrays`, `strings`) best fits the problem.

### 2. Create Module
If the category doesn't exist:
```bash
mkdir src/category_name
```

Create or update `src/category_name/mod.rs`:
```rust
pub mod problem_name;
```

Update `src/lib.rs`:
```rust
pub mod category_name;
```

### 3. Implement Problem
Create `src/category_name/problem_name.rs` following the template above.

### 4. Write Tests First (TDD)
- Add tests for all three implementations
- Run `cargo test` to verify RED state
- Implement solutions to achieve GREEN state
- Refactor while keeping tests green

### 5. Quality Checks
```bash
cargo fmt
cargo clippy -- -W clippy::pedantic -W clippy::nursery
cargo test
```

## Testing Standards

- **Test all three implementations** separately
- **Include LeetCode examples** as test cases
- **Add edge cases:** empty inputs, single elements, max constraints
- **Cross-implementation tests:** Verify all three approaches return same results
- **Test coverage target:** 85-90% minimum

## Clippy Allowances

Common allowances for LeetCode problems:
- `#[allow(clippy::needless_pass_by_value)]` - LeetCode signatures use owned types
- `#[allow(clippy::cast_possible_truncation)]` - Problem constraints guarantee safe casts
- `#[allow(clippy::cast_possible_wrap)]` - Problem constraints guarantee safe casts

## gittype Integration

Once you have a collection of problems:

1. **Use gittype** to practice typing the entire file
2. **Focus on one category** at a time (e.g., all string problems)
3. **Type all three implementations** to internalize different patterns
4. **Repeat regularly** - muscle memory requires repetition

### Common Patterns to Practice
- Sliding window with HashSet/HashMap
- Two pointers (start/end, slow/fast)
- BFS/DFS with Vec as queue/stack
- Dynamic programming with 1D/2D Vec
- Pattern matching with `match` and `if let`
- Iterator chains (`.iter()`, `.filter()`, `.map()`, `.collect()`)
- Error handling with `Option` and `Result`

## Coding Patterns & Idioms

### Collections
```rust
use std::collections::{HashSet, HashMap, VecDeque};

let mut set = HashSet::new();
let mut map = HashMap::new();
let mut queue = VecDeque::new();
```

### String Handling
```rust
let chars: Vec<char> = s.chars().collect();
let bytes = s.as_bytes();
```

### Iterators
```rust
for (i, &item) in items.iter().enumerate() { }
let result: Vec<_> = items.iter().filter(|&&x| x > 0).collect();
```

### Pattern Matching
```rust
match value {
    Some(x) => x,
    None => return 0,
}

if let Some(x) = optional { }
```

## Interview Strategy

When using these solutions for interview practice:

1. **Start with brute force** - Demonstrates you understand the problem
2. **Identify bottlenecks** - Explain what makes it slow
3. **Optimize incrementally** - Show the thought process
4. **Arrive at optimal** - Explain why it's optimal
5. **Test edge cases** - Show thoroughness

## Complexity Notation

Use Big-O notation in doc comments:
- **Time:** O(n), O(n²), O(n log n), O(2^n)
- **Space:** O(1), O(n), O(n²)

Explain what n represents and any other variables (m, k, etc.).

## Components Completed

### Fundamentals
- [x] Borrowing - Borrow checker patterns, lifetimes, ownership (19 patterns)
- [x] Iterators - 14 common patterns (map, filter, fold, etc.)
- [x] Collections - HashMap, HashSet, VecDeque, BinaryHeap
- [x] Error Handling - Option/Result combinators, ? operator
- [x] Pattern Matching - match, if let, destructuring, guards
- [x] Strings - String/&str operations, parsing, manipulation

### LeetCode Problems

#### Strings
- [x] #3 - Longest Substring Without Repeating Characters (3 implementations)

#### Arrays
- [ ] TBD

#### Linked Lists
- [ ] TBD

#### Trees
- [ ] TBD

#### Graphs
- [ ] TBD

#### Dynamic Programming
- [ ] TBD

## Future Enhancements

- [ ] Add benchmarks with Criterion for performance comparison
- [ ] Create scripts to generate template files
- [ ] Add problem difficulty tags (Easy/Medium/Hard)
- [ ] Track gittype practice sessions
- [ ] Generate statistics (problems solved, accuracy, speed)
