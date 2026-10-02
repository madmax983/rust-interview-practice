//! # Cache Locality
//!
//! "Row-major is faster than column-major" is easy to say and hard to *test*,
//! because wall-clock timings depend on the machine. This module turns locality
//! claims into exact, deterministic numbers with a small set-associative LRU
//! [`CacheSim`], and runs it on address traces generated from the **same index
//! orders the real code uses**, so the simulated and the executed access
//! pattern cannot drift apart.
//!
//! Drills:
//!
//! | Pattern | Real code | What the simulator shows |
//! |---------|-----------|--------------------------|
//! | Traversal order | [`sum_in_order`] with [`RowMajor`] / [`ColumnMajor`] | Column-major on a large matrix misses on *every* access |
//! | Blocking | [`transpose`] with [`RowMajor`] / [`Tiled`] | Tiling cuts misses ~4x |
//! | Data layout | [`ParticleAos`] vs [`ParticlesSoa`] | A one-field scan of AoS drags whole structs through the cache |
//! | Pointer chasing | [`build_chain`] + [`sum_chain`] | A shuffled chain misses ~4x more than a sequential one |
//! | Padding | [`HeaderPadded`] / [`HeaderPacked`] | Field order changes `size_of` |
//! | False sharing | [`CachePadded`], [`same_cache_line`] | Adjacent atomics share a line; padded ones never do |
//!
//! ```
//! use rust_interview_practice::performance::cache_locality::{
//!     CacheConfig, CacheSim, ColumnMajor, RowMajor, matrix_trace,
//! };
//!
//! let config = CacheConfig::new(64, 1, 64).unwrap(); // 4 KiB, fully associative
//! let (rows, cols) = (128, 64); // 64 KiB of u64
//! let row = CacheSim::new(config).run(matrix_trace(&RowMajor { rows, cols }, 8));
//! let col = CacheSim::new(config).run(matrix_trace(&ColumnMajor { rows, cols }, 8));
//! assert_eq!(row.misses, 1_024); // one miss per 64-byte line
//! assert_eq!(col.misses, 8_192); // one miss per element
//! ```

use std::fmt;
use std::mem::size_of;
use std::ops::{Deref, DerefMut};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::testing_craft::sim_rng::SimRng;

// ============================================================================
// Cache simulator
// ============================================================================

/// Why a [`CacheConfig`] was rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheConfigError {
    /// Line size must be a non-zero power of two.
    LineSize(usize),
    /// Set count must be a non-zero power of two.
    Sets(usize),
    /// Associativity must be at least 1.
    Ways,
}

impl fmt::Display for CacheConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LineSize(n) => write!(f, "line size {n} is not a non-zero power of two"),
            Self::Sets(n) => write!(f, "set count {n} is not a non-zero power of two"),
            Self::Ways => f.write_str("associativity must be at least 1"),
        }
    }
}

impl std::error::Error for CacheConfigError {}

/// Geometry of a simulated cache. Capacity is `line_size * sets * ways`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CacheConfig {
    line_size: usize,
    sets: usize,
    ways: usize,
}

impl CacheConfig {
    /// Validates and builds a configuration.
    ///
    /// # Errors
    ///
    /// Returns [`CacheConfigError`] if `line_size` or `sets` is not a non-zero
    /// power of two, or `ways` is zero.
    pub const fn new(line_size: usize, sets: usize, ways: usize) -> Result<Self, CacheConfigError> {
        if !line_size.is_power_of_two() {
            return Err(CacheConfigError::LineSize(line_size));
        }
        if !sets.is_power_of_two() {
            return Err(CacheConfigError::Sets(sets));
        }
        if ways == 0 {
            return Err(CacheConfigError::Ways);
        }
        Ok(Self {
            line_size,
            sets,
            ways,
        })
    }

    /// Bytes per cache line.
    #[must_use]
    pub const fn line_size(&self) -> usize {
        self.line_size
    }

    /// Total capacity in bytes.
    #[must_use]
    pub const fn capacity_bytes(&self) -> usize {
        self.line_size * self.sets * self.ways
    }

    /// Total capacity in lines.
    #[must_use]
    pub const fn capacity_lines(&self) -> usize {
        self.sets * self.ways
    }
}

/// Result of one simulated access.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access {
    /// The line was resident.
    Hit,
    /// The line had to be fetched (and possibly evicted another).
    Miss,
}

/// Hit / miss totals.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CacheStats {
    /// Accesses that found their line resident.
    pub hits: u64,
    /// Accesses that had to fetch their line.
    pub misses: u64,
}

