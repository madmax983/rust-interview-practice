//! # Allocation Accounting
//!
//! Wall-clock time is noisy. Allocation *counts* are not: the same code on the
//! same input makes the same number of heap calls every run. That makes them the
//! best performance metric to assert on in a unit test or a CI gate.
//!
//! [`CountingAlloc`] wraps any [`GlobalAlloc`] and counts every `alloc`,
//! `dealloc` and `realloc`, plus bytes and peak live bytes. [`measure`] runs a
//! closure and reports what it allocated as an [`AllocStats`].
//!
//! Counters are **per thread**. That is deliberate: `cargo test` runs tests on
//! parallel threads, and global counters would mix their allocations together.
//! The flip side is that work a closure hands to *another* thread is not counted.
//!
//! This crate installs `CountingAlloc<System>` as the global allocator only in
//! its own unit-test binary (`#[cfg(test)]`), so it never changes the allocator
//! of a program that depends on the library. A binary or doctest opts in itself:
//!
//! ```
//! use std::alloc::System;
//! use rust_interview_practice::performance::alloc_accounting::{measure, CountingAlloc};
//!
//! #[global_allocator]
//! static GLOBAL: CountingAlloc<System> = CountingAlloc::new(System);
//!
//! let (v, stats) = measure(|| Vec::<u64>::with_capacity(100));
//! assert_eq!(stats.allocations, 1);
//! assert_eq!(stats.bytes_allocated, 800);
//! # drop(v);
//! ```
//!
//! The rest of the module is drills: each pair does the same job with a careless
//! allocation pattern and a deliberate one, and the tests pin the difference.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::fmt::{self, Write as _};
use std::sync::atomic::{AtomicBool, Ordering};

// ============================================================================
// The counting allocator
// ============================================================================

/// Per-thread raw counters. `Cell<u64>` has no destructor, so the thread-local
/// below needs no lazy initialization or destructor registration, either of
/// which could allocate (and re-enter the allocator).
struct Counters {
    allocations: Cell<u64>,
    deallocations: Cell<u64>,
    reallocations: Cell<u64>,
    bytes_allocated: Cell<u64>,
    bytes_deallocated: Cell<u64>,
    live: Cell<i64>,
    peak: Cell<i64>,
}

impl Counters {
    const fn new() -> Self {
        Self {
            allocations: Cell::new(0),
            deallocations: Cell::new(0),
            reallocations: Cell::new(0),
            bytes_allocated: Cell::new(0),
            bytes_deallocated: Cell::new(0),
            live: Cell::new(0),
            peak: Cell::new(0),
        }
    }

    #[allow(clippy::cast_possible_wrap)] // a single allocation cannot exceed isize::MAX
    fn grow(&self, bytes: usize) {
        self.bytes_allocated
            .set(self.bytes_allocated.get().wrapping_add(bytes as u64));
        let live = self.live.get().wrapping_add(bytes as i64);
        self.live.set(live);
        if live > self.peak.get() {
            self.peak.set(live);
        }
    }

    #[allow(clippy::cast_possible_wrap)] // a single allocation cannot exceed isize::MAX
    fn shrink(&self, bytes: usize) {
        self.bytes_deallocated
            .set(self.bytes_deallocated.get().wrapping_add(bytes as u64));
        self.live.set(self.live.get().wrapping_sub(bytes as i64));
    }
}

thread_local! {
    static COUNTERS: Counters = const { Counters::new() };
}

/// Set the first time a [`CountingAlloc`] handles a request.
static INSTALLED: AtomicBool = AtomicBool::new(false);

/// Runs `f` on this thread's counters. Silently does nothing while the thread
/// is being torn down and its thread-locals are gone.
fn record(f: impl FnOnce(&Counters)) {
    INSTALLED.store(true, Ordering::Relaxed);
    let _ = COUNTERS.try_with(f);
}

/// A [`GlobalAlloc`] that forwards to `A` and counts every call.
#[derive(Debug, Default)]
pub struct CountingAlloc<A = System> {
    inner: A,
}

impl<A> CountingAlloc<A> {
    /// Wraps `inner`. `const` so it can initialize a `#[global_allocator]` static.
    pub const fn new(inner: A) -> Self {
        Self { inner }
    }
}

