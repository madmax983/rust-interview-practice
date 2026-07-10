//! # Bytes Implementation
//!
//! A custom byte slice container that enables cheap, zero-copy splitting and slicing by
//! managing the underlying allocation with an atomic reference count.
//!
//! **Replaces Crates:** `bytes`
//!
//! **Real-world Usage:**
//! - Networking code (TCP/HTTP parsing) where packets are split into headers/bodies.
//! - Protocol buffers and serialization.
//! - I/O buffers where you need to pass ownership of a slice to another thread without copying.
//!
//! **Why build it yourself?**
//! The `bytes` crate is ubiquitous in Rust async ecosystems (like Tokio and Hyper). Building it
//! teaches you the difference between a pointer to an allocation and a pointer into an allocation.
//! You'll learn how to implement `Clone` that just bumps an `Arc` reference count, and how to
//! decouple the "view" of the data (ptr + len) from the "owner" of the data (`Arc<[u8]>`).

use std::ops::Deref;
use std::sync::Arc;

// =========================================================================================
// Architecture
// =========================================================================================
//
// Data Structure:
//
//      Bytes
//      ├── ptr: *const u8  (Points anywhere inside the shared allocation)
//      ├── len: usize      (Length of this specific view)
//      └── data: Arc<[u8]> (The actual heap allocation, keeping it alive)
//
// Invariants:
// 1. `ptr` and `len` define a valid slice `[u8]` that is completely contained within `data`.
// 2. The memory pointed to by `ptr` is kept alive because `Bytes` holds a clone of `Arc<[u8]>`.
// 3. Since the underlying `data` is `Arc<[u8]>` (immutable), it's safe to have multiple `Bytes`
//    instances pointing to overlapping or disjoint regions of it.
//
// Complexity:
// ┌───────────┬────────┬────────┐
// │ Operation │ Time   │ Space  │
// ├───────────┼────────┼────────┤
// │ clone     │ O(1)   │ O(1)   │ (Atomic increment)
// │ split_off │ O(1)   │ O(1)   │ (Atomic increment + pointer math)
// │ slice     │ O(1)   │ O(1)   │ (Atomic increment + pointer math)
// └───────────┴────────┴────────┘
//
// Design Decisions:
// - **`Arc<[u8]>` vs `Arc<Vec<u8>>`**: We use `Arc<[u8]>` because the data is strictly immutable
//   once inside `Bytes`. `Arc<[u8]>` avoids the double indirection of `Arc<Vec<u8>>`.
// - **Opaque Pointers**: We explicitly store `ptr` and `len` separate from the `Arc` so a `Bytes`
//   instance can represent a sub-slice. The real `bytes` crate uses a custom vtable and inline
//   storage optimization, but the core concept is exactly this decoupled view/owner structure.

/// A cheaply cloneable and sliceable chunk of contiguous memory.
#[derive(Clone)]
pub struct Bytes {
    /// Pointer to the start of the view.
    ptr: *const u8,
    /// Length of the view.
    len: usize,
    /// The underlying shared allocation that keeps the memory alive.
    // RUST INSIGHT:
    // Even though `ptr` points into `data`, we must hold onto `data` to prevent the
    // `Arc` count from dropping to zero and freeing the memory.
    #[allow(dead_code)] // Stored only for its Drop side-effects (refcount decrement)
    data: Arc<[u8]>,
}

// UNSAFE JUSTIFICATION:
// `Bytes` is `Send` and `Sync` because the underlying data is immutable (`Arc<[u8]>` is Send/Sync)
// and `ptr` merely aliases into it safely.
unsafe impl Send for Bytes {}
unsafe impl Sync for Bytes {}

impl Bytes {
    /// Creates a new `Bytes` from a static slice.
    /// This requires an allocation to create the `Arc`, mimicking `Bytes::copy_from_slice`.
    #[must_use]
    pub fn copy_from_slice(data: &[u8]) -> Self {
        let arc: Arc<[u8]> = Arc::from(data);
        Self {
            ptr: arc.as_ptr(),
            len: arc.len(),
            data: arc,
        }
    }

    /// Creates an empty `Bytes` instance.
    #[must_use]
    pub fn new() -> Self {
        Self::copy_from_slice(&[])
    }

