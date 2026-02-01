//! # Performance Patterns
//!
//! Performance optimization techniques: inlining, allocation optimization,
//! cache-friendly patterns, and hot path optimization for production Rust.

use std::collections::HashMap;

// ============================================================================
// Inlining Strategies
// ============================================================================

/// Always inline - small, frequently called functions.
#[inline(always)]
fn add_inline_always(a: i32, b: i32) -> i32 {
    a + b
}

/// Hint to inline - let compiler decide.
#[inline]
fn multiply_inline(a: i32, b: i32) -> i32 {
    a * b
}

/// Never inline - large functions, debugging, or preventing code bloat.
#[inline(never)]
fn complex_operation(x: i32) -> i32 {
    // Complex logic that shouldn't be inlined
    let mut result = x;
    for i in 1..100 {
        result = result.wrapping_mul(i).wrapping_add(i);
    }
    result
}

#[allow(dead_code)]
fn demonstrate_inlining() {
    // When to use inline(always):
    // - Tiny functions (getters, setters)
    // - Hot path code
    // - Functions where function call overhead matters

    // When to use inline:
    // - Most public API functions (cross-crate inlining hint)
    // - Medium-sized functions that benefit from inlining

    // When to use inline(never):
    // - Large functions
    // - Cold path code
    // - When you want stable stack traces
    // - Functions with loops/recursion

    let _ = add_inline_always(1, 2);
    let _ = multiply_inline(3, 4);
    let _ = complex_operation(5);
}

// ============================================================================
// Allocation Optimization
// ============================================================================

#[allow(dead_code)]
fn demonstrate_allocation_optimization() {
    // Pattern 1: Pre-allocate with capacity
    let mut vec = Vec::with_capacity(1000); // Avoid reallocations
    for i in 0..1000 {
        vec.push(i);
    }

    let mut map = HashMap::with_capacity(100);
    for i in 0..100 {
        map.insert(i, i * 2);
    }

    // Pattern 2: Reuse allocations
    let mut buffer = Vec::with_capacity(1024);
    for _ in 0..10 {
        buffer.clear(); // Doesn't deallocate
        // Fill buffer with new data
        buffer.extend(0..100);
    }

    // Pattern 3: String building
    // Bad: creates many intermediate strings
    let _bad = "Hello".to_string() + " " + "World";

    // Good: pre-allocate
    let mut good = String::with_capacity(11);
    good.push_str("Hello");
    good.push(' ');
    good.push_str("World");

    // Pattern 4: Avoid cloning when possible
    fn process_data(data: &[i32]) -> i32 {
        // Works with borrowed data, no allocation
        data.iter().sum()
    }

    let data = vec![1, 2, 3, 4, 5];
    let _ = process_data(&data); // No clone needed
}

// ============================================================================
// Cache-Friendly Patterns
// ============================================================================

/// Struct of Arrays (SoA) - cache friendly for iteration.
#[allow(dead_code)]
struct ParticlesSOA {
    x: Vec<f32>,
    y: Vec<f32>,
    z: Vec<f32>,
    mass: Vec<f32>,
}

impl ParticlesSOA {
    fn new(capacity: usize) -> Self {
        ParticlesSOA {
            x: Vec::with_capacity(capacity),
            y: Vec::with_capacity(capacity),
            z: Vec::with_capacity(capacity),
            mass: Vec::with_capacity(capacity),
        }
    }

    // Cache-friendly: all x values are contiguous
    fn sum_x(&self) -> f32 {
        self.x.iter().sum()
    }
}

/// Array of Structs (AoS) - cache friendly for single-element access.
#[derive(Clone, Copy)]
#[allow(dead_code)]
struct Particle {
    x: f32,
    y: f32,
    z: f32,
    mass: f32,
}

#[allow(dead_code)]
struct ParticlesAOS {
    particles: Vec<Particle>,
}

impl ParticlesAOS {
    fn new(capacity: usize) -> Self {
        ParticlesAOS {
            particles: Vec::with_capacity(capacity),
        }
    }

    // Less cache-friendly: jumps between structs
    fn sum_x(&self) -> f32 {
        self.particles.iter().map(|p| p.x).sum()
    }
}

#[allow(dead_code)]
fn demonstrate_data_layout() {
    // Use SoA when:
    // - Processing one field across many elements
    // - SIMD operations
    // - Column-wise data access

    // Use AoS when:
    // - Processing whole objects together
    // - Random access to individual elements
    // - Simpler API
}

/// Memory layout control with repr.
#[repr(C)] // C-compatible layout
#[allow(dead_code)]
struct CCompatible {
    a: u32,
    b: u16,
    c: u8,
}

#[repr(packed)] // No padding - may hurt performance
#[allow(dead_code)]
struct Packed {
    a: u8,
    b: u32, // Unaligned!
}

