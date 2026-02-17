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
//! All objects are freed at once when the Arena is dropped.
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
//! | Dealloc | N/A | N/A |
//! | Drop Arena | O(1) | O(N) |

use std::alloc::{Layout, alloc, dealloc};
use std::cell::UnsafeCell;
use std::marker::PhantomData;
use std::ptr::NonNull;

/// A simple Bump Allocator that manages a fixed-size chunk of memory.
// RUST INSIGHT: We use `UnsafeCell` to allow interior mutability (bumping the pointer)
// even if we hand out shared references to the allocated objects. Wait, no.
// If we use `&self` for alloc, we need interior mutability.
// If we use `&mut self`, we can't allocate multiple objects and use them simultaneously if they borrow from self.
// The classic Arena pattern in Rust uses interior mutability (`RefCell` or `UnsafeCell`) so `alloc` takes `&self`.
pub struct BumpArena {
    start: NonNull<u8>,
    end: NonNull<u8>,
    next: UnsafeCell<NonNull<u8>>,
    // Ensure we are not Send/Sync automatically unless we add synchronization.
    _marker: PhantomData<*mut u8>,
}

// UNSAFE JUSTIFICATION: BumpArena owns the memory block. It is safe to move the Arena to another thread
// because the memory address is stable (heap allocated via `alloc`) and `UnsafeCell` is only accessed
// via `&self`. However, it is NOT `Sync` because `alloc` mutates the internal pointer without synchronization.
unsafe impl Send for BumpArena {}

impl BumpArena {
    /// Creates a new Arena with the specified capacity in bytes.
    pub fn new(capacity: usize) -> Self {
        let layout = Layout::from_size_align(capacity, 1).unwrap();
        // UNSAFE JUSTIFICATION: We are manually allocating a raw block of memory.
        let ptr = unsafe { alloc(layout) };
        if ptr.is_null() {
            std::alloc::handle_alloc_error(layout);
        }

        let start = NonNull::new(ptr).unwrap();
        let end = NonNull::new(unsafe { ptr.add(capacity) }).unwrap();

        BumpArena {
            start,
            end,
            next: UnsafeCell::new(start),
            _marker: PhantomData,
        }
    }

    /// Allocates a value of type `T` in the arena.
    // RUST INSIGHT: The returned reference `&T` has the same lifetime as `&self`.
    // Because `alloc` takes `&self` (shared borrow), multiple objects can be allocated and alive simultaneously.
    // We use `UnsafeCell` to mutate the `next` pointer internally.
    pub fn alloc<T>(&self, value: T) -> &mut T {
        let layout = Layout::new::<T>();

        // Calculate alignment
        // UNSAFE JUSTIFICATION: We read the current pointer to calculate alignment.
        // This is safe because we are single-threaded (not Sync) or would need a Mutex.
        // For this simple implementation, we assume single-threaded access or external synchronization if Sync was implemented (it's not).
        let current_ptr = unsafe { *self.next.get() };
        let current_addr = current_ptr.as_ptr() as usize;

        let align_offset = (layout.align() - (current_addr % layout.align())) % layout.align();
        let alloc_start = current_addr + align_offset;
        let alloc_end = alloc_start + layout.size();

        // Check capacity
        // UNSAFE JUSTIFICATION: Pointer comparison is valid within the same allocation.
        if alloc_end > self.end.as_ptr() as usize {
            panic!("BumpArena out of memory");
        }

        unsafe {
            let ptr = alloc_start as *mut T;
            // Write the value into the memory
            std::ptr::write(ptr, value);

            // Update the next pointer
            let new_next = NonNull::new_unchecked(alloc_end as *mut u8);
            *self.next.get() = new_next;

            // Return a mutable reference.
            // GOTCHA: We are handing out a `&mut T` from a `&self`. This is generally unsafe ("aliasing XOR mutability").
            // However, in an Arena, the objects are distinct.
            // BUT, if `alloc` returns `&mut T`, we can't call `alloc` again if we hold that `&mut T`?
            // No, because `alloc` takes `&self`.
            // Rust allows `&self` and multiple `&mut T` IF the `&mut T` don't overlap and don't alias `self`.
            // The references point to the heap memory owned by `BumpArena`.
            // The compiler sees `&self` -> `&mut T`. This is actually unsound if `T` has interior mutability or if we unsafe-cast it.
            // Standard arenas usually return `&mut T` but ensure `T` doesn't reference `self`'s metadata.
            // Actually, `&self` -> `&mut T` allows creating multiple mutable references to *different* T's, which is fine,
            // provided they don't overlap.
            &mut *ptr
        }
    }
}

impl Drop for BumpArena {
    fn drop(&mut self) {
        // UNSAFE JUSTIFICATION: We must deallocate the memory chunk we allocated in `new`.
        // We do NOT drop the individual objects `T` because we don't know their types or locations anymore.
        // This is a "POD" (Plain Old Data) arena. If `T` implements `Drop`, it will leak!
        // PRODUCTION NOTE: A production Arena (like `bumpalo`) might register destructors or require `T: Copy`.
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
        let arena = BumpArena::new(10); // Small arena
        arena.alloc(0u64); // 8 bytes
        arena.alloc(0u64); // 8 bytes -> Panic
    }
}

// Footer
//
// *   **Comparison**: Similar to `bumpalo` or `typed-arena` but simplified.
// *   **Missing features**: Destructor support (`Drop`), resizing/chaining chunks, thread-safety (`Sync`).