    /// Returns the length of the view.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.len
    }

    /// Returns `true` if the view is empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Returns a slice of the view.
    #[must_use]
    pub const fn as_slice(&self) -> &[u8] {
        // UNSAFE JUSTIFICATION:
        // `ptr` and `len` are guaranteed to be valid and within the bounds of `self.data`.
        // The memory is kept alive by `self.data`.
        unsafe { std::slice::from_raw_parts(self.ptr, self.len) }
    }

    /// Returns a new `Bytes` that points to a sub-slice of this one.
    /// Panics if the range is out of bounds.
    #[must_use]
    pub fn slice(&self, range: std::ops::Range<usize>) -> Self {
        assert!(range.end <= self.len, "slice out of bounds");
        assert!(range.start <= range.end, "invalid slice range");

        // UNSAFE JUSTIFICATION:
        // Pointer arithmetic is safe because we verified bounds.
        let new_ptr = unsafe { self.ptr.add(range.start) };
        let new_len = range.end - range.start;

        Self {
            ptr: new_ptr,
            len: new_len,
            data: Arc::clone(&self.data), // O(1) atomic bump
        }
    }

    /// Splits the buffer into two at the given index.
    /// `self` becomes `[0..at]`, and the returned `Bytes` becomes `[at..len]`.
    pub fn split_off(&mut self, at: usize) -> Self {
        assert!(at <= self.len, "split_off out of bounds");

        let tail_len = self.len - at;
        // UNSAFE JUSTIFICATION:
        // Pointer arithmetic is safe because we verified bounds.
        let tail_ptr = unsafe { self.ptr.add(at) };

        let tail = Self {
            ptr: tail_ptr,
            len: tail_len,
            data: Arc::clone(&self.data),
        };

        self.len = at;
        tail
    }

    /// Shortens the buffer, keeping the first `len` bytes and dropping the rest.
    pub const fn truncate(&mut self, len: usize) {
        if len < self.len {
            self.len = len;
        }
    }

    /// Advances the start of the buffer by `cnt` bytes.
    pub fn advance(&mut self, cnt: usize) {
        assert!(cnt <= self.len, "advance out of bounds");
        // UNSAFE JUSTIFICATION:
        // Pointer arithmetic is safe because we verified bounds.
        self.ptr = unsafe { self.ptr.add(cnt) };
        self.len -= cnt;
    }
}

impl Deref for Bytes {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

impl From<Vec<u8>> for Bytes {
    fn from(vec: Vec<u8>) -> Self {
        let arc: Arc<[u8]> = Arc::from(vec);
        Self {
            ptr: arc.as_ptr(),
            len: arc.len(),
            data: arc,
        }
    }
}

impl From<&'static [u8]> for Bytes {
    // A more optimal implementation would avoid the Arc allocation for static strings,
    // but for simplicity we treat it like a normal slice here.
    fn from(slice: &'static [u8]) -> Self {
        Self::copy_from_slice(slice)
    }
}

impl Default for Bytes {
    fn default() -> Self {
        Self::new()
    }
}

// =========================================================================================
// Tests
// =========================================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bytes_basic() {
        let b = Bytes::from(vec![1, 2, 3, 4, 5]);
        assert_eq!(b.len(), 5);
        assert_eq!(&*b, &[1, 2, 3, 4, 5]);
    }

    #[test]
    fn test_slice() {
        let b = Bytes::from(vec![1, 2, 3, 4, 5]);
        let sub = b.slice(1..4);

        assert_eq!(sub.len(), 3);
        assert_eq!(&*sub, &[2, 3, 4]);

        // Ensure they point to the same underlying data (Arc ref count bumped)
        assert_eq!(Arc::strong_count(&b.data), 2);
    }

    #[test]
    fn test_split_off() {
        let mut b = Bytes::from(vec![1, 2, 3, 4, 5]);
        let tail = b.split_off(2);

        assert_eq!(&*b, &[1, 2]);
        assert_eq!(&*tail, &[3, 4, 5]);
        assert_eq!(Arc::strong_count(&b.data), 2);
    }

    #[test]
    fn test_advance_and_truncate() {
        let mut b = Bytes::from(vec![1, 2, 3, 4, 5]);

        b.advance(2);
        assert_eq!(&*b, &[3, 4, 5]);

        b.truncate(2);
        assert_eq!(&*b, &[3, 4]);
    }

    #[test]
    #[should_panic(expected = "slice out of bounds")]
    fn test_slice_out_of_bounds() {
        let b = Bytes::from(vec![1, 2, 3]);
        let _ = b.slice(0..4);
    }
}

// =========================================================================================
// Footer
// =========================================================================================
// Comparison to `bytes`:
// The production `bytes` crate is significantly more complex. `Bytes` there is 4 words long and
// uses inline storage for strings <= 14 bytes (on 64-bit systems) to avoid allocations entirely.
// It also has specialized vtables for static strings, standard `Arc`-backed strings, and custom
// allocators.
//
// What's missing:
// - `BytesMut`: A mutable variant that allows appending and transitioning to immutable `Bytes`.
// - Inline small-string optimization (SSO).
// - Zero-allocation `&'static [u8]` support.
// - `Buf` and `BufMut` trait implementations for reading/writing primitive types (e.g., `get_u32`).
//
// Next steps:
// - Implement `BytesMut` with a capacity and `split_to` to mimic typical async networking buffers.