// SAFETY: every method forwards to `inner` with the caller's arguments
// unchanged, so `inner`'s guarantees carry over. The bookkeeping touches only
// a const-initialized thread-local of `Cell`s, which neither allocates nor
// panics.
unsafe impl<A: GlobalAlloc> GlobalAlloc for CountingAlloc<A> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: forwarded verbatim; the caller upholds `alloc`'s contract.
        let ptr = unsafe { self.inner.alloc(layout) };
        if !ptr.is_null() {
            record(|c| {
                c.allocations.set(c.allocations.get() + 1);
                c.grow(layout.size());
            });
        }
        ptr
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        // SAFETY: forwarded verbatim; the caller upholds `alloc_zeroed`'s contract.
        let ptr = unsafe { self.inner.alloc_zeroed(layout) };
        if !ptr.is_null() {
            record(|c| {
                c.allocations.set(c.allocations.get() + 1);
                c.grow(layout.size());
            });
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        record(|c| {
            c.deallocations.set(c.deallocations.get() + 1);
            c.shrink(layout.size());
        });
        // SAFETY: forwarded verbatim; the caller upholds `dealloc`'s contract.
        unsafe { self.inner.dealloc(ptr, layout) };
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        // SAFETY: forwarded verbatim; the caller upholds `realloc`'s contract.
        let new_ptr = unsafe { self.inner.realloc(ptr, layout, new_size) };
        if !new_ptr.is_null() {
            record(|c| {
                c.reallocations.set(c.reallocations.get() + 1);
                c.shrink(layout.size());
                c.grow(new_size);
            });
        }
        new_ptr
    }
}

/// Installs the counter for this crate's own unit tests only.
#[cfg(test)]
#[global_allocator]
static TEST_ALLOCATOR: CountingAlloc<System> = CountingAlloc::new(System);

/// Whether a [`CountingAlloc`] has handled at least one request in this
/// process. When `false`, [`measure`] reports zeros, because nothing is counting.
#[must_use]
pub fn is_installed() -> bool {
    INSTALLED.load(Ordering::Relaxed)
}

// ============================================================================
// Measuring
// ============================================================================

/// Heap activity on the current thread during one [`measure`] call.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AllocStats {
    /// Calls to `alloc` / `alloc_zeroed`.
    pub allocations: u64,
    /// Calls to `dealloc`.
    pub deallocations: u64,
    /// Calls to `realloc` (a growing `Vec` or `String`).
    pub reallocations: u64,
    /// Total bytes requested, including the new size of each realloc.
    pub bytes_allocated: u64,
    /// Total bytes released, including the old size of each realloc.
    pub bytes_deallocated: u64,
    /// Highest live-byte count reached above the starting point.
    pub peak_bytes: u64,
}

impl AllocStats {
    /// Allocations plus reallocations: every trip into the allocator that
    /// handed out memory.
    #[must_use]
    pub const fn heap_requests(&self) -> u64 {
        self.allocations + self.reallocations
    }
}

#[derive(Clone, Copy)]
struct Snapshot {
    allocations: u64,
    deallocations: u64,
    reallocations: u64,
    bytes_allocated: u64,
    bytes_deallocated: u64,
    live: i64,
}

fn snapshot() -> Snapshot {
    COUNTERS.with(|c| Snapshot {
        allocations: c.allocations.get(),
        deallocations: c.deallocations.get(),
        reallocations: c.reallocations.get(),
        bytes_allocated: c.bytes_allocated.get(),
        bytes_deallocated: c.bytes_deallocated.get(),
        live: c.live.get(),
    })
}

/// Runs `f` and returns its result with the allocations it made on this thread.
///
/// The returned value is created inside the measurement, so its own buffer
/// counts as an allocation; it is dropped by the caller, outside of it.
#[allow(clippy::cast_sign_loss)] // clamped to >= 0 first
pub fn measure<R>(f: impl FnOnce() -> R) -> (R, AllocStats) {
    let before = snapshot();
    // Restart peak tracking from the current live count.
    COUNTERS.with(|c| c.peak.set(before.live));
    let value = f();
    let after = snapshot();
    let peak = COUNTERS.with(|c| c.peak.get());
    let stats = AllocStats {
        allocations: after.allocations - before.allocations,
        deallocations: after.deallocations - before.deallocations,
        reallocations: after.reallocations - before.reallocations,
        bytes_allocated: after.bytes_allocated - before.bytes_allocated,
        bytes_deallocated: after.bytes_deallocated - before.bytes_deallocated,
        peak_bytes: (peak - before.live).max(0) as u64,
    };
    (value, stats)
}

