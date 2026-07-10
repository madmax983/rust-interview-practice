//! # SIMD (Single Instruction, Multiple Data) Patterns
//!
//! Master SIMD programming in Rust using `std::arch` intrinsics and portable SIMD.
//! Learn vectorization, platform detection, and performance optimization.
//!
//! SIMD allows processing multiple data elements in parallel with a single instruction,
//! providing significant performance improvements for data-parallel workloads.

#![allow(clippy::missing_panics_doc)] // Examples may panic for demonstration

// ============================================================================
// Platform Detection
// ============================================================================

/// Check if CPU supports SSE4.2 (`x86/x86_64`).
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[must_use]
pub fn has_sse42() -> bool {
    std::is_x86_feature_detected!("sse4.2")
}

/// Check if CPU supports AVX (Advanced Vector Extensions).
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[must_use]
pub fn has_avx() -> bool {
    std::is_x86_feature_detected!("avx")
}

/// Check if CPU supports AVX2.
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[must_use]
pub fn has_avx2() -> bool {
    std::is_x86_feature_detected!("avx2")
}

/// Check if CPU supports FMA (Fused Multiply-Add).
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[must_use]
pub fn has_fma() -> bool {
    std::is_x86_feature_detected!("fma")
}

// ============================================================================
// SSE2 - Basic SIMD (128-bit vectors)
// ============================================================================

#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::{__m128, _mm_movehl_ps, _mm_add_ps, _mm_shuffle_ps, _mm_add_ss, _mm_cvtss_f32, _mm_loadu_ps, _mm_storeu_ps, _mm_mul_ps, _mm_setzero_ps, _mm_set1_ps, _mm_max_ps, _mm256_loadu_ps, _mm256_add_ps, _mm256_storeu_ps, _mm256_fmadd_ps, _mm256_setzero_ps, _mm256_castps256_ps128, _mm256_extractf128_ps, _mm_movehdup_ps, _mm_loadu_si128, _mm_add_epi32, _mm_storeu_si128, _mm256_loadu_si256, _mm256_add_epi32, _mm256_storeu_si256, _mm256_setzero_si256, _mm256_castsi256_si128, _mm256_extracti128_si256, _mm_hadd_epi32, _mm_cvtsi128_si32, _mm_cmpgt_ps, _mm_movemask_ps};

#[cfg(target_arch = "x86")]
use std::arch::x86::*;

/// Horizontal sum of a 128-bit vector of 4 `f32` lanes, using only SSE/SSE2
/// intrinsics (no SSE3 `_mm_movehdup_ps`).
///
/// # Safety
/// The CPU must support SSE2 (guaranteed by `#[target_feature(enable = "sse2")]`).
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[target_feature(enable = "sse2")]
unsafe fn hsum_ps_sse2(v: __m128) -> f32 {
    // Every intrinsic used here (`_mm_movehl_ps`, `_mm_add_ps`, `_mm_shuffle_ps`,
    // `_mm_add_ss`, `_mm_cvtss_f32`) is part of the SSE/SSE2 baseline. Because this
    // function carries `#[target_feature(enable = "sse2")]`, calling those SSE2
    // intrinsics is safe here and needs no `unsafe` block (they only require the
    // feature this function already guarantees). The `unsafe fn` marker remains so
    // that callers must themselves establish SSE2 availability.
    // v = [a, b, c, d]
    let shuf = _mm_movehl_ps(v, v); // [c, d, c, d]
    let sums = _mm_add_ps(v, shuf); // [a+c, b+d, _, _]
    let shuf = _mm_shuffle_ps(sums, sums, 0b00_00_00_01); // move lane 1 -> lane 0
    let sums = _mm_add_ss(sums, shuf); // (a+c) + (b+d)
    _mm_cvtss_f32(sums)
}

/// Add two arrays of f32 using SSE (4 floats at a time).
///
/// SSE processes 128 bits = 4 x 32-bit floats in parallel.
///
/// This is the SAFE public wrapper: it performs runtime feature detection and
/// falls back to scalar code when SSE2 is unavailable.
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub fn add_floats_sse(a: &[f32], b: &[f32], result: &mut [f32]) {
    assert_eq!(a.len(), b.len());
    assert_eq!(a.len(), result.len());

    if std::is_x86_feature_detected!("sse2") {
        // SAFETY: We just confirmed SSE2 support at runtime, satisfying the
        // `#[target_feature(enable = "sse2")]` contract of `add_floats_sse2`.
        unsafe { add_floats_sse2(a, b, result) };
    } else {
        for ((r, &x), &y) in result.iter_mut().zip(a).zip(b) {
            *r = x + y;
        }
    }
}

