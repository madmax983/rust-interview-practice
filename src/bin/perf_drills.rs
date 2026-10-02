//! Runs the `performance` drills for real: wall-clock benchmarks, single
//! workloads for Valgrind to profile, and reports over Valgrind's output.
//!
//! ```bash
//! cargo run --release --bin perf_drills -- bench
//! cargo run --release --bin perf_drills -- valgrind-check     # needs valgrind on PATH
//! ```
//!
//! Run without arguments for the full usage.

use std::alloc::System;
use std::hint::black_box;
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::process::{Command, ExitCode};
use std::sync::OnceLock;

use rust_interview_practice::performance::alloc_accounting::{
    CountingAlloc, join_naive, join_presized, measure, render_lines_direct, render_lines_naive,
    sum_of_squares_collected, sum_of_squares_streaming, tokenize_borrowed, tokenize_owned,
};
use rust_interview_practice::performance::bench_harness::{BenchConfig, MonotonicClock, bench};
use rust_interview_practice::performance::cache_locality::{
    ChainNode, ColumnMajor, ParticleAos, ParticlesSoa, RowMajor, Tiled, build_chain,
    shuffled_order, sum_chain, sum_in_order, total_mass_aos, total_mass_soa, transpose,
};
use rust_interview_practice::performance::profilers::{
    CostProfile, DhatProfile, PINNED_CACHE_GEOMETRY, ValgrindTool, valgrind_args,
};
use rust_interview_practice::performance::regression::{
    Thresholds, Verdict, compare_counts, compare_samples,
};

#[global_allocator]
static GLOBAL: CountingAlloc<System> = CountingAlloc::new(System);

// ============================================================================
// Inputs: built once, lazily, so benchmarks time the routine and not its setup
// ============================================================================

const ROWS: usize = 1_024;
const COLS: usize = 1_024;
const CHAIN: usize = 1 << 16;
const PARTICLES: usize = 1 << 20;
const WORDS: usize = 4_096;

fn matrix() -> &'static [u64] {
    static M: OnceLock<Vec<u64>> = OnceLock::new();
    M.get_or_init(|| (0..(ROWS * COLS) as u64).collect())
}

fn mass_of(i: usize) -> f32 {
    f32::from(u8::try_from(i % 7).unwrap_or(0))
}

fn particles_aos() -> &'static [ParticleAos] {
    static P: OnceLock<Vec<ParticleAos>> = OnceLock::new();
    P.get_or_init(|| {
        (0..PARTICLES)
            .map(|i| ParticleAos {
                mass: mass_of(i),
                id: i as u64,
                ..ParticleAos::default()
            })
            .collect()
    })
}

/// Built directly (not converted from `AoS`) so a profile of the `SoA` workload
/// never touches the `AoS` data.
fn particles_soa() -> &'static ParticlesSoa {
    static P: OnceLock<ParticlesSoa> = OnceLock::new();
    P.get_or_init(|| ParticlesSoa {
        pos: vec![[0.0; 3]; PARTICLES],
        vel: vec![[0.0; 3]; PARTICLES],
        mass: (0..PARTICLES).map(mass_of).collect(),
        id: (0..PARTICLES as u64).collect(),
        alive: vec![false; PARTICLES],
    })
}

type Chain = (Vec<ChainNode>, Option<u32>);

fn chain_sequential_input() -> &'static Chain {
    static C: OnceLock<Chain> = OnceLock::new();
    C.get_or_init(|| build_chain(&(0..CHAIN).collect::<Vec<_>>()))
}

fn chain_shuffled_input() -> &'static Chain {
    static C: OnceLock<Chain> = OnceLock::new();
    C.get_or_init(|| build_chain(&shuffled_order(CHAIN, 42)))
}

fn words() -> &'static [&'static str] {
    static W: OnceLock<Vec<&'static str>> = OnceLock::new();
    W.get_or_init(|| vec!["performance"; WORDS])
}

fn text() -> &'static str {
    static T: OnceLock<String> = OnceLock::new();
    T.get_or_init(|| words().join(" "))
}

