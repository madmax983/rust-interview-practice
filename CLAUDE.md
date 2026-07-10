# Rust Interview Practice Repository

## Purpose

This repository contains Rust implementations of coding-interview problems, core language
fundamentals, and larger systems-design exercises. It is designed for use with **gittype**
to build muscle memory for coding interviews: the goal is to internalize Rust syntax
patterns through repeated typing practice, eliminating stumbles during interviews where
autocomplete isn't available.

## Crate Overview

- **Crate name:** `rust-interview-practice` (v0.1.0)
- **Edition:** 2024
- **Location:** the crate lives at the **repository root** — `Cargo.toml` is at `./Cargo.toml`
  and sources are under `./src`. There is **no `leetcode/` subdirectory**.
- **Toolchain:** builds on **stable** Rust (developed against rustc 1.94.1). There is no
  `rust-toolchain.toml`. One nightly-only code path exists (portable SIMD) and is gated behind
  a cfg flag — see [Feature Flags](#feature-flags).
- **Always-on dependency:** `crc32fast` (the only non-optional dependency). Everything else is
  behind an optional feature flag.
- **Binaries:** `src/main.rs` (a placeholder `Hello, world!`) and `src/bin/bench_sha256.rs`
  (a benchmark helper for the cryptography module).

## Directory Structure

```
.
├── Cargo.toml
├── CLAUDE.md
└── src/
    ├── lib.rs                  # Module exports (pub mod for every category)
    ├── main.rs                 # Placeholder binary
    ├── bin/
    │   └── bench_sha256.rs     # sha256 benchmark helper
    ├── fundamentals/           # Core Rust patterns (not interview problems)
    ├── arrays/                 # Array problems
    ├── strings/                # String manipulation problems
    ├── linked_lists/           # Linked list problems
    ├── trees/                  # Binary tree / BST / trie problems
    ├── graphs/                 # Graph traversal & shortest paths
    ├── dynamic_programming/    # DP problems
    ├── backtracking/           # Backtracking problems
    ├── binary_search/          # Binary search problems
    ├── heaps/                  # Heap / priority-queue problems
    ├── stacks/                 # Stack / monotonic-stack problems
    ├── concurrency/            # Concurrency primitives & patterns
    ├── cryptography/           # Crypto primitives (sha256, jwt, rand)
    ├── networking/             # Networking building blocks (HTTP, DNS, RPC, ...)
    ├── serialization/          # Encoders/decoders (json, protobuf, msgpack, ...)
    ├── data_structures/        # Advanced data structures (tries, filters, caches, ...)
    ├── design_patterns/        # Rust-flavored design patterns
    └── systems/                # Larger systems-design exercises (LSM, raft, VM, ...)
```

## Category Organization

Interview problems are organized by **primary data structure** or **algorithmic technique**.
When a problem fits multiple categories, its **primary data structure** wins (some problems
therefore appear under more than one category, e.g. rotated-array search under both `arrays`
and `binary_search`). Beyond the classic interview categories, this repo also contains larger
implementation exercises (`data_structures`, `systems`, `networking`, `serialization`,
`cryptography`, `concurrency`, `design_patterns`) that are practiced the same way with gittype.

## Fundamentals Category

The `fundamentals/` directory contains **Rust idioms and patterns** (not interview problems)
that are essential for fluent coding — patterns you'll type repeatedly in any interview.

### Always-compiled fundamentals (stable)

- `asm.rs` — inline assembly (`asm!`), register constraints, x86_64/aarch64 intrinsics
- `borrowing.rs` — borrow checker, lifetimes, ownership, splitting/reborrowing
- `closures.rs` — `Fn`/`FnMut`/`FnOnce`, capturing, returning closures
- `collections.rs` — HashMap, HashSet, VecDeque, BinaryHeap
- `concurrency.rs` — Arc, Mutex, RwLock, channels, atomics, thread patterns
- `design_patterns.rs` — builder, newtype, type-state, RAII, visitor, strategy
- `error_handling.rs` — Option/Result combinators, `?` operator
- `error_types.rs` — thiserror/anyhow-style custom errors, recovery strategies
- `iterators.rs` — map, filter, fold, zip, windows, and other iterator patterns
- `macros.rs` — declarative macros, repetition, DSLs, debugging
- `numeric_ops.rs` — bit manipulation, safe arithmetic, number algorithms
- `pattern_matching.rs` — match, if let, destructuring, guards, slice patterns
- `performance.rs` — inlining, allocation, cache-friendly patterns, hot paths
- `simd.rs` — SSE/AVX intrinsics with `is_x86_feature_detected!`. **The portable-SIMD
  (`std::simd`) parts require nightly** and are gated on the `nightly_portable_simd` cfg
  (see [Feature Flags](#feature-flags)); the x86 intrinsic parts build on stable.
- `smart_pointers.rs` — Box, Rc, RefCell, Cow, ownership patterns
- `strings.rs` — String/&str operations, parsing, manipulation
- `testing.rs` — unit tests, fixtures, TDD workflow, doc tests
- `types_and_traits.rs` — generics, trait bounds, From/Into, trait objects, type state
- `unsafe_rust.rs` — raw pointers, FFI, unsafe traits, safety invariants

### Feature-gated fundamentals

- `async_and_parallel.rs` — tokio async/await + rayon data parallelism. Requires
  `--features async-parallel`.
- `serde_patterns.rs` — serde serialization/deserialization patterns. Requires
  `--features serde-patterns`.
- `cli_patterns.rs` — ratatui TUI development (components, layouts, events). Requires
  `--features cli-patterns`.

## Feature Flags

All feature flags are declared in `Cargo.toml`. Everything except `crc32fast` is optional.

| Feature | Enables (deps) | Purpose |
|---------|----------------|---------|
| `async-parallel` | tokio, tokio-stream, futures, rayon | `fundamentals::async_and_parallel` |
| `serde-patterns` | serde, serde_json, serde_yaml, toml, bincode | `fundamentals::serde_patterns` |
| `cli-patterns` | ratatui, crossterm, clap | `fundamentals::cli_patterns` |
| `testing-extras` | proptest | property-based testing helpers |
| `simd-patterns` | *(nothing)* | **no-op**, retained for backward compatibility only |

Enable features on stable, e.g.:

```bash
cargo test --features async-parallel
cargo test --features serde-patterns
cargo test --all-features          # builds cleanly on stable (see note below)
```

### Portable SIMD (nightly) — the `nightly_portable_simd` cfg

The portable-SIMD examples in `src/fundamentals/simd.rs` use `std::simd`
(`#![feature(portable_simd)]`) and therefore require **nightly**. They are **NOT** gated on a
Cargo feature — if they were, `cargo build --all-features` would try to compile nightly-only
code on stable and fail with `E0658`. Instead they are gated on the **cfg flag**
`nightly_portable_simd`, which `--all-features` cannot enable. This keeps stable
`--all-features` building cleanly. `[lints.rust]` in `Cargo.toml` declares
`unexpected_cfgs` with `check-cfg = ['cfg(nightly_portable_simd)']` so the custom cfg does not
trigger a warning. (Introduced in PR #442.)

The legacy `simd-patterns` Cargo feature is a **no-op** kept only for backward compatibility;
enabling it does nothing.

To build the nightly portable-SIMD path:

```bash
# 1. Add to the crate root: #![cfg_attr(nightly_portable_simd, feature(portable_simd))]
# 2. Compile on nightly:
RUSTFLAGS="--cfg nightly_portable_simd" cargo +nightly build
```

## Three-Implementation Pattern (Interview Problems)

Classic interview problems (arrays, strings, trees, DP, etc.) typically include **three
implementations** to demonstrate algorithmic progression:

### 1. Brute Force (`_brute_force` suffix)
- **Purpose:** Demonstrates understanding of the problem.
- Straightforward, naive approach; often O(n²) or O(n³); easy to explain.

### 2. Optimized (`_optimized` suffix)
- **Purpose:** Shows you can improve on brute force.
- Better time/space complexity; may use HashSet/HashMap/Vec; still readable.

### 3. Optimal (`_optimal` suffix)
- **Purpose:** Best possible solution.
- Optimal time and space; may use advanced techniques; production-ready.

### Main Entry Point
Each problem also exports a main function (e.g. `length_of_longest_substring`) that calls the
optimal solution.

> Note: the larger implementation exercises under `systems/`, `data_structures/`,
> `networking/`, `serialization/`, `cryptography/`, `concurrency/`, and `design_patterns/`
> are single cohesive implementations rather than three-tier brute/optimized/optimal problems.

## File Template (interview problems)

```rust
//! # [Problem Number]. [Problem Title]
//!
//! [Problem description]
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::category::problem::function_name;
//!
//! assert_eq!(function_name(input), expected);
//! ```
//!
//! ## Constraints
//!
//! - [Constraints]

/// Brute force approach: [Brief description]
/// Time: O(?) - [explanation]
/// Space: O(?) - [explanation]
#[must_use]
#[allow(clippy::needless_pass_by_value)] // interview signature uses owned types
#[allow(clippy::cast_possible_truncation)] // constraints guarantee it fits
#[allow(clippy::cast_possible_wrap)]
pub fn function_name_brute_force(input: Type) -> ReturnType {
    // Implementation
}

/// Optimized approach: [Brief description]
/// Time: O(?) - [explanation]
/// Space: O(?) - [explanation]
#[must_use]
pub fn function_name_optimized(input: Type) -> ReturnType {
    // Implementation
}

/// Optimal approach: [Brief description]
///
/// Time: O(?) - [explanation]
/// Space: O(?) - [explanation]
#[must_use]
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

    #[test]
    fn test_brute_force_example_1() { /* ... */ }

    #[test]
    fn test_optimized_example_1() { /* ... */ }

    #[test]
    fn test_optimal_example_1() { /* ... */ }

    // Cross-implementation verification
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
Determine which category (e.g. `arrays`, `strings`) best fits the problem.

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
- Add tests for all implementations.
- Run `cargo test` to verify RED state, implement to GREEN, then refactor.

### 5. Quality Checks
```bash
cargo fmt
cargo clippy -- -W clippy::pedantic -W clippy::nursery
cargo test
```

## Testing Standards

- **Test each implementation** separately (brute force / optimized / optimal where applicable).
- **Include the canonical examples** as test cases.
- **Add edge cases:** empty inputs, single elements, max constraints.
- **Cross-implementation tests:** verify all approaches return the same result.
- The library currently has a large passing unit-test suite (~1731 tests at last count).

## Clippy Allowances

Common allowances for interview-style problems:
- `#[allow(clippy::needless_pass_by_value)]` — interview signatures use owned types
- `#[allow(clippy::cast_possible_truncation)]` — problem constraints guarantee safe casts
- `#[allow(clippy::cast_possible_wrap)]` — problem constraints guarantee safe casts

The crate root (`src/lib.rs`) also sets `#![allow(clippy::module_name_repetitions)]` because
items deliberately repeat their module name (e.g. `two_sum::two_sum_brute_force`) as a gittype
typing-practice convention.

## Complexity Notation

Use Big-O notation in doc comments:
- **Time:** O(n), O(n²), O(n log n), O(2^n)
- **Space:** O(1), O(n), O(n²)

Explain what `n` represents and any other variables (m, k, etc.).

## gittype Integration

1. **Use gittype** to practice typing entire files.
2. **Focus on one category** at a time (e.g. all string problems, or all of `systems/`).
3. **Type all implementations** to internalize different patterns.
4. **Repeat regularly** — muscle memory requires repetition.

### Common Patterns to Practice
- Sliding window with HashSet/HashMap
- Two pointers (start/end, slow/fast)
- BFS/DFS with Vec/VecDeque as queue/stack
- Dynamic programming with 1D/2D Vec
- Pattern matching with `match` and `if let`
- Iterator chains (`.iter().filter().map().collect()`)
- Error handling with `Option` and `Result`

## Interview Strategy

1. **Start with brute force** — demonstrates you understand the problem.
2. **Identify bottlenecks** — explain what makes it slow.
3. **Optimize incrementally** — show the thought process.
4. **Arrive at optimal** — explain why it's optimal.
5. **Test edge cases** — show thoroughness.

## Components Present

The lists below reflect the modules actually declared in each category's `mod.rs`.

### Fundamentals
asm, borrowing, closures, collections, concurrency, design_patterns, error_handling,
error_types, iterators, macros, numeric_ops, pattern_matching, performance, simd,
smart_pointers, strings, testing, types_and_traits, unsafe_rust.
Feature-gated: async_and_parallel (`async-parallel`), serde_patterns (`serde-patterns`),
cli_patterns (`cli-patterns`).

### Arrays
best_time_to_buy_and_sell_stock, container_with_most_water, contains_duplicate,
find_minimum_in_rotated_sorted_array, first_missing_positive, gas_station, insert_interval,
jump_game, longest_consecutive_sequence, majority_element, maximum_subarray, merge_intervals,
move_zeroes, product_except_self, rotate_array, rotate_image, search_a_2d_matrix,
search_in_rotated_sorted_array, sliding_window_maximum, sort_colors, spiral_matrix,
subarray_sum_equals_k, three_sum, top_k_frequent_elements, trapping_rain_water, two_sum,
two_sum_ii, valid_sudoku.

### Strings
aho_corasick, basic_calculator_ii, find_all_anagrams_in_a_string, group_anagrams,
longest_common_prefix, longest_palindromic_substring, longest_repeating_character_replacement,
longest_substring_without_repeating, mini_parser, minimum_window_substring, parser_combinator,
regex, reverse_words_in_a_string, roman_to_integer, semver, string_to_integer_atoi,
text_justification, valid_anagram, valid_number, valid_palindrome.

### Linked Lists
add_two_numbers, linked_list_cycle, merge_k_sorted_lists, merge_two_sorted_lists,
middle_of_the_linked_list, palindrome_linked_list, remove_duplicates_from_sorted_list,
remove_nth_node_from_end_of_list, reverse_linked_list, swap_nodes_in_pairs.

### Trees
balanced_binary_tree, binary_tree_maximum_path_sum, bst_iterator,
construct_binary_tree_from_preorder_and_inorder_traversal,
design_add_and_search_words_data_structure, diameter_of_binary_tree, implement_trie,
invert_binary_tree, kth_smallest_element_in_a_bst, level_order_traversal,
lowest_common_ancestor, max_depth, red_black_tree, same_tree,
serialize_and_deserialize_binary_tree, symmetric_tree, validate_binary_search_tree.

### Graphs
clone_graph, course_schedule, dijkstra, network_delay_time, number_of_islands,
pacific_atlantic_water_flow, reconstruct_itinerary, redundant_connection, rotting_oranges,
word_ladder.

### Dynamic Programming
climbing_stairs, coin_change, edit_distance, house_robber, longest_common_subsequence,
longest_increasing_subsequence, maximum_product_subarray, maximum_subarray,
partition_equal_subset_sum, unique_paths, word_break.

### Backtracking
combination_sum, combinations, generate_parentheses, letter_combinations, n_queens,
palindrome_partitioning, permutations, subsets, sudoku_solver, word_search.

### Binary Search
binary_search, find_minimum_in_rotated_sorted_array, koko_eating_bananas, search_a_2d_matrix,
search_in_rotated_sorted_array, time_based_key_value_store.

### Heaps
find_median_from_data_stream, k_closest_points_to_origin, kth_largest_element_in_an_array.

### Stacks
asteroid_collision, daily_temperatures, decode_string, evaluate_reverse_polish_notation,
flatten_nested_list_iterator, largest_rectangle_in_histogram, min_stack, simplify_path,
valid_parentheses.

### Concurrency
actor_system, arc, async_executor, async_mutex, barrier, channel, dining_philosophers,
event_loop, lock_free_queue, mutex, once_cell, parking_lot, promise, read_write_lock,
semaphore, thread_local, thread_pool, work_stealing_pool.

### Cryptography
jwt, rand, sha256.

### Networking
dns_resolver, http_client, http_router, http_server, load_balancer, middleware, rpc,
tcp_connection_pool, url, websocket.

### Serialization
base64, bencode, bincode, csv, ini, json, msgpack, protobuf, resp, serde_framework, toml,
varint.

### Data Structures
b_tree, binary_heap, bit_vec, bloom_filter, bytes, concurrent_hash_map, count_min_sketch,
crdt, cuckoo_filter, graph, hash_map, hashed_wheel_timer, hyperloglog, index_map,
interval_tree, lfu_cache, lru_cache, merkle_tree, quadtree, radix_trie, ring_buffer, rope,
segment_tree, skip_list, slotmap, smallvec, sparse_set, spsc_ring_buffer, type_map, union_find.

### Design Patterns
active_record_vs_repository, actor, adapter, borrowed_owned_duality, bridge, builder,
callbacks, chain_of_responsibility, command, composite, concurrency_patterns, conversions,
decorator, dependency_injection, drop_bomb, error_handling, extension_traits, facade, factory,
flyweight, handle_pattern, interior_mutability, interpreter, iterators, macros, marker_traits,
mediator, memento, middleware, newtype, null_object, object_pool, observer, oop_correction,
oop_correction_factor, oop_mindset_cures, parse_dont_validate, plugin, polymorphism, prototype,
proxy, raii_guards, registry, sealed_traits, self_referential, session_types, singleton, state,
strategy, template_method, typestate, visitor.

### Systems
arc_cache, bitcask, bloom_filter, buddy_allocator, bump_allocator, circuit_breaker, cli_parser,
concurrent_cache, connection_pool, consistent_hashing, cron, datetime, deflate,
dependency_injection, design_twitter, ecs, error_framework, garbage_collector, inverted_index,
job_queue, lfu_cache, log_structured_storage, lru_cache, lsm_tree, metrics_registry, mvcc,
pub_sub, raft, rate_limiter, reactive_signals, slab_allocator, snowflake, sql_engine,
task_scheduler, template_engine, tracing, ttl_cache, uuid, vdom, vector_clock, virtual_machine,
w_tiny_lfu_cache, wal, write_strategies.

## Future Enhancements

- [ ] Add benchmarks with Criterion for performance comparison
- [ ] Create scripts to generate template files
- [ ] Add problem difficulty tags (Easy/Medium/Hard)
- [ ] Track gittype practice sessions
- [ ] Generate statistics (problems solved, accuracy, speed)