/// SSE2 implementation of [`add_floats_sse`].
///
/// # Safety
/// The CPU must support SSE2. Callers must verify this (e.g. via
/// `is_x86_feature_detected!("sse2")`); the safe wrapper does so.
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[target_feature(enable = "sse2")]
unsafe fn add_floats_sse2(a: &[f32], b: &[f32], result: &mut [f32]) {
    let chunks = a.len() / 4;
    let remainder = a.len() % 4;

    // SAFETY: SSE2 is guaranteed by the target feature. We only iterate over full
    // 4-lane chunks, so every `add(offset)` and the 16 bytes read/written by the
    // unaligned `_mm_loadu_ps`/`_mm_storeu_ps` stay within the (equal-length,
    // asserted) slices. Unaligned loads/stores impose no alignment requirement.
    unsafe {
        for i in 0..chunks {
            let offset = i * 4;
            let va = _mm_loadu_ps(a.as_ptr().add(offset));
            let vb = _mm_loadu_ps(b.as_ptr().add(offset));
            let vr = _mm_add_ps(va, vb);
            _mm_storeu_ps(result.as_mut_ptr().add(offset), vr);
        }
    }

    // Handle remaining elements with scalar code
    for i in (chunks * 4)..(chunks * 4 + remainder) {
        result[i] = a[i] + b[i];
    }
}

/// Multiply two arrays of f32 using SSE.
///
/// SAFE public wrapper with runtime detection and scalar fallback.
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub fn mul_floats_sse(a: &[f32], b: &[f32], result: &mut [f32]) {
    assert_eq!(a.len(), b.len());
    assert_eq!(a.len(), result.len());

    if std::is_x86_feature_detected!("sse2") {
        // SAFETY: SSE2 confirmed at runtime, satisfying `mul_floats_sse2`.
        unsafe { mul_floats_sse2(a, b, result) };
    } else {
        for ((r, &x), &y) in result.iter_mut().zip(a).zip(b) {
            *r = x * y;
        }
    }
}

/// SSE2 implementation of [`mul_floats_sse`].
///
/// # Safety
/// The CPU must support SSE2.
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[target_feature(enable = "sse2")]
unsafe fn mul_floats_sse2(a: &[f32], b: &[f32], result: &mut [f32]) {
    let chunks = a.len() / 4;
    let remainder = a.len() % 4;

    // SAFETY: SSE2 guaranteed by target feature; only full 4-lane chunks are read
    // and written via unaligned intrinsics, all in-bounds of the equal-length slices.
    unsafe {
        for i in 0..chunks {
            let offset = i * 4;
            let va = _mm_loadu_ps(a.as_ptr().add(offset));
            let vb = _mm_loadu_ps(b.as_ptr().add(offset));
            let vr = _mm_mul_ps(va, vb);
            _mm_storeu_ps(result.as_mut_ptr().add(offset), vr);
        }
    }

    for i in (chunks * 4)..(chunks * 4 + remainder) {
        result[i] = a[i] * b[i];
    }
}

/// Dot product of two f32 arrays using SSE.
///
/// Demonstrates horizontal operations (summing across vector lanes).
///
/// SAFE public wrapper with runtime detection and scalar fallback.
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[must_use]
pub fn dot_product_sse(a: &[f32], b: &[f32]) -> f32 {
    assert_eq!(a.len(), b.len());

    if std::is_x86_feature_detected!("sse2") {
        // SAFETY: SSE2 confirmed at runtime, satisfying `dot_product_sse2`.
        unsafe { dot_product_sse2(a, b) }
    } else {
        a.iter().zip(b).map(|(&x, &y)| x * y).sum()
    }
}

