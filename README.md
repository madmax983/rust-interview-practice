# Rust Interview Practice

A collection of LeetCode solutions and Rust fundamentals designed for **gittype muscle memory training**. The goal is to internalize Rust syntax patterns through repeated typing practice, eliminating stumbles during coding interviews where autocomplete isn't available.

[![Rust](https://img.shields.io/badge/rust-2024-orange.svg)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

## Why This Repository?

Traditional LeetCode practice focuses on *understanding* algorithms. This repository adds a second dimension: **typing fluency**. By using [gittype](https://github.com/unhappychoice/gittype) to repeatedly type out solutions, you build muscle memory for:

- Iterator chains (`.iter().filter().map().collect()`)
- Pattern matching (`match`, `if let`, destructuring)
- Collection APIs (HashMap, HashSet, VecDeque)
- Error handling (`Option`, `Result`, `?` operator)
- Borrowing patterns (avoiding common borrow checker issues)

**Result:** In interviews, you focus on *problem-solving* instead of fighting syntax.

## Repository Structure

```
rust-interview-practice/
├── src/
│   ├── fundamentals/       # Core Rust patterns (not LeetCode problems)
│   │   ├── borrowing.rs        # Borrow checker, lifetimes, ownership (19 patterns)
│   │   ├── iterators.rs        # 14 iterator patterns (map, filter, fold, etc.)
│   │   ├── collections.rs      # HashMap, HashSet, VecDeque, BinaryHeap
│   │   ├── error_handling.rs   # Option/Result combinators, ? operator
│   │   ├── pattern_matching.rs # match, if let, destructuring, guards
│   │   └── strings.rs          # String/&str operations, parsing
│   ├── arrays/             # Array-based problems
│   │   ├── two_sum.rs          # #1 - Hash map lookup optimization
│   │   ├── three_sum.rs        # #15 - Two pointers, deduplication
│   │   └── trapping_rain_water.rs # #42 - Two pointers, DP alternatives
│   ├── strings/            # String manipulation problems
│   │   └── longest_substring_without_repeating.rs # #3
│   ├── linked_lists/       # Linked list problems
│   ├── trees/              # Tree problems (BST, binary tree, etc.)
│   ├── graphs/             # Graph algorithms (BFS, DFS, etc.)
│   ├── dynamic_programming/# DP problems
│   └── ...
├── Cargo.toml
├── CLAUDE.md              # Detailed architecture & patterns
└── README.md
```

## The Three-Implementation Pattern

**Every LeetCode problem includes three implementations** to demonstrate algorithmic progression:

### 1. Brute Force (`_brute_force` suffix)
- **Purpose:** Demonstrates understanding of the problem
- **Characteristics:** Straightforward, often O(n²) or O(n³)
- **Interview value:** Shows you can solve it, even if not optimally

### 2. Optimized (`_optimized` suffix)
- **Purpose:** Shows you can improve on brute force
- **Characteristics:** Better complexity, uses basic data structures
- **Interview value:** Demonstrates optimization thinking

### 3. Optimal (`_optimal` suffix)
- **Purpose:** Best possible solution
- **Characteristics:** Optimal time/space complexity, production-ready
- **Interview value:** Shows mastery and deep understanding

### Example: Two Sum

```rust
// Brute force: O(n²) nested loops
pub fn two_sum_brute_force(nums: Vec<i32>, target: i32) -> Vec<i32> { /* ... */ }

// Optimized: O(n) with HashMap
pub fn two_sum_optimized(nums: Vec<i32>, target: i32) -> Vec<i32> { /* ... */ }

// Optimal: Single-pass HashMap (same as optimized for this problem)
pub fn two_sum_optimal(nums: Vec<i32>, target: i32) -> Vec<i32> { /* ... */ }
```

## Getting Started

### Prerequisites

- Rust 2024 edition (install via [rustup](https://rustup.rs/))
- [gittype](https://github.com/unhappychoice/gittype) (for muscle memory training)

### Installation

```bash
git clone https://github.com/madmax983/rust-interview-practice.git
cd rust-interview-practice
cargo test
```

### Usage

#### Run Tests
```bash
cargo test                    # Run all tests
cargo test arrays::           # Test only array problems
cargo test two_sum            # Test specific problem
```

#### Format & Lint
```bash
cargo fmt
cargo clippy -- -W clippy::pedantic -W clippy::nursery
```

#### gittype Practice

1. **Choose a category** (e.g., `src/arrays/two_sum.rs`)
2. **Delete the file** or create a practice branch
3. **Use gittype** to retype the entire file from commit history
4. **Run tests** to verify correctness
5. **Repeat** until typing becomes automatic

```bash
# Example gittype session
gittype --file src/arrays/two_sum.rs
```

## Fundamentals Category

The `fundamentals/` directory contains **Rust idioms** essential for fluent coding:

| File | Focus | Key Patterns |
|------|-------|--------------|
| **borrowing.rs** | Borrow checker | Immutable/mutable borrows, lifetimes, NLL, ownership |
| **iterators.rs** | Iterator patterns | map, filter, fold, enumerate, zip, collect |
| **collections.rs** | Standard collections | HashMap (entry API), HashSet, VecDeque, BinaryHeap |
| **error_handling.rs** | Option/Result | unwrap_or, map, and_then, ?, pattern matching |
| **pattern_matching.rs** | Match expressions | Destructuring, guards, @ bindings, or patterns |
| **strings.rs** | String operations | String vs &str, char iteration, parsing, splitting |

**Practice these alongside LeetCode problems** to build comprehensive fluency.

## LeetCode Problems Implemented

### Arrays
- ✅ **#1** - Two Sum (Easy) - Hash map lookup, O(n²) → O(n) optimization
- ✅ **#15** - Three Sum (Medium) - Two pointers, sorting, deduplication
- ✅ **#42** - Trapping Rain Water (Hard) - Two pointers, DP alternatives

### Strings
- ✅ **#3** - Longest Substring Without Repeating Characters (Medium) - Sliding window

## Interview Strategy

When using these solutions for interview practice:

1. ✅ **Start with brute force** - Demonstrates you understand the problem
2. ✅ **Identify bottlenecks** - Explain what makes it slow
3. ✅ **Optimize incrementally** - Show the thought process
4. ✅ **Arrive at optimal** - Explain why it's optimal
5. ✅ **Test edge cases** - Show thoroughness

## Common Patterns to Practice

- **Sliding window** with HashSet/HashMap
- **Two pointers** (start/end, slow/fast)
- **BFS/DFS** with Vec as queue/stack
- **Dynamic programming** with 1D/2D Vec
- **Pattern matching** with `match` and `if let`
- **Iterator chains** (`.iter()`, `.filter()`, `.map()`, `.collect()`)
- **Error handling** with `Option` and `Result`

## Contributing

This is a personal practice repository, but feel free to:
- Open issues for bugs or incorrect solutions
- Suggest additional problems or patterns
- Share your own gittype practice results

## Coding Standards

- **Test-driven development** (TDD): Write tests first
- **Test coverage:** 85-90% minimum
- **Clippy compliance:** Pedantic + nursery lints
- **Documentation:** Doc comments on all public APIs with time/space complexity

## License

MIT License - See LICENSE file for details

## Acknowledgments

- [LeetCode](https://leetcode.com/) for problem sets
- [gittype](https://github.com/unhappychoice/gittype) for muscle memory training methodology
- Rust community for excellent tooling and documentation

---

**Practice deliberately. Type intentionally. Interview confidently.**
