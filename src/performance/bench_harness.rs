//! # Benchmark Design
//!
//! A tiny Criterion-shaped harness, built so that every classic benchmarking
//! mistake can be *demonstrated by a test* instead of described in prose.
//!
//! The four rules a benchmark has to get right:
//!
//! 1. **Keep the work alive.** The optimizer deletes computations whose results
//!    are unused. Every routine result goes through [`std::hint::black_box`].
//! 2. **Beat the timer resolution.** A 5 ns operation timed one call at a time
//!    measures the clock, not the code. [`calibrate`] doubles the batch size until
//!    one batch takes at least `target_sample_ns`, and reports time *per iteration*.
//! 3. **Don't time the setup.** Building inputs inside the timed region inflates
//!    the result. [`bench_batched`] builds a batch of inputs first, then starts the
//!    clock, and drops the outputs after stopping it.
//! 4. **Report a distribution, not a number.** One run is an anecdote. [`Stats`]
//!    keeps min / median / mean / MAD / percentiles and Tukey outlier counts,
//!    because the median and MAD survive the occasional context switch while the
//!    mean does not.
//!
//! The clock is a trait ([`Clock`]), so tests drive the harness with a
//! [`ManualClock`] whose time only moves when the routine says so, which keeps
//! every test exact and deterministic. Real measurements use [`MonotonicClock`].
//!
//! ```
//! use rust_interview_practice::performance::bench_harness::{bench, BenchConfig, ManualClock};
//!
//! let clock = ManualClock::new(1);
//! // Every call to the routine "costs" exactly 40 ns of virtual time.
//! let result = bench(&clock, &BenchConfig::default(), "add", || {
//!     clock.advance(40);
//!     2 + 2
//! });
//! assert_eq!(result.stats.median, 40.0);
//! assert!(result.iters_per_sample > 1); // batched to beat the timer
//! ```

use std::cell::Cell;
use std::hint::black_box;
use std::num::NonZeroUsize;
use std::time::Instant;

// ============================================================================
// Clocks
// ============================================================================

/// A source of monotonically non-decreasing nanosecond timestamps.
pub trait Clock {
    /// Current time in nanoseconds since an arbitrary, fixed origin.
    fn now_ns(&self) -> u64;
}

/// Wall-clock time from [`Instant`]. Use this for real measurements.
#[derive(Debug, Clone, Copy)]
pub struct MonotonicClock {
    origin: Instant,
}

impl MonotonicClock {
    /// Creates a clock whose origin is "now".
    #[must_use]
    pub fn new() -> Self {
        Self {
            origin: Instant::now(),
        }
    }
}

impl Default for MonotonicClock {
    fn default() -> Self {
        Self::new()
    }
}

impl Clock for MonotonicClock {
    #[allow(clippy::cast_possible_truncation)] // u64 nanoseconds cover ~584 years
    fn now_ns(&self) -> u64 {
        self.origin.elapsed().as_nanos() as u64
    }
}

/// A virtual clock that only moves when [`ManualClock::advance`] is called.
///
/// `resolution_ns` models a coarse timer: [`Clock::now_ns`] rounds the true
/// virtual time *down* to a multiple of it, exactly like a real timer that ticks
/// every `resolution_ns`.
#[derive(Debug)]
pub struct ManualClock {
    now: Cell<u64>,
    resolution_ns: u64,
}

impl ManualClock {
    /// Creates a clock at time zero. A `resolution_ns` of 0 is treated as 1.
    #[must_use]
    pub const fn new(resolution_ns: u64) -> Self {
        Self {
            now: Cell::new(0),
            resolution_ns: if resolution_ns == 0 { 1 } else { resolution_ns },
        }
    }

    /// Moves virtual time forward by `ns`.
    pub fn advance(&self, ns: u64) {
        self.now.set(self.now.get().saturating_add(ns));
    }

    /// The exact virtual time, ignoring resolution.
    #[must_use]
    pub const fn true_now_ns(&self) -> u64 {
        self.now.get()
    }
}

impl Clock for ManualClock {
    fn now_ns(&self) -> u64 {
        let t = self.now.get();
        t - t % self.resolution_ns
    }
}

// ============================================================================
// Statistics
// ============================================================================