/// SSE2 implementation of [`dot_product_sse`].
///
/// Uses an SSE2-only horizontal reduction (`hsum_ps_sse2`) instead of the SSE3
/// `_mm_movehdup_ps`, so it stays within the `x86_64` SSE2 baseline.
///
/// # Safety
/// The CPU must support SSE2.
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[target_feature(enable = "sse2")]
unsafe fn dot_product_sse2(a: &[f32], b: &[f32]) -> f32 {
    let chunks = a.len() / 4;
    let remainder = a.len() % 4;

    // SAFETY: SSE2 guaranteed by target feature. `_mm_setzero_ps` and the loop's
    // unaligned loads over full 4-lane chunks stay in-bounds of the equal-length
    // slices. `hsum_ps_sse2` requires SSE2, which is satisfied here.
    let mut result = unsafe {
        let mut sum_vec = _mm_setzero_ps();
        for i in 0..chunks {
            let offset = i * 4;
            let va = _mm_loadu_ps(a.as_ptr().add(offset));
            let vb = _mm_loadu_ps(b.as_ptr().add(offset));
            let prod = _mm_mul_ps(va, vb);
            sum_vec = _mm_add_ps(sum_vec, prod);
        }
        hsum_ps_sse2(sum_vec)
    };

    // Add remainder with scalar code
    for i in (chunks * 4)..(chunks * 4 + remainder) {
        result += a[i] * b[i];
    }

    result
}

/// Sum all elements in f32 array using SSE.
///
/// SAFE public wrapper with runtime detection and scalar fallback.
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[must_use]
pub fn sum_floats_sse(data: &[f32]) -> f32 {
    if std::is_x86_feature_detected!("sse2") {
        // SAFETY: SSE2 confirmed at runtime, satisfying `sum_floats_sse2`.
        unsafe { sum_floats_sse2(data) }
    } else {
        data.iter().sum()
    }
}

/// SSE2 implementation of [`sum_floats_sse`].
///
/// Uses the SSE2-only `hsum_ps_sse2` reduction (no SSE3 `_mm_movehdup_ps`).
///
/// # Safety
/// The CPU must support SSE2.
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[target_feature(enable = "sse2")]
unsafe fn sum_floats_sse2(data: &[f32]) -> f32 {
    let chunks = data.len() / 4;
    let remainder = data.len() % 4;

    // SAFETY: SSE2 guaranteed by target feature; unaligned loads over full 4-lane
    // chunks stay in-bounds of `data`. `hsum_ps_sse2` requires SSE2, satisfied here.
    let mut result = unsafe {
        let mut sum_vec = _mm_setzero_ps();
        for i in 0..chunks {
            let v = _mm_loadu_ps(data.as_ptr().add(i * 4));
            sum_vec = _mm_add_ps(sum_vec, v);
        }
        hsum_ps_sse2(sum_vec)
    };

    for i in (chunks * 4)..(chunks * 4 + remainder) {
        result += data[i];
    }

    result
}

/// Find maximum value in f32 array using SSE.
///
/// SAFE public wrapper with runtime detection and scalar fallback.
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[must_use]
pub fn max_floats_sse(data: &[f32]) -> f32 {
    if data.is_empty() {
        return f32::NEG_INFINITY;
    }

    if std::is_x86_feature_detected!("sse2") {
        // SAFETY: SSE2 confirmed at runtime, satisfying `max_floats_sse2`.
        unsafe { max_floats_sse2(data) }
    } else {
        data.iter().copied().fold(f32::NEG_INFINITY, f32::max)
    }
}

/// SSE2 implementation of [`max_floats_sse`].
///
/// # Safety
/// The CPU must support SSE2. `data` must be non-empty (the wrapper guarantees it).
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[target_feature(enable = "sse2")]
unsafe fn max_floats_sse2(data: &[f32]) -> f32 {
    let chunks = data.len() / 4;
    let remainder = data.len() % 4;

    // SAFETY: SSE2 guaranteed by target feature. All intrinsics used
    // (`_mm_set1_ps`, `_mm_max_ps`, `_mm_shuffle_ps`, `_mm_cvtss_f32`) are
    // SSE/SSE2. Unaligned loads over full 4-lane chunks stay in-bounds of `data`.
    let mut result = unsafe {
        let mut max_vec = _mm_set1_ps(f32::NEG_INFINITY);
        for i in 0..chunks {
            let v = _mm_loadu_ps(data.as_ptr().add(i * 4));
            max_vec = _mm_max_ps(max_vec, v);
        }
        // Horizontal max
        let shuf = _mm_shuffle_ps(max_vec, max_vec, 0b00_00_11_10);
        let max1 = _mm_max_ps(max_vec, shuf);
        let shuf = _mm_shuffle_ps(max1, max1, 0b00_00_00_01);
        let max2 = _mm_max_ps(max1, shuf);
        _mm_cvtss_f32(max2)
    };

    for i in (chunks * 4)..(chunks * 4 + remainder) {
        result = result.max(data[i]);
    }

    result
}

