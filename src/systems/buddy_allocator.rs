//! # Buddy Allocator
//!
//! Implements a Buddy Memory Allocator from scratch.
//!
//! **Replaces Crates:** `buddy_system_allocator`, `wee_alloc` (conceptual alternative), `talc`
//!
//! **Real-world Usage:**
//! - Core physical page allocator in the Linux kernel.
//! - Underlying memory management for WebAssembly (`#![no_std]`) environments.
//! - Game engine memory subsystems for rapid bulk allocations to avoid fragmentation.
//!
//! **Why build it yourself?**
//! Standard `malloc` is a black box that just "works." Building a buddy allocator demystifies
//! how an operating system or a bare-metal environment hands out memory. You will understand
//! the mechanics of fragmentation (internal vs. external) and the sheer power of bitwise operations
//! for calculating memory offsets and merging freed blocks in `O(1)` time.
//!
//! # Architecture
//!
//! **Data Structure:**
//! A Buddy Allocator divides memory into partitions to try to satisfy a memory request as suitably as possible.
//! It splits memory into halves to try to give a best-fit. When memory is freed, it checks if its "buddy"
//! (the other half of the split) is also free. If so, they merge back together.
//!
//! We represent free lists for various powers of two (orders).
//!
//! ```text
//! Level (Order)   Block Size
//!   3 (Max)       [---------------- 64 bytes ----------------]
//!                 /                                          \
//!   2           [------ 32 bytes ------]   [------ 32 bytes ------]
//!               /                      \
//!   1         [-- 16 --]   [-- 16 --]  [-- 16 --]   [-- 16 --]
//!             /        \
//!   0       [8]  [8]   [8]  [8]       ...
//! ```
//!
//! **Invariants:**
//! 1. Memory is always allocated in powers of two.
//! 2. Two blocks can only merge if they are "buddies" (they came from the same split of a larger block).
//! 3. The base memory address and total size must be aligned to powers of two.
//!
//! **Complexity:**
//! | Operation  | Time Complexity | Space Complexity |
//! | :---       | :---            | :---             |
//! | `alloc`    | `O(log N)`      | `O(1)` overhead  |
//! | `dealloc`  | `O(log N)`      | `O(1)` overhead  |
//!
//! *(Where `N` is the number of orders/levels in the allocator)*
//!
//! **Design Decisions and Tradeoffs:**
//! - **Array-based Free Lists**: Instead of raw pointers embedded in the free memory blocks
//!   (which is how a true bare-metal allocator avoids memory overhead), we use a `Vec`-based arena
//!   for safety and clarity. This means we have an external metadata overhead. A real bare-metal allocator
//!   would cast the free block `&mut [u8]` into a `*mut Node` to build intrusive linked lists.
//! - **Internal Fragmentation**: Because allocations are rounded up to the next power of two,
//!   requesting 33 bytes will allocate a 64-byte block, wasting 31 bytes.
//! - **External Fragmentation**: Extremely low. Buddy merging naturally resists external fragmentation.

use std::collections::HashSet;

// =========================================================================================
// Implementation
// =========================================================================================

/// A memory allocator that uses the buddy system to manage allocations.
///
/// In this safe abstraction, instead of managing raw memory pointers, we manage
/// offsets into an imagined contiguous memory block of size `1 << max_order`.
/// A foundational trait representing a generic memory allocator strategy.
///
/// RUST INSIGHT: By defining a trait first, we enable swappable allocator strategies
/// (e.g., Buddy Allocator, Slab Allocator, Bump Allocator) behind a unified interface.
/// In the standard library, `std::alloc::GlobalAlloc` serves a similar purpose.
pub trait Allocator {
    /// Allocates memory of at least `size` bytes.
    /// Returns the offset/pointer into the memory block if successful, or `None` if out of memory.
    fn alloc(&mut self, size: usize) -> Option<usize>;

    /// Deallocates the block starting at `offset` originally requested with `size`.
    fn dealloc(&mut self, offset: usize, size: usize);
}

pub struct BuddyAllocator {
    /// The maximum order this allocator supports (total memory = 2^max_order bytes).
    max_order: usize,
    /// The minimum order this allocator supports (smallest block = 2^min_order bytes).
    min_order: usize,
    /// Array of free lists. `free_lists[order]` contains a set of starting offsets
    /// for free blocks of size `1 << order`.
    ///
    /// // RUST INSIGHT:
    /// We use a `HashSet` here for `O(1)` removal during buddy merging.
    /// In a true `#![no_std]` environment, this would be an intrusive doubly-linked list
    /// using raw pointers stored directly inside the free memory blocks to ensure zero
    /// heap allocation overhead.
    free_lists: Vec<HashSet<usize>>,
}