impl CacheStats {
    /// Total accesses.
    #[must_use]
    pub const fn accesses(&self) -> u64 {
        self.hits + self.misses
    }

    /// Misses as a fraction of accesses (0 when there were none).
    #[must_use]
    #[allow(clippy::cast_precision_loss)] // ratios only; exactness not needed
    pub fn miss_rate(&self) -> f64 {
        if self.accesses() == 0 {
            0.0
        } else {
            self.misses as f64 / self.accesses() as f64
        }
    }
}

/// A set-associative cache with LRU replacement, tracking tags only.
///
/// Address -> line = `addr / line_size`, set = `line % sets`, tag = `line / sets`.
/// Each set keeps its resident tags ordered least- to most-recently used.
#[derive(Debug, Clone)]
pub struct CacheSim {
    config: CacheConfig,
    sets: Vec<Vec<usize>>,
    stats: CacheStats,
}

impl CacheSim {
    /// Creates an empty (cold) cache.
    #[must_use]
    pub fn new(config: CacheConfig) -> Self {
        Self {
            config,
            sets: vec![Vec::with_capacity(config.ways); config.sets],
            stats: CacheStats::default(),
        }
    }

    /// Simulates one access to byte address `addr`.
    ///
    /// Time: O(ways).
    pub fn access(&mut self, addr: usize) -> Access {
        let line = addr / self.config.line_size;
        let set = &mut self.sets[line % self.config.sets];
        let tag = line / self.config.sets;
        if let Some(pos) = set.iter().position(|&t| t == tag) {
            set.remove(pos);
            set.push(tag);
            self.stats.hits += 1;
            Access::Hit
        } else {
            if set.len() == self.config.ways {
                set.remove(0); // evict least recently used
            }
            set.push(tag);
            self.stats.misses += 1;
            Access::Miss
        }
    }

    /// Feeds a whole trace and returns the cumulative stats.
    pub fn run(&mut self, trace: impl IntoIterator<Item = usize>) -> CacheStats {
        for addr in trace {
            self.access(addr);
        }
        self.stats
    }

    /// Cumulative stats so far.
    #[must_use]
    pub const fn stats(&self) -> CacheStats {
        self.stats
    }

    /// Empties the cache and zeroes the stats.
    pub fn reset(&mut self) {
        for set in &mut self.sets {
            set.clear();
        }
        self.stats = CacheStats::default();
    }
}

// ============================================================================
// Drills 1 and 2: traversal order and blocking
// ============================================================================

/// An order in which to visit the cells of a `rows x cols` matrix.
///
/// Internal iteration (`for_each_cell` calls you back) rather than an
/// `Iterator` on purpose: nested loops calling an inlined closure compile to
/// the same machine code as hand-written loops, while a chain of `flat_map` +
/// `step_by` adapters can cost more than the cache misses tiling saves. The
/// real code ([`sum_in_order`], [`transpose`]) and the simulator traces
/// ([`matrix_trace`], [`transpose_trace`]) all drive the same `for_each_cell`,
/// so what is simulated is exactly what runs.
pub trait Traversal {
    /// Number of rows.
    fn rows(&self) -> usize;
    /// Number of columns.
    fn cols(&self) -> usize;
    /// Calls `f(row, col)` once for every cell, in this traversal's order.
    fn for_each_cell(&self, f: impl FnMut(usize, usize));

    /// Every cell, in order. Handy in tests; allocates.
    fn cells(&self) -> Vec<(usize, usize)> {
        let mut out = Vec::with_capacity(self.rows() * self.cols());
        self.for_each_cell(|r, c| out.push((r, c)));
        out
    }
}

/// Rows outer, columns inner: walks a row-major matrix contiguously.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RowMajor {
    /// Number of rows.
    pub rows: usize,
    /// Number of columns.
    pub cols: usize,
}

impl Traversal for RowMajor {
    fn rows(&self) -> usize {
        self.rows
    }

    fn cols(&self) -> usize {
        self.cols
    }

    #[inline]
    fn for_each_cell(&self, mut f: impl FnMut(usize, usize)) {
        for r in 0..self.rows {
            for c in 0..self.cols {
                f(r, c);
            }
        }
    }
}

/// Columns outer, rows inner: jumps `cols * elem_size` bytes per step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ColumnMajor {
    /// Number of rows.
    pub rows: usize,
    /// Number of columns.
    pub cols: usize,
}