fn items() -> &'static [(u32, &'static str)] {
    static I: OnceLock<Vec<(u32, &'static str)>> = OnceLock::new();
    I.get_or_init(|| (0..4_096).map(|i| (i, "widget")).collect())
}

// ============================================================================
// Workloads (each `inline(never)` so profilers show it by name, each returning
// a checksum so the optimizer cannot delete the work)
// ============================================================================

#[inline(never)]
fn row_major() -> u64 {
    sum_in_order(
        black_box(matrix()),
        &RowMajor {
            rows: ROWS,
            cols: COLS,
        },
    )
}

#[inline(never)]
fn column_major() -> u64 {
    sum_in_order(
        black_box(matrix()),
        &ColumnMajor {
            rows: ROWS,
            cols: COLS,
        },
    )
}

#[inline(never)]
fn transpose_naive() -> u64 {
    transpose(
        black_box(matrix()),
        &RowMajor {
            rows: ROWS,
            cols: COLS,
        },
    )[1]
}

#[inline(never)]
fn transpose_tiled() -> u64 {
    let order = Tiled {
        rows: ROWS,
        cols: COLS,
        tile: 8,
    };
    transpose(black_box(matrix()), &order)[1]
}

#[inline(never)]
fn aos_mass() -> u64 {
    u64::from(total_mass_aos(black_box(particles_aos())).to_bits())
}

#[inline(never)]
fn soa_mass() -> u64 {
    u64::from(total_mass_soa(black_box(particles_soa())).to_bits())
}

#[inline(never)]
fn chain_sequential() -> u64 {
    let (nodes, head) = chain_sequential_input();
    sum_chain(black_box(nodes), *head)
}

#[inline(never)]
fn chain_shuffled() -> u64 {
    let (nodes, head) = chain_shuffled_input();
    sum_chain(black_box(nodes), *head)
}

#[inline(never)]
fn join_naive_w() -> u64 {
    join_naive(black_box(words()), ",").len() as u64
}

#[inline(never)]
fn join_presized_w() -> u64 {
    join_presized(black_box(words()), ",").len() as u64
}

#[inline(never)]
fn tokenize_owned_w() -> u64 {
    tokenize_owned(black_box(text())).len() as u64
}

#[inline(never)]
fn tokenize_borrowed_w() -> u64 {
    tokenize_borrowed(black_box(text())).len() as u64
}

#[inline(never)]
fn render_naive_w() -> u64 {
    render_lines_naive(black_box(items())).len() as u64
}

#[inline(never)]
fn render_direct_w() -> u64 {
    render_lines_direct(black_box(items())).len() as u64
}

#[inline(never)]
fn sum_collected_w() -> u64 {
    sum_of_squares_collected(black_box(1 << 16))
}

/// Benchmarks at ~1 ns no matter how large `n` is: LLVM's scalar evolution
/// replaces the loop with the closed form `n(n-1)(2n-1)/6`. `black_box` on the
/// input stops constant folding, not algebra. A result that does not scale with
/// input size means you are measuring the compiler, not the algorithm.
#[inline(never)]
fn sum_streaming_w() -> u64 {
    sum_of_squares_streaming(black_box(1 << 16))
}

/// A named workload returning a checksum (so its work cannot be optimized out).
type Workload = (&'static str, fn() -> u64);

/// Every workload.
const WORKLOADS: &[Workload] = &[
    ("row_major", row_major),
    ("column_major", column_major),
    ("transpose_tiled", transpose_tiled),
    ("transpose_naive", transpose_naive),
    ("soa_mass", soa_mass),
    ("aos_mass", aos_mass),
    ("chain_sequential", chain_sequential),
    ("chain_shuffled", chain_shuffled),
    ("join_presized", join_presized_w),
    ("join_naive", join_naive_w),
    ("tokenize_borrowed", tokenize_borrowed_w),
    ("tokenize_owned", tokenize_owned_w),
    ("render_direct", render_direct_w),
    ("render_naive", render_naive_w),
    ("sum_streaming", sum_streaming_w),
    ("sum_collected", sum_collected_w),
];

/// `(good, bad)` pairs: the second should be measurably worse.
const PAIRS: &[(&str, &str)] = &[
    ("row_major", "column_major"),
    ("transpose_tiled", "transpose_naive"),
    ("soa_mass", "aos_mass"),
    ("chain_sequential", "chain_shuffled"),
    ("join_presized", "join_naive"),
    ("tokenize_borrowed", "tokenize_owned"),
    ("render_direct", "render_naive"),
    ("sum_streaming", "sum_collected"),
];

fn workload(name: &str) -> Result<fn() -> u64, String> {
    WORKLOADS
        .iter()
        .find(|(n, _)| *n == name)
        .map(|&(_, f)| f)
        .ok_or_else(|| format!("unknown workload {name:?}; try `perf_drills list`"))
}

// ============================================================================
// Subcommands
// ============================================================================

fn cmd_bench() -> Result<(), String> {
    let clock = MonotonicClock::new();
    let config = BenchConfig {
        warmup_iters: 2,
        samples: NonZeroUsize::new(15).ok_or("zero samples")?,
        target_sample_ns: 2_000_000,
        max_iters_per_sample: 1 << 16,
    };
    let thresholds = Thresholds::default();
    println!(
        "{:<20} {:>12} {:>8} {:>8}   {:<20} {:>12} {:>8} {:>8}   {:>8}  verdict",
        "good", "median ns", "allocs", "peak KB", "bad", "median ns", "allocs", "peak KB", "change"
    );
    for &(good, bad) in PAIRS {
        let (g, b) = (workload(good)?, workload(bad)?);
        // Build the lazily-initialized inputs first, so neither the allocation
        // counts nor the timings include setup.
        black_box((g(), b()));
        let (_, g_alloc) = measure(g);
        let (_, b_alloc) = measure(b);
        let g_res = bench(&clock, &config, good, g);
        let b_res = bench(&clock, &config, bad, b);
        let cmp = compare_samples(&g_res.samples, &b_res.samples, &thresholds);
        println!(
            "{:<20} {:>12.0} {:>8} {:>8}   {:<20} {:>12.0} {:>8} {:>8}   {:>+7.1}%  {}",
            good,
            g_res.stats.median,
            g_alloc.heap_requests(),
            g_alloc.peak_bytes / 1024,
            bad,
            b_res.stats.median,
            b_alloc.heap_requests(),
            b_alloc.peak_bytes / 1024,
            cmp.change * 100.0,
            cmp.verdict
        );
    }
    println!("\nverdict = bad vs good (REGRESSED means the bad variant is measurably slower).");
    Ok(())
}

fn cmd_run(name: &str, reps: &str) -> Result<(), String> {
    let f = workload(name)?;
    let reps: u32 = reps
        .parse()
        .map_err(|_| format!("bad repetition count {reps:?}"))?;
    let mut checksum = 0_u64;
    for _ in 0..reps {
        checksum = checksum.wrapping_add(black_box(f()));
    }
    println!("{name}: checksum {checksum}");
    Ok(())
}

fn read(path: &str) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))
}