/// Summary statistics for a set of per-iteration timings (nanoseconds).
#[derive(Debug, Clone, PartialEq)]
pub struct Stats {
    /// Number of samples.
    pub n: usize,
    /// Smallest sample.
    pub min: f64,
    /// Largest sample.
    pub max: f64,
    /// Arithmetic mean. Sensitive to outliers.
    pub mean: f64,
    /// Median (50th percentile). Robust to outliers.
    pub median: f64,
    /// Sample standard deviation (n - 1 denominator). 0 for a single sample.
    pub std_dev: f64,
    /// Median absolute deviation from the median. A robust spread measure.
    pub mad: f64,
    /// 25th percentile.
    pub q1: f64,
    /// 75th percentile.
    pub q3: f64,
    /// 95th percentile.
    pub p95: f64,
    /// Samples outside the 1.5 x IQR Tukey fences (includes severe ones).
    pub mild_outliers: usize,
    /// Samples outside the 3 x IQR Tukey fences.
    pub severe_outliers: usize,
}

impl Stats {
    /// Computes statistics, or `None` for an empty slice.
    ///
    /// Time: O(n log n) - sorting. Space: O(n) - a sorted copy.
    #[must_use]
    #[allow(clippy::cast_precision_loss)] // sample counts are far below 2^52
    pub fn from_samples(samples: &[f64]) -> Option<Self> {
        if samples.is_empty() {
            return None;
        }
        let mut sorted = samples.to_vec();
        sorted.sort_by(f64::total_cmp);
        let n = sorted.len();
        let mean = sorted.iter().sum::<f64>() / n as f64;
        let std_dev = if n > 1 {
            let var = sorted.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (n - 1) as f64;
            var.sqrt()
        } else {
            0.0
        };
        let median = percentile_sorted(&sorted, 0.5);
        let mut deviations: Vec<f64> = sorted.iter().map(|x| (x - median).abs()).collect();
        deviations.sort_by(f64::total_cmp);
        let mad = percentile_sorted(&deviations, 0.5);
        let q1 = percentile_sorted(&sorted, 0.25);
        let q3 = percentile_sorted(&sorted, 0.75);
        let iqr = q3 - q1;
        let outside = |k: f64| {
            sorted
                .iter()
                .filter(|&&x| x < k.mul_add(-iqr, q1) || x > k.mul_add(iqr, q3))
                .count()
        };
        Some(Self {
            n,
            min: sorted[0],
            max: sorted[n - 1],
            mean,
            median,
            std_dev,
            mad,
            q1,
            q3,
            p95: percentile_sorted(&sorted, 0.95),
            mild_outliers: outside(1.5),
            severe_outliers: outside(3.0),
        })
    }
}

/// Linear-interpolated percentile of an already-sorted, non-empty slice.
///
/// `p` is clamped to `0.0..=1.0`. Uses the "type 7" definition (the default in
/// R and `NumPy`): rank `p * (n - 1)`, interpolating between neighbours.
///
/// # Panics
///
/// Panics if `sorted` is empty.
#[must_use]
#[allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)] // rank is in 0..n, non-negative, and n is far below 2^52
pub fn percentile_sorted(sorted: &[f64], p: f64) -> f64 {
    assert!(!sorted.is_empty(), "percentile of an empty slice");
    let p = p.clamp(0.0, 1.0);
    let rank = p * (sorted.len() - 1) as f64;
    let lo = rank.floor() as usize;
    let hi = rank.ceil() as usize;
    let frac = rank - lo as f64;
    (sorted[hi] - sorted[lo]).mul_add(frac, sorted[lo])
}

// ============================================================================
// Harness
// ============================================================================

/// How a benchmark is run.
#[derive(Debug, Clone)]
pub struct BenchConfig {
    /// Untimed calls before measuring (fills caches, trains branch predictors).
    pub warmup_iters: u64,
    /// Number of timed samples. Each sample is one batch of iterations.
    pub samples: NonZeroUsize,
    /// Calibration grows a batch until it takes at least this long.
    pub target_sample_ns: u64,
    /// Upper bound on the batch size (keeps a near-zero-cost routine finite).
    pub max_iters_per_sample: u64,
}

impl Default for BenchConfig {
    fn default() -> Self {
        Self {
            warmup_iters: 8,
            samples: NonZeroUsize::new(20).unwrap_or(NonZeroUsize::MIN),
            target_sample_ns: 10_000,
            max_iters_per_sample: 1 << 20,
        }
    }
}

