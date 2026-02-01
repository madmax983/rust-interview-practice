# Rust Mastery through Gittype Practice

A comprehensive collection of Rust patterns and algorithms designed for **gittype muscle memory training**. Build deep fluency in Rust syntax, idioms, and ecosystem patterns through deliberate, repeated typing practice.

[![Rust](https://img.shields.io/badge/rust-2024-orange.svg)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

## Why This Repository?

Traditional coding practice focuses on *understanding* concepts. This repository adds a second dimension: **typing fluency**. By using [gittype](https://github.com/unhappychoice/gittype) to repeatedly type out patterns, you build muscle memory for Rust's syntax and idioms.

**What you'll master:**
- Iterator chains (`.iter().filter().map().collect()`)
- Smart pointers (Box, Rc, RefCell, Arc)
- Concurrent patterns (Mutex, RwLock, channels, atomics)
- Bit manipulation and numeric algorithms
- Closure traits and capturing semantics
- Pattern matching and destructuring
- Error handling with Option/Result
- Async/await patterns (tokio)
- Data parallelism (rayon)

**Result:** Write idiomatic Rust fluently without fighting the compiler. Perfect for coding interviews, open source contributions, or production work.

## Repository Structure

```
rust-interview-practice/
├── src/
│   ├── fundamentals/           # Core Rust patterns for mastery
│   │   ├── borrowing.rs            # Borrow checker, lifetimes, ownership (19 patterns)
│   │   ├── closures.rs             # Fn/FnMut/FnOnce, capturing, returning closures
│   │   ├── collections.rs          # HashMap, HashSet, VecDeque, BinaryHeap
│   │   ├── concurrency.rs          # Arc, Mutex, RwLock, channels, atomics
│   │   ├── error_handling.rs       # Option/Result combinators, ? operator
│   │   ├── iterators.rs            # map, filter, fold, enumerate, zip, etc.
│   │   ├── numeric_ops.rs          # Bit manipulation, safe arithmetic, algorithms
│   │   ├── pattern_matching.rs     # match, if let, destructuring, guards
│   │   ├── smart_pointers.rs       # Box, Rc, RefCell, Cow, ownership patterns
│   │   ├── strings.rs              # String/&str operations, parsing
│   │   ├── types_and_traits.rs     # Generics, trait bounds, From/Into, type state
│   │   └── async_and_parallel.rs   # tokio async/await, rayon (coming soon)
│   ├── arrays/                 # Array-based algorithms
│   │   ├── two_sum.rs              # #1 - Hash map lookup optimization
│   │   ├── three_sum.rs            # #15 - Two pointers, deduplication
│   │   └── trapping_rain_water.rs  # #42 - Two pointers, DP alternatives
│   ├── strings/                # String manipulation algorithms
│   │   └── longest_substring_without_repeating.rs # #3 - Sliding window
│   └── ...                     # More categories coming
├── docs/
│   └── plans/                  # Design documents
├── Cargo.toml
├── CLAUDE.md                   # Detailed architecture & patterns
└── README.md
```

## Fundamentals: The Core of Mastery

The `fundamentals/` directory contains **11 comprehensive modules** covering essential Rust patterns:

| Module | Focus | Key Patterns |
|--------|-------|--------------|
| **borrowing.rs** | Borrow checker | Immutable/mutable borrows, lifetimes, NLL, splitting borrows |
| **closures.rs** | Closure patterns | Fn/FnMut/FnOnce traits, capturing, move semantics, returning closures |
| **collections.rs** | Standard collections | HashMap entry API, HashSet operations, VecDeque, BinaryHeap |
| **concurrency.rs** | Thread-safe patterns | Arc, Mutex, RwLock, channels, atomics, worker pools, map-reduce |
| **error_handling.rs** | Option/Result | unwrap_or, map, and_then, ?, pattern matching, collecting Results |
| **iterators.rs** | Iterator patterns | map, filter, fold, enumerate, zip, windows, partition |
| **numeric_ops.rs** | Numeric operations | Bit manipulation, safe arithmetic, GCD/LCM, fast exponentiation |
| **pattern_matching.rs** | Match expressions | Destructuring, guards, @ bindings, or patterns, slice patterns |
| **smart_pointers.rs** | Ownership patterns | Box, Rc, RefCell, Cow, Rc<RefCell<T>> for graphs |
| **strings.rs** | String operations | String vs &str, char iteration, parsing, splitting, building |
| **types_and_traits.rs** | Generics & traits | Type parameters, bounds, From/Into, impl Trait, type state |

**Coming soon:** `async_and_parallel.rs` - tokio async/await patterns and rayon data parallelism

These modules are **not LeetCode problems** - they're curated Rust idioms you'll type thousands of times in production code.

## The Three-Implementation Pattern (Algorithm Problems)

**LeetCode problems include three implementations** to demonstrate algorithmic progression:

### 1. Brute Force (`_brute_force` suffix)
- Straightforward, naive approach (often O(n²) or O(n³))
- Demonstrates problem understanding
- Good starting point in interviews

### 2. Optimized (`_optimized` suffix)
- Better time/space complexity
- Uses basic data structures (HashMap, HashSet)
- Shows optimization thinking

### 3. Optimal (`_optimal` suffix)
- Best possible solution
- Optimal time and space complexity
- Production-ready code

### Example: Two Sum

```rust
// Brute force: O(n²) nested loops
pub fn two_sum_brute_force(nums: Vec<i32>, target: i32) -> Vec<i32> { /* ... */ }

// Optimized: O(n) with HashMap
pub fn two_sum_optimized(nums: Vec<i32>, target: i32) -> Vec<i32> { /* ... */ }

// Optimal: Single-pass HashMap
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

#### gittype Practice Workflow

1. **Choose a module** (e.g., `src/fundamentals/closures.rs` or `src/arrays/two_sum.rs`)
2. **Delete the file** or create a practice branch
3. **Use gittype** to retype the entire file from commit history
4. **Run tests** to verify correctness
5. **Repeat** until typing becomes automatic

```bash
# Example gittype session
gittype --file src/fundamentals/closures.rs
```

**Pro tip:** Start with fundamentals (smaller, focused patterns) before tackling full algorithm implementations.

## Algorithms Implemented

### Arrays
- ✅ **#1** - Two Sum (Easy) - Hash map lookup, O(n²) → O(n) optimization
- ✅ **#15** - Three Sum (Medium) - Two pointers, sorting, deduplication
- ✅ **#42** - Trapping Rain Water (Hard) - Two pointers, DP alternatives

### Strings
- ✅ **#3** - Longest Substring Without Repeating Characters (Medium) - Sliding window

### Coming Soon
- Linked Lists
- Binary Trees & BSTs
- Graphs (BFS, DFS, topological sort)
- Dynamic Programming
- Backtracking

## Common Patterns Covered

- **Sliding window** with HashSet/HashMap
- **Two pointers** (start/end, slow/fast)
- **BFS/DFS** with Vec as queue/stack
- **Dynamic programming** with 1D/2D Vec
- **Bit manipulation** (masks, XOR tricks, power of 2 checks)
- **Pattern matching** with `match` and `if let`
- **Iterator chains** with closures
- **Smart pointers** for recursive data structures
- **Concurrency** (Arc<Mutex<T>>, channels, atomics)
- **Error handling** with Option/Result combinators

## Use Cases

### 🎯 Coding Interviews
Master Rust syntax so you focus on problem-solving, not fighting the compiler. The three-implementation pattern teaches you to recognize optimization opportunities.

### 🚀 Production Rust
Build muscle memory for patterns you'll use daily: iterator chains, error handling, smart pointers, async/await, concurrency primitives.

### 🌟 Open Source Contributions
Navigate unfamiliar codebases with confidence when you can read and write idiomatic Rust fluently.

### 📚 Learning Rust Deeply
Go beyond "understanding" to true fluency through deliberate practice with gittype.

## Coding Standards

- **Test-driven development** (TDD): Write tests first
- **Test coverage:** 85-90% minimum (for algorithm problems)
- **Clippy compliance:** Pedantic + nursery lints
- **Documentation:** Doc comments with time/space complexity analysis
- **Rust 2024 edition:** Latest language features

## Contributing

This is a personal mastery repository, but feel free to:
- Open issues for bugs or incorrect solutions
- Suggest additional patterns or algorithms
- Share your own gittype practice results
- Request specific Rust patterns you want to master

## Philosophy

> "Knowledge is not skill. Knowledge plus ten thousand times is skill."
> — Shinichi Suzuki

Reading about Rust patterns creates knowledge. Typing them 10,000 times creates mastery.

## License

MIT License - See LICENSE file for details

## Acknowledgments

- [LeetCode](https://leetcode.com/) for algorithm problem sets
- [gittype](https://github.com/unhappychoice/gittype) for muscle memory training methodology
- Rust community for excellent tooling and documentation
- [The Rust Book](https://doc.rust-lang.org/book/) and [Rust by Example](https://doc.rust-lang.org/rust-by-example/)

---

**Practice deliberately. Type intentionally. Master Rust fluently.**