impl Traversal for ColumnMajor {
    fn rows(&self) -> usize {
        self.rows
    }

    fn cols(&self) -> usize {
        self.cols
    }

    #[inline]
    fn for_each_cell(&self, mut f: impl FnMut(usize, usize)) {
        for c in 0..self.cols {
            for r in 0..self.rows {
                f(r, c);
            }
        }
    }
}

/// Finishes each `tile x tile` block (row-major inside the block) before
/// moving on.
///
/// For a transpose this keeps the block's source *and* destination lines
/// resident. A `tile` of 0 acts as 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tiled {
    /// Number of rows.
    pub rows: usize,
    /// Number of columns.
    pub cols: usize,
    /// Block edge length.
    pub tile: usize,
}

impl Traversal for Tiled {
    fn rows(&self) -> usize {
        self.rows
    }

    fn cols(&self) -> usize {
        self.cols
    }

    #[inline]
    fn for_each_cell(&self, mut f: impl FnMut(usize, usize)) {
        let tile = self.tile.max(1);
        for r0 in (0..self.rows).step_by(tile) {
            let r_end = (r0 + tile).min(self.rows);
            for c0 in (0..self.cols).step_by(tile) {
                let c_end = (c0 + tile).min(self.cols);
                for r in r0..r_end {
                    for c in c0..c_end {
                        f(r, c);
                    }
                }
            }
        }
    }
}

/// Byte addresses touched when visiting a row-major matrix of
/// `elem_size`-byte elements in `order`.
#[must_use]
pub fn matrix_trace(order: &impl Traversal, elem_size: usize) -> Vec<usize> {
    let cols = order.cols();
    let mut trace = Vec::with_capacity(order.rows() * cols);
    order.for_each_cell(|r, c| trace.push((r * cols + c) * elem_size));
    trace
}

/// Sums a row-major `matrix` visiting cells in `order`.
///
/// Same answer for every order; very different cache behaviour.
///
/// # Panics
///
/// Panics if `matrix.len() != order.rows() * order.cols()`.
#[must_use]
pub fn sum_in_order(matrix: &[u64], order: &impl Traversal) -> u64 {
    let cols = order.cols();
    assert_eq!(matrix.len(), order.rows() * cols, "matrix size mismatch");
    let mut sum = 0_u64;
    order.for_each_cell(|r, c| sum = sum.wrapping_add(matrix[r * cols + c]));
    sum
}

/// Transposes a row-major `rows x cols` matrix into a row-major `cols x rows`
/// one, visiting source cells in `order`.
///
/// With [`RowMajor`] the reads are sequential and the writes stride by `rows`;
/// [`Tiled`] keeps both local.
///
/// # Panics
///
/// Panics if `src.len() != order.rows() * order.cols()`.
#[must_use]
pub fn transpose(src: &[u64], order: &impl Traversal) -> Vec<u64> {
    let (rows, cols) = (order.rows(), order.cols());
    assert_eq!(src.len(), rows * cols, "matrix size mismatch");
    let mut dst = vec![0; src.len()];
    order.for_each_cell(|r, c| dst[c * rows + r] = src[r * cols + c]);
    dst
}

/// Addresses touched by [`transpose`]: a read of `src` then a write of `dst`
/// (placed at `dst_base`) per cell.
#[must_use]
pub fn transpose_trace(order: &impl Traversal, elem_size: usize, dst_base: usize) -> Vec<usize> {
    let (rows, cols) = (order.rows(), order.cols());
    let mut trace = Vec::with_capacity(2 * rows * cols);
    order.for_each_cell(|r, c| {
        trace.push((r * cols + c) * elem_size);
        trace.push(dst_base + (c * rows + r) * elem_size);
    });
    trace
}

// ============================================================================
// Drill 3: array-of-structs vs struct-of-arrays
// ============================================================================

/// A particle stored as one struct per element (array of structs).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ParticleAos {
    /// Position.
    pub pos: [f32; 3],
    /// Velocity.
    pub vel: [f32; 3],
    /// Mass.
    pub mass: f32,
    /// Identifier.
    pub id: u64,
    /// Whether the particle is still simulated.
    pub alive: bool,
}

/// The same particles with one `Vec` per field (struct of arrays).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ParticlesSoa {
    /// Positions.
    pub pos: Vec<[f32; 3]>,
    /// Velocities.
    pub vel: Vec<[f32; 3]>,
    /// Masses.
    pub mass: Vec<f32>,
    /// Identifiers.
    pub id: Vec<u64>,
    /// Liveness flags.
    pub alive: Vec<bool>,
}

