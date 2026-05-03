//! # Buddy Allocator Implementation
//!
//! A memory allocator that divides memory into partitions to try to satisfy a memory request as suitably as possible.
//!
//! **Replaces Crates:** `buddy_system_allocator`, `talc`
//!
//! **Real-world Usage:**
//! - Kernel memory management (Linux kernel's physical page allocator).
//! - Embedded systems (bare-metal) custom allocators (`#[global_allocator]`).
//! - WebAssembly or Arena allocation where heap size is fixed and fragmentation must be managed tightly.
//!
//! **Why build it yourself?**
//! Building a buddy allocator from scratch teaches you about bitwise math and memory fragmentation.
//! Instead of wrestling with raw pointers (`*mut u8`), this implementation abstracts memory as integer offsets.
//! You learn how to use XOR to efficiently locate "buddies" in memory and how to manage a hierarchy of free lists.

use std::alloc::Layout;
use std::collections::{HashMap, HashSet};

// =========================================================================================
// Architecture
// =========================================================================================
//
// Data Structure Diagram:
//
//      Order 3 (Size 8)   [------------------------]  (Free list for size 8: empty)
//                                     / \
//      Order 2 (Size 4)   [----------]   [----------] (Free list for size 4: [0])
//                             / \            / \
//      Order 1 (Size 2)   [----][----]   [----][----] (Free list for size 2: [6])
//                           /\    /\       /\    /\
//      Order 0 (Size 1)   [][]  [][]     [][]  [][]   (Free list for size 1: [4, 5])
//
// Given a block of size `S` at offset `O`, its buddy is located at `O ^ S`.
//
// Invariants:
// 1. Memory is logically divided into power-of-two sizes (orders).
// 2. An allocation request is rounded up to the nearest power of two.
// 3. If a block of the requested size is unavailable, a larger block is split into two halves (buddies).
// 4. When a block is freed, if its buddy is also free, they are coalesced into a larger block.
//
// Complexity:
// ┌────────────┬───────────┬─────────────┐
// │ Operation  │ Time      │ Space       │
// ├────────────┼───────────┼─────────────┤
// │ Allocate   │ O(log N)  │ O(log N)    │
// │ Deallocate │ O(log N)  │ O(1)        │
// └────────────┴───────────┴─────────────┘
// N is the total capacity. The time complexity is bounded by the number of orders (max level).
//
// Design Decisions:
// - **Offset-based Allocation**: Real allocators return raw pointers (`*mut u8`). We return `usize` offsets
//   to represent memory addresses. This makes testing and reasoning completely safe without `unsafe` blocks.
// - **Free Lists**: We use an array of `HashSet<usize>` where the index represents the "order" of the block.
//   - *Alternative*: Production implementations embed intrusive linked lists directly into the free memory blocks
//     to achieve zero-overhead allocation state, but this requires `unsafe` pointer casts.

/// A generalized trait for memory allocators.
pub trait Allocator {
    /// Allocates memory according to the given layout, returning the offset.
    fn allocate(&mut self, layout: Layout) -> Option<usize>;

    /// Deallocates the memory at the given offset with the given layout.
    fn deallocate(&mut self, offset: usize, layout: Layout);
}

/// A Buddy System Allocator operating on integer offsets.
pub struct BuddyAllocator {
    /// Array of free lists. `free_lists[order]` contains the offsets of free blocks of size `2^order`.
    free_lists: Vec<HashSet<usize>>,
    /// Minimum allocation size (order 0). Must be a power of two.
    min_block_size: usize,
    /// Maximum order index.
    max_order: usize,
    /// Keep track of allocations to know the original size when freeing (optional but safe).
    allocations: HashMap<usize, usize>, // offset -> order
}