/// A limit on how much a piece of code may allocate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AllocBudget {
    /// Maximum [`AllocStats::heap_requests`].
    pub max_heap_requests: u64,
    /// Maximum [`AllocStats::peak_bytes`].
    pub max_peak_bytes: u64,
}

/// Which limit of an [`AllocBudget`] was broken, and by how much.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BudgetExceeded {
    /// Too many trips to the allocator.
    HeapRequests {
        /// The budget.
        limit: u64,
        /// What was measured.
        actual: u64,
    },
    /// Too much memory live at once.
    PeakBytes {
        /// The budget.
        limit: u64,
        /// What was measured.
        actual: u64,
    },
}

impl fmt::Display for BudgetExceeded {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::HeapRequests { limit, actual } => {
                write!(f, "{actual} heap requests exceeds budget of {limit}")
            }
            Self::PeakBytes { limit, actual } => {
                write!(f, "{actual} peak bytes exceeds budget of {limit}")
            }
        }
    }
}

impl std::error::Error for BudgetExceeded {}

impl AllocBudget {
    /// A budget of zero heap requests and zero bytes: the steady-state goal of
    /// a hot loop.
    pub const ZERO: Self = Self {
        max_heap_requests: 0,
        max_peak_bytes: 0,
    };

    /// Checks `stats` against the budget.
    ///
    /// # Errors
    ///
    /// Returns the first broken limit (heap requests are checked first).
    pub const fn check(&self, stats: &AllocStats) -> Result<(), BudgetExceeded> {
        let requests = stats.heap_requests();
        if requests > self.max_heap_requests {
            return Err(BudgetExceeded::HeapRequests {
                limit: self.max_heap_requests,
                actual: requests,
            });
        }
        if stats.peak_bytes > self.max_peak_bytes {
            return Err(BudgetExceeded::PeakBytes {
                limit: self.max_peak_bytes,
                actual: stats.peak_bytes,
            });
        }
        Ok(())
    }
}

// ============================================================================
// Drills: careless vs deliberate allocation
// ============================================================================

/// Careless: grows the output one piece at a time, reallocating as it goes.
///
/// Time: O(total) amortized. Heap: O(log total) reallocations.
#[must_use]
pub fn join_naive(words: &[&str], sep: &str) -> String {
    let mut out = String::new();
    for (i, word) in words.iter().enumerate() {
        if i > 0 {
            out.push_str(sep);
        }
        out.push_str(word);
    }
    out
}

/// Deliberate: sums the final length first and allocates exactly once.
///
/// Time: O(total). Heap: 1 allocation (0 for empty input).
#[must_use]
pub fn join_presized(words: &[&str], sep: &str) -> String {
    let total =
        words.iter().map(|w| w.len()).sum::<usize>() + sep.len() * words.len().saturating_sub(1);
    let mut out = String::with_capacity(total);
    for (i, word) in words.iter().enumerate() {
        if i > 0 {
            out.push_str(sep);
        }
        out.push_str(word);
    }
    out
}

/// Careless: `format!` builds a temporary `String` per line, then copies it.
///
/// Heap: at least one allocation per item.
#[must_use]
pub fn render_lines_naive(items: &[(u32, &str)]) -> String {
    let mut out = String::new();
    for (id, name) in items {
        let line = format!("{id}: {name}\n");
        out.push_str(&line);
    }
    out
}

/// Deliberate: `write!` formats straight into one growing buffer.
///
/// Heap: one buffer, grown O(log total) times; no per-item temporaries.
#[must_use]
pub fn render_lines_direct(items: &[(u32, &str)]) -> String {
    let mut out = String::new();
    for (id, name) in items {
        // Writing to a `String` cannot fail.
        let _ = writeln!(out, "{id}: {name}");
    }
    out
}

/// Careless: copies every token into its own `String`.
///
/// Heap: one allocation per token plus the `Vec`.
#[must_use]
pub fn tokenize_owned(text: &str) -> Vec<String> {
    text.split_whitespace().map(str::to_owned).collect()
}