// ============================================================================
// AVX - 256-bit SIMD (8 floats at a time)
// ============================================================================

/// Add two arrays of f32 using AVX (8 floats at a time).
///
/// AVX processes 256 bits = 8 x 32-bit floats in parallel.
/// Requires CPU support - check with `has_avx()`.
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[target_feature(enable = "avx")]
pub unsafe fn add_floats_avx(a: &[f32], b: &[f32], result: &mut [f32]) {
    assert_eq!(a.len(), b.len());
    assert_eq!(a.len(), result.len());

    let chunks = a.len() / 8;
    let remainder = a.len() % 8;

    // SAFETY: Caller guarantees AVX support via #[target_feature]
    unsafe {
        for i in 0..chunks {
            let offset = i * 8;
            let va = _mm256_loadu_ps(a.as_ptr().add(offset));
            let vb = _mm256_loadu_ps(b.as_ptr().add(offset));
            let vr = _mm256_add_ps(va, vb);
            _mm256_storeu_ps(result.as_mut_ptr().add(offset), vr);
        }
    }

    for i in (chunks * 8)..(chunks * 8 + remainder) {
        result[i] = a[i] + b[i];
    }
}

/// Fused multiply-add using AVX/FMA: result = a * b + c.
///
/// FMA is more accurate and faster than separate multiply + add.
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[target_feature(enable = "avx")]
#[target_feature(enable = "fma")]
pub unsafe fn fma_floats(a: &[f32], b: &[f32], c: &[f32], result: &mut [f32]) {
    assert_eq!(a.len(), b.len());
    assert_eq!(a.len(), c.len());
    assert_eq!(a.len(), result.len());

    let chunks = a.len() / 8;
    let remainder = a.len() % 8;

    // SAFETY: Caller guarantees FMA support via #[target_feature]
    unsafe {
        for i in 0..chunks {
            let offset = i * 8;
            let va = _mm256_loadu_ps(a.as_ptr().add(offset));
            let vb = _mm256_loadu_ps(b.as_ptr().add(offset));
            let vc = _mm256_loadu_ps(c.as_ptr().add(offset));
            let vr = _mm256_fmadd_ps(va, vb, vc);
            _mm256_storeu_ps(result.as_mut_ptr().add(offset), vr);
        }
    }

    for i in (chunks * 8)..(chunks * 8 + remainder) {
        result[i] = a[i].mul_add(b[i], c[i]);
    }
}

/// Sum all elements using AVX.
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[target_feature(enable = "avx")]
#[must_use]
pub unsafe fn sum_floats_avx(data: &[f32]) -> f32 {
    let chunks = data.len() / 8;
    let remainder = data.len() % 8;

    // SAFETY: Caller guarantees AVX support via #[target_feature]
    let mut result = unsafe {
        let mut sum_vec = _mm256_setzero_ps();

        for i in 0..chunks {
            let v = _mm256_loadu_ps(data.as_ptr().add(i * 8));
            sum_vec = _mm256_add_ps(sum_vec, v);
        }

        let low = _mm256_castps256_ps128(sum_vec);
        let high = _mm256_extractf128_ps(sum_vec, 1);
        let sum128 = _mm_add_ps(low, high);
        let shuf = _mm_movehdup_ps(sum128);
        let sums = _mm_add_ps(sum128, shuf);
        let shuf = _mm_movehl_ps(shuf, sums);
        let sums = _mm_add_ss(sums, shuf);
        _mm_cvtss_f32(sums)
    };

    for i in (chunks * 8)..(chunks * 8 + remainder) {
        result += data[i];
    }

    result
}

// ============================================================================
// Integer SIMD with SSE2/AVX2
// ============================================================================

/// Add two arrays of i32 using SSE2 (4 integers at a time).
///
/// SAFE public wrapper with runtime detection and scalar fallback.
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub fn add_i32_sse(a: &[i32], b: &[i32], result: &mut [i32]) {
    assert_eq!(a.len(), b.len());
    assert_eq!(a.len(), result.len());

    if std::is_x86_feature_detected!("sse2") {
        // SAFETY: SSE2 confirmed at runtime, satisfying `add_i32_sse2`.
        unsafe { add_i32_sse2(a, b, result) };
    } else {
        for ((r, &x), &y) in result.iter_mut().zip(a).zip(b) {
            *r = x.wrapping_add(y);
        }
    }
}