impl BuddyAllocator {
    /// Creates a new Buddy Allocator capable of allocating up to `capacity` bytes.
    /// `min_block_size` dictates the smallest possible allocation.
    pub fn new(capacity: usize, min_block_size: usize) -> Self {
        assert!(
            capacity.is_power_of_two(),
            "Capacity must be a power of two"
        );
        assert!(
            min_block_size.is_power_of_two(),
            "Min block size must be a power of two"
        );
        assert!(
            capacity >= min_block_size,
            "Capacity must be >= min block size"
        );

        // RUST INSIGHT: `.trailing_zeros()` is a fast hardware intrinsic for log2 on powers of two.
        let max_order = (capacity / min_block_size).trailing_zeros() as usize;

        let mut free_lists = vec![HashSet::new(); max_order + 1];
        // Initially, the entire capacity is one free block of the maximum order at offset 0.
        free_lists[max_order].insert(0);

        Self {
            free_lists,
            min_block_size,
            max_order,
            allocations: HashMap::new(),
        }
    }

    /// Converts a layout size to the required order.
    fn size_to_order(&self, size: usize) -> Option<usize> {
        let size = size.max(self.min_block_size);
        let blocks_needed = (size + self.min_block_size - 1) / self.min_block_size;
        let required_blocks = blocks_needed.next_power_of_two();
        let order = required_blocks.trailing_zeros() as usize;

        if order > self.max_order {
            None
        } else {
            Some(order)
        }
    }

    /// Calculates the size in bytes for a given order.
    fn order_to_size(&self, order: usize) -> usize {
        self.min_block_size * (1 << order)
    }

    /// Gets the offset of a buddy block.
    fn buddy_offset(&self, offset: usize, order: usize) -> usize {
        let size = self.order_to_size(order);
        // GOTCHA: Buddy calculation works precisely because block sizes and offsets are aligned powers of two.
        offset ^ size
    }
}

impl Allocator for BuddyAllocator {
    fn allocate(&mut self, layout: Layout) -> Option<usize> {
        let required_order = self.size_to_order(layout.size())?;

        // 1. Find the smallest available block that is >= required_order
        for order in required_order..=self.max_order {
            if !self.free_lists[order].is_empty() {
                // RUST INSIGHT: HashSet does not guarantee deterministic pop order.
                // We use `.iter().next().copied()` to grab *any* available block.
                let offset = self.free_lists[order].iter().next().copied().unwrap();
                self.free_lists[order].remove(&offset);

                // 2. Split larger blocks down to the required order
                for current_order in (required_order..order).rev() {
                    let size = self.order_to_size(current_order);
                    let buddy_offset = offset + size;
                    self.free_lists[current_order].insert(buddy_offset);
                }

                self.allocations.insert(offset, required_order);
                return Some(offset);
            }
        }

        None // Out of memory
    }