impl BuddyAllocator {
    /// Creates a new `BuddyAllocator`.
    ///
    /// The total manageable memory size will be `1 << max_order`.
    /// The smallest allocatable block size will be `1 << min_order`.
    #[must_use]
    pub fn new(max_order: usize, min_order: usize) -> Self {
        assert!(
            max_order >= min_order,
            "max_order must be greater than or equal to min_order"
        );

        let mut free_lists = Vec::with_capacity(max_order + 1);
        for _ in 0..=max_order {
            free_lists.push(HashSet::new());
        }

        // Initially, all memory is one giant free block at offset 0 of size 2^max_order.
        free_lists[max_order].insert(0);

        Self {
            max_order,
            min_order,
            free_lists,
        }
    }

    // --- Private Helpers ---

    /// Converts a requested byte size to the nearest power-of-two order.
    fn size_to_order(&self, size: usize) -> usize {
        let mut order = self.min_order;
        while (1 << order) < size {
            order += 1;
        }
        order
    }

    /// Recursively splits a block at `current_order` down to `target_order`.
    /// Returns the offset of the usable block.
    fn split_down(&mut self, offset: usize, current_order: usize, target_order: usize) -> usize {
        let mut order = current_order;

        while order > target_order {
            order -= 1;

            // GOTCHA: When we split a block of `order + 1` into two blocks of `order`,
            // the two buddies are located at `offset` and `offset + (1 << order)`.
            let buddy_offset = offset + (1 << order);

            // The buddy goes onto the free list, while we continue to split the left half.
            self.free_lists[order].insert(buddy_offset);
        }

        offset
    }

    /// Recursively merges a block at `offset` of `order` with its buddies.
    fn merge_up(&mut self, offset: usize, mut order: usize) {
        let mut current_offset = offset;

        while order < self.max_order {
            // RUST INSIGHT / GOTCHA: The magic of the buddy allocator!
            // Two buddies of order `K` differ in exactly the `K`th bit of their address.
            // By XORing the offset with the size of the block, we instantly find the buddy.
            let buddy_offset = current_offset ^ (1 << order);

            if self.free_lists[order].contains(&buddy_offset) {
                // Buddy is free! Remove it from the free list and merge.
                self.free_lists[order].remove(&buddy_offset);

                // The merged block starts at the lower of the two offsets.
                current_offset = std::cmp::min(current_offset, buddy_offset);
                order += 1;
            } else {
                // Buddy is not free (it's allocated or further split). Stop merging.
                break;
            }
        }

        // Insert the fully merged block into the appropriate free list.
        self.free_lists[order].insert(current_offset);
    }
}

impl Allocator for BuddyAllocator {
    fn alloc(&mut self, size: usize) -> Option<usize> {
        if size == 0 {
            return None;
        }

        let target_order = self.size_to_order(size);
        if target_order > self.max_order {
            return None; // Requested size exceeds maximum possible block
        }

        // Find the smallest available block that is >= target_order
        for current_order in target_order..=self.max_order {
            if !self.free_lists[current_order].is_empty() {
                // RUST INSIGHT: `HashSet::iter().next().copied()` is an idiomatic
                // way to "pop" an arbitrary element from a set.
                let offset = self.free_lists[current_order].iter().next().copied().unwrap();
                self.free_lists[current_order].remove(&offset);

                // Split the block down until we reach the target_order
                return Some(self.split_down(offset, current_order, target_order));
            }
        }

        // OOM (Out of Memory)
        None
    }

    fn dealloc(&mut self, offset: usize, size: usize) {
        // Fix dealloc bug: The original requested size might be smaller than the minimum block size.
        // We calculate the required order for the requested size. `size_to_order` handles sizes < min_order
        // by returning min_order.
        assert!(size >= (1 << self.min_order), "Invalid deallocation size");
        let order = self.size_to_order(size);

        assert!(order <= self.max_order, "Invalid deallocation size");
        assert!(offset % (1 << order) == 0, "Invalid deallocation offset alignment");

        self.merge_up(offset, order);
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `buddy_system_allocator`: A `#![no_std]` crate that uses intrusive linked lists
//   to manage free blocks directly in memory. It wraps the logic in a `Mutex` or spinlock
//   to serve as a `#[global_allocator]`.
// - Our implementation uses `HashSet` and `Vec`, meaning it relies on an existing heap
//   allocator to function. It is a logical simulation of the buddy algorithm.
//
// Missing vs. Production:
// - **Intrusive Pointers**: A real bare-metal allocator cannot use `HashSet`. It must store pointers
//   to the next free block directly inside the free memory itself.
// - **Alignment Requirements**: `alloc::alloc::Layout` provides both size and alignment constraints.
//   Our implementation assumes natural alignment based on the block size.
// - **Thread Safety**: This structure is not `Sync`. It would need a `Mutex` to be used globally.
//
// Next Steps:
// 1. Refactor using raw pointers and intrusive lists to remove the `std` dependency.
// 2. Implement the `std::alloc::GlobalAlloc` trait.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_alloc_exact_power_of_two() {
        // Total memory: 1024 bytes (order 10), minimum block: 8 bytes (order 3)
        let mut allocator = BuddyAllocator::new(10, 3);

        // Request exactly 1024 bytes
        let offset = allocator.alloc(1024);
        assert_eq!(offset, Some(0));

        // Memory should be full now
        assert_eq!(allocator.alloc(8), None);
    }