/// SSE2 implementation of [`add_i32_sse`].
///
/// # Safety
/// The CPU must support SSE2.
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[target_feature(enable = "sse2")]
unsafe fn add_i32_sse2(a: &[i32], b: &[i32], result: &mut [i32]) {
    let chunks = a.len() / 4;
    let remainder = a.len() % 4;

    // SAFETY: SSE2 guaranteed by target feature. Unaligned 128-bit loads/stores
    // over full 4-lane chunks stay in-bounds of the equal-length slices; the
    // `.cast()` reinterprets `*const i32`/`*mut i32` as the `__m128i` pointer
    // type expected by the unaligned intrinsics (no alignment requirement).
    unsafe {
        for i in 0..chunks {
            let offset = i * 4;
            let va = _mm_loadu_si128(a.as_ptr().add(offset).cast());
            let vb = _mm_loadu_si128(b.as_ptr().add(offset).cast());
            let vr = _mm_add_epi32(va, vb);
            _mm_storeu_si128(result.as_mut_ptr().add(offset).cast(), vr);
        }
    }

    for i in (chunks * 4)..(chunks * 4 + remainder) {
        result[i] = a[i].wrapping_add(b[i]);
    }
}

/// Add two arrays of i32 using AVX2 (8 integers at a time).
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[target_feature(enable = "avx2")]
pub unsafe fn add_i32_avx2(a: &[i32], b: &[i32], result: &mut [i32]) {
    assert_eq!(a.len(), b.len());
    assert_eq!(a.len(), result.len());

    let chunks = a.len() / 8;
    let remainder = a.len() % 8;

    // SAFETY: Caller guarantees AVX2 support via #[target_feature]
    unsafe {
        for i in 0..chunks {
            let offset = i * 8;
            let va = _mm256_loadu_si256(a.as_ptr().add(offset).cast());
            let vb = _mm256_loadu_si256(b.as_ptr().add(offset).cast());
            let vr = _mm256_add_epi32(va, vb);
            _mm256_storeu_si256(result.as_mut_ptr().add(offset).cast(), vr);
        }
    }

    for i in (chunks * 8)..(chunks * 8 + remainder) {
        result[i] = a[i].wrapping_add(b[i]);
    }
}

/// Sum array of i32 using AVX2.
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[target_feature(enable = "avx2")]
#[must_use]
pub unsafe fn sum_i32_avx2(data: &[i32]) -> i32 {
    let chunks = data.len() / 8;
    let remainder = data.len() % 8;

    // SAFETY: Caller guarantees AVX2 support via #[target_feature]
    let mut result = unsafe {
        let mut sum_vec = _mm256_setzero_si256();

        for i in 0..chunks {
            let v = _mm256_loadu_si256(data.as_ptr().add(i * 8).cast());
            sum_vec = _mm256_add_epi32(sum_vec, v);
        }

        let low = _mm256_castsi256_si128(sum_vec);
        let high = _mm256_extracti128_si256(sum_vec, 1);
        let sum128 = _mm_add_epi32(low, high);
        let sum64 = _mm_hadd_epi32(sum128, sum128);
        let sum32 = _mm_hadd_epi32(sum64, sum64);
        _mm_cvtsi128_si32(sum32)
    };

    for i in (chunks * 8)..(chunks * 8 + remainder) {
        result = result.wrapping_add(data[i]);
    }

    result
}

// ============================================================================
// Alignment
// ============================================================================

/// A growable `f32` buffer, kept as a teaching example about alignment.
///
/// # Alignment caveat (important)
///
/// A previous version of this type carried `#[repr(align(32))]` and claimed its
/// data was 32-byte aligned for AVX. That was misleading: `#[repr(align)]` only
/// raises the alignment of the `AlignedVec` *handle* — the pointer/len/capacity
/// struct that lives on the stack — NOT the heap buffer that `Vec<f32>` owns.
/// The heap allocation is only aligned to `align_of::<f32>()` (4 bytes).
///
/// Because the underlying data is therefore NOT over-aligned, callers must use
/// unaligned SIMD loads/stores (`_mm_loadu_ps`, `_mm256_loadu_ps`), exactly as
/// the functions in this module already do. Achieving genuinely over-aligned
/// heap storage requires a custom allocation (e.g. `std::alloc::alloc` with a
/// 32-byte `Layout`, or a crate like `aligned-vec`), which we deliberately avoid
/// here to keep the example allocator-safe. The misleading attribute has been
/// removed and the docs corrected rather than adding a risky custom allocator.
pub struct AlignedVec {
    data: Vec<f32>,
}

