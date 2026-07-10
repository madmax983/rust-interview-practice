//! # `SmallVec` Implementation
//!
//! A vector-like contiguous container that stores its elements inline on the stack up to a certain capacity,
//! and falls back to a heap allocation when it grows beyond that capacity.
//!
//! **Replaces Crates:** `smallvec`, `tinyvec`
//!
//! **Real-world Usage:**
//! - High-performance parsers (e.g., HTML/JSON AST nodes with a few children).
//! - Short strings or paths where SSO (Small String Optimization) is beneficial.
//! - Passing a small number of arguments to functions without allocating.
//!
//! **Why build it yourself?**
//! Building `SmallVec` introduces you to `MaybeUninit` for safe uninitialized memory on the stack,
//! manual memory management, and `std::mem::ManuallyDrop`. It shows how to optimize away heap
//! allocations for the common case (small arrays) while retaining the flexibility of `Vec` for
//! edge cases, and demonstrates managing the tricky "inline vs heap" state transitions safely.

use std::mem::MaybeUninit;
use std::ops::{Deref, DerefMut};
use std::ptr;
use std::slice;

// =========================================================================================
// Architecture
// =========================================================================================
//
// Data Structure:
//
//      SmallVec<T, const N: usize>
//      ├── len: usize
//      └── data: enum State
//                  ├── Inline([MaybeUninit<T>; N])
//                  └── Heap(Vec<T>)
//
// Invariants:
// 1. `len` always accurately reflects the number of initialized elements.
// 2. If the state is `Inline`, elements `0..len` are initialized and valid.
// 3. If the state is `Heap`, `len` matches the `Vec`'s length.
// 4. Memory must not leak when `SmallVec` is dropped or panic occurs during insertion.
//
// Complexity:
// ┌───────────┬────────┬───────────┐
// │ Operation │ Time   │ Space     │
// ├───────────┼────────┼───────────┤
// │ push      │ O(1)*  │ O(1)/O(N) │
// │ pop       │ O(1)   │ O(1)      │
// │ index     │ O(1)   │ O(1)      │
// └───────────┴────────┴───────────┘
// * Amortized O(1). Reallocation occurs when switching from inline to heap or when heap grows.
//
// Design Decisions:
// - **Memory Layout**: We use an enum to represent the two states (Inline/Heap). Real `smallvec`
//   uses a tagged union for tighter packing, but `enum` with `MaybeUninit` provides a solid, safe-ish baseline.
// - **Uninitialized Memory**: `MaybeUninit` is crucial because `T` might not implement `Default`,
//   and we don't want to pay the cost of initializing empty slots.

/// Internal representation of the storage.
// RUST INSIGHT:
// Using `enum` here is safe and idiomatic, but has slightly more overhead than a union
// due to the enum discriminant. `smallvec` crate uses union and bit-packing to fit the
// discriminant inside the capacity/length fields, but that requires heavy `unsafe`.
enum Storage<T, const N: usize> {
    Inline([MaybeUninit<T>; N]),
    // PRODUCTION NOTE:
    // A real SmallVec often avoids using `Vec<T>` directly to save space, managing the raw pointer,
    // capacity, and length manually alongside the inline array in a union.
    Heap(Vec<T>),
}

/// A vector that stores up to `N` elements on the stack, spilling to the heap if necessary.
pub struct SmallVec<T, const N: usize> {
    len: usize,
    storage: Storage<T, N>,
}

impl<T, const N: usize> SmallVec<T, N> {
    /// Creates a new, empty `SmallVec` backed by stack storage initially.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            len: 0,
            storage: Storage::Inline(unsafe { MaybeUninit::uninit().assume_init() }),
            // UNSAFE JUSTIFICATION:
            // Creating an uninitialized array of `MaybeUninit<T>` is safe because `MaybeUninit`
            // does not require initialization. The `assume_init` here is just telling the compiler
            // the array itself is formed, though its contents remain uninitialized.
            // In modern Rust, `[MaybeUninit::uninit(); N]` might be unstable for generic T without Copy,
            // so this unsafe block is the standard workaround.
        }
    }

    /// Returns the number of elements in the vector.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.len
    }

    /// Returns `true` if the vector contains no elements.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Returns the capacity of the vector (inline or heap).
    #[must_use]
    pub const fn capacity(&self) -> usize {
        match &self.storage {
            Storage::Inline(_) => N,
            Storage::Heap(v) => v.capacity(),
        }
    }

    /// Appends an element to the back of a collection.
    pub fn push(&mut self, value: T) {
        if self.len < self.capacity() {
            // Fast path: room available.
            match &mut self.storage {
                Storage::Inline(arr) => {
                    arr[self.len].write(value);
                }
                Storage::Heap(v) => {
                    v.push(value);
                }
            }
            self.len += 1;
        } else {
            // Slow path: out of capacity.
            self.spill_and_push(value);
        }
    }

    /// Pops an element from the back of the collection.
    pub fn pop(&mut self) -> Option<T> {
        if self.len == 0 {
            return None;
        }
        self.len -= 1;
        match &mut self.storage {
            Storage::Inline(arr) => {
                // UNSAFE JUSTIFICATION:
                // We just decremented `self.len`, so `arr[self.len]` was previously initialized
                // and is now logically removed. We read it out, transferring ownership.
                Some(unsafe { arr[self.len].assume_init_read() })
            }
            Storage::Heap(v) => {
                // len on SmallVec is synchronized with Vec.
                v.pop()
            }
        }
    }

    /// Transitions from inline storage to heap storage, pushing the new value.
    #[cold]
    fn spill_and_push(&mut self, value: T) {
        match &mut self.storage {
            Storage::Inline(arr) => {
                // GOTCHA:
                // We must be careful not to double-drop elements. We move the elements
                // out of the inline array into a new Vec, taking ownership.
                let mut v = Vec::with_capacity(N.saturating_mul(2).max(1));

                // UNSAFE JUSTIFICATION:
                // We know elements `0..self.len` are initialized.
                // We copy them bitwise to the Vec and set the Vec's length.
                unsafe {
                    let src = arr.as_ptr().cast::<T>();
                    let dst = v.as_mut_ptr();
                    ptr::copy_nonoverlapping(src, dst, self.len);
                    v.set_len(self.len);
                }

                v.push(value);
                self.storage = Storage::Heap(v);
                self.len += 1;
            }
            Storage::Heap(v) => {
                // This shouldn't typically be hit if we only call this when len == capacity,
                // because Vec handles its own reallocation. But just in case:
                v.push(value);
                self.len += 1;
            }
        }
    }
}

