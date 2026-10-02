//! # Performance
//!
//! Drills for *measuring* performance honestly, as opposed to the idioms for
//! *writing* fast code in [`fundamentals::performance`](crate::fundamentals::performance).
//! Every claim here is pinned by a deterministic test: virtual clocks, exact
//! allocation counts, a cache simulator, and Valgrind's instruction, cache and
//! heap counts replace "it felt faster".
//!
//! | Module | Question it answers | Deterministic metric |
//! |--------|---------------------|----------------------|
//! | [`bench_harness`] | How long does this take, really? | Virtual-clock timings |
//! | [`alloc_accounting`] | How often does this hit the allocator? | Allocation counts, peak bytes |
//! | [`cache_locality`] | Why is the same loop 10x slower in a different order? | Simulated cache misses |
//! | [`regression`] | Did this change make it slower? | Rank tests, bootstrap CIs, count gates |
//! | [`profilers`] | What does the real binary do? | Callgrind `Ir`, cachegrind `D1` misses, DHAT blocks |
//!
//! ```text
//!   bench_harness ──Stats──► regression ◄──counts── alloc_accounting
//!                              ▲    ▲
//!        cache_locality ─misses┘    └─Ir / D1 misses / heap blocks── profilers (valgrind)
//! ```
//!
//! Outside the library:
//!
//! - `src/bin/perf_drills.rs` runs paired good/bad workloads against the real
//!   clock (`bench`), is the target Valgrind profiles (`run <workload>`), reads
//!   Valgrind output (`callgrind`, `cachegrind`, `dhat`), and gates every pair
//!   under Valgrind (`valgrind-check`, a CI job).
//! - `tests/dhat_heap.rs` uses `dhat-rs` in-process heap assertions
//!   (`cargo test --features dhat-heap --test dhat_heap`).
//!
//! See `docs/adr/0002-performance-allocation-accounting.md` for why the
//! counting allocator, `dhat-rs` and Valgrind each live where they do.

pub mod alloc_accounting;
pub mod bench_harness;
pub mod cache_locality;
pub mod profilers;
pub mod regression;