#[repr(align(64))] // Cache line aligned
#[allow(dead_code)]
struct CacheLineAligned {
    data: [u8; 64],
}

// ============================================================================
// Iterator Optimization
// ============================================================================

#[allow(dead_code)]
fn demonstrate_iterator_optimization() {
    let numbers: Vec<i32> = (0..1000).collect();

    // Pattern 1: Avoid unnecessary collect()
    // Bad: collects intermediate results
    let _bad: i32 = numbers
        .iter()
        .filter(|&&x| x % 2 == 0)
        .collect::<Vec<_>>() // Unnecessary allocation
        .iter()
        .copied()
        .sum();

    // Good: chain operations
    let _good: i32 = numbers
        .iter()
        .filter(|&&x| x % 2 == 0)
        .sum(); // No intermediate collection

    // Pattern 2: Use extend instead of repeated push
    let mut result = Vec::with_capacity(numbers.len());
    // Bad:
    // for n in &numbers {
    //     result.push(*n);
    // }

    // Good:
    result.extend(numbers.iter().copied());

    // Pattern 3: fold vs collect for simple cases
    // collect creates allocation:
    let _sum1: i32 = numbers.iter().copied().collect::<Vec<_>>().iter().sum();

    // fold doesn't:
    let _sum2 = numbers.iter().fold(0, |acc, &x| acc + x);
}

// ============================================================================
// Cow for Conditional Cloning
// ============================================================================

use std::borrow::Cow;

/// Only clone when modification is needed.
#[allow(dead_code)]
fn process_string(s: &str) -> Cow<str> {
    if s.contains("bad") {
        // Need to modify - create owned String
        Cow::Owned(s.replace("bad", "good"))
    } else {
        // No modification - borrow
        Cow::Borrowed(s)
    }
}

#[allow(dead_code)]
fn demonstrate_cow() {
    let s1 = "hello world";
    let result1 = process_string(s1);
    // No allocation - borrowed

    let s2 = "bad input";
    let result2 = process_string(s2);
    // Allocated - owned

    println!("{result1}, {result2}");
}

// ============================================================================
// Benchmarking Patterns
// ============================================================================

use std::hint::black_box;

#[allow(dead_code)]
fn demonstrate_black_box() {
    // black_box prevents compiler from optimizing away code

    // Without black_box - might be optimized away:
    let _result = expensive_computation(100);

    // With black_box - forces computation:
    let result = black_box(expensive_computation(black_box(100)));
    black_box(result); // Prevents dead code elimination
}

fn expensive_computation(n: i32) -> i32 {
    (0..n).sum()
}

/// Micro-benchmark pattern (use criterion for real benchmarks).
#[allow(dead_code)]
fn micro_benchmark() {
    use std::time::Instant;

    let iterations = 10_000;

    // Warmup
    for _ in 0..1000 {
        black_box(expensive_computation(black_box(100)));
    }

    let start = Instant::now();
    for _ in 0..iterations {
        black_box(expensive_computation(black_box(100)));
    }
    let duration = start.elapsed();

    println!(
        "Average: {:?}",
        duration / iterations
    );
}

// ============================================================================
// Hot Path Optimization
// ============================================================================

/// Fast path / slow path pattern.
#[allow(dead_code)]
fn process_item(item: &str) -> String {
    // Fast path for common case
    if item.len() < 10 {
        return item.to_uppercase(); // Inline-able
    }

    // Slow path for rare case
    slow_complex_processing(item)
}

#[inline(never)] // Keep slow path out of fast path
fn slow_complex_processing(item: &str) -> String {
    // Complex logic
    item.chars().rev().collect()
}

/// Reducing bounds checks.
#[allow(dead_code)]
fn sum_slice(slice: &[i32]) -> i32 {
    let mut sum = 0;

    // Compiler can't eliminate bounds checks in regular loop
    for i in 0..slice.len() {
        sum += slice[i]; // Bounds check on each access
    }

    // Iterator eliminates bounds checks
    let sum_iter: i32 = slice.iter().copied().sum();

    // Unsafe to completely remove bounds checks (when proven safe)
    let sum_unsafe = unsafe {
        let mut total = 0;
        for i in 0..slice.len() {
            total += slice.get_unchecked(i);
        }
        total
    };

    let _ = sum_unsafe;
    sum_iter
}

/// Branch prediction hints (unstable - for demonstration).
#[allow(dead_code)]
fn with_branch_hints(x: i32) -> i32 {
    // In nightly Rust:
    // if std::intrinsics::likely(x > 0) {
    //     x * 2
    // } else {
    //     0
    // }

    // Stable alternative: structure code to hint at likely path
    if x > 0 {
        // Common path first
        x * 2
    } else {
        // Rare path second
        0
    }
}