/// The outcome of one benchmark.
#[derive(Debug, Clone)]
pub struct BenchResult {
    /// Benchmark name.
    pub name: String,
    /// Iterations per timed batch, as chosen by [`calibrate`].
    pub iters_per_sample: u64,
    /// Per-iteration time of each sample, in nanoseconds.
    pub samples: Vec<f64>,
    /// Summary of `samples`.
    pub stats: Stats,
}

impl BenchResult {
    #[allow(clippy::cast_precision_loss)] // batch sizes are far below 2^52
    fn from_batches(name: &str, iters: u64, batch_ns: &[u64]) -> Self {
        let samples: Vec<f64> = batch_ns
            .iter()
            .map(|&ns| ns as f64 / iters as f64)
            .collect();
        let Some(stats) = Stats::from_samples(&samples) else {
            unreachable!("BenchConfig::samples is NonZeroUsize, so there is at least one sample")
        };
        Self {
            name: name.to_owned(),
            iters_per_sample: iters,
            samples,
            stats,
        }
    }
}

/// Times `iters` calls of `routine` as one batch.
fn time_batch<C: Clock, T, F: FnMut() -> T>(clock: &C, iters: u64, routine: &mut F) -> u64 {
    let start = clock.now_ns();
    for _ in 0..iters {
        black_box(routine());
    }
    clock.now_ns().saturating_sub(start)
}

/// Finds a batch size whose run time reaches `config.target_sample_ns`.
///
/// Starts at one iteration and doubles. This is what makes sub-resolution
/// routines measurable: with a 1 µs timer and a 10 ns routine, a batch of 128
/// takes 1.28 µs and its per-iteration share is accurate to within ~1%.
///
/// Time: `O(log(max_iters_per_sample))` batches.
pub fn calibrate<C: Clock, T, F: FnMut() -> T>(
    clock: &C,
    config: &BenchConfig,
    routine: &mut F,
) -> u64 {
    let cap = config.max_iters_per_sample.max(1);
    let mut iters = 1;
    loop {
        let elapsed = time_batch(clock, iters, routine);
        if elapsed >= config.target_sample_ns || iters >= cap {
            return iters;
        }
        iters = iters.saturating_mul(2).min(cap);
    }
}

/// Benchmarks `routine`: warm up, calibrate, then take `config.samples` batches.
///
/// The routine's return value is passed through [`black_box`] so it cannot be
/// optimized away. Return the thing you computed, not `()`.
pub fn bench<C: Clock, T, F: FnMut() -> T>(
    clock: &C,
    config: &BenchConfig,
    name: &str,
    mut routine: F,
) -> BenchResult {
    for _ in 0..config.warmup_iters {
        black_box(routine());
    }
    let iters = calibrate(clock, config, &mut routine);
    let batches: Vec<u64> = (0..config.samples.get())
        .map(|_| time_batch(clock, iters, &mut routine))
        .collect();
    BenchResult::from_batches(name, iters, &batches)
}

/// Benchmarks `routine` on fresh inputs from `setup`, timing only `routine`.
///
/// For each sample: build `iters` inputs (untimed), start the clock, run the
/// routine on each input, stop the clock, *then* drop the outputs. Use this when
/// the routine consumes or mutates its input (sorting, draining, parsing).
pub fn bench_batched<C, I, T, S, R>(
    clock: &C,
    config: &BenchConfig,
    name: &str,
    mut setup: S,
    mut routine: R,
) -> BenchResult
where
    C: Clock,
    S: FnMut() -> I,
    R: FnMut(I) -> T,
{
    for _ in 0..config.warmup_iters {
        black_box(routine(setup()));
    }
    let mut timed_batch = |iters: u64| {
        let inputs: Vec<I> = (0..iters).map(|_| setup()).collect();
        let mut outputs: Vec<T> = Vec::with_capacity(inputs.len());
        let start = clock.now_ns();
        for input in inputs {
            outputs.push(black_box(routine(input)));
        }
        let elapsed = clock.now_ns().saturating_sub(start);
        drop(outputs); // destructors run outside the timed region
        elapsed
    };
    let cap = config.max_iters_per_sample.max(1);
    let mut iters = 1;
    while iters < cap && timed_batch(iters) < config.target_sample_ns {
        iters = iters.saturating_mul(2).min(cap);
    }
    let batches: Vec<u64> = (0..config.samples.get())
        .map(|_| timed_batch(iters))
        .collect();
    BenchResult::from_batches(name, iters, &batches)
}