fn cmd_cost_report(path: &str, top: &str) -> Result<(), String> {
    let profile = CostProfile::parse(&read(path)?).map_err(|e| format!("{path}: {e}"))?;
    let top: usize = top.parse().map_err(|_| format!("bad count {top:?}"))?;
    println!("events: {}", profile.events().join(" "));
    for event in profile.events() {
        let total = profile.total(event).unwrap_or(0);
        let check = match profile.summary(event) {
            Some(s) if s == total => "matches summary".to_owned(),
            Some(s) => format!("summary says {s}"),
            None => "no summary line".to_owned(),
        };
        println!("  {event:<6} total {total:>14}  ({check})");
    }
    let event = &profile.events()[0];
    println!("\ntop {top} functions by self {event}:");
    for (name, cost) in profile.top_self(event, top) {
        let incl = profile.inclusive_cost(name, event).unwrap_or(cost);
        println!("  {cost:>14} self {incl:>14} incl  {name}");
    }
    if profile.events().iter().any(|e| e == "D1mr") {
        println!("\ntop {top} functions by D1 read misses:");
        for (name, cost) in profile.top_self("D1mr", top) {
            println!("  {cost:>14}  {name}");
        }
    }
    Ok(())
}

fn cmd_dhat_report(path: &str, top: &str) -> Result<(), String> {
    let profile = DhatProfile::parse(&read(path)?).map_err(|e| format!("{path}: {e}"))?;
    let top: usize = top.parse().map_err(|_| format!("bad count {top:?}"))?;
    println!(
        "mode {}: {} blocks, {} bytes total, {} bytes at peak, {} sites",
        profile.mode,
        profile.total_blocks(),
        profile.total_bytes(),
        profile.peak_bytes(),
        profile.sites.len()
    );
    println!("\ntop {top} sites by blocks:");
    for site in profile.top_sites_by_blocks(top) {
        // The first frame outside the allocator is the interesting one.
        let frame = site
            .frames
            .iter()
            .find(|f| !f.contains("alloc") && !f.contains("[root]"))
            .map_or("?", String::as_str);
        println!(
            "  {:>8} blocks {:>10} bytes  {}",
            site.total_blocks, site.total_bytes, frame
        );
    }
    Ok(())
}