// ============================================================================
// String Interning
// ============================================================================

use std::collections::HashSet;

/// Simple string interner for deduplication.
#[allow(dead_code)]
struct StringInterner {
    strings: HashSet<String>,
}

impl StringInterner {
    fn new() -> Self {
        StringInterner {
            strings: HashSet::new(),
        }
    }

    fn intern(&mut self, s: &str) -> &str {
        if !self.strings.contains(s) {
            self.strings.insert(s.to_string());
        }
        // SAFETY: We just inserted it if it wasn't there
        self.strings.get(s).unwrap()
    }
}

#[allow(dead_code)]
fn demonstrate_interning() {
    let mut interner = StringInterner::new();

    {
        let s1 = interner.intern("hello");
        let _ = s1;
    }
    {
        let s2 = interner.intern("hello");
        let _ = s2;
    }

    // Same string is reused (demonstration only - can't compare pointers across borrows)
}

// ============================================================================
// Object Pools
// ============================================================================

/// Simple object pool to reuse allocations.
#[allow(dead_code)]
struct ObjectPool<T> {
    pool: Vec<T>,
    factory: fn() -> T,
}

impl<T> ObjectPool<T> {
    fn new(factory: fn() -> T, initial_capacity: usize) -> Self {
        let mut pool = Vec::with_capacity(initial_capacity);
        for _ in 0..initial_capacity {
            pool.push(factory());
        }
        ObjectPool { pool, factory }
    }

    fn acquire(&mut self) -> T {
        self.pool.pop().unwrap_or_else(self.factory)
    }

    fn release(&mut self, item: T) {
        self.pool.push(item);
    }
}

#[allow(dead_code)]
fn demonstrate_object_pool() {
    let mut pool = ObjectPool::new(Vec::new, 10);

    let mut buffer: Vec<u8> = pool.acquire();
    buffer.extend_from_slice(b"hello");
    // Use buffer

    buffer.clear(); // Prepare for reuse
    pool.release(buffer);
}

// ============================================================================
// Lazy Initialization
// ============================================================================

use std::sync::OnceLock;

/// Global static initialized lazily.
static EXPENSIVE_RESOURCE: OnceLock<String> = OnceLock::new();

#[allow(dead_code)]
fn get_resource() -> &'static str {
    EXPENSIVE_RESOURCE.get_or_init(|| {
        // Expensive initialization happens once
        "initialized".to_string()
    })
}

/// Lazy initialization pattern (pre-OnceLock).
#[allow(dead_code)]
struct LazyInit<T> {
    value: Option<T>,
    init: fn() -> T,
}

impl<T> LazyInit<T> {
    const fn new(init: fn() -> T) -> Self {
        LazyInit { value: None, init }
    }

    fn get(&mut self) -> &T {
        self.value.get_or_insert_with(self.init)
    }
}

// ============================================================================
// Small String Optimization (SSO)
// ============================================================================

/// Small string optimization - avoid heap allocation for short strings.
#[allow(dead_code)]
enum SmallString {
    Inline([u8; 23], u8), // 23 bytes + 1 byte len
    Heap(String),
}

impl SmallString {
    fn new(s: &str) -> Self {
        if s.len() <= 23 {
            let mut buf = [0u8; 23];
            buf[..s.len()].copy_from_slice(s.as_bytes());
            SmallString::Inline(buf, s.len() as u8)
        } else {
            SmallString::Heap(s.to_string())
        }
    }

    fn as_str(&self) -> &str {
        match self {
            SmallString::Inline(buf, len) => {
                std::str::from_utf8(&buf[..*len as usize]).unwrap()
            }
            SmallString::Heap(s) => s,
        }
    }
}

#[allow(dead_code)]
fn demonstrate_sso() {
    let small = SmallString::new("hello"); // No heap allocation
    let large = SmallString::new("this is a very long string"); // Heap allocated

    println!("{}, {}", small.as_str(), large.as_str());
}

// ============================================================================
// Arena Allocation
// ============================================================================

/// Simple bump allocator (arena).
#[allow(dead_code)]
struct Arena {
    buffer: Vec<u8>,
    pos: usize,
}

impl Arena {
    fn with_capacity(capacity: usize) -> Self {
        Arena {
            buffer: vec![0; capacity],
            pos: 0,
        }
    }

    fn allocate(&mut self, size: usize) -> &mut [u8] {
        let start = self.pos;
        self.pos += size;
        if self.pos > self.buffer.len() {
            panic!("Arena exhausted");
        }
        &mut self.buffer[start..self.pos]
    }

    fn reset(&mut self) {
        self.pos = 0; // Free all at once
    }
}