// ============================================================================
// Deliberately wrong harnesses (each test shows what they get wrong)
// ============================================================================

/// BUG: times every call individually, so a routine faster than the timer's
/// resolution measures as either 0 or one full tick.
pub fn bench_single_shot_buggy<C: Clock, T, F: FnMut() -> T>(
    clock: &C,
    samples: NonZeroUsize,
    name: &str,
    mut routine: F,
) -> BenchResult {
    let batches: Vec<u64> = (0..samples.get())
        .map(|_| time_batch(clock, 1, &mut routine))
        .collect();
    BenchResult::from_batches(name, 1, &batches)
}

/// BUG: builds the input inside the timed region, so the result is
/// `setup + routine` rather than `routine`.
pub fn bench_with_setup_buggy<C, I, T, S, R>(
    clock: &C,
    config: &BenchConfig,
    name: &str,
    mut setup: S,
    mut routine: R,
) -> BenchResult
where
    C: Clock,
    S: FnMut() -> I,
    R: FnMut(I) -> T,
{
    bench(clock, config, name, || routine(setup()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    fn samples(n: usize) -> NonZeroUsize {
        NonZeroUsize::new(n).unwrap()
    }

    #[test]
    fn test_stats_of_known_data() {
        let s = Stats::from_samples(&[5.0, 1.0, 3.0, 2.0, 4.0]).unwrap();
        assert_eq!(s.n, 5);
        assert!(close(s.min, 1.0));
        assert!(close(s.max, 5.0));
        assert!(close(s.mean, 3.0));
        assert!(close(s.median, 3.0));
        assert!(close(s.q1, 2.0));
        assert!(close(s.q3, 4.0));
        assert!(close(s.mad, 1.0));
        assert!(close(s.std_dev, 2.5_f64.sqrt()));
        assert_eq!(s.mild_outliers, 0);
    }

    #[test]
    fn test_stats_single_sample() {
        let s = Stats::from_samples(&[7.0]).unwrap();
        assert!(close(s.median, 7.0));
        assert!(close(s.std_dev, 0.0));
        assert!(close(s.mad, 0.0));
        assert!(close(s.p95, 7.0));
    }

    #[test]
    fn test_stats_empty_is_none() {
        assert!(Stats::from_samples(&[]).is_none());
    }

    #[test]
    fn test_percentile_interpolates() {
        let sorted = [10.0, 20.0, 30.0, 40.0];
        assert!(close(percentile_sorted(&sorted, 0.0), 10.0));
        assert!(close(percentile_sorted(&sorted, 1.0), 40.0));
        assert!(close(percentile_sorted(&sorted, 0.5), 25.0));
        assert!(close(percentile_sorted(&sorted, 2.0), 40.0)); // clamped
    }

    #[test]
    fn test_outlier_moves_mean_not_median() {
        let mut data = vec![100.0; 19];
        data.push(10_000.0); // one context switch
        let s = Stats::from_samples(&data).unwrap();
        assert!(close(s.median, 100.0));
        assert!(close(s.mad, 0.0));
        assert!(s.mean > 590.0);
        assert_eq!(s.severe_outliers, 1);
    }

    #[test]
    fn test_manual_clock_resolution_rounds_down() {
        let clock = ManualClock::new(1_000);
        clock.advance(999);
        assert_eq!(clock.now_ns(), 0);
        clock.advance(1);
        assert_eq!(clock.now_ns(), 1_000);
        assert_eq!(clock.true_now_ns(), 1_000);
        assert_eq!(ManualClock::new(0).now_ns(), 0); // resolution 0 treated as 1
    }

    #[test]
    fn test_calibrate_reaches_target() {
        let clock = ManualClock::new(1);
        let config = BenchConfig {
            target_sample_ns: 1_000,
            ..BenchConfig::default()
        };
        let iters = calibrate(&clock, &config, &mut || clock.advance(10));
        assert_eq!(iters, 128); // 64 * 10 = 640 < 1000 <= 128 * 10
    }

    #[test]
    fn test_calibrate_respects_cap() {
        let clock = ManualClock::new(1);
        let config = BenchConfig {
            max_iters_per_sample: 16,
            ..BenchConfig::default()
        };
        // Zero-cost routine never reaches the target; the cap stops the loop.
        assert_eq!(calibrate(&clock, &config, &mut || 0_u8), 16);
    }

    #[test]
    fn test_bench_reports_exact_per_iteration_cost() {
        let clock = ManualClock::new(1);
        let r = bench(&clock, &BenchConfig::default(), "fixed", || {
            clock.advance(25);
        });
        assert_eq!(r.name, "fixed");
        assert_eq!(r.samples.len(), 20);
        assert!(r.samples.iter().all(|&s| close(s, 25.0)));
        assert!(close(r.stats.median, 25.0));
    }

    #[test]
    fn test_warmup_runs_routine_untimed() {
        let clock = ManualClock::new(1);
        let calls = Cell::new(0_u64);
        let config = BenchConfig {
            warmup_iters: 5,
            samples: samples(1),
            target_sample_ns: 1,
            ..BenchConfig::default()
        };
        let r = bench(&clock, &config, "count", || {
            calls.set(calls.get() + 1);
            clock.advance(1);
        });
        // 5 warmup + 1 calibration batch of 1 + 1 sample batch of 1
        assert_eq!(calls.get(), 7);
        assert_eq!(r.iters_per_sample, 1);
    }

    #[test]
    fn test_coarse_timer_breaks_single_shot_but_not_batching() {
        // Timer ticks every 1000 ns; the routine costs 10 ns.
        let clock = ManualClock::new(1_000);
        let single = bench_single_shot_buggy(&clock, samples(200), "single", || clock.advance(10));
        // 200 calls span 2000 ns of virtual time: 198 samples see no tick (0 ns)
        // and the 2 that straddle a tick boundary see a full 1000 ns.
        assert!(close(single.stats.median, 0.0));
        assert!(close(single.stats.max, 1_000.0));

        let clock = ManualClock::new(1_000);
        let config = BenchConfig {
            target_sample_ns: 100_000,
            ..BenchConfig::default()
        };
        let batched = bench(&clock, &config, "batched", || clock.advance(10));
        assert!(
            (batched.stats.median - 10.0).abs() < 0.2,
            "{}",
            batched.stats.median
        );
    }

    #[test]
    fn test_batched_excludes_setup_cost() {
        let clock = ManualClock::new(1);
        let config = BenchConfig::default();
        let setup = || {
            clock.advance(100); // expensive input construction
            vec![3, 1, 2]
        };
        let routine = |mut v: Vec<i32>| {
            clock.advance(10);
            v.sort_unstable();
            v
        };
        let good = bench_batched(&clock, &config, "sort", setup, routine);
        assert!(close(good.stats.median, 10.0));

        let bad = bench_with_setup_buggy(&clock, &config, "sort", setup, routine);
        assert!(close(bad.stats.median, 110.0));
    }

    #[test]
    fn test_bench_with_real_clock_happy_path() {
        let clock = MonotonicClock::new();
        let config = BenchConfig {
            samples: samples(5),
            target_sample_ns: 50_000,
            ..BenchConfig::default()
        };
        let data: Vec<u64> = (0..1_000).collect();
        let r = bench(&clock, &config, "sum", || {
            black_box(&data).iter().sum::<u64>()
        });
        assert_eq!(r.samples.len(), 5);
        assert!(r.stats.min > 0.0);
        assert!(r.stats.min <= r.stats.median && r.stats.median <= r.stats.max);
    }
}

#[cfg(all(test, feature = "testing-extras"))]
mod proptests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn prop_order_statistics_are_ordered(data in prop::collection::vec(0.0_f64..1e9, 1..200)) {
            let s = Stats::from_samples(&data).unwrap();
            prop_assert!(s.min <= s.q1 && s.q1 <= s.median);
            prop_assert!(s.median <= s.q3 && s.q3 <= s.p95 && s.p95 <= s.max);
            prop_assert!(s.min <= s.mean && s.mean <= s.max);
            prop_assert!(s.mad >= 0.0 && s.std_dev >= 0.0);
            prop_assert!(s.severe_outliers <= s.mild_outliers);
        }

        #[test]
        fn prop_stats_are_permutation_invariant(mut data in prop::collection::vec(0.0_f64..1e6, 1..100)) {
            let a = Stats::from_samples(&data).unwrap();
            data.reverse();
            let b = Stats::from_samples(&data).unwrap();
            prop_assert_eq!(a.median.to_bits(), b.median.to_bits());
            prop_assert_eq!(a.mad.to_bits(), b.mad.to_bits());
            prop_assert_eq!(a.min.to_bits(), b.min.to_bits());
        }
    }
}