impl AlignedVec {
    /// Create aligned vector with capacity.
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            data: Vec::with_capacity(capacity),
        }
    }

    /// Get slice of data.
    #[must_use]
    pub fn as_slice(&self) -> &[f32] {
        &self.data
    }

    /// Get mutable slice of data.
    pub fn as_mut_slice(&mut self) -> &mut [f32] {
        &mut self.data
    }

    /// Push element.
    pub fn push(&mut self, value: f32) {
        self.data.push(value);
    }
}

// ============================================================================
// Comparison and Masking
// ============================================================================

/// Count elements greater than threshold using SSE comparisons.
///
/// SAFE public wrapper with runtime detection and scalar fallback.
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[must_use]
pub fn count_greater_sse(data: &[f32], threshold: f32) -> usize {
    if std::is_x86_feature_detected!("sse2") {
        // SAFETY: SSE2 confirmed at runtime, satisfying `count_greater_sse2`.
        unsafe { count_greater_sse2(data, threshold) }
    } else {
        data.iter().filter(|&&x| x > threshold).count()
    }
}

/// SSE2 implementation of [`count_greater_sse`].
///
/// # Safety
/// The CPU must support SSE2.
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[target_feature(enable = "sse2")]
unsafe fn count_greater_sse2(data: &[f32], threshold: f32) -> usize {
    let chunks = data.len() / 4;
    let remainder = data.len() % 4;

    let mut count = 0;

    // SAFETY: SSE2 guaranteed by target feature. `_mm_set1_ps`, `_mm_cmpgt_ps`
    // and `_mm_movemask_ps` are SSE/SSE2; unaligned loads over full 4-lane chunks
    // stay in-bounds of `data`.
    unsafe {
        let threshold_vec = _mm_set1_ps(threshold);
        for i in 0..chunks {
            let v = _mm_loadu_ps(data.as_ptr().add(i * 4));

            // Compare: returns 0xFFFFFFFF for true, 0x00000000 for false
            let mask = _mm_cmpgt_ps(v, threshold_vec);

            // Convert mask to integer and count bits
            let mask_int = _mm_movemask_ps(mask);
            count += mask_int.count_ones() as usize;
        }
    }

    for i in (chunks * 4)..(chunks * 4 + remainder) {
        if data[i] > threshold {
            count += 1;
        }
    }

    count
}

// ============================================================================
// Portable SIMD (std::simd - Nightly)
// ============================================================================

/// Portable SIMD examples using std::simd (requires nightly).
///
/// This is the future of SIMD in Rust - cross-platform vectorization.
///
/// # Building this module
///
/// `std::simd` is a nightly-only API guarded by `#![feature(portable_simd)]`, so
/// this module is gated on the `nightly_portable_simd` cfg flag rather than a
/// Cargo feature. That keeps `cargo build --all-features` working on stable
/// (a Cargo feature would be turned on by `--all-features` and fail with E0658).
///
/// To compile it on nightly:
/// 1. Add `#![cfg_attr(nightly_portable_simd, feature(portable_simd))]` to the crate root.
/// 2. Build with `RUSTFLAGS="--cfg nightly_portable_simd" cargo +nightly build`.
#[cfg(nightly_portable_simd)]
#[allow(dead_code)]
mod portable_simd {
    use std::simd::prelude::*;
    use std::simd::{StdFloat, f32x4, f32x8, i32x4};

    /// Add arrays using portable SIMD (4-wide).
    pub fn add_floats_portable(a: &[f32], b: &[f32], result: &mut [f32]) {
        assert_eq!(a.len(), b.len());
        assert_eq!(a.len(), result.len());

        let chunks = a.len() / 4;
        let remainder = a.len() % 4;

        for i in 0..chunks {
            let offset = i * 4;
            let va = f32x4::from_slice(&a[offset..offset + 4]);
            let vb = f32x4::from_slice(&b[offset..offset + 4]);
            let vr = va + vb;
            result[offset..offset + 4].copy_from_slice(vr.as_array());
        }

        for i in (chunks * 4)..(chunks * 4 + remainder) {
            result[i] = a[i] + b[i];
        }
    }

