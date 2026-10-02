//! # Regression Detection
//!
//! A benchmark is only useful if something *acts* on it. This module is the
//! "did this change make it slower?" half: given a baseline and a candidate,
//! return a [`Verdict`] that is robust to noise.
//!
//! Three layers, from cheapest to strongest:
//!
//! 1. [`compare_summaries`] - median change against a relative threshold, with a
//!    noise floor of `k x MAD`. Works from a stored baseline file ([`Baseline`]).
//! 2. [`compare_samples`] - adds a Mann-Whitney U rank test ([`mann_whitney_u`]),
//!    so a change must be both *statistically real* and *big enough to care*.
//! 3. [`bootstrap_median_ratio_ci`] - a seeded bootstrap confidence interval for
//!    `candidate / baseline`, for when you need to say *how much* slower.
//!
//! And one rule that beats all three for CI gates: **gate on counts, not time**.
//! [`compare_counts`] compares deterministic metrics (allocations from
//! [`alloc_accounting`](super::alloc_accounting), cache misses from
//! [`cache_locality`](super::cache_locality)) exactly, with zero noise.
//!
//! The classic mistake is comparing means: [`compare_means_buggy`] flags a
//! "regression" from a single context switch. The tests show it.
//!
//! ```
//! use rust_interview_practice::performance::regression::{compare_samples, Thresholds, Verdict};
//!
//! let baseline: Vec<f64> = (0..30).map(|i| 100.0 + f64::from(i % 5)).collect();
//! let slower: Vec<f64> = baseline.iter().map(|x| x * 1.25).collect();
//! let t = Thresholds::default();
//! assert_eq!(compare_samples(&baseline, &slower, &t).verdict, Verdict::Regressed);
//! assert_eq!(compare_samples(&baseline, &baseline, &t).verdict, Verdict::Unchanged);
//! ```

use std::collections::BTreeMap;
use std::fmt::{self, Write as _};

use super::bench_harness::{Stats, percentile_sorted};
use crate::testing_craft::sim_rng::SimRng;

/// Scales a MAD to a standard-deviation estimate for normally distributed data.
pub const MAD_TO_SIGMA: f64 = 1.4826;

// ============================================================================
// Verdicts and thresholds
// ============================================================================

/// The outcome of comparing a candidate against a baseline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// Faster by more than the threshold.
    Improved,
    /// Slower by more than the threshold.
    Regressed,
    /// No change worth reporting (within noise, or too small to matter).
    Unchanged,
    /// Not enough data to decide.
    Inconclusive,
}

impl fmt::Display for Verdict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Improved => "improved",
            Self::Regressed => "REGRESSED",
            Self::Unchanged => "unchanged",
            Self::Inconclusive => "inconclusive",
        })
    }
}

/// How large and how certain a change must be to count.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Thresholds {
    /// Relative median change that matters, e.g. `0.05` = 5%.
    pub max_change: f64,
    /// Changes smaller than `noise_sigmas x (MAD x 1.4826)` are noise.
    pub noise_sigmas: f64,
    /// Significance level for the rank test.
    pub alpha: f64,
    /// Fewer samples than this on either side is [`Verdict::Inconclusive`].
    pub min_samples: usize,
}

impl Default for Thresholds {
    fn default() -> Self {
        Self {
            max_change: 0.05,
            noise_sigmas: 3.0,
            alpha: 0.01,
            min_samples: 5,
        }
    }
}

/// The robust summary a baseline needs to store: median, MAD, sample count.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Summary {
    /// Median, nanoseconds per iteration.
    pub median: f64,
    /// Median absolute deviation.
    pub mad: f64,
    /// Number of samples behind the summary.
    pub n: usize,
}

impl From<&Stats> for Summary {
    fn from(s: &Stats) -> Self {
        Self {
            median: s.median,
            mad: s.mad,
            n: s.n,
        }
    }
}

/// The detail behind a [`Verdict`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Comparison {
    /// Baseline median.
    pub baseline: f64,
    /// Candidate median.
    pub candidate: f64,
    /// `(candidate - baseline) / baseline`; `0.0` when undefined.
    pub change: f64,
    /// Two-sided rank-test p-value, when samples were available.
    pub p_value: Option<f64>,
    /// The decision.
    pub verdict: Verdict,
}

