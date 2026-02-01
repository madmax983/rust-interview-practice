# Additional Fundamentals Modules Design

**Date:** 2026-02-01
**Status:** Approved

## Overview

Add four new fundamental modules to complement the existing seven (borrowing, iterators, collections, error_handling, pattern_matching, strings, types_and_traits). These modules cover essential Rust patterns for coding interviews that require muscle memory through gittype practice.

## Modules to Add

### 1. closures.rs - Closure Patterns

**Purpose:** Master closure syntax and function trait bounds that appear in iterator chains and callback patterns.

**Content:**
- **Function trait bounds**
  - `impl Fn` - Can call multiple times, borrows immutably
  - `impl FnMut` - Can call multiple times, borrows mutably
  - `impl FnOnce` - Consumes captured values, call once

- **Capturing patterns**
  - Immutable capture (closure reads variable)
  - Mutable capture (closure modifies variable)
  - Move capture (`move` keyword - takes ownership)
  - Multiple captures from environment

- **Common iterator patterns**
  - `map` with closures
  - `filter` with closures
  - `fold` with closures
  - Chaining: `.filter().map().collect()`

- **Returning closures**
  - `impl Fn() -> T` return type
  - `Box<dyn Fn() -> T>` for storing different closure types

- **Closure type inference**
  - Type annotation when needed
  - `|x: i32| -> i32 { x + 1 }` vs `|x| x + 1`

### 2. numeric_ops.rs - Numeric Operations

**Purpose:** Bit manipulation and numeric patterns that frequently appear in algorithm problems.

**Content:**
- **Bit manipulation basics**
  - Setting a bit: `n | (1 << pos)`
  - Clearing a bit: `n & !(1 << pos)`
  - Toggling a bit: `n ^ (1 << pos)`
  - Checking if bit is set: `(n & (1 << pos)) != 0`
  - Counting set bits: `n.count_ones()`
  - Left/right shifts: `<<`, `>>`

- **Bit manipulation tricks**
  - Check if power of 2: `n & (n - 1) == 0`
  - Find rightmost set bit: `n & -n`
  - XOR for finding unique element
  - Creating bit masks
  - Isolating lowest/highest bit

- **Safe arithmetic**
  - `wrapping_*` - Wrap on overflow
  - `saturating_*` - Clamp at bounds
  - `checked_*` - Returns Option
  - `overflowing_*` - Returns tuple with overflow flag

- **Common numeric operations**
  - `abs()`, `abs_diff()`
  - `min()`, `max()`, `clamp()`
  - `pow()`, `sqrt()`
  - Integer division: `/`, `div_euclid()`
  - Modulo: `%`, `rem_euclid()`

- **Number algorithms**
  - GCD (greatest common divisor)
  - LCM (least common multiple)
  - Fast exponentiation (binary exponentiation)
  - Prime checking basics

- **Parsing and conversions**
  - `parse::<i32>()` with error handling
  - `from_str_radix()` for binary/hex
  - Digit extraction: `n % 10`, `n / 10`

### 3. smart_pointers.rs - Smart Pointer Patterns

**Purpose:** Essential for implementing linked lists, trees, and graphs without unsafe code.

**Content:**
- **Box<T> - Heap allocation**
  - Basic heap allocation: `Box::new(value)`
  - Recursive types (essential for linked lists/trees)
  - Dereferencing: `*boxed_value`
  - Pattern matching on Box
  - When to use: large values, recursive structures

- **Rc<T> - Reference counting**
  - Creating: `Rc::new(value)`
  - Cloning references: `Rc::clone(&rc)`
  - Strong count: `Rc::strong_count()`
  - Weak references: `Rc::downgrade()`, `Weak::upgrade()`
  - When to use: shared ownership (multiple parents in graphs)

- **RefCell<T> - Interior mutability**
  - Creating: `RefCell::new(value)`
  - Borrowing: `borrow()`, `borrow_mut()`
  - Runtime borrow checking (panics on violation)
  - When to use: mutation through shared reference