/// What a valgrind check measures.
#[derive(Clone, Copy)]
enum Metric {
    /// Callgrind total instructions.
    Instructions,
    /// Cachegrind total D1 misses, reads plus writes (`D1mr + D1mw`). Reads
    /// alone would miss a naive transpose, whose damage is all in the writes.
    D1Misses,
    /// DHAT total heap blocks.
    HeapBlocks,
    /// DHAT bytes live at the global peak.
    PeakBytes,
}

impl Metric {
    const fn tool(self) -> ValgrindTool {
        match self {
            Self::Instructions => ValgrindTool::Callgrind,
            Self::D1Misses => ValgrindTool::Cachegrind,
            Self::HeapBlocks | Self::PeakBytes => ValgrindTool::Dhat,
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::Instructions => "callgrind Ir",
            Self::D1Misses => "cachegrind D1 miss",
            Self::HeapBlocks => "dhat blocks",
            Self::PeakBytes => "dhat peak bytes",
        }
    }
}

/// Runs `workload` under valgrind and extracts `metric` from the output.
fn profile(metric: Metric, workload: &str) -> Result<u64, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let exe = exe.to_str().ok_or("non-UTF-8 executable path")?;
    let out: PathBuf = std::env::temp_dir().join(format!(
        "perf_drills-{}-{}-{workload}.out",
        std::process::id(),
        metric.tool().name()
    ));
    let out_str = out.to_str().ok_or("non-UTF-8 temp path")?;
    // Cachegrind: same numbers on every machine, not the host's cache sizes.
    let extra: &[&str] = if metric.tool() == ValgrindTool::Cachegrind {
        &PINNED_CACHE_GEOMETRY
    } else {
        &[]
    };
    let args = valgrind_args(metric.tool(), out_str, extra, exe, &["run", workload, "1"]);
    let status = Command::new("valgrind")
        .arg("--quiet")
        .args(&args)
        .output()
        .map_err(|e| format!("could not run valgrind (is it installed?): {e}"))?;
    if !status.status.success() {
        return Err(format!(
            "valgrind {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&status.stderr)
        ));
    }
    let text = read(out_str)?;
    let _ = std::fs::remove_file(&out);
    let missing = |what: &str| format!("{workload}: no {what} in {}", metric.label());
    match metric {
        Metric::Instructions => CostProfile::parse(&text)
            .map_err(|e| e.to_string())?
            .total("Ir")
            .ok_or_else(|| missing("Ir")),
        Metric::D1Misses => {
            let p = CostProfile::parse(&text).map_err(|e| e.to_string())?;
            let reads = p.total("D1mr").ok_or_else(|| missing("D1mr"))?;
            let writes = p.total("D1mw").ok_or_else(|| missing("D1mw"))?;
            Ok(reads + writes)
        }
        Metric::HeapBlocks => Ok(DhatProfile::parse(&text)
            .map_err(|e| e.to_string())?
            .total_blocks()),
        Metric::PeakBytes => Ok(DhatProfile::parse(&text)
            .map_err(|e| e.to_string())?
            .peak_bytes()),
    }
}

