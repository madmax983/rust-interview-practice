//! Bump Allocator (Arena) implementation.
//!
//! # Header
//!
//! *   **Problem Name**: Bump Allocator (Arena)
//! *   **Difficulty**: Medium
//! *   **Link**: <https://os.phil-opp.com/allocator-designs/>
//! *   **Why this matters in Rust**: Demonstrates manual memory management, pointer arithmetic, and lifetime safety.
//!
//! # Architecture
//!
//! A Bump Allocator (or Arena) allocates memory by simply incrementing a pointer. It is extremely fast (O(1)) but cannot free individual objects.
//! All objects are freed at once when the Arena is dropped or reset.
//!
//! **Diagram:**
//!
//! ```text
//! [ Object A | Object B | Object C | ... Free Space ... ]
//! ^ Start    ^ Next                                    ^ End
//! ```
//!
//! **Invariants:**
//! *   Allocated objects must be properly aligned.
//! *   References returned must not outlive the Arena.
//! *   The allocator must not overflow its capacity.
//!
//! **Complexity:**
//!
//! | Operation | Time | Space |
//! | :--- | :--- | :--- |
//! | Alloc | O(1) | O(1) overhead |
//! | Reset | O(1) | O(1) |
//! | Drop | O(1) | O(N) |

use std::alloc::{Layout, alloc, dealloc};
use std::cell::UnsafeCell;
use std::ptr::NonNull;

/// A simple Bump Allocator that manages a fixed-size chunk of memory.
pub struct BumpArena {
    start: NonNull<u8>,
    end: NonNull<u8>,
    next: UnsafeCell<NonNull<u8>>,
}

// SAFETY:
// BumpArena is Send because it owns the memory block. Moving it to another thread is safe.
// BumpArena is !Sync because `alloc` uses `UnsafeCell` without synchronization.
// Concurrent access to `alloc` would cause data races on `next`.
unsafe impl Send for BumpArena {}

impl BumpArena {
    /// Creates a new Arena with the specified capacity in bytes.
    ///
    /// # Panics
    /// Panics if allocation fails or capacity is 0.
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "Capacity must be positive");
        let layout = Layout::from_size_align(capacity, 1).expect("Invalid layout");

        // UNSAFE JUSTIFICATION: We are manually allocating a raw block of memory.
        let ptr = unsafe { alloc(layout) };
        if ptr.is_null() {
            std::alloc::handle_alloc_error(layout);
        }

        let start = NonNull::new(ptr).unwrap();
        // Calculate end pointer. `ptr.add(capacity)` is safe because we just allocated it.
        let end = NonNull::new(unsafe { ptr.add(capacity) }).unwrap();

        BumpArena {
            start,
            end,
            next: UnsafeCell::new(start),
        }
    }

    /// Allocates a value of type `T` in the arena.
    ///
    /// Returns a mutable reference to the allocated value.
    /// The reference lifetime is tied to `&self`, meaning it cannot outlive the arena.
    ///
    /// # Panics
    /// Panics if the arena is out of memory.
    #[allow(clippy::mut_from_ref)]
    pub fn alloc<T>(&self, value: T) -> &mut T {
        let layout = Layout::new::<T>();

        unsafe {
            // Read current pointer
            let current_ptr = *self.next.get();
            let current_addr = current_ptr.as_ptr() as usize;

            // Calculate alignment padding
            let align_offset = (layout.align() - (current_addr % layout.align())) % layout.align();
            let alloc_start = current_addr + align_offset;
            let alloc_end = alloc_start + layout.size();

            // Check capacity
            // Note: We cast to usize for comparison.
            if alloc_end > self.end.as_ptr() as usize {
                panic!("BumpArena out of memory");
            }

            let ptr = alloc_start as *mut T;

            // Write the value into the memory
            // `ptr::write` is safe because we verified bounds and alignment.
            std::ptr::write(ptr, value);

            // Update the next pointer
            let new_next = NonNull::new_unchecked(alloc_end as *mut u8);
            *self.next.get() = new_next;

            // Return a mutable reference.
            // RUST INSIGHT: Returning `&mut T` from `&self`.
            // This is allowed because `UnsafeCell` permits mutation through shared reference.
            // Safety relies on the fact that every call to `alloc` returns a pointer to *new*, disjoint memory.
            // Thus, we never hand out two mutable references to the *same* T.
            // However, this does not prevent creating multiple mutable references to *different* T's within the same arena,
            // which is perfectly safe.
            &mut *ptr
        }
    }

    /// Resets the allocator, clearing all allocations.
    ///
    /// # Safety
    /// This is unsafe because it invalidates all references previously handed out.
    /// The caller must ensure that no references to allocated objects are used after calling reset.
    /// Note: This does NOT run destructors (`Drop`) for allocated objects. They are simply forgotten.
    pub unsafe fn reset(&mut self) {
        *self.next.get_mut() = self.start;
    }

    /// Returns the total capacity of the arena.
    pub fn capacity(&self) -> usize {
        unsafe { self.end.as_ptr().offset_from(self.start.as_ptr()) as usize }
    }

    /// Returns the number of bytes used.
    pub fn used(&self) -> usize {
        unsafe {
            let next = *self.next.get();
            next.as_ptr().offset_from(self.start.as_ptr()) as usize
        }
    }
}

impl Drop for BumpArena {
    fn drop(&mut self) {
        unsafe {
            let capacity = self.end.as_ptr() as usize - self.start.as_ptr() as usize;
            let layout = Layout::from_size_align(capacity, 1).unwrap();
            dealloc(self.start.as_ptr(), layout);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, PartialEq, Eq)]
    struct Point {
        x: i32,
        y: i32,
    }

    #[test]
    fn test_basic_allocation() {
        let arena = BumpArena::new(1024);

        let a = arena.alloc(10u32);
        let b = arena.alloc(20u32);
        let c = arena.alloc(Point { x: 1, y: 2 });

        assert_eq!(*a, 10);
        assert_eq!(*b, 20);
        assert_eq!(*c, Point { x: 1, y: 2 });

        // Mutate
        *a = 30;
        assert_eq!(*a, 30);
    }

    #[test]
    fn test_alignment() {
        let arena = BumpArena::new(1024);

        // Allocate a byte to offset the pointer
        arena.alloc(1u8);

        // Allocate a u32, which requires 4-byte alignment
        let val = arena.alloc(100u32);

        // Check alignment
        let ptr_addr = val as *const _ as usize;
        assert_eq!(ptr_addr % 4, 0, "Address {} is not aligned to 4", ptr_addr);
    }

    #[test]
    #[should_panic(expected = "BumpArena out of memory")]
    fn test_out_of_memory() {
        let arena = BumpArena::new(16);
        arena.alloc(0u64); // 8 bytes
        arena.alloc(0u64); // 8 bytes -> 16 used.
        arena.alloc(1u8); // Boom
    }

    #[test]
    fn test_reset() {
        let mut arena = BumpArena::new(1024);

        let _a = arena.alloc(10);
        assert!(arena.used() >= 4);

        unsafe {
            arena.reset();
        }

        assert_eq!(arena.used(), 0);

        // Allocate again
        let b = arena.alloc(20);
        assert_eq!(*b, 20);
    }
}