impl ParticlesSoa {
    /// Converts from the array-of-structs layout.
    #[must_use]
    pub fn from_aos(particles: &[ParticleAos]) -> Self {
        Self {
            pos: particles.iter().map(|p| p.pos).collect(),
            vel: particles.iter().map(|p| p.vel).collect(),
            mass: particles.iter().map(|p| p.mass).collect(),
            id: particles.iter().map(|p| p.id).collect(),
            alive: particles.iter().map(|p| p.alive).collect(),
        }
    }

    /// Number of particles.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.mass.len()
    }

    /// Whether there are no particles.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.mass.is_empty()
    }
}

/// Total mass, array of structs: every 4-byte read pulls in a whole struct's worth of lines.
#[must_use]
pub fn total_mass_aos(particles: &[ParticleAos]) -> f32 {
    particles.iter().map(|p| p.mass).sum()
}

/// Total mass, struct of arrays: reads one dense `f32` array.
#[must_use]
pub fn total_mass_soa(particles: &ParticlesSoa) -> f32 {
    particles.mass.iter().sum()
}

/// Addresses of one field across `n` structs of size `stride` (an `AoS` scan).
pub fn aos_field_trace(
    n: usize,
    stride: usize,
    field_offset: usize,
) -> impl Iterator<Item = usize> {
    (0..n).map(move |i| i * stride + field_offset)
}

/// Addresses of a dense field array of `n` elements (an `SoA` scan).
pub fn soa_field_trace(n: usize, field_size: usize) -> impl Iterator<Item = usize> {
    (0..n).map(move |i| i * field_size)
}

// ============================================================================
// Drill 4: pointer chasing
// ============================================================================

/// A node of an index-linked list living in a `Vec` arena.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChainNode {
    /// Payload.
    pub value: u64,
    /// Index of the next node, or `None` at the tail.
    pub next: Option<u32>,
}

/// A permutation of `0..n`, shuffled deterministically from `seed`
/// (Fisher-Yates over [`SimRng`]).
#[must_use]
pub fn shuffled_order(n: usize, seed: u64) -> Vec<usize> {
    let mut order: Vec<usize> = (0..n).collect();
    let mut rng = SimRng::new(seed);
    for i in (1..n).rev() {
        order.swap(i, rng.below(i + 1));
    }
    order
}

/// Builds a list whose nodes are visited in `order` (`order[0]` is the head).
/// Node `i` holds value `i`.
///
/// Returns the arena and the head index (`None` when `order` is empty).
///
/// # Panics
///
/// Panics if `order` is not a permutation of `0..order.len()` or is longer than
/// `u32::MAX` nodes.
#[must_use]
pub fn build_chain(order: &[usize]) -> (Vec<ChainNode>, Option<u32>) {
    let n = order.len();
    let to_u32 = |i: usize| u32::try_from(i).expect("chain longer than u32::MAX nodes");
    let mut nodes: Vec<ChainNode> = (0..n)
        .map(|i| ChainNode {
            value: i as u64,
            next: None,
        })
        .collect();
    let mut seen = vec![false; n];
    for &i in order {
        assert!(i < n && !seen[i], "order is not a permutation of 0..{n}");
        seen[i] = true;
    }
    for pair in order.windows(2) {
        nodes[pair[0]].next = Some(to_u32(pair[1]));
    }
    (nodes, order.first().map(|&h| to_u32(h)))
}

/// Walks the list from `head`, summing values. Each step is a dependent load:
/// the CPU cannot fetch node k+1 until node k has arrived.
#[must_use]
pub fn sum_chain(nodes: &[ChainNode], head: Option<u32>) -> u64 {
    let mut sum = 0_u64;
    let mut cur = head;
    while let Some(i) = cur {
        let node = &nodes[i as usize];
        sum = sum.wrapping_add(node.value);
        cur = node.next;
    }
    sum
}

/// Addresses of the nodes visited by [`sum_chain`].
#[must_use]
pub fn chain_trace(nodes: &[ChainNode], head: Option<u32>) -> Vec<usize> {
    let mut trace = Vec::with_capacity(nodes.len());
    let mut cur = head;
    while let Some(i) = cur {
        trace.push(i as usize * size_of::<ChainNode>());
        cur = nodes[i as usize].next;
    }
    trace
}

// ============================================================================
// Drill 5: field order and padding
// ============================================================================