    #[test]
    fn test_alloc_rounding_up() {
        let mut allocator = BuddyAllocator::new(10, 3); // 1024 total

        // Requesting 6 bytes should give an order 3 block (8 bytes)
        let offset1 = allocator.alloc(6);
        assert_eq!(offset1, Some(0));

        // Requesting 10 bytes should give an order 4 block (16 bytes)
        let offset2 = allocator.alloc(10);
        // It will be allocated right after the first 8 byte block, but since we split,
        // the available blocks will dictate the next offset.
        // Initially 1024 splits down to 512 + 512.
        // Left 512 splits to 256 + 256... down to 8 + 8 at offsets 0 and 8.
        // `offset1` took offset 0. The block at offset 8 (order 3) is too small for 10 bytes.
        // The allocator will find the next available order 4 block.
        // During the split of order 5 (32 bytes) at offset 0, we got order 4 blocks at 0 and 16.
        // Offset 0 was split further. Offset 16 is free!
        assert_eq!(offset2, Some(16));
    }

    #[test]
    fn test_dealloc_and_merge() {
        let mut allocator = BuddyAllocator::new(6, 4); // 64 total, 16 min

        // Allocate two 16-byte blocks
        let o1 = allocator.alloc(16).unwrap(); // offset 0
        let o2 = allocator.alloc(16).unwrap(); // offset 16

        // They shouldn't be the same
        assert_ne!(o1, o2);

        // Allocate a 32-byte block
        let o3 = allocator.alloc(32).unwrap(); // offset 32

        // Memory is full
        assert_eq!(allocator.alloc(16), None);

        // Free the two 16-byte blocks. They are buddies, so they should merge into a 32-byte block at offset 0.
        allocator.dealloc(o1, 16);
        allocator.dealloc(o2, 16);

        // Now we should be able to allocate a 32-byte block
        let o4 = allocator.alloc(32).unwrap();
        assert_eq!(o4, 0);

        // Free everything
        allocator.dealloc(o4, 32);
        allocator.dealloc(o3, 32);

        // They should merge up to a 64-byte block.
        let o5 = allocator.alloc(64).unwrap();
        assert_eq!(o5, 0);
    }

    #[test]
    fn test_out_of_memory() {
        let mut allocator = BuddyAllocator::new(5, 5); // 32 total, 32 min
        assert_eq!(allocator.alloc(32), Some(0));
        assert_eq!(allocator.alloc(32), None);
    }

    #[test]
    fn test_fragmentation() {
        let mut allocator = BuddyAllocator::new(7, 4); // 128 total, 16 min

        // Allocate 4 blocks of 16 bytes
        let mut offsets = vec![
            allocator.alloc(16).unwrap(),
            allocator.alloc(16).unwrap(),
            allocator.alloc(16).unwrap(),
            allocator.alloc(16).unwrap(),
        ];

        offsets.sort_unstable(); // Sort offsets to guarantee 0, 16, 32, 48

        // Free alternating blocks (0 and 32)
        allocator.dealloc(offsets[0], 16);
        allocator.dealloc(offsets[2], 16);

        // We have 32 bytes of free memory, but it's fragmented (external fragmentation).
        // Since buddy system can only merge buddies, 0 and 32 are NOT buddies of order 4.
        // (0's buddy is 16, 32's buddy is 48).
        // Therefore, we cannot allocate a 32-byte block out of the original 64-byte block (which was split).
        // Wait, the total memory is 128 bytes. The remaining 64 bytes is currently a single block!
        // We need to allocate out the rest of the memory to force the fragmentation issue.
        let _o5 = allocator.alloc(64).unwrap(); // Exhaust the remaining 64 bytes

        // NOW we only have 0 and 32 free. They cannot merge into 32.
        assert_eq!(allocator.alloc(32), None);

        // We can still allocate 16-byte blocks
        assert!(allocator.alloc(16).is_some());
    }

    #[test]
    #[should_panic(expected = "Invalid deallocation size")]
    fn test_invalid_dealloc_size() {
        let mut allocator = BuddyAllocator::new(5, 3);
        let o = allocator.alloc(8).unwrap();
        allocator.dealloc(o, 4); // Panics: below min_order
    }

    #[test]
    #[should_panic(expected = "Invalid deallocation offset alignment")]
    fn test_invalid_dealloc_alignment() {
        let mut allocator = BuddyAllocator::new(5, 3);
        allocator.dealloc(4, 8); // Panics: offset 4 is not aligned to 8
    }
}

// ⚡ BENCHMARK NOTE:
// To benchmark this implementation vs standard `malloc`, one would use `criterion`:
// ```rust
// pub fn criterion_benchmark(c: &mut Criterion) {
//     c.bench_function("buddy_alloc", |b| {
//         let mut allocator = BuddyAllocator::new(20, 4); // 1MB total
//         b.iter(|| {
//             let o = allocator.alloc(black_box(128)).unwrap();
//             allocator.dealloc(o, 128);
//         })
//     });
// }
// ```