fn classify(change: f64, max_change: f64) -> Verdict {
    if change > max_change {
        Verdict::Regressed
    } else if change < -max_change {
        Verdict::Improved
    } else {
        Verdict::Unchanged
    }
}

/// Compares medians with a relative threshold and a MAD-based noise floor.
///
/// Time: O(1).
#[must_use]
pub fn compare_summaries(baseline: Summary, candidate: Summary, t: &Thresholds) -> Comparison {
    let mut out = Comparison {
        baseline: baseline.median,
        candidate: candidate.median,
        change: 0.0,
        p_value: None,
        verdict: Verdict::Inconclusive,
    };
    if baseline.n < t.min_samples || candidate.n < t.min_samples || baseline.median <= 0.0 {
        return out;
    }
    let delta = candidate.median - baseline.median;
    out.change = delta / baseline.median;
    let noise = t.noise_sigmas * MAD_TO_SIGMA * baseline.mad.max(candidate.mad);
    out.verdict = if delta.abs() <= noise {
        Verdict::Unchanged
    } else {
        classify(out.change, t.max_change)
    };
    out
}

/// Compares raw samples: a change must pass the Mann-Whitney U test at
/// `t.alpha` *and* move the median by more than `t.max_change`.
///
/// Time: O((n + m) log(n + m)).
#[must_use]
pub fn compare_samples(baseline: &[f64], candidate: &[f64], t: &Thresholds) -> Comparison {
    let (Some(b), Some(c)) = (
        Stats::from_samples(baseline),
        Stats::from_samples(candidate),
    ) else {
        return Comparison {
            baseline: f64::NAN,
            candidate: f64::NAN,
            change: 0.0,
            p_value: None,
            verdict: Verdict::Inconclusive,
        };
    };
    let mut out = Comparison {
        baseline: b.median,
        candidate: c.median,
        change: 0.0,
        p_value: None,
        verdict: Verdict::Inconclusive,
    };
    if b.n < t.min_samples || c.n < t.min_samples || b.median <= 0.0 {
        return out;
    }
    out.change = (c.median - b.median) / b.median;
    let p = mann_whitney_u(baseline, candidate).map_or(1.0, |r| r.p_value);
    out.p_value = Some(p);
    out.verdict = if p >= t.alpha {
        Verdict::Unchanged
    } else {
        classify(out.change, t.max_change)
    };
    out
}

/// BUG: compares means. One 100x outlier (a context switch, a page fault) in
/// the candidate moves its mean far enough to be called a regression.
#[must_use]
#[allow(clippy::cast_precision_loss)] // sample counts are far below 2^52
pub fn compare_means_buggy(baseline: &[f64], candidate: &[f64], max_change: f64) -> Verdict {
    if baseline.is_empty() || candidate.is_empty() {
        return Verdict::Inconclusive;
    }
    let mean = |xs: &[f64]| xs.iter().sum::<f64>() / xs.len() as f64;
    let base = mean(baseline);
    if base <= 0.0 {
        return Verdict::Inconclusive;
    }
    classify((mean(candidate) - base) / base, max_change)
}

/// Exact comparison of a deterministic counter (allocations, cache misses,
/// instructions).
///
/// Any increase beyond `max_increase` (relative) is a
/// regression; any decrease is an improvement. No noise floor is needed.
#[must_use]
#[allow(clippy::cast_precision_loss)] // counters far below 2^52 in practice
pub fn compare_counts(baseline: u64, current: u64, max_increase: f64) -> Verdict {
    if current < baseline {
        Verdict::Improved
    } else if current as f64 > baseline as f64 * (1.0 + max_increase) {
        Verdict::Regressed
    } else {
        Verdict::Unchanged
    }
}

// ============================================================================
// Mann-Whitney U
// ============================================================================

/// Result of a two-sided Mann-Whitney U test (normal approximation).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RankTest {
    /// U statistic for the first sample.
    pub u: f64,
    /// Standardized statistic (with continuity and tie correction).
    pub z: f64,
    /// Two-sided p-value.
    pub p_value: f64,
}

