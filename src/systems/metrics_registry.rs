//! # Metrics Registry Implementation
//!
//! Implements a foundational telemetry and metrics registry, handling thread-safe lock-free
//! Counters, Gauges, and concurrent Histograms, with the ability to export metrics
//! in the standard Prometheus text format.
//!
//! **Replaces Crates:** `prometheus`, `metrics`
//!
//! **Real-world Usage:**
//! - Microservice observability (tracking requests, errors, latency).
//! - System monitoring (CPU, memory, disk I/O).
//! - Custom business metrics (signups, purchases).
//!
//! **Why build it yourself?**
//! Building a metrics registry teaches you about low-overhead concurrent data structures.
//! Metrics must be extremely fast to record, so they don't slow down the hot path of the application.
//! You learn how to use atomic operations for counters/gauges and lock-free or low-contention
//! approaches for histograms. You also learn how to serialize state for external scrapers like Prometheus.

// Registry `RwLock` guards are intentionally held across the read/insert/export critical sections.
#![allow(clippy::significant_drop_tightening)]

use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

// =========================================================================================
// Architecture
// =========================================================================================
//
// Data Structure:
//
//      Registry
//      ├── counters: RwLock<HashMap<String, Arc<Counter>>>
//      ├── gauges: RwLock<HashMap<String, Arc<Gauge>>>
//      └── histograms: RwLock<HashMap<String, Arc<Histogram>>>
//
// Note: The `RwLock` is only acquired when *registering* or *exporting* a new metric.
// During normal operation, the application holds an `Arc<Metric>` and updates it directly
// using atomic operations (lock-free), ensuring maximum performance on the hot path.
//
// Invariants:
// 1. Metric names must be unique across all types (though stored separately here, exporters expect uniqueness).
// 2. `Histogram` buckets must be strictly monotonically increasing.
// 3. A Counter's value must never decrease.
//
// Types of Metrics:
// 1. **Counter**: A cumulative metric that can only increase (e.g., total requests).
//    - Uses `AtomicU64`.
// 2. **Gauge**: A metric that can go up and down (e.g., current active connections).
//    - Uses `AtomicI64`.
// 3. **Histogram**: Tracks the distribution of observations (e.g., request latency).
//    - Implemented here using fixed buckets and `AtomicU64` counters per bucket.
//
// Complexity:
// ┌───────────────┬─────────────┬─────────────┐
// │ Operation     │ Time        │ Space       │
// ├───────────────┼─────────────┼─────────────┤
// │ Record Metric │ O(1)        │ O(1)        │
// │ Register      │ O(1) amort. │ O(1) amort. │
// │ Export        │ O(M + B)    │ O(M + B)    │
// └───────────────┴─────────────┴─────────────┘
// *M = number of metrics, B = number of histogram buckets.
//
// Design Decisions:
// - **Lock-Free Updates**: We use `AtomicU64`/`AtomicI64` to ensure updates are lock-free.
// - **Prometheus Format**: The `export_prometheus` method outputs text format for standard tooling.
// - **Labels**: For simplicity, this foundational implementation maps metrics purely by name.
//   Production registries support multi-dimensional labels (e.g., `requests{status="200"}`).

/// A cumulative metric that only goes up.
#[derive(Debug, Default)]
pub struct Counter {
    value: AtomicU64,
}

impl Counter {
    pub fn inc(&self) {
        self.inc_by(1);
    }

    pub fn inc_by(&self, amount: u64) {
        // RUST INSIGHT: Relaxed ordering is sufficient for independent metrics
        // as we only care about the final value, not cross-thread synchronization.
        self.value.fetch_add(amount, Ordering::Relaxed);
    }

    pub fn get(&self) -> u64 {
        self.value.load(Ordering::Relaxed)
    }
}

/// A metric that can arbitrarily go up or down.
#[derive(Debug, Default)]
pub struct Gauge {
    value: AtomicI64,
}