/// `(metric, good, bad, max_increase)`: the check passes when `bad` exceeds
/// `good` by more than `max_increase` (i.e. the gate flags it as a regression).
///
/// Note what is *not* here: `join_presized` vs `join_naive` under callgrind.
/// Presizing makes fewer heap blocks (DHAT gate below) but its extra
/// length-summing pass executes slightly *more* instructions than the naive
/// version's handful of amortized reallocations. Fewer allocations is not the
/// same as fewer instructions; gate each claim on the metric that shows it.
const CHECKS: &[(Metric, &str, &str, f64)] = &[
    (Metric::D1Misses, "row_major", "column_major", 1.0),
    (Metric::D1Misses, "transpose_tiled", "transpose_naive", 0.5),
    (Metric::D1Misses, "chain_sequential", "chain_shuffled", 0.5),
    (Metric::D1Misses, "soa_mass", "aos_mass", 0.5),
    (
        Metric::Instructions,
        "tokenize_borrowed",
        "tokenize_owned",
        0.1,
    ),
    (Metric::Instructions, "render_direct", "render_naive", 0.1),
    (Metric::HeapBlocks, "join_presized", "join_naive", 0.0),
    (Metric::HeapBlocks, "render_direct", "render_naive", 1.0),
    (Metric::PeakBytes, "sum_streaming", "sum_collected", 1.0),
];

fn cmd_valgrind_check() -> Result<(), String> {
    let mut failures = 0;
    println!(
        "{:<19} {:<18} {:>14}   {:<16} {:>14}   gate",
        "metric", "good", "value", "bad", "value"
    );
    for &(metric, good, bad, max_increase) in CHECKS {
        let g = profile(metric, good)?;
        let b = profile(metric, bad)?;
        let verdict = compare_counts(g, b, max_increase);
        let ok = verdict == Verdict::Regressed;
        if !ok {
            failures += 1;
        }
        println!(
            "{:<19} {:<18} {:>14}   {:<16} {:>14}   {} (expected REGRESSED){}",
            metric.label(),
            good,
            g,
            bad,
            b,
            verdict,
            if ok { "" } else { "  <-- FAIL" }
        );
    }
    if failures == 0 {
        println!(
            "\nall {} valgrind gates caught the bad variant",
            CHECKS.len()
        );
        Ok(())
    } else {
        Err(format!("{failures} valgrind gate(s) did not fire"))
    }
}

const USAGE: &str = "\
usage: perf_drills <command>

  list                          list workloads and good/bad pairs
  bench                         wall-clock benchmark every pair, with allocation counts
  run <workload> [reps]         run one workload (a target for valgrind)
  callgrind <file> [top]        report a callgrind or cachegrind output file
  cachegrind <file> [top]       same as `callgrind`
  dhat <file> [top]             report a DHAT (valgrind or dhat-rs) JSON file
  valgrind-check                profile each pair under valgrind and gate the bad variant";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let result = match args.as_slice() {
        ["list"] => {
            for (name, _) in WORKLOADS {
                println!("{name}");
            }
            println!();
            for (good, bad) in PAIRS {
                println!("{good} vs {bad}");
            }
            Ok(())
        }
        ["bench"] => cmd_bench(),
        ["run", name] => cmd_run(name, "1"),
        ["run", name, reps] => cmd_run(name, reps),
        ["callgrind" | "cachegrind", path] => cmd_cost_report(path, "10"),
        ["callgrind" | "cachegrind", path, top] => cmd_cost_report(path, top),
        ["dhat", path] => cmd_dhat_report(path, "10"),
        ["dhat", path, top] => cmd_dhat_report(path, top),
        ["valgrind-check"] => cmd_valgrind_check(),
        _ => Err(USAGE.to_owned()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}