/// Mann-Whitney U test: are values from `a` systematically smaller or larger
/// than values from `b`? Uses ranks, so it ignores *how far* an outlier is,
/// which is exactly the robustness timing data needs.
///
/// Returns `None` if either sample is empty.
///
/// Time: O((n + m) log(n + m)).
#[must_use]
#[allow(clippy::cast_precision_loss)] // sample counts are far below 2^52
#[allow(clippy::many_single_char_names)] // standard statistical notation (n, u, z, ...)
pub fn mann_whitney_u(a: &[f64], b: &[f64]) -> Option<RankTest> {
    if a.is_empty() || b.is_empty() {
        return None;
    }
    let (n1, n2) = (a.len() as f64, b.len() as f64);
    let mut all: Vec<(f64, bool)> = a
        .iter()
        .map(|&x| (x, true))
        .chain(b.iter().map(|&x| (x, false)))
        .collect();
    all.sort_by(|x, y| x.0.total_cmp(&y.0));

    let mut rank_sum_a = 0.0;
    let mut tie_term = 0.0;
    let mut i = 0;
    while i < all.len() {
        let mut j = i;
        while j + 1 < all.len() && all[j + 1].0.total_cmp(&all[i].0).is_eq() {
            j += 1;
        }
        let avg_rank = (i + j) as f64 / 2.0 + 1.0; // ranks are 1-based
        let ties = (j - i + 1) as f64;
        tie_term += ties.powi(3) - ties;
        rank_sum_a += avg_rank * all[i..=j].iter().filter(|e| e.1).count() as f64;
        i = j + 1;
    }

    let u = rank_sum_a - n1 * (n1 + 1.0) / 2.0;
    let n = n1 + n2;
    let mu = n1 * n2 / 2.0;
    let variance = n1 * n2 / 12.0 * ((n + 1.0) - tie_term / (n * (n - 1.0)));
    if variance <= 0.0 {
        // Every value identical: no evidence of any difference.
        return Some(RankTest {
            u,
            z: 0.0,
            p_value: 1.0,
        });
    }
    let diff = u - mu;
    let corrected = (diff.abs() - 0.5).max(0.0).copysign(diff);
    let z = corrected / variance.sqrt();
    let p_value = erfc(z.abs() / std::f64::consts::SQRT_2).min(1.0);
    Some(RankTest { u, z, p_value })
}

/// Complementary error function, fractional error < 1.2e-7
/// (Numerical Recipes' Chebyshev fit).
#[must_use]
pub fn erfc(x: f64) -> f64 {
    let z = x.abs();
    let t = 1.0 / 0.5_f64.mul_add(z, 1.0);
    let poly = t.mul_add(0.170_872_77, -0.822_152_23);
    let poly = t.mul_add(poly, 1.488_515_87);
    let poly = t.mul_add(poly, -1.135_203_98);
    let poly = t.mul_add(poly, 0.278_868_07);
    let poly = t.mul_add(poly, -0.186_288_06);
    let poly = t.mul_add(poly, 0.096_784_18);
    let poly = t.mul_add(poly, 0.374_091_96);
    let poly = t.mul_add(poly, 1.000_023_68);
    let poly = t.mul_add(poly, -1.265_512_23);
    let r = t * (-z).mul_add(z, poly).exp();
    if x >= 0.0 { r } else { 2.0 - r }
}

// ============================================================================
// Bootstrap
// ============================================================================