impl Gauge {
    pub fn inc(&self) {
        self.inc_by(1);
    }

    pub fn dec(&self) {
        self.dec_by(1);
    }

    pub fn inc_by(&self, amount: i64) {
        self.value.fetch_add(amount, Ordering::Relaxed);
    }

    pub fn dec_by(&self, amount: i64) {
        self.value.fetch_sub(amount, Ordering::Relaxed);
    }

    pub fn set(&self, value: i64) {
        self.value.store(value, Ordering::Relaxed);
    }

    pub fn get(&self) -> i64 {
        self.value.load(Ordering::Relaxed)
    }
}

/// A metric that samples observations and counts them in configurable buckets.
#[derive(Debug)]
pub struct Histogram {
    buckets: Vec<f64>,
    counts: Vec<AtomicU64>,
    sum: AtomicU64,   // Stored as bits of f64 for lock-free sum tracking
    count: AtomicU64, // Total number of observations
}

impl Histogram {
    /// Creates a new histogram with specific upper bounds for buckets.
    /// E.g., `vec![0.1, 0.5, 1.0, 5.0]`
    #[must_use]
    pub fn new(mut buckets: Vec<f64>) -> Self {
        // Ensure buckets are sorted
        buckets.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        // Ensure +Inf bucket exists
        if buckets.last() != Some(&f64::INFINITY) {
            buckets.push(f64::INFINITY);
        }

        let len = buckets.len();
        let mut counts = Vec::with_capacity(len);
        for _ in 0..len {
            counts.push(AtomicU64::new(0));
        }

        Self {
            buckets,
            counts,
            sum: AtomicU64::new(0f64.to_bits()),
            count: AtomicU64::new(0),
        }
    }

    pub fn observe(&self, value: f64) {
        // Update total count
        self.count.fetch_add(1, Ordering::Relaxed);

        // Update sum (lock-free float addition using compare_exchange loop)
        let mut current_sum_bits = self.sum.load(Ordering::Relaxed);
        loop {
            let current_sum = f64::from_bits(current_sum_bits);
            let new_sum = current_sum + value;
            match self.sum.compare_exchange_weak(
                current_sum_bits,
                new_sum.to_bits(),
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(actual) => current_sum_bits = actual,
            }
        }

        // Increment appropriate bucket
        // Note: In Prometheus, buckets are cumulative (le="0.5" includes le="0.1").
        // For efficiency during observe, we increment all buckets >= value.
        for (i, &bound) in self.buckets.iter().enumerate() {
            if value <= bound {
                // Since Prometheus expects cumulative counts, we must increment
                // this bucket and all subsequent buckets.
                for count_atomic in self.counts.iter().skip(i) {
                    count_atomic.fetch_add(1, Ordering::Relaxed);
                }
                break;
            }
        }
    }

    pub fn buckets(&self) -> &[f64] {
        &self.buckets
    }

    pub fn bucket_counts(&self) -> Vec<u64> {
        self.counts
            .iter()
            .map(|c| c.load(Ordering::Relaxed))
            .collect()
    }

    pub fn total_sum(&self) -> f64 {
        f64::from_bits(self.sum.load(Ordering::Relaxed))
    }

    pub fn total_count(&self) -> u64 {
        self.count.load(Ordering::Relaxed)
    }
}

/// The central registry managing all metrics.
#[derive(Debug, Default)]
pub struct Registry {
    counters: RwLock<HashMap<String, Arc<Counter>>>,
    gauges: RwLock<HashMap<String, Arc<Gauge>>>,
    histograms: RwLock<HashMap<String, Arc<Histogram>>>,
}

impl Registry {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a counter or returns the existing one.
    ///
    /// # Panics
    /// Panics if the counters `RwLock` is poisoned.
    pub fn counter(&self, name: &str) -> Arc<Counter> {
        // Check with read lock first
        if let Some(counter) = self.counters.read().unwrap().get(name) {
            return Arc::clone(counter);
        }

        // Upgrade to write lock
        let mut counters = self.counters.write().unwrap();
        // Double-check pattern
        let counter = counters
            .entry(name.to_string())
            .or_insert_with(|| Arc::new(Counter::default()));
        Arc::clone(counter)
    }

