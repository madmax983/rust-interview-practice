//! DHAT heap profiling in-process with `dhat-rs`.
//!
//! ```bash
//! cargo test --features dhat-heap --test dhat_heap
//! ```
//!
//! `dhat::Alloc` must be the global allocator, so this lives in its own test
//! binary. Only one `dhat::Profiler` may run at a time, so everything is in a
//! single `#[test]` that runs its phases in sequence.

use rust_interview_practice::performance::alloc_accounting::{
    LineEncoder, join_naive, join_presized, render_lines_direct, render_lines_naive,
    sum_of_squares_collected, sum_of_squares_streaming,
};
use rust_interview_practice::performance::profilers::DhatProfile;

#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

/// Heap blocks and bytes allocated while running `f`, plus the peak live
/// bytes reached above the starting point.
fn heap_delta<R>(f: impl FnOnce() -> R) -> (R, u64, u64) {
    let before = dhat::HeapStats::get();
    let value = f();
    let after = dhat::HeapStats::get();
    (
        value,
        after.total_blocks - before.total_blocks,
        after.total_bytes - before.total_bytes,
    )
}

#[test]
fn dhat_heap_drills() {
    // Phase 1: testing mode turns heap counts into assertions. On failure,
    // `dhat::assert*` also writes dhat-heap.json for inspection in dh_view.
    {
        let _profiler = dhat::Profiler::builder().testing().build();
        let words = ["dhat"; 64];

        let (s, blocks, bytes) = heap_delta(|| join_presized(&words, ","));
        dhat::assert_eq!(blocks, 1);
        dhat::assert_eq!(bytes, s.len() as u64);

        // DHAT counts each realloc as a new block, so amortized growth shows up.
        let (_, naive_blocks, _) = heap_delta(|| join_naive(&words, ","));
        dhat::assert!(naive_blocks > 1);

        let items: Vec<(u32, &str)> = (0..100).map(|i| (i, "item")).collect();
        let (_, direct_blocks, _) = heap_delta(|| render_lines_direct(&items));
        let (_, naive_render_blocks, _) = heap_delta(|| render_lines_naive(&items));
        dhat::assert!(naive_render_blocks >= 100 + direct_blocks);

        let (_, streaming_blocks, _) = heap_delta(|| sum_of_squares_streaming(10_000));
        dhat::assert_eq!(streaming_blocks, 0);
        let (_, collected_blocks, collected_bytes) =
            heap_delta(|| sum_of_squares_collected(10_000));
        dhat::assert_eq!(collected_blocks, 1);
        dhat::assert_eq!(collected_bytes, 80_000);

        // A warm reusable buffer allocates nothing.
        let mut enc = LineEncoder::new();
        let _ = enc.encode(&["GET", "/", "HTTP/1.1"]);
        let (_, warm_blocks, _) = heap_delta(|| enc.encode(&["GET", "/", "HTTP/1.1"]).len());
        dhat::assert_eq!(warm_blocks, 0);
    }

    // Phase 2: profile mode writes the same JSON as `valgrind --tool=dhat`,
    // and `DhatProfile` reads it back.
    let path = std::env::temp_dir().join(format!("dhat-heap-{}.json", std::process::id()));
    {
        let _profiler = dhat::Profiler::builder().file_name(&path).build();
        let items: Vec<(u32, &str)> = (0..500).map(|i| (i, "item")).collect();
        std::hint::black_box(render_lines_naive(&items));
    }
    let text = std::fs::read_to_string(&path).expect("dhat-rs wrote its profile");
    let _ = std::fs::remove_file(&path);
    let profile = DhatProfile::parse(&text).expect("dhat-rs JSON parses");
    assert_eq!(profile.mode, "rust-heap");
    assert!(profile.total_blocks() >= 500, "{}", profile.total_blocks());
    assert!(
        profile.sites_matching("render_lines_naive").count() >= 1,
        "no site attributed to render_lines_naive"
    );
}