/// A `confidence` (e.g. `0.95`) percentile-bootstrap interval for
/// `median(candidate) / median(baseline)`.
///
/// Draws `resamples` resamples with a [`SimRng`] seeded by `seed`, so the
/// interval is reproducible.
///
/// An interval entirely above 1.0 is a slowdown you can quote.
///
/// Returns `None` for empty input, zero resamples, or a zero baseline median.
#[must_use]
pub fn bootstrap_median_ratio_ci(
    baseline: &[f64],
    candidate: &[f64],
    resamples: usize,
    confidence: f64,
    seed: u64,
) -> Option<(f64, f64)> {
    if baseline.is_empty() || candidate.is_empty() || resamples == 0 {
        return None;
    }
    let mut rng = SimRng::new(seed);
    let mut scratch = Vec::with_capacity(baseline.len().max(candidate.len()));
    let mut resample_median = |xs: &[f64], rng: &mut SimRng| {
        scratch.clear();
        scratch.extend((0..xs.len()).map(|_| xs[rng.below(xs.len())]));
        scratch.sort_by(f64::total_cmp);
        percentile_sorted(&scratch, 0.5)
    };
    let mut ratios = Vec::with_capacity(resamples);
    for _ in 0..resamples {
        let b = resample_median(baseline, &mut rng);
        let c = resample_median(candidate, &mut rng);
        if b <= 0.0 {
            return None;
        }
        ratios.push(c / b);
    }
    ratios.sort_by(f64::total_cmp);
    let tail = (1.0 - confidence.clamp(0.0, 1.0)) / 2.0;
    Some((
        percentile_sorted(&ratios, tail),
        percentile_sorted(&ratios, 1.0 - tail),
    ))
}

// ============================================================================
// Baselines and suite reports
// ============================================================================

/// A benchmark name: non-empty, no whitespace, not starting with `#`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BenchName(String);

impl BenchName {
    /// Validates a name.
    ///
    /// # Errors
    ///
    /// Returns [`BaselineError::InvalidName`] if the name is empty, contains
    /// whitespace, or starts with `#` (the comment marker).
    pub fn new(name: &str) -> Result<Self, BaselineError> {
        if name.is_empty() || name.starts_with('#') || name.chars().any(char::is_whitespace) {
            return Err(BaselineError::InvalidName(name.to_owned()));
        }
        Ok(Self(name.to_owned()))
    }

    /// The name as a string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for BenchName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Errors from building or parsing a [`Baseline`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BaselineError {
    /// A benchmark name broke the [`BenchName`] rules.
    InvalidName(String),
    /// A line of a baseline file did not parse.
    Parse {
        /// 1-based line number.
        line: usize,
        /// What was wrong.
        reason: String,
    },
}

impl fmt::Display for BaselineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidName(name) => write!(f, "invalid benchmark name {name:?}"),
            Self::Parse { line, reason } => write!(f, "line {line}: {reason}"),
        }
    }
}

impl std::error::Error for BaselineError {}

/// Stored benchmark summaries, keyed by name. Serializes to a plain text file
/// (`name median mad n` per line) that diffs well in code review.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Baseline {
    entries: BTreeMap<BenchName, Summary>,
}

impl Baseline {
    /// An empty baseline.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Records (or replaces) a summary.
    pub fn record(&mut self, name: BenchName, summary: Summary) {
        self.entries.insert(name, summary);
    }

    /// Looks up a summary.
    #[must_use]
    pub fn get(&self, name: &BenchName) -> Option<&Summary> {
        self.entries.get(name)
    }

    /// Number of entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether there are no entries.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Serializes to text. Rust's `f64` `Display` is shortest-round-trip, so
    /// [`Baseline::parse`] reads back exactly the same numbers.
    #[must_use]
    pub fn to_text(&self) -> String {
        let mut out = String::from("# name median_ns mad_ns samples\n");
        for (name, s) in &self.entries {
            // Writing to a `String` cannot fail.
            let _ = writeln!(out, "{name} {} {} {}", s.median, s.mad, s.n);
        }
        out
    }

    /// Parses text produced by [`Baseline::to_text`]. Blank lines and lines
    /// starting with `#` are ignored.
    ///
    /// # Errors
    ///
    /// Returns [`BaselineError::Parse`] with the 1-based line number of the first
    /// malformed line.
    pub fn parse(text: &str) -> Result<Self, BaselineError> {
        let mut baseline = Self::new();
        for (idx, raw) in text.lines().enumerate() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let err = |reason: &str| BaselineError::Parse {
                line: idx + 1,
                reason: reason.to_owned(),
            };
            let fields: Vec<&str> = line.split_whitespace().collect();
            let [name, median, mad, n] = fields[..] else {
                return Err(err("expected 4 fields: name median mad samples"));
            };
            let number = |s: &str, what: &str| {
                s.parse::<f64>()
                    .ok()
                    .filter(|x| x.is_finite() && *x >= 0.0)
                    .ok_or_else(|| err(&format!("{what} {s:?} is not a non-negative number")))
            };
            let summary = Summary {
                median: number(median, "median")?,
                mad: number(mad, "mad")?,
                n: n.parse()
                    .map_err(|_| err(&format!("samples {n:?} is not an integer")))?,
            };
            let name = BenchName::new(name).map_err(|e| err(&e.to_string()))?;
            if baseline.entries.insert(name, summary).is_some() {
                return Err(err("duplicate benchmark name"));
            }
        }
        Ok(baseline)
    }
}