/// Deliberate: borrows tokens from the input. Only the `Vec` allocates.
#[must_use]
pub fn tokenize_borrowed(text: &str) -> Vec<&str> {
    text.split_whitespace().collect()
}

/// Careless: materializes every value before summing them.
///
/// Peak memory: O(n).
#[must_use]
pub fn sum_of_squares_collected(n: u64) -> u64 {
    let squares: Vec<u64> = (0..n).map(|i| i * i).collect();
    squares.iter().sum()
}

/// Deliberate: streams the values. Peak memory: O(1), zero allocations.
#[must_use]
pub fn sum_of_squares_streaming(n: u64) -> u64 {
    (0..n).map(|i| i * i).sum()
}

/// A reusable scratch buffer for a hot loop. After the first call has grown the
/// buffer, later calls of the same size allocate nothing.
#[derive(Debug, Default)]
pub struct LineEncoder {
    buf: Vec<u8>,
}

impl LineEncoder {
    /// Creates an encoder with an empty buffer.
    #[must_use]
    pub const fn new() -> Self {
        Self { buf: Vec::new() }
    }

    /// Encodes `fields` as a length-prefixed record and returns the bytes.
    /// The returned slice borrows the internal buffer and is valid until the
    /// next call.
    ///
    /// Format: for each field, a `u16` big-endian length then the bytes.
    /// Fields longer than `u16::MAX` bytes are truncated.
    pub fn encode(&mut self, fields: &[&str]) -> &[u8] {
        self.buf.clear(); // keeps capacity: the key to zero steady-state allocations
        for field in fields {
            let bytes = &field.as_bytes()[..field.len().min(usize::from(u16::MAX))];
            let len = u16::try_from(bytes.len()).unwrap_or(u16::MAX);
            self.buf.extend_from_slice(&len.to_be_bytes());
            self.buf.extend_from_slice(bytes);
        }
        &self.buf
    }
}