impl<T, const N: usize> Default for SmallVec<T, N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T, const N: usize> Drop for SmallVec<T, N> {
    fn drop(&mut self) {
        if let Storage::Inline(arr) = &mut self.storage {
            // UNSAFE JUSTIFICATION:
            // We must manually drop the initialized elements in the inline array.
            // If it's a Heap, the Vec's Drop implementation handles it.
            unsafe {
                let slice = slice::from_raw_parts_mut(arr.as_mut_ptr().cast::<T>(), self.len);
                ptr::drop_in_place(slice);
            }
        }
    }
}

impl<T, const N: usize> Deref for SmallVec<T, N> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        match &self.storage {
            Storage::Inline(arr) => {
                // UNSAFE JUSTIFICATION:
                // Elements `0..self.len` are guaranteed initialized.
                unsafe { slice::from_raw_parts(arr.as_ptr().cast::<T>(), self.len) }
            }
            Storage::Heap(v) => v.as_slice(),
        }
    }
}

impl<T, const N: usize> DerefMut for SmallVec<T, N> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match &mut self.storage {
            Storage::Inline(arr) => {
                // UNSAFE JUSTIFICATION:
                // Elements `0..self.len` are guaranteed initialized.
                unsafe { slice::from_raw_parts_mut(arr.as_mut_ptr().cast::<T>(), self.len) }
            }
            Storage::Heap(v) => v.as_mut_slice(),
        }
    }
}

// =========================================================================================
// Tests
// =========================================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inline_push_pop() {
        let mut vec: SmallVec<i32, 4> = SmallVec::new();
        assert_eq!(vec.capacity(), 4);
        assert!(vec.is_empty());

        vec.push(1);
        vec.push(2);
        vec.push(3);
        assert_eq!(vec.len(), 3);
        assert_eq!(&*vec, &[1, 2, 3]);

        assert_eq!(vec.pop(), Some(3));
        assert_eq!(vec.pop(), Some(2));
        assert_eq!(vec.len(), 1);
    }

    #[test]
    fn test_spill_to_heap() {
        let mut vec: SmallVec<i32, 2> = SmallVec::new();

        vec.push(1);
        vec.push(2);
        assert_eq!(vec.capacity(), 2);

        // This should trigger spill
        vec.push(3);
        assert!(vec.capacity() > 2);
        assert_eq!(vec.len(), 3);
        assert_eq!(&*vec, &[1, 2, 3]);

        vec.push(4);
        assert_eq!(&*vec, &[1, 2, 3, 4]);

        assert_eq!(vec.pop(), Some(4));
        assert_eq!(vec.pop(), Some(3));
        assert_eq!(vec.pop(), Some(2));
        assert_eq!(vec.pop(), Some(1));
        assert_eq!(vec.pop(), None);
    }

    #[test]
    fn test_drop_behavior() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicUsize, Ordering};

        struct DropTracker(Arc<AtomicUsize>);

        impl Drop for DropTracker {
            fn drop(&mut self) {
                self.0.fetch_add(1, Ordering::SeqCst);
            }
        }

        let drops = Arc::new(AtomicUsize::new(0));

        {
            let mut vec: SmallVec<DropTracker, 2> = SmallVec::new();
            vec.push(DropTracker(Arc::clone(&drops)));
            // Drops inline element on drop
        }
        assert_eq!(drops.load(Ordering::SeqCst), 1);

        drops.store(0, Ordering::SeqCst);

        {
            let mut vec: SmallVec<DropTracker, 2> = SmallVec::new();
            vec.push(DropTracker(Arc::clone(&drops)));
            vec.push(DropTracker(Arc::clone(&drops)));
            vec.push(DropTracker(Arc::clone(&drops))); // Spills to heap
            // Drops heap elements on drop
        }
        assert_eq!(drops.load(Ordering::SeqCst), 3);
    }
}

// =========================================================================================
// Footer
// =========================================================================================
// Comparison to `smallvec`:
// The actual `smallvec` crate uses a union instead of an enum to avoid the memory overhead
// of the discriminant and align the lengths. It also provides macros (`smallvec![]`) and implements
// many more standard library traits (`Extend`, `FromIterator`, `IntoIterator`).
//
// What's missing:
// - `Insert` and `Remove` operations with index shifting.
// - `IntoIterator` to avoid leaking when consumed.
// - `Clone`, `Debug`, `PartialEq` bounds implementations.
// - Union-based memory layout for true optimal sizing.
//
// Next steps:
// - Implement `IntoIter` that handles draining from both `Inline` and `Heap` states.
// - Swap `enum` for `union` and manage the discriminant implicitly via a tagged pointer or highest-bit capacity tricks.