    /// Sum using portable SIMD (8-wide).
    pub fn sum_floats_portable(data: &[f32]) -> f32 {
        let chunks = data.len() / 8;
        let remainder = data.len() % 8;

        let mut sum_vec = f32x8::splat(0.0);

        for i in 0..chunks {
            let offset = i * 8;
            let v = f32x8::from_slice(&data[offset..offset + 8]);
            sum_vec += v;
        }

        let mut result = sum_vec.reduce_sum();

        for i in (chunks * 8)..(chunks * 8 + remainder) {
            result += data[i];
        }

        result
    }

    /// Multiply-add using portable SIMD.
    pub fn mul_add_portable(a: &[f32], b: &[f32], c: &[f32], result: &mut [f32]) {
        assert_eq!(a.len(), b.len());
        assert_eq!(a.len(), c.len());

        let chunks = a.len() / 4;
        let remainder = a.len() % 4;

        for i in 0..chunks {
            let offset = i * 4;
            let va = f32x4::from_slice(&a[offset..offset + 4]);
            let vb = f32x4::from_slice(&b[offset..offset + 4]);
            let vc = f32x4::from_slice(&c[offset..offset + 4]);
            let vr = va.mul_add(vb, vc);
            result[offset..offset + 4].copy_from_slice(vr.as_array());
        }

        for i in (chunks * 4)..(chunks * 4 + remainder) {
            result[i] = a[i].mul_add(b[i], c[i]);
        }
    }

    /// Integer operations with portable SIMD.
    pub fn add_i32_portable(a: &[i32], b: &[i32], result: &mut [i32]) {
        let chunks = a.len() / 4;
        let remainder = a.len() % 4;

        for i in 0..chunks {
            let offset = i * 4;
            let va = i32x4::from_slice(&a[offset..offset + 4]);
            let vb = i32x4::from_slice(&b[offset..offset + 4]);
            let vr = va + vb;
            result[offset..offset + 4].copy_from_slice(vr.as_array());
        }

        for i in (chunks * 4)..(chunks * 4 + remainder) {
            result[i] = a[i].wrapping_add(b[i]);
        }
    }
}

// ============================================================================
// Performance Comparison
// ============================================================================