#[allow(dead_code)]
fn demonstrate_arena() {
    let mut arena = Arena::with_capacity(1024);

    let slice1 = arena.allocate(100);
    slice1[0] = 42; // Use slice

    let slice2 = arena.allocate(200);
    slice2[0] = 99; // Use slice

    // Note: slices are still valid here, pointing into arena buffer
    // In real usage, you'd process data before reset

    // Clear arena for next batch (invalidates all slices)
    arena.reset();
}

// ============================================================================
// Practical Performance Patterns
// ============================================================================

/// Pattern 1: Avoid allocation in loops.
#[allow(dead_code)]
fn process_records(records: &[&str]) -> Vec<String> {
    let mut results = Vec::with_capacity(records.len());
    let mut buffer = String::with_capacity(64); // Reuse allocation

    for record in records {
        buffer.clear();
        buffer.push_str("processed: ");
        buffer.push_str(record);
        results.push(buffer.clone()); // Clone to store
    }

    results
}

/// Pattern 2: Batch operations.
#[allow(dead_code)]
fn batch_insert(map: &mut HashMap<i32, i32>, items: &[(i32, i32)]) {
    map.reserve(items.len()); // Pre-allocate

    for &(k, v) in items {
        map.insert(k, v);
    }
}

/// Pattern 3: Early exit on hot path.
#[allow(dead_code)]
fn find_first_match(items: &[i32], target: i32) -> Option<usize> {
    // Early exit as soon as match found
    for (i, &item) in items.iter().enumerate() {
        if item == target {
            return Some(i);
        }
    }
    None
}

/// Pattern 4: Avoiding String concatenation in loops.
#[allow(dead_code)]
fn build_query(conditions: &[&str]) -> String {
    let capacity = conditions.iter().map(|c| c.len()).sum::<usize>() + conditions.len() * 5;
    let mut query = String::with_capacity(capacity);

    query.push_str("WHERE ");
    for (i, condition) in conditions.iter().enumerate() {
        if i > 0 {
            query.push_str(" AND ");
        }
        query.push_str(condition);
    }

    query
}

// ============================================================================
// Profiling Integration Points
// ============================================================================

#[allow(dead_code)]
fn demonstrate_profiling() {
    // Profiling tools:
    // - cargo flamegraph (flamegraph generation)
    // - perf (Linux)
    // - Instruments (macOS)
    // - VTune (Intel)

    // Integration points:
    // 1. Mark hot functions for profiling
    #[inline(never)] // Easier to see in profiles
    fn hot_function() {
        // ...
    }

    // 2. Add manual instrumentation points
    let _start = std::time::Instant::now();
    expensive_computation(100);
    let _duration = _start.elapsed();
    // Log or aggregate durations

    // 3. Use criterion for micro-benchmarks
    // (See testing.rs for benchmark patterns)
}

// ============================================================================
// Common Performance Pitfalls
// ============================================================================

#[allow(dead_code)]
fn demonstrate_pitfalls() {
    // Pitfall 1: Unnecessary cloning
    fn bad(s: String) -> String {
        s.clone() // Unnecessary clone
    }

    fn good(s: &str) -> String {
        s.to_string() // Clone only when needed
    }

    // Pitfall 2: Growing vectors without capacity
    let mut bad_vec = Vec::new();
    for i in 0..10000 {
        bad_vec.push(i); // Many reallocations
    }

    let mut good_vec = Vec::with_capacity(10000);
    for i in 0..10000 {
        good_vec.push(i); // One allocation
    }

    // Pitfall 3: String concatenation in loops
    let mut bad_str = String::new();
    for i in 0..100 {
        bad_str = format!("{}{}", bad_str, i); // Reallocates each time
    }

    let mut good_str = String::with_capacity(300);
    for i in 0..100 {
        good_str.push_str(&i.to_string());
    }

    let _ = (bad, good, good_vec, good_str);
}

// ============================================================================
// Usage Notes
// ============================================================================

/// Performance optimization guidelines:
///
/// 1. **Measure first** - Profile before optimizing
/// 2. **Focus on hot paths** - 80/20 rule applies
/// 3. **Use release builds** - `cargo build --release`
/// 4. **Enable LTO** - Link-time optimization in Cargo.toml:
///    ```toml
///    [profile.release]
///    lto = true
///    codegen-units = 1
///    ```
/// 5. **Benchmark** - Use criterion for accurate measurements
/// 6. **Don't micro-optimize cold paths** - Clarity over speed for rare code
///
/// Tools:
/// - cargo-flamegraph: `cargo install flamegraph`
/// - cargo-criterion: `cargo bench`
/// - perf (Linux): `perf record -g target/release/app`
#[allow(dead_code)]
const PERFORMANCE_NOTES: &str = "See module docs";