- **Rc<RefCell<T>> - The graph pattern**
  - Shared mutable state
  - Common in tree/graph implementations
  - Creating nodes with multiple parents
  - Modifying shared nodes

- **Cow<T> - Clone-on-write**
  - `Cow::Borrowed` vs `Cow::Owned`
  - `to_mut()` for getting mutable access
  - When to use: avoiding clones until necessary
  - String operations with `Cow<str>`

- **Comparison table**
  - When to use Box vs Rc vs RefCell vs Arc
  - Ownership models
  - Performance characteristics

### 4. concurrency.rs - Concurrent Programming Patterns

**Purpose:** Thread-safe patterns for concurrent programming, useful for systems interviews and parallel algorithms.

**Content:**
- **Thread spawning and joining**
  - `thread::spawn()` with closures
  - `join()` to wait for completion
  - Returning values from threads with `JoinHandle`
  - Scoped threads: `thread::scope()` for borrowing

- **Arc<T> - Atomic reference counting**
  - Creating: `Arc::new(value)`
  - Cloning for thread sharing: `Arc::clone(&arc)`
  - Thread-safe shared ownership
  - Difference from Rc (thread-safe vs single-threaded)

- **Mutex<T> - Mutual exclusion**
  - Creating: `Mutex::new(value)`
  - Locking: `lock().unwrap()`
  - Guard pattern (RAII unlock)
  - Arc<Mutex<T>> for shared mutable state across threads
  - Handling lock poisoning

- **RwLock<T> - Reader-writer lock**
  - `read()` for shared access (multiple readers)
  - `write()` for exclusive access (single writer)
  - When to use vs Mutex (read-heavy workloads)

- **Channels - Message passing**
  - `mpsc::channel()` (multi-producer, single-consumer)
  - `send()` and `recv()`
  - `try_recv()` for non-blocking
  - Bounded vs unbounded channels
  - Producer-consumer pattern

- **Atomic types**
  - `AtomicBool`, `AtomicUsize`, `AtomicI32`
  - `load()`, `store()`, `fetch_add()`
  - Ordering: `Relaxed`, `Acquire`, `Release`, `SeqCst`
  - Lock-free counters and flags

- **Common patterns**
  - Shared counter with Arc<Mutex<i32>>
  - Fan-out work distribution
  - Parallel map-reduce
  - Worker pool with channels

## Design Principles

Following existing fundamentals pattern:
- **Comprehensive examples** with inline comments
- **Muscle memory focus** - patterns you'll type repeatedly
- **Interview relevance** - common patterns in coding interviews
- **Progressive complexity** - simple to advanced within each module
- **No tests required** - These are reference implementations for practice, not library code

## File Structure

```
src/fundamentals/
├── mod.rs              # Add new module exports
├── closures.rs         # NEW
├── numeric_ops.rs      # NEW
├── smart_pointers.rs   # NEW
├── concurrency.rs      # NEW
├── borrowing.rs        # existing
├── iterators.rs        # existing
├── collections.rs      # existing
├── error_handling.rs   # existing
├── pattern_matching.rs # existing
├── strings.rs          # existing
└── types_and_traits.rs # existing
```

## Implementation Plan

1. Create `closures.rs` with function trait patterns
2. Create `numeric_ops.rs` with bit manipulation and arithmetic
3. Create `smart_pointers.rs` with Box/Rc/RefCell patterns
4. Create `concurrency.rs` with thread-safe patterns
5. Update `src/fundamentals/mod.rs` to export new modules
6. Update `CLAUDE.md` Components Completed section
7. Run `cargo fmt` and `cargo clippy`
8. Verify all modules compile

## Success Criteria

- All four modules follow existing fundamentals style
- Inline comments explain each pattern
- Code compiles without warnings
- Patterns are practical for interview use
- CLAUDE.md updated to reflect completion

## Future gittype Practice

Once complete, users can practice:
- Typing closure syntax until `.map(|x| x * 2)` is automatic
- Bit manipulation patterns for algorithm problems
- Smart pointer patterns for tree/graph problems
- Concurrent patterns for systems programming interviews