    fn deallocate(&mut self, offset: usize, _layout: Layout) {
        // PRODUCTION NOTE: Real allocators do not usually store a hashmap of allocations.
        // The caller must provide the exact layout (size/alignment) they originally requested,
        // or the allocator stores a header in memory right before the returned pointer.
        let mut order = self
            .allocations
            .remove(&offset)
            .expect("Attempted to free an untracked offset or double free");

        let mut current_offset = offset;

        // 3. Coalesce buddies if possible
        while order < self.max_order {
            let buddy = self.buddy_offset(current_offset, order);

            if self.free_lists[order].contains(&buddy) {
                // Buddy is free! Coalesce them.
                self.free_lists[order].remove(&buddy);
                current_offset = current_offset.min(buddy);
                order += 1;
            } else {
                // Buddy is not free, stop coalescing.
                break;
            }
        }

        self.free_lists[order].insert(current_offset);
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `buddy_system_allocator`: A `no_std` crate that manages raw pointers using intrusive linked
//   lists stored inside the free blocks themselves. This avoids the overhead of `Vec` or `HashSet`.
// - `talc`: A highly optimized allocator that handles fragmentation better than standard buddy allocators.
//
// Missing vs. Production:
// - **Intrusive Free Lists**: We use standard collections (`HashSet`). Bare-metal allocators cannot allocate
//   HashSets to manage their own allocations! They store free list pointers inside the unused memory blocks.
// - **Alignment Handling**: We ignore `Layout::align()` for simplicity, assuming power-of-two offset alignment
//   naturally satisfies most alignment requirements up to the block size.
//
// Suggested Next Steps:
// 1. Refactor to use raw pointers (`*mut u8`) and an intrusive linked list to make it `no_std` compatible.
// 2. Implement `GlobalAlloc` trait so it can be used as `#[global_allocator]`.
//
// Benchmarking Note:
// To benchmark this allocator, use `criterion` to compare `BuddyAllocator::allocate`
// and `BuddyAllocator::deallocate` against the standard system allocator (via `Box::new`).
// Measure the allocation throughput under heavy fragmentation by making randomized
// allocation and deallocation sequences of varying block sizes.
// Example: `b.iter(|| std::hint::black_box(allocator.allocate(layout)))`

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_buddy_allocator_basic() {
        let mut allocator = BuddyAllocator::new(1024, 16);

        // Allocate 16 bytes (order 0)
        let layout1 = Layout::from_size_align(16, 1).unwrap();
        let ptr1 = allocator.allocate(layout1).unwrap();

        // Allocate 32 bytes (order 1)
        let layout2 = Layout::from_size_align(32, 1).unwrap();
        let ptr2 = allocator.allocate(layout2).unwrap();

        // Check they don't overlap. Since HashSet iteration order is non-deterministic,
        // we can't assert exact offsets, but we can verify distance/overlap.
        assert!(ptr1 != ptr2);
        assert!((ptr1 as isize - ptr2 as isize).abs() >= 16);

        allocator.deallocate(ptr1, layout1);
        allocator.deallocate(ptr2, layout2);

        // Should be fully coalesced back to one block of 1024.
        assert_eq!(allocator.free_lists[allocator.max_order].len(), 1);
        assert!(allocator.free_lists[allocator.max_order].contains(&0));
    }

    #[test]
    fn test_out_of_memory() {
        let mut allocator = BuddyAllocator::new(64, 16);
        let layout = Layout::from_size_align(32, 1).unwrap();

        let ptr1 = allocator.allocate(layout).unwrap();
        let ptr2 = allocator.allocate(layout).unwrap();

        // 64 bytes total, we allocated 2x32. The next one should fail.
        let ptr3 = allocator.allocate(layout);
        assert!(ptr3.is_none());

        allocator.deallocate(ptr1, layout);
        allocator.deallocate(ptr2, layout);
    }

    #[test]
    fn test_coalescing() {
        let mut allocator = BuddyAllocator::new(64, 16);
        let layout = Layout::from_size_align(16, 1).unwrap();

        let mut ptrs = Vec::new();
        for _ in 0..4 {
            ptrs.push(allocator.allocate(layout).unwrap());
        }

        // Deallocate in random order (which HashSet might do) or specific order
        // and ensure we coalesce back to root.
        allocator.deallocate(ptrs[2], layout);
        allocator.deallocate(ptrs[1], layout);
        allocator.deallocate(ptrs[3], layout);
        allocator.deallocate(ptrs[0], layout);

        assert_eq!(allocator.free_lists[allocator.max_order].len(), 1);
    }

    #[test]
    fn test_non_deterministic_offsets() {
        let mut allocator = BuddyAllocator::new(128, 16);
        let layout = Layout::from_size_align(16, 1).unwrap();

        let mut offsets = Vec::new();
        for _ in 0..8 {
            offsets.push(allocator.allocate(layout).unwrap());
        }

        // The offsets could be returned in any order because of HashSet.
        // We sort them to verify all 8 unique 16-byte slots were used.
        offsets.sort_unstable();
        let expected: Vec<usize> = (0..8).map(|i| i * 16).collect();
        assert_eq!(offsets, expected);
    }
}