/// `repr(C)` keeps declaration order: `u8, u64, u8` pads to 24 bytes.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct HeaderPadded {
    /// 1 byte, then 7 bytes of padding so `id` is 8-aligned.
    pub flag: u8,
    /// 8 bytes.
    pub id: u64,
    /// 1 byte, then 7 bytes of tail padding so arrays stay aligned.
    pub kind: u8,
}

/// `repr(C)` with fields sorted by alignment, largest first: 16 bytes.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct HeaderPacked {
    /// 8 bytes.
    pub id: u64,
    /// 1 byte.
    pub flag: u8,
    /// 1 byte, then 6 bytes of tail padding.
    pub kind: u8,
}

/// Default `repr(Rust)`: the compiler may reorder fields.
///
/// rustc does sort them to minimize padding today, but the layout is unspecified, so use `repr(C)`
/// whenever the layout itself matters (FFI, on-disk formats, hand-tuned cache
/// lines).
#[derive(Debug, Clone, Copy, Default)]
pub struct HeaderAuto {
    /// 1 byte.
    pub flag: u8,
    /// 8 bytes.
    pub id: u64,
    /// 1 byte.
    pub kind: u8,
}

// ============================================================================
// Drill 6: false sharing
// ============================================================================

/// Aligns (and so pads) `T` to 128 bytes, so two `CachePadded` values never
/// share a cache line.
///
/// 128 rather than 64 because Intel's adjacent-line
/// prefetcher pulls lines in pairs and Apple's M-series uses 128-byte lines
/// (the same choice as `crossbeam_utils::CachePadded`).
#[repr(align(128))]
#[derive(Debug, Default)]
pub struct CachePadded<T>(pub T);

impl<T> Deref for CachePadded<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.0
    }
}

impl<T> DerefMut for CachePadded<T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.0
    }
}

/// Whether two addresses fall on the same `line_size`-byte cache line.
#[must_use]
pub const fn same_cache_line(a: usize, b: usize, line_size: usize) -> bool {
    a / line_size == b / line_size
}

fn hammer<'a>(slots: impl Iterator<Item = &'a AtomicU64>, iters: u64) {
    std::thread::scope(|s| {
        for slot in slots {
            s.spawn(move || {
                for _ in 0..iters {
                    slot.fetch_add(1, Ordering::Relaxed);
                }
            });
        }
    });
}

/// One counter per thread, packed next to each other. Every increment
/// invalidates the line in every other core's cache: false sharing.
#[must_use]
pub fn parallel_count_adjacent(threads: usize, iters: u64) -> Vec<u64> {
    let slots: Vec<AtomicU64> = (0..threads).map(|_| AtomicU64::new(0)).collect();
    hammer(slots.iter(), iters);
    slots.into_iter().map(AtomicU64::into_inner).collect()
}