/// One benchmark's line in a [`SuiteReport`].
#[derive(Debug, Clone, PartialEq)]
pub struct SuiteRow {
    /// Benchmark name.
    pub name: BenchName,
    /// The comparison against its baseline.
    pub comparison: Comparison,
}

/// A whole-suite comparison, ready to print in CI.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SuiteReport {
    /// Benchmarks present in both baseline and current run.
    pub rows: Vec<SuiteRow>,
    /// In the baseline but not the current run (deleted or renamed?).
    pub missing: Vec<BenchName>,
    /// In the current run but not the baseline (new; no verdict possible).
    pub added: Vec<BenchName>,
}

impl SuiteReport {
    /// Rows whose verdict is [`Verdict::Regressed`].
    pub fn regressions(&self) -> impl Iterator<Item = &SuiteRow> {
        self.rows
            .iter()
            .filter(|r| r.comparison.verdict == Verdict::Regressed)
    }

    /// `true` when nothing regressed: the CI gate.
    #[must_use]
    pub fn passed(&self) -> bool {
        self.regressions().next().is_none()
    }
}

impl fmt::Display for SuiteReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for row in &self.rows {
            let c = &row.comparison;
            writeln!(
                f,
                "{:<24} {:>12.1} -> {:>12.1} ns  {:>+7.1}%  {}",
                row.name.as_str(),
                c.baseline,
                c.candidate,
                c.change * 100.0,
                c.verdict
            )?;
        }
        for name in &self.missing {
            writeln!(f, "{:<24} missing from current run", name.as_str())?;
        }
        for name in &self.added {
            writeln!(f, "{:<24} new (no baseline)", name.as_str())?;
        }
        Ok(())
    }
}