/// Compare scalar vs SIMD performance for array addition.
#[allow(dead_code)]
fn benchmark_scalar_vs_simd() {
    const SIZE: usize = 1024;
    let a: Vec<f32> = (0..SIZE).map(|i| i as f32).collect();
    let b: Vec<f32> = (0..SIZE).map(|i| (i * 2) as f32).collect();
    let mut result_scalar = vec![0.0; SIZE];
    let mut result_simd = vec![0.0; SIZE];

    // Scalar version
    for i in 0..SIZE {
        result_scalar[i] = a[i] + b[i];
    }

    // SIMD version (SSE)
    add_floats_sse(&a, &b, &mut result_simd);

    // Results should match
    for i in 0..SIZE {
        assert!((result_scalar[i] - result_simd[i]).abs() < f32::EPSILON);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_feature_detection() {
        // Just ensure these don't crash
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        {
            let _ = has_sse42();
            let _ = has_avx();
            let _ = has_avx2();
            let _ = has_fma();
        }
    }

    #[test]
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    fn test_add_floats_sse() {
        let a = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let b = vec![5.0, 4.0, 3.0, 2.0, 1.0];
        let mut result = vec![0.0; 5];

        add_floats_sse(&a, &b, &mut result);

        assert_eq!(result, vec![6.0, 6.0, 6.0, 6.0, 6.0]);
    }

    #[test]
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    fn test_mul_floats_sse() {
        let a = vec![1.0, 2.0, 3.0, 4.0];
        let b = vec![2.0, 3.0, 4.0, 5.0];
        let mut result = vec![0.0; 4];

        mul_floats_sse(&a, &b, &mut result);

        assert_eq!(result, vec![2.0, 6.0, 12.0, 20.0]);
    }

    #[test]
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    fn test_dot_product_sse() {
        let a = vec![1.0, 2.0, 3.0, 4.0];
        let b = vec![5.0, 6.0, 7.0, 8.0];

        let result = dot_product_sse(&a, &b);

        // 1*5 + 2*6 + 3*7 + 4*8 = 5 + 12 + 21 + 32 = 70
        assert!((result - 70.0).abs() < f32::EPSILON);
    }

    #[test]
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    fn test_sum_floats_sse() {
        let data = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let result = sum_floats_sse(&data);
        assert!((result - 15.0).abs() < f32::EPSILON);
    }

    #[test]
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    fn test_max_floats_sse() {
        let data = vec![3.0, 7.0, 2.0, 9.0, 1.0, 5.0];
        let result = max_floats_sse(&data);
        assert!((result - 9.0).abs() < f32::EPSILON);
    }

    #[test]
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    fn test_add_i32_sse() {
        let a = vec![1, 2, 3, 4, 5];
        let b = vec![10, 20, 30, 40, 50];
        let mut result = vec![0; 5];

        add_i32_sse(&a, &b, &mut result);

        assert_eq!(result, vec![11, 22, 33, 44, 55]);
    }

    #[test]
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    fn test_count_greater_sse() {
        let data = vec![1.0, 5.0, 3.0, 8.0, 2.0, 7.0, 9.0, 4.0];
        let count = count_greater_sse(&data, 5.0);
        assert_eq!(count, 3); // 8.0, 7.0, 9.0
    }

    /// Exercises every SAFE SSE wrapper. Each wrapper performs runtime feature
    /// detection internally (SSE2 path or scalar fallback), so this test is sound
    /// on any x86/x86_64 CPU regardless of SSE2 availability, and produces the
    /// same results either way.
    #[test]
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    fn test_sse_safe_wrappers() {
        // Non-multiple-of-4 length to exercise the scalar remainder tail too.
        let a = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let b = vec![10.0, 20.0, 30.0, 40.0, 50.0];

        let mut add = vec![0.0; 5];
        add_floats_sse(&a, &b, &mut add);
        assert_eq!(add, vec![11.0, 22.0, 33.0, 44.0, 55.0]);

        let mut mul = vec![0.0; 5];
        mul_floats_sse(&a, &b, &mut mul);
        assert_eq!(mul, vec![10.0, 40.0, 90.0, 160.0, 250.0]);

        // dot = 10+40+90+160+250 = 550
        assert!((dot_product_sse(&a, &b) - 550.0).abs() < 1e-3);
        assert!((sum_floats_sse(&a) - 15.0).abs() < f32::EPSILON);
        assert!((max_floats_sse(&a) - 5.0).abs() < f32::EPSILON);
        assert_eq!(count_greater_sse(&a, 2.5), 3);

        let ia = vec![1, 2, 3, 4, 5];
        let ib = vec![10, 20, 30, 40, 50];
        let mut isum = vec![0; 5];
        add_i32_sse(&ia, &ib, &mut isum);
        assert_eq!(isum, vec![11, 22, 33, 44, 55]);
    }

    #[test]
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    fn test_avx_if_supported() {
        if has_avx() {
            let a = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
            let b = vec![8.0, 7.0, 6.0, 5.0, 4.0, 3.0, 2.0, 1.0];
            let mut result = vec![0.0; 8];

            unsafe {
                add_floats_avx(&a, &b, &mut result);
            }

            assert_eq!(result, vec![9.0; 8]);
        }
    }

    #[test]
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    fn test_sum_avx_if_supported() {
        if has_avx() {
            let data = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
            let result = unsafe { sum_floats_avx(&data) };
            assert!((result - 36.0).abs() < f32::EPSILON);
        }
    }

    #[test]
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    fn test_avx2_if_supported() {
        if has_avx2() {
            let a = vec![1, 2, 3, 4, 5, 6, 7, 8];
            let b = vec![8, 7, 6, 5, 4, 3, 2, 1];
            let mut result = vec![0; 8];

            unsafe {
                add_i32_avx2(&a, &b, &mut result);
            }

            assert_eq!(result, vec![9; 8]);
        }
    }

    #[test]
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    fn test_sum_i32_avx2_if_supported() {
        if has_avx2() {
            let data = vec![1, 2, 3, 4, 5, 6, 7, 8];
            let result = unsafe { sum_i32_avx2(&data) };
            assert_eq!(result, 36);
        }
    }
}