/// One counter per thread, each on its own line. Same result, no ping-pong.
#[must_use]
pub fn parallel_count_padded(threads: usize, iters: u64) -> Vec<u64> {
    let slots: Vec<CachePadded<AtomicU64>> = (0..threads)
        .map(|_| CachePadded(AtomicU64::new(0)))
        .collect();
    hammer(slots.iter().map(|p| &p.0), iters);
    slots.into_iter().map(|p| p.0.into_inner()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::{align_of, offset_of};

    fn fully_associative_4k() -> CacheConfig {
        CacheConfig::new(64, 1, 64).unwrap()
    }

    #[test]
    fn test_config_validation() {
        assert_eq!(
            CacheConfig::new(48, 1, 1),
            Err(CacheConfigError::LineSize(48))
        );
        assert_eq!(
            CacheConfig::new(0, 1, 1),
            Err(CacheConfigError::LineSize(0))
        );
        assert_eq!(CacheConfig::new(64, 3, 1), Err(CacheConfigError::Sets(3)));
        assert_eq!(CacheConfig::new(64, 4, 0), Err(CacheConfigError::Ways));
        let c = CacheConfig::new(64, 64, 8).unwrap(); // a typical 32 KiB L1d
        assert_eq!(c.capacity_bytes(), 32 * 1024);
        assert_eq!(c.capacity_lines(), 512);
        assert_eq!(c.line_size(), 64);
        assert_eq!(
            CacheConfigError::Ways.to_string(),
            "associativity must be at least 1"
        );
    }

    #[test]
    fn test_sim_hits_same_line() {
        let mut sim = CacheSim::new(fully_associative_4k());
        assert_eq!(sim.access(0), Access::Miss);
        assert_eq!(sim.access(63), Access::Hit); // same 64-byte line
        assert_eq!(sim.access(64), Access::Miss);
        assert_eq!(sim.stats(), CacheStats { hits: 1, misses: 2 });
        assert!((sim.stats().miss_rate() - 2.0 / 3.0).abs() < 1e-12);
        sim.reset();
        assert_eq!(sim.stats(), CacheStats::default());
        assert_eq!(sim.access(0), Access::Miss); // cold again
        assert!((CacheStats::default().miss_rate()).abs() < f64::EPSILON);
    }

    #[test]
    fn test_sim_lru_eviction() {
        // 2-way, 1 set: lines A, B, then C evicts A (least recently used).
        let mut sim = CacheSim::new(CacheConfig::new(64, 1, 2).unwrap());
        sim.run([0, 64]);
        assert_eq!(sim.access(0), Access::Hit); // A is now MRU, B is LRU
        assert_eq!(sim.access(128), Access::Miss); // evicts B
        assert_eq!(sim.access(0), Access::Hit);
        assert_eq!(sim.access(64), Access::Miss);
    }

    #[test]
    fn test_sim_conflict_misses_in_direct_mapped_cache() {
        // Direct-mapped, 4 sets: addresses 0 and 256 map to the same set and
        // keep evicting each other even though the cache is nearly empty.
        let mut sim = CacheSim::new(CacheConfig::new(64, 4, 1).unwrap());
        let stats = sim.run([0, 256, 0, 256, 0, 256]);
        assert_eq!(stats.misses, 6);
        // Two-way associativity fixes it.
        let mut sim = CacheSim::new(CacheConfig::new(64, 2, 2).unwrap());
        assert_eq!(sim.run([0, 256, 0, 256, 0, 256]).misses, 2);
    }

    #[test]
    fn test_row_major_beats_column_major() {
        let (rows, cols) = (128, 64);
        let row =
            CacheSim::new(fully_associative_4k()).run(matrix_trace(&RowMajor { rows, cols }, 8));
        let col =
            CacheSim::new(fully_associative_4k()).run(matrix_trace(&ColumnMajor { rows, cols }, 8));
        assert_eq!(row.misses, (rows * cols * 8 / 64) as u64);
        assert_eq!(col.misses, (rows * cols) as u64);
    }

    #[test]
    fn test_column_major_is_fine_when_matrix_fits() {
        // 32 rows: a column pass touches 32 lines, fewer than the cache's 64.
        let (rows, cols) = (32, 64);
        let col =
            CacheSim::new(fully_associative_4k()).run(matrix_trace(&ColumnMajor { rows, cols }, 8));
        assert_eq!(col.misses, (rows * cols * 8 / 64) as u64);
    }

    #[test]
    fn test_sum_order_does_not_change_answer() {
        let (rows, cols) = (17, 9);
        let m: Vec<u64> = (0..(rows * cols) as u64).collect();
        let expected: u64 = m.iter().sum();
        assert_eq!(sum_in_order(&m, &RowMajor { rows, cols }), expected);
        assert_eq!(sum_in_order(&m, &ColumnMajor { rows, cols }), expected);
        assert_eq!(
            sum_in_order(
                &m,
                &Tiled {
                    rows,
                    cols,
                    tile: 4
                }
            ),
            expected
        );
        assert_eq!(sum_in_order(&[], &RowMajor { rows: 0, cols: 0 }), 0);
    }

    #[test]
    #[should_panic(expected = "matrix size mismatch")]
    fn test_sum_rejects_wrong_shape() {
        let _ = sum_in_order(&[1, 2, 3], &RowMajor { rows: 2, cols: 2 });
    }

    #[test]
    fn test_transpose_orders_agree() {
        for &(rows, cols, tile) in &[(1, 1, 4), (3, 5, 2), (8, 8, 8), (13, 7, 4), (4, 4, 0)] {
            let src: Vec<u64> = (0..(rows * cols) as u64).collect();
            let naive = transpose(&src, &RowMajor { rows, cols });
            let tiled = transpose(&src, &Tiled { rows, cols, tile });
            let by_column = transpose(&src, &ColumnMajor { rows, cols });
            assert_eq!(naive, tiled);
            assert_eq!(naive, by_column);
            for r in 0..rows {
                for c in 0..cols {
                    assert_eq!(naive[c * rows + r], src[r * cols + c]);
                }
            }
        }
    }

    #[test]
    fn test_every_traversal_visits_every_cell_once() {
        let (rows, cols) = (10, 6);
        let all = RowMajor { rows, cols }.cells();
        assert_eq!(all.len(), 60);
        assert_eq!(all[1], (0, 1)); // row-major: column moves first
        assert_eq!(ColumnMajor { rows, cols }.cells()[1], (1, 0));
        assert_eq!(
            Tiled {
                rows,
                cols,
                tile: 4
            }
            .cells()[4],
            (1, 0)
        ); // next row in tile
        for mut seen in [
            ColumnMajor { rows, cols }.cells(),
            Tiled {
                rows,
                cols,
                tile: 4,
            }
            .cells(),
            Tiled {
                rows,
                cols,
                tile: 0,
            }
            .cells(),
            Tiled {
                rows,
                cols,
                tile: 100,
            }
            .cells(),
        ] {
            seen.sort_unstable();
            assert_eq!(seen, all);
        }
    }

    #[test]
    fn test_tiling_cuts_transpose_misses() {
        let n = 64;
        let base = n * n * 8;
        let naive = CacheSim::new(fully_associative_4k()).run(transpose_trace(
            &RowMajor { rows: n, cols: n },
            8,
            base,
        ));
        let tiled = CacheSim::new(fully_associative_4k()).run(transpose_trace(
            &Tiled {
                rows: n,
                cols: n,
                tile: 8,
            },
            8,
            base,
        ));
        // Naive: 512 read misses + a miss on every one of the 4096 writes.
        assert_eq!(naive.misses, 512 + 4_096);
        // Tiled 8x8: 8 src lines + 8 dst lines per tile, each fetched once.
        assert_eq!(tiled.misses, 512 + 512);
    }

    fn particles(n: usize) -> Vec<ParticleAos> {
        (0..n)
            .map(|i| ParticleAos {
                mass: f32::from(u8::try_from(i % 7).unwrap()),
                id: i as u64,
                alive: i % 2 == 0,
                ..ParticleAos::default()
            })
            .collect()
    }

    #[test]
    fn test_aos_and_soa_agree() {
        let aos = particles(100);
        let soa = ParticlesSoa::from_aos(&aos);
        assert_eq!(soa.len(), 100);
        assert!(!soa.is_empty());
        assert!(ParticlesSoa::default().is_empty());
        assert!((total_mass_aos(&aos) - total_mass_soa(&soa)).abs() < f32::EPSILON);
        assert_eq!(soa.id[42], 42);
        assert!(soa.alive[42]);
    }

    #[test]
    fn test_soa_field_scan_touches_fewer_lines() {
        let n = 1_024;
        let stride = size_of::<ParticleAos>();
        assert!(stride >= 40, "ParticleAos is {stride} bytes");
        let offset = offset_of!(ParticleAos, mass);
        let aos = CacheSim::new(fully_associative_4k()).run(aos_field_trace(n, stride, offset));
        let soa = CacheSim::new(fully_associative_4k()).run(soa_field_trace(n, size_of::<f32>()));
        assert_eq!(soa.misses, (n * 4 / 64) as u64); // 16 masses per line
        // AoS fetches at least stride/64 lines per particle: >= 10x more.
        assert!(aos.misses >= 10 * soa.misses, "aos={aos:?} soa={soa:?}");
    }

    #[test]
    fn test_chain_sum_and_shape() {
        let order = shuffled_order(100, 7);
        let (nodes, head) = build_chain(&order);
        assert_eq!(head, Some(u32::try_from(order[0]).unwrap()));
        assert_eq!(sum_chain(&nodes, head), (0..100).sum::<u64>());
        assert_eq!(chain_trace(&nodes, head).len(), 100);
        let (empty, none) = build_chain(&[]);
        assert!(empty.is_empty());
        assert_eq!(sum_chain(&empty, none), 0);
    }

    #[test]
    #[should_panic(expected = "not a permutation")]
    fn test_build_chain_rejects_repeats() {
        let _ = build_chain(&[0, 1, 0]);
    }

    #[test]
    #[should_panic(expected = "not a permutation")]
    fn test_build_chain_rejects_out_of_range() {
        let _ = build_chain(&[0, 5]);
    }

    #[test]
    fn test_shuffled_order_is_a_deterministic_permutation() {
        let a = shuffled_order(50, 3);
        assert_eq!(a, shuffled_order(50, 3));
        assert_ne!(a, shuffled_order(50, 4));
        let mut sorted = a;
        sorted.sort_unstable();
        assert_eq!(sorted, (0..50).collect::<Vec<_>>());
    }

    #[test]
    fn test_pointer_chasing_shuffled_misses_more() {
        let n = 4_096; // 16-byte nodes: 64 KiB, far bigger than the 4 KiB cache
        assert_eq!(size_of::<ChainNode>(), 16);
        let (seq_nodes, seq_head) = build_chain(&(0..n).collect::<Vec<_>>());
        let (shuf_nodes, shuf_head) = build_chain(&shuffled_order(n, 42));
        let seq = CacheSim::new(fully_associative_4k()).run(chain_trace(&seq_nodes, seq_head));
        let shuf = CacheSim::new(fully_associative_4k()).run(chain_trace(&shuf_nodes, shuf_head));
        assert_eq!(seq.misses, (n / 4) as u64); // 4 nodes per line
        assert!(shuf.misses > 3 * seq.misses, "seq={seq:?} shuf={shuf:?}");
    }

    #[test]
    fn test_field_order_changes_size() {
        assert_eq!(size_of::<HeaderPadded>(), 24);
        assert_eq!(size_of::<HeaderPacked>(), 16);
        assert_eq!(offset_of!(HeaderPadded, id), 8);
        assert_eq!(offset_of!(HeaderPacked, flag), 8);
        // Unspecified but true on every current rustc: repr(Rust) reorders.
        assert!(size_of::<HeaderAuto>() <= 16);
    }

    #[test]
    fn test_cache_padded_layout() {
        assert_eq!(align_of::<CachePadded<u8>>(), 128);
        assert_eq!(size_of::<CachePadded<AtomicU64>>(), 128);
        let mut p = CachePadded(5_u32);
        *p += 1;
        assert_eq!(*p, 6);
    }

    #[test]
    fn test_adjacent_atomics_share_lines_padded_never_do() {
        let packed: Vec<AtomicU64> = (0..8).map(|_| AtomicU64::new(0)).collect();
        let padded: Vec<CachePadded<AtomicU64>> =
            (0..8).map(|_| CachePadded(AtomicU64::new(0))).collect();
        let addr = |a: &AtomicU64| std::ptr::from_ref(a) as usize;
        // 8 x 8 bytes = 64 bytes cross at most one line boundary, so at least
        // 6 of the 7 neighbouring pairs share a line.
        let shared = packed
            .windows(2)
            .filter(|w| same_cache_line(addr(&w[0]), addr(&w[1]), 64))
            .count();
        assert!(shared >= 6);
        assert!(
            padded
                .windows(2)
                .all(|w| !same_cache_line(addr(&w[0]), addr(&w[1]), 128))
        );
    }

    #[test]
    fn test_parallel_counts_are_exact() {
        assert_eq!(parallel_count_adjacent(4, 1_000), vec![1_000; 4]);
        assert_eq!(parallel_count_padded(4, 1_000), vec![1_000; 4]);
        assert!(parallel_count_padded(0, 10).is_empty());
    }
}

#[cfg(all(test, feature = "testing-extras"))]
mod proptests {
    use super::*;
    use proptest::prelude::*;
    use std::collections::HashSet;

    proptest! {
        #[test]
        fn prop_hits_plus_misses_is_accesses(trace in prop::collection::vec(0_usize..1 << 16, 0..500)) {
            let stats = CacheSim::new(CacheConfig::new(64, 4, 2).unwrap()).run(trace.iter().copied());
            prop_assert_eq!(stats.accesses(), trace.len() as u64);
        }

        #[test]
        fn prop_misses_at_least_distinct_lines(trace in prop::collection::vec(0_usize..1 << 16, 0..500)) {
            let distinct = trace.iter().map(|a| a / 64).collect::<HashSet<_>>().len() as u64;
            let stats = CacheSim::new(CacheConfig::new(64, 4, 2).unwrap()).run(trace.iter().copied());
            prop_assert!(stats.misses >= distinct); // compulsory misses
            // A cache big enough for every line only takes compulsory misses.
            let big = CacheSim::new(CacheConfig::new(64, 1, 1024).unwrap()).run(trace.iter().copied());
            prop_assert_eq!(big.misses, distinct);
        }

        #[test]
        fn prop_immediate_reaccess_hits(addrs in prop::collection::vec(0_usize..1 << 20, 1..100)) {
            let mut sim = CacheSim::new(CacheConfig::new(64, 8, 1).unwrap());
            for a in addrs {
                sim.access(a);
                prop_assert_eq!(sim.access(a), Access::Hit);
            }
        }
    }
}