/// Compares every benchmark in `current` against `baseline`.
///
/// Time: O(k log k) for k benchmarks.
#[must_use]
pub fn compare_suite(baseline: &Baseline, current: &Baseline, t: &Thresholds) -> SuiteReport {
    let mut report = SuiteReport::default();
    for (name, cur) in &current.entries {
        match baseline.get(name) {
            Some(base) => report.rows.push(SuiteRow {
                name: name.clone(),
                comparison: compare_summaries(*base, *cur, t),
            }),
            None => report.added.push(name.clone()),
        }
    }
    report.missing = baseline
        .entries
        .keys()
        .filter(|name| current.get(name).is_none())
        .cloned()
        .collect();
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::performance::alloc_accounting::{join_naive, join_presized, measure};
    use crate::performance::cache_locality::{
        CacheConfig, CacheSim, ColumnMajor, RowMajor, matrix_trace,
    };

    /// 30 samples around 100 ns with deterministic jitter.
    fn steady(seed: u64, center: f64) -> Vec<f64> {
        let mut rng = SimRng::new(seed);
        (0..30)
            .map(|_| center + f64::from(u32::try_from(rng.below(5)).unwrap()))
            .collect()
    }

    fn summary(median: f64, mad: f64, n: usize) -> Summary {
        Summary { median, mad, n }
    }

    #[test]
    fn test_erfc_known_values() {
        assert!((erfc(0.0) - 1.0).abs() < 1e-7);
        assert!((erfc(1.0) - 0.157_299_207).abs() < 1e-7);
        assert!((erfc(2.0) - 0.004_677_735).abs() < 1e-7);
        assert!((erfc(-1.0) - 1.842_700_793).abs() < 1e-7);
    }

    #[test]
    fn test_mann_whitney_fully_separated() {
        let r = mann_whitney_u(&[1.0, 2.0, 3.0, 4.0, 5.0], &[6.0, 7.0, 8.0, 9.0, 10.0]).unwrap();
        assert!((r.u - 0.0).abs() < f64::EPSILON);
        assert!(r.z < 0.0);
        // scipy.stats.mannwhitneyu(..., method="asymptotic") -> 0.01219
        assert!((r.p_value - 0.012_19).abs() < 1e-4, "{r:?}");
    }

    #[test]
    fn test_mann_whitney_identical_and_empty() {
        let r = mann_whitney_u(&[5.0; 10], &[5.0; 10]).unwrap();
        assert!((r.p_value - 1.0).abs() < f64::EPSILON);
        assert!(mann_whitney_u(&[], &[1.0]).is_none());
        let same = mann_whitney_u(&steady(1, 100.0), &steady(1, 100.0)).unwrap();
        assert!(same.p_value > 0.9);
    }

    #[test]
    fn test_mann_whitney_ignores_outlier_magnitude() {
        let base = steady(2, 100.0);
        let mut cand = steady(3, 100.0);
        cand[0] = 1e9;
        let r = mann_whitney_u(&base, &cand).unwrap();
        assert!(r.p_value > 0.05, "{r:?}");
    }

    #[test]
    fn test_compare_samples_detects_real_shifts() {
        let base = steady(4, 100.0);
        let t = Thresholds::default();
        let slow = compare_samples(&base, &steady(5, 120.0), &t);
        assert_eq!(slow.verdict, Verdict::Regressed);
        assert!(slow.change > 0.15 && slow.change < 0.25);
        assert!(slow.p_value.unwrap() < 1e-6);
        let fast = compare_samples(&base, &steady(6, 80.0), &t);
        assert_eq!(fast.verdict, Verdict::Improved);
    }

    #[test]
    fn test_compare_samples_ignores_tiny_but_significant_shift() {
        // A consistent +2% is statistically detectable but below the 5% bar.
        let base = steady(7, 100.0);
        let cand: Vec<f64> = base.iter().map(|x| x * 1.02).collect();
        let c = compare_samples(&base, &cand, &Thresholds::default());
        assert!(c.p_value.unwrap() < 0.05);
        assert_eq!(c.verdict, Verdict::Unchanged);
    }

    #[test]
    fn test_compare_samples_needs_enough_data() {
        let t = Thresholds::default();
        assert_eq!(
            compare_samples(&[1.0, 2.0], &[5.0, 6.0], &t).verdict,
            Verdict::Inconclusive
        );
        assert_eq!(
            compare_samples(&[], &[1.0], &t).verdict,
            Verdict::Inconclusive
        );
    }

    #[test]
    fn test_mean_comparison_is_fooled_by_one_outlier() {
        let base = steady(8, 100.0);
        let mut cand = steady(9, 100.0);
        cand[17] = 10_000.0; // one preempted sample
        assert_eq!(compare_means_buggy(&base, &cand, 0.05), Verdict::Regressed);
        let robust = compare_samples(&base, &cand, &Thresholds::default());
        assert_eq!(robust.verdict, Verdict::Unchanged);
        // Both agree on a real regression.
        let slow = steady(10, 130.0);
        assert_eq!(compare_means_buggy(&base, &slow, 0.05), Verdict::Regressed);
        assert_eq!(compare_means_buggy(&[], &slow, 0.05), Verdict::Inconclusive);
    }

    #[test]
    fn test_compare_summaries_noise_floor() {
        let t = Thresholds::default();
        // +10% but MAD is huge: within 3 sigma of noise.
        let noisy = compare_summaries(summary(100.0, 10.0, 30), summary(110.0, 10.0, 30), &t);
        assert_eq!(noisy.verdict, Verdict::Unchanged);
        assert!((noisy.change - 0.10).abs() < 1e-12);
        // Same +10% with tight MAD: real.
        let tight = compare_summaries(summary(100.0, 0.5, 30), summary(110.0, 0.5, 30), &t);
        assert_eq!(tight.verdict, Verdict::Regressed);
        let better = compare_summaries(summary(100.0, 0.5, 30), summary(90.0, 0.5, 30), &t);
        assert_eq!(better.verdict, Verdict::Improved);
        let few = compare_summaries(summary(100.0, 0.5, 3), summary(200.0, 0.5, 30), &t);
        assert_eq!(few.verdict, Verdict::Inconclusive);
        let zero = compare_summaries(summary(0.0, 0.0, 30), summary(1.0, 0.0, 30), &t);
        assert_eq!(zero.verdict, Verdict::Inconclusive);
    }

    #[test]
    fn test_bootstrap_ci_brackets_true_ratio() {
        let base = steady(11, 100.0);
        let (lo, hi) = bootstrap_median_ratio_ci(&base, &steady(12, 100.0), 500, 0.95, 1).unwrap();
        assert!(lo <= 1.0 && 1.0 <= hi, "({lo}, {hi})");
        let (lo, hi) = bootstrap_median_ratio_ci(&base, &steady(13, 120.0), 500, 0.95, 1).unwrap();
        assert!(lo > 1.1 && hi < 1.3, "({lo}, {hi})");
        // Seeded: identical inputs give an identical interval.
        assert_eq!(
            bootstrap_median_ratio_ci(&base, &base, 100, 0.9, 77),
            bootstrap_median_ratio_ci(&base, &base, 100, 0.9, 77)
        );
    }

    #[test]
    fn test_bootstrap_rejects_degenerate_input() {
        assert!(bootstrap_median_ratio_ci(&[], &[1.0], 10, 0.95, 0).is_none());
        assert!(bootstrap_median_ratio_ci(&[1.0], &[1.0], 0, 0.95, 0).is_none());
        assert!(bootstrap_median_ratio_ci(&[0.0], &[1.0], 10, 0.95, 0).is_none());
    }

    #[test]
    fn test_bench_name_validation() {
        assert!(BenchName::new("sum_row_major").is_ok());
        assert!(BenchName::new("").is_err());
        assert!(BenchName::new("has space").is_err());
        assert!(BenchName::new("#comment").is_err());
        assert_eq!(
            BenchName::new("a b").unwrap_err().to_string(),
            "invalid benchmark name \"a b\""
        );
    }

    fn name(s: &str) -> BenchName {
        BenchName::new(s).unwrap()
    }

    #[test]
    fn test_baseline_round_trips_exactly() {
        let mut b = Baseline::new();
        assert!(b.is_empty());
        b.record(name("sum_rows"), summary(1_234.567_891, 3.25, 20));
        b.record(name("sum_cols"), summary(0.1 + 0.2, 0.0, 5));
        let text = b.to_text();
        assert_eq!(Baseline::parse(&text).unwrap(), b);
        assert_eq!(b.len(), 2);
        assert_eq!(b.get(&name("sum_rows")).unwrap().n, 20);
    }

    #[test]
    fn test_baseline_parse_errors_name_the_line() {
        let cases = [
            ("a 1 2", 1, "expected 4 fields"),
            ("# header\n\na x 2 3", 3, "median \"x\""),
            ("a 1 -2 3", 1, "mad \"-2\""),
            ("a 1 2 many", 1, "samples \"many\""),
            ("a 1 2 3\na 4 5 6", 2, "duplicate"),
            ("a NaN 2 3", 1, "median \"NaN\""),
        ];
        for (text, line, needle) in cases {
            match Baseline::parse(text) {
                Err(BaselineError::Parse { line: l, reason }) => {
                    assert_eq!(l, line, "{text:?}");
                    assert!(reason.contains(needle), "{text:?}: {reason}");
                }
                other => panic!("{text:?} parsed as {other:?}"),
            }
        }
    }

    #[test]
    fn test_suite_report_gate() {
        let base = Baseline::parse("fast 100 1 20\nslow 100 1 20\ngone 50 1 20\n").unwrap();
        let cur = Baseline::parse("fast 99 1 20\nslow 130 1 20\nnew 10 1 20\n").unwrap();
        let report = compare_suite(&base, &cur, &Thresholds::default());
        assert!(!report.passed());
        let regressed: Vec<&str> = report.regressions().map(|r| r.name.as_str()).collect();
        assert_eq!(regressed, ["slow"]);
        assert_eq!(report.missing, [name("gone")]);
        assert_eq!(report.added, [name("new")]);
        let text = report.to_string();
        assert!(text.contains("REGRESSED"));
        assert!(text.contains("+30.0%"));
        assert!(text.contains("gone"));
        assert!(text.contains("new (no baseline)"));

        let ok = compare_suite(&base, &base, &Thresholds::default());
        assert!(ok.passed());
    }

    #[test]
    fn test_count_gate_basics() {
        assert_eq!(compare_counts(10, 10, 0.0), Verdict::Unchanged);
        assert_eq!(compare_counts(10, 11, 0.0), Verdict::Regressed);
        assert_eq!(compare_counts(10, 11, 0.1), Verdict::Unchanged);
        assert_eq!(compare_counts(10, 9, 0.0), Verdict::Improved);
        assert_eq!(compare_counts(0, 1, 10.0), Verdict::Regressed);
    }

    #[test]
    fn test_count_gate_catches_allocation_regression() {
        let words = ["regression"; 64];
        let (_, good) = measure(|| join_presized(&words, "-"));
        let (_, bad) = measure(|| join_naive(&words, "-"));
        // Swapping the implementation is caught exactly, every run, no noise.
        assert_eq!(
            compare_counts(good.heap_requests(), good.heap_requests(), 0.0),
            Verdict::Unchanged
        );
        assert_eq!(
            compare_counts(good.heap_requests(), bad.heap_requests(), 0.0),
            Verdict::Regressed
        );
    }

    #[test]
    fn test_count_gate_catches_locality_regression() {
        let config = CacheConfig::new(64, 1, 64).unwrap();
        let (rows, cols) = (128, 64);
        let good = CacheSim::new(config).run(matrix_trace(&RowMajor { rows, cols }, 8));
        let bad = CacheSim::new(config).run(matrix_trace(&ColumnMajor { rows, cols }, 8));
        assert_eq!(
            compare_counts(good.misses, bad.misses, 0.5),
            Verdict::Regressed
        );
    }

    #[test]
    fn test_verdict_display() {
        assert_eq!(Verdict::Regressed.to_string(), "REGRESSED");
        assert_eq!(Verdict::Improved.to_string(), "improved");
        assert_eq!(Verdict::Unchanged.to_string(), "unchanged");
        assert_eq!(Verdict::Inconclusive.to_string(), "inconclusive");
        let e = BaselineError::Parse {
            line: 3,
            reason: "bad".into(),
        };
        assert_eq!(e.to_string(), "line 3: bad");
    }

    #[test]
    fn test_summary_from_stats() {
        let stats = Stats::from_samples(&[1.0, 2.0, 3.0]).unwrap();
        assert_eq!(Summary::from(&stats), summary(2.0, 1.0, 3));
    }
}

#[cfg(all(test, feature = "testing-extras"))]
mod proptests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn prop_comparing_with_itself_is_never_a_regression(
            data in prop::collection::vec(1.0_f64..1e6, 5..60)
        ) {
            let c = compare_samples(&data, &data, &Thresholds::default());
            prop_assert_eq!(c.verdict, Verdict::Unchanged);
        }

        #[test]
        fn prop_p_value_is_a_probability(
            a in prop::collection::vec(0.0_f64..1e3, 1..40),
            b in prop::collection::vec(0.0_f64..1e3, 1..40),
        ) {
            let r = mann_whitney_u(&a, &b).unwrap();
            prop_assert!((0.0..=1.0).contains(&r.p_value), "{:?}", r);
        }

        #[test]
        fn prop_baseline_round_trips(
            entries in prop::collection::btree_map("[a-z_]{1,12}", (0.0_f64..1e9, 0.0_f64..1e6, 1_usize..1000), 0..10)
        ) {
            let mut b = Baseline::new();
            for (k, (median, mad, n)) in entries {
                b.record(BenchName::new(&k).unwrap(), Summary { median, mad, n });
            }
            prop_assert_eq!(Baseline::parse(&b.to_text()).unwrap(), b);
        }
    }
}
