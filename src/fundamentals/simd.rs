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

/// Check if CPU supports SSE4.2 (x86/x86_64).
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
use std::arch::x86_64::*;

#[cfg(target_arch = "x86")]
use std::arch::x86::*;

/// Add two arrays of f32 using SSE (4 floats at a time).
///
/// SSE processes 128 bits = 4 x 32-bit floats in parallel.
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub fn add_floats_sse(a: &[f32], b: &[f32], result: &mut [f32]) {
    assert_eq!(a.len(), b.len());
    assert_eq!(a.len(), result.len());

    // Process in chunks of 4
    let chunks = a.len() / 4;
    let remainder = a.len() % 4;

    unsafe {
        for i in 0..chunks {
            let offset = i * 4;

            // Load 4 floats from each array
            let va = _mm_loadu_ps(a.as_ptr().add(offset));
            let vb = _mm_loadu_ps(b.as_ptr().add(offset));

            // Add vectors
            let vr = _mm_add_ps(va, vb);

            // Store result
            _mm_storeu_ps(result.as_mut_ptr().add(offset), vr);
        }
    }

    // Handle remaining elements with scalar code
    for i in (chunks * 4)..(chunks * 4 + remainder) {
        result[i] = a[i] + b[i];
    }
}

/// Multiply two arrays of f32 using SSE.
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub fn mul_floats_sse(a: &[f32], b: &[f32], result: &mut [f32]) {
    assert_eq!(a.len(), b.len());
    assert_eq!(a.len(), result.len());

    let chunks = a.len() / 4;
    let remainder = a.len() % 4;

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
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[must_use]
pub fn dot_product_sse(a: &[f32], b: &[f32]) -> f32 {
    assert_eq!(a.len(), b.len());

    let chunks = a.len() / 4;
    let remainder = a.len() % 4;

    // Accumulator vector (4 partial sums)
    let mut sum_vec = unsafe { _mm_setzero_ps() };

    unsafe {
        for i in 0..chunks {
            let offset = i * 4;
            let va = _mm_loadu_ps(a.as_ptr().add(offset));
            let vb = _mm_loadu_ps(b.as_ptr().add(offset));

            // Multiply and accumulate
            let prod = _mm_mul_ps(va, vb);
            sum_vec = _mm_add_ps(sum_vec, prod);
        }
    }

    // Horizontal sum: reduce 4 lanes to 1
    let mut result = unsafe {
        // Shuffle and add to reduce
        let shuf = _mm_movehdup_ps(sum_vec); // Duplicate high halves
        let sums = _mm_add_ps(sum_vec, shuf);
        let shuf = _mm_movehl_ps(shuf, sums);
        let sums = _mm_add_ss(sums, shuf);
        _mm_cvtss_f32(sums) // Extract lowest float
    };

    // Add remainder with scalar code
    for i in (chunks * 4)..(chunks * 4 + remainder) {
        result += a[i] * b[i];
    }

    result
}

/// Sum all elements in f32 array using SSE.
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[must_use]
pub fn sum_floats_sse(data: &[f32]) -> f32 {
    let chunks = data.len() / 4;
    let remainder = data.len() % 4;

    let mut sum_vec = unsafe { _mm_setzero_ps() };

    unsafe {
        for i in 0..chunks {
            let v = _mm_loadu_ps(data.as_ptr().add(i * 4));
            sum_vec = _mm_add_ps(sum_vec, v);
        }
    }

    // Horizontal sum
    let mut result = unsafe {
        let shuf = _mm_movehdup_ps(sum_vec);
        let sums = _mm_add_ps(sum_vec, shuf);
        let shuf = _mm_movehl_ps(shuf, sums);
        let sums = _mm_add_ss(sums, shuf);
        _mm_cvtss_f32(sums)
    };

    for i in (chunks * 4)..(chunks * 4 + remainder) {
        result += data[i];
    }

    result
}

/// Find maximum value in f32 array using SSE.
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[must_use]
pub fn max_floats_sse(data: &[f32]) -> f32 {
    if data.is_empty() {
        return f32::NEG_INFINITY;
    }

    let chunks = data.len() / 4;
    let remainder = data.len() % 4;

    let mut max_vec = unsafe { _mm_set1_ps(f32::NEG_INFINITY) };

    unsafe {
        for i in 0..chunks {
            let v = _mm_loadu_ps(data.as_ptr().add(i * 4));
            max_vec = _mm_max_ps(max_vec, v);
        }
    }

    // Horizontal max
    let mut result = unsafe {
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
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub fn add_i32_sse(a: &[i32], b: &[i32], result: &mut [i32]) {
    assert_eq!(a.len(), b.len());
    assert_eq!(a.len(), result.len());

    let chunks = a.len() / 4;
    let remainder = a.len() % 4;

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

/// Aligned array for optimal SIMD performance.
///
/// SIMD operations are faster with aligned memory (16-byte for SSE, 32-byte for AVX).
#[repr(align(32))]
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
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[must_use]
pub fn count_greater_sse(data: &[f32], threshold: f32) -> usize {
    let chunks = data.len() / 4;
    let remainder = data.len() % 4;

    let threshold_vec = unsafe { _mm_set1_ps(threshold) };
    let mut count = 0;

    unsafe {
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
#[cfg(feature = "simd-patterns")]
#[allow(dead_code)]
mod portable_simd {
    use std::simd::prelude::*;
    use std::simd::{f32x4, f32x8, i32x4, StdFloat};

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