    /// Registers a gauge or returns the existing one.
    ///
    /// # Panics
    /// Panics if the gauges `RwLock` is poisoned.
    pub fn gauge(&self, name: &str) -> Arc<Gauge> {
        if let Some(gauge) = self.gauges.read().unwrap().get(name) {
            return Arc::clone(gauge);
        }

        let mut gauges = self.gauges.write().unwrap();
        let gauge = gauges
            .entry(name.to_string())
            .or_insert_with(|| Arc::new(Gauge::default()));
        Arc::clone(gauge)
    }

    /// Registers a histogram or returns the existing one.
    ///
    /// # Panics
    /// Panics if the histograms `RwLock` is poisoned.
    pub fn histogram(&self, name: &str, buckets: Vec<f64>) -> Arc<Histogram> {
        if let Some(hist) = self.histograms.read().unwrap().get(name) {
            return Arc::clone(hist);
        }

        let mut histograms = self.histograms.write().unwrap();
        let hist = histograms
            .entry(name.to_string())
            .or_insert_with(|| Arc::new(Histogram::new(buckets)));
        Arc::clone(hist)
    }

    /// Exports all registered metrics in the Prometheus text format.
    ///
    /// # Panics
    /// Panics if any of the metric `RwLock`s are poisoned.
    pub fn export_prometheus(&self) -> String {
        use std::fmt::Write;

        // ⚡ BOLT OPTIMIZATION: Pre-allocate the String buffer to avoid reallocations during metrics export.
        // A capacity of 1024 bytes is a reasonable starting point for a small number of metrics.
        let mut output = String::with_capacity(1024);

        // ⚡ BOLT OPTIMIZATION: Avoid intermediate string allocations during metrics export.
        // Replaced `output.push_str(&format!(...))` with `writeln!(output, ...)`.
        // `format!` creates an intermediate String on the heap, which is then copied into `output` and dropped.
        // `writeln!` writes directly into the `String` buffer, eliminating the intermediate allocation.
        // Export Counters
        let counters = self.counters.read().unwrap();
        for (name, counter) in counters.iter() {
            writeln!(output, "# TYPE {name} counter").expect("writing to String cannot fail");
            writeln!(output, "{} {}", name, counter.get()).expect("writing to String cannot fail");
        }

        // Export Gauges
        let gauges = self.gauges.read().unwrap();
        for (name, gauge) in gauges.iter() {
            writeln!(output, "# TYPE {name} gauge").expect("writing to String cannot fail");
            writeln!(output, "{} {}", name, gauge.get()).expect("writing to String cannot fail");
        }

        // Export Histograms
        let histograms = self.histograms.read().unwrap();
        for (name, hist) in histograms.iter() {
            writeln!(output, "# TYPE {name} histogram").expect("writing to String cannot fail");

            let bounds = hist.buckets();
            let counts = hist.bucket_counts();

            for (bound, count) in bounds.iter().zip(counts.iter()) {
                if *bound == f64::INFINITY {
                    writeln!(output, "{name}_bucket{{le=\"+Inf\"}} {count}")
                        .expect("writing to String cannot fail");
                } else {
                    writeln!(output, "{name}_bucket{{le=\"{bound}\"}} {count}")
                        .expect("writing to String cannot fail");
                }
            }
            writeln!(output, "{}_sum {}", name, hist.total_sum())
                .expect("writing to String cannot fail");
            writeln!(output, "{}_count {}", name, hist.total_count())
                .expect("writing to String cannot fail");
        }

        output
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `prometheus`: The official client. Features extensive validation, complex multi-dimensional
//   labels, and Summary metrics (calculating quantiles over a sliding time window).
// - `metrics`: A facade crate that allows libraries to record metrics without choosing
//   a specific backend (similar to the `log` crate).
//
// Missing vs. Production:
// - **Labels (Dimensions)**: Production registries allow `requests{status="200"}`, which multiplies
//   the underlying time series. Implementing labels efficiently requires parsing and hashing label sets.
// - **Summary Metrics**: Calculating true quantiles (p95, p99) requires complex algorithms like
//   CKMS or T-Digest to keep memory bounded, rather than simple fixed buckets.
//
// Next Steps:
// 1. Add support for labels (e.g., `counter.with_label_values(&["200"]).inc()`).
// 2. Implement a `Summary` metric using a sliding window of observations.
//
// Benchmarking Note:
// To benchmark, spawn multiple threads sharing a single `Arc<Counter>` and measure the
// throughput of `counter.inc()`. Compare this to a `Mutex<u64>` to observe the performance
// difference of lock-free atomics under high contention. Criterion's `Bencher::iter` combined
// with `std::hint::black_box` should be used.

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_counter() {
        let registry = Registry::new();
        let counter = registry.counter("requests_total");

        counter.inc();
        counter.inc_by(5);

        assert_eq!(counter.get(), 6);
    }

    #[test]
    fn test_gauge() {
        let registry = Registry::new();
        let gauge = registry.gauge("active_connections");

        gauge.inc();
        gauge.inc_by(2);
        gauge.dec();

        assert_eq!(gauge.get(), 2);

        gauge.set(10);
        assert_eq!(gauge.get(), 10);
    }

    #[test]
    fn test_histogram() {
        let registry = Registry::new();
        let hist = registry.histogram("request_latency", vec![0.1, 0.5, 1.0]);

        hist.observe(0.05);
        hist.observe(0.4);
        hist.observe(0.9);
        hist.observe(2.5);

        let counts = hist.bucket_counts();

        // 0.05 is <= 0.1
        // 0.4 is <= 0.5
        // 0.9 is <= 1.0
        // 2.5 is <= +Inf

        // Due to cumulative buckets:
        // le="0.1" -> 1 (0.05)
        // le="0.5" -> 2 (0.05, 0.4)
        // le="1.0" -> 3 (0.05, 0.4, 0.9)
        // le="+Inf" -> 4 (All)
        assert_eq!(counts, vec![1, 2, 3, 4]);
        assert_eq!(hist.total_count(), 4);
        assert!((hist.total_sum() - 3.85).abs() < f64::EPSILON);
    }

    #[test]
    fn test_concurrent_counter() {
        let registry = Registry::new();
        let counter = registry.counter("threads_total");

        let mut handles = vec![];
        for _ in 0..10 {
            let c = Arc::clone(&counter);
            handles.push(thread::spawn(move || {
                for _ in 0..100 {
                    c.inc();
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        assert_eq!(counter.get(), 1000);
    }

    #[test]
    fn test_export_format() {
        let registry = Registry::new();

        let reqs = registry.counter("http_requests_total");
        reqs.inc_by(42);

        let mem = registry.gauge("memory_bytes");
        mem.set(1024);

        let latency = registry.histogram("latency_sec", vec![1.0, 5.0]);
        latency.observe(0.5);
        latency.observe(2.0);

        let output = registry.export_prometheus();

        assert!(output.contains("# TYPE http_requests_total counter"));
        assert!(output.contains("http_requests_total 42"));

        assert!(output.contains("# TYPE memory_bytes gauge"));
        assert!(output.contains("memory_bytes 1024"));

        assert!(output.contains("# TYPE latency_sec histogram"));
        assert!(output.contains("latency_sec_bucket{le=\"1\"} 1"));
        assert!(output.contains("latency_sec_bucket{le=\"5\"} 2"));
        assert!(output.contains("latency_sec_bucket{le=\"+Inf\"} 2"));
        assert!(output.contains("latency_sec_count 2"));
        assert!(output.contains("latency_sec_sum 2.5"));
    }
}