/// Careless counterpart of [`LineEncoder::encode`]: a fresh `Vec` per call.
#[must_use]
pub fn encode_fresh(fields: &[&str]) -> Vec<u8> {
    let mut enc = LineEncoder::new();
    enc.encode(fields).to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    const WORDS: [&str; 40] = ["alpha"; 40];

    #[test]
    fn test_counting_allocator_is_installed_in_tests() {
        let _warm = Box::new(1_u8);
        assert!(is_installed());
    }

    #[test]
    fn test_measure_counts_one_box() {
        let (b, stats) = measure(|| Box::new([0_u64; 4]));
        assert_eq!(stats.allocations, 1);
        assert_eq!(stats.bytes_allocated, 32);
        assert_eq!(stats.deallocations, 0);
        assert_eq!(stats.peak_bytes, 32);
        drop(b);
    }

    #[test]
    fn test_measure_counts_free_and_peak() {
        let ((), stats) = measure(|| {
            let a = vec![0_u8; 1_000];
            drop(a);
            let b = vec![0_u8; 10];
            drop(b);
        });
        assert_eq!(stats.allocations, 2);
        assert_eq!(stats.deallocations, 2);
        assert_eq!(stats.bytes_allocated, 1_010);
        assert_eq!(stats.bytes_deallocated, 1_010);
        assert_eq!(stats.peak_bytes, 1_000); // never both live at once
    }

    #[test]
    fn test_measure_counts_realloc() {
        let ((), stats) = measure(|| {
            let mut v: Vec<u8> = Vec::with_capacity(1);
            v.extend_from_slice(&[0; 100]);
            assert_eq!(v.len(), 100);
        });
        assert_eq!(stats.allocations, 1);
        assert!(stats.reallocations >= 1);
        assert_eq!(
            stats.heap_requests(),
            stats.allocations + stats.reallocations
        );
    }

    #[test]
    fn test_counters_are_per_thread() {
        let ((), stats) = measure(|| {
            std::thread::scope(|s| {
                s.spawn(|| vec![0_u8; 4_096]);
            });
        });
        // The spawned thread's 4 KiB buffer is not attributed to this thread.
        assert!(stats.bytes_allocated < 4_096, "{stats:?}");
    }

    #[test]
    fn test_nothing_allocates_nothing() {
        let (x, stats) = measure(|| 2 + 2);
        assert_eq!(x, 4);
        assert_eq!(stats, AllocStats::default());
        assert_eq!(AllocBudget::ZERO.check(&stats), Ok(()));
    }

    #[test]
    fn test_join_presized_allocates_exactly_once() {
        let (s, stats) = measure(|| join_presized(&WORDS, ", "));
        assert_eq!(s.len(), 40 * 5 + 39 * 2);
        assert_eq!(stats.allocations, 1);
        assert_eq!(stats.reallocations, 0);
        assert_eq!(stats.bytes_allocated, s.len() as u64);
    }

    #[test]
    fn test_join_naive_reallocates() {
        let (s, stats) = measure(|| join_naive(&WORDS, ", "));
        assert_eq!(s, join_presized(&WORDS, ", "));
        assert!(stats.reallocations >= 3, "{stats:?}");
    }

    #[test]
    fn test_join_edge_cases() {
        assert_eq!(join_presized(&[], ","), "");
        assert_eq!(join_naive(&[], ","), "");
        assert_eq!(join_presized(&["a"], ","), "a");
        let (_, stats) = measure(|| join_presized(&[], ","));
        assert_eq!(stats.heap_requests(), 0); // String::with_capacity(0) does not allocate
    }

    #[test]
    fn test_render_lines_direct_skips_temporaries() {
        let items: Vec<(u32, &str)> = (0..50).map(|i| (i, "widget")).collect();
        let (naive, naive_stats) = measure(|| render_lines_naive(&items));
        let (direct, direct_stats) = measure(|| render_lines_direct(&items));
        assert_eq!(naive, direct);
        assert!(naive_stats.allocations >= 50, "{naive_stats:?}");
        assert_eq!(direct_stats.allocations, 1);
    }

    #[test]
    fn test_tokenize_borrowed_allocates_only_the_vec() {
        let text = "the quick brown fox jumps over the lazy dog";
        let (owned, owned_stats) = measure(|| tokenize_owned(text));
        let (borrowed, borrowed_stats) = measure(|| tokenize_borrowed(text));
        assert_eq!(owned, borrowed);
        assert_eq!(owned_stats.allocations, 1 + 9);
        assert_eq!(borrowed_stats.allocations, 1);
    }

    #[test]
    fn test_streaming_sum_has_no_peak() {
        let (a, collected) = measure(|| sum_of_squares_collected(1_000));
        let (b, streaming) = measure(|| sum_of_squares_streaming(1_000));
        assert_eq!(a, b);
        assert_eq!(collected.peak_bytes, 8_000);
        assert_eq!(streaming, AllocStats::default());
    }

    #[test]
    fn test_line_encoder_steady_state_is_allocation_free() {
        let fields = ["GET", "/index.html", "HTTP/1.1"];
        let mut enc = LineEncoder::new();
        let (first, cold) = measure(|| enc.encode(&fields).to_vec());
        assert!(cold.heap_requests() >= 1);
        // Warm: same-size record, buffer already big enough.
        let (len, warm) = measure(|| enc.encode(&fields).len());
        assert_eq!(len, first.len());
        assert_eq!(AllocBudget::ZERO.check(&warm), Ok(()));
        // The fresh-buffer version pays every time.
        let (fresh, fresh_stats) = measure(|| encode_fresh(&fields));
        assert_eq!(fresh, first);
        assert!(fresh_stats.heap_requests() >= 2);
    }

    #[test]
    fn test_line_encoder_format() {
        let mut enc = LineEncoder::new();
        assert_eq!(enc.encode(&["ab", ""]), &[0, 2, b'a', b'b', 0, 0]);
        assert_eq!(enc.encode(&[]), &[] as &[u8]);
    }

    #[test]
    fn test_budget_reports_the_broken_limit() {
        let stats = AllocStats {
            allocations: 3,
            reallocations: 2,
            peak_bytes: 10,
            ..AllocStats::default()
        };
        let tight = AllocBudget {
            max_heap_requests: 4,
            max_peak_bytes: 100,
        };
        assert_eq!(
            tight.check(&stats),
            Err(BudgetExceeded::HeapRequests {
                limit: 4,
                actual: 5
            })
        );
        let small = AllocBudget {
            max_heap_requests: 5,
            max_peak_bytes: 9,
        };
        let err = small.check(&stats).unwrap_err();
        assert_eq!(err.to_string(), "10 peak bytes exceeds budget of 9");
    }
}
