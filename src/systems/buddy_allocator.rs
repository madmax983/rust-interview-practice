//! # Buddy Allocator Implementation
//!
//! Implements a Buddy Memory Allocator from scratch.
//! This demonstrates how operating systems and embedded systems manage memory
//! fragmentation by dividing memory into power-of-two blocks.
//!
//! **Replaces Crates:** `buddy_system_allocator`, `talc`
//!
//! **Real-world Usage:**
//! - Linux kernel physical page allocation.
//! - Embedded systems (`no_std`) memory management.
//! - High-performance custom memory arenas.
//!
//! **Why build it yourself?**
//! Understanding memory allocators is the core of systems programming. The buddy system
//! is an elegant algorithm that achieves fast allocation/deallocation and avoids
//! external fragmentation, while introducing internal fragmentation. It teaches you
//! bitwise operations for block calculations.
//!
//! # Architecture
//!
//! **Data Structure:**
//!
//! Instead of using raw memory pointers (which requires `unsafe`), this implementation
//! manages an abstraction of "offsets" within a logical memory region. This makes it
//! 100% safe Rust while preserving the exact algorithm used in real allocators.
//!
//! A real allocator would use `*mut u8`, but we manage integer offsets `0..total_size`.
//!
//! ```text
//! Order 3 (Size 8)   [------------------------0-----------------------]
//!                    /                                                \
//! Order 2 (Size 4)   [-----------0------------]                       [-----------4------------]
//!                    /                        \                       /                        \
//! Order 1 (Size 2)   [----0----]              [----2----]             [----4----]              [----6----]
//!                    /         \              /         \             /         \              /         \
//! Order 0 (Size 1)   [--0--]   [--1--]        [--2--]   [--3--]       [--4--]   [--5--]        [--6--]   [--7--]
//! ```
//!
//! **Invariants:**
//! 1. Memory sizes and block sizes are always powers of 2.
//! 2. A block of size `S` at offset `O` can only be split into two blocks of size `S/2` at `O` and `O + S/2`.
//! 3. The "buddy" of a block at offset `O` with size `S` is at `O ^ S`.
//!
//! **Time Complexity:**
//! - Allocation: O(log N) where N is the total memory size.
//! - Deallocation: O(log N) (due to coalescing).
//!
//! **Space Complexity:**
//! - Overhead: `O(N/min_block_size)` nodes in the free lists.

use std::collections::HashSet;

// =========================================================================================
// Traits
// =========================================================================================

/// A generic allocator trait, similar to `std::alloc::GlobalAlloc` but dealing with offsets
/// instead of raw pointers for educational safety.
pub trait Allocator {
    /// Allocate a block of memory of at least `size` bytes.
    /// Returns the offset to the start of the block, or None if out of memory.
    fn allocate(&mut self, size: usize) -> Option<usize>;

    /// Deallocate a block of memory starting at `offset` with the given `size`.
    /// The size must match the allocated size exactly.
    fn deallocate(&mut self, offset: usize, size: usize);
}

// =========================================================================================
// Implementation
// =========================================================================================

/// A safe Buddy Allocator using integer offsets.
pub struct BuddyAllocator {
    /// Total size of managed memory. Must be a power of 2.
    total_size: usize,
    /// Minimum block size. Must be a power of 2.
    min_block_size: usize,
    /// Number of distinct block sizes (orders).
    max_order: usize,
    /// Free lists. Index `i` holds the free blocks of size `min_block_size * 2^i`.
    /// We use a `HashSet` to quickly add/remove blocks. In a real allocator (C/unsafe Rust),
    /// this would be an intrusive linked list embedded in the free memory blocks themselves.
    free_lists: Vec<HashSet<usize>>,
}

impl BuddyAllocator {
    /// Creates a new Buddy Allocator.
    ///
    /// Both `total_size` and `min_block_size` must be powers of two.
    ///
    /// # Panics
    /// Panics if `total_size` or `min_block_size` is not a power of two.
    #[must_use]
    pub fn new(total_size: usize, min_block_size: usize) -> Self {
        assert!(
            total_size.is_power_of_two(),
            "Total size must be a power of two"
        );
        assert!(
            min_block_size.is_power_of_two(),
            "Min block size must be a power of two"
        );
        assert!(
            total_size >= min_block_size,
            "Total size must be >= min block size"
        );

        let max_order = (total_size / min_block_size).trailing_zeros() as usize;
        let mut free_lists = vec![HashSet::new(); max_order + 1];

        // Initially, all memory is one giant free block of max order
        free_lists[max_order].insert(0);

        Self {
            total_size,
            min_block_size,
            max_order,
            free_lists,
        }
    }

    /// Converts a requested size into an order index.
    fn size_to_order(&self, size: usize) -> usize {
        let max_size = std::cmp::max(size, self.min_block_size);
        let next_power_of_two = max_size.next_power_of_two();
        (next_power_of_two / self.min_block_size).trailing_zeros() as usize
    }

    /// Converts an order index to its block size in bytes.
    const fn order_to_size(&self, order: usize) -> usize {
        self.min_block_size << order
    }
}

impl Allocator for BuddyAllocator {
    fn allocate(&mut self, size: usize) -> Option<usize> {
        if size > self.total_size {
            return None;
        }

        let target_order = self.size_to_order(size);
        if target_order > self.max_order {
            return None;
        }

        // 1. Find the smallest available block that is >= target_order
        let mut found_order = target_order;
        while found_order <= self.max_order && self.free_lists[found_order].is_empty() {
            found_order += 1;
        }

        // Out of memory (or fragmentation too high)
        if found_order > self.max_order {
            return None;
        }

        // 2. Remove the found block from its free list
        // RUST INSIGHT: HashSet's iter().next().copied() is a quick way to pop an arbitrary element.
        // We use an arbitrary element because any block of the correct size works.
        let offset = {
            let iter = self.free_lists[found_order].iter().next().copied();
            let val = iter.unwrap();
            self.free_lists[found_order].remove(&val);
            val
        };

        // 3. Split the block down to the target_order
        // If we found a block larger than needed, we split it in half repeatedly.
        while found_order > target_order {
            found_order -= 1;
            let block_size = self.order_to_size(found_order);

            // The left half is our current offset, which we will continue to split or use.
            // The right half (buddy) goes into the free list of the new smaller order.
            let buddy_offset = offset + block_size;
            self.free_lists[found_order].insert(buddy_offset);
        }

        Some(offset)
    }

    fn deallocate(&mut self, mut offset: usize, size: usize) {
        let mut order = self.size_to_order(size);

        // PRODUCTION NOTE: A real allocator wouldn't require `size` to be passed to `deallocate`.
        // It would store metadata (like block size) in a header just before the block pointer.
        // We require it here to keep the offset abstraction pure and simple.

        // Coalesce blocks upwards
        while order < self.max_order {
            let block_size = self.order_to_size(order);

            // GOTCHA: The bitwise XOR trick is the heart of the buddy allocator.
            // Two blocks are buddies if and only if their addresses differ only by the block size bit.
            // Example for size 4: Block 0 (0b000) and Block 4 (0b100) are buddies: 0 ^ 4 = 4, 4 ^ 4 = 0.
            let buddy_offset = offset ^ block_size;

            // If the buddy is free, remove it from the free list, merge them, and move up an order.
            if self.free_lists[order].contains(&buddy_offset) {
                self.free_lists[order].remove(&buddy_offset);
                // The new offset is the start of the merged block (the smaller of the two offsets)
                offset = std::cmp::min(offset, buddy_offset);
                order += 1;
            } else {
                // Buddy is not free, we can't coalesce further.
                break;
            }
        }

        // Insert the coalesced block into the appropriate free list
        self.free_lists[order].insert(offset);
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `buddy_system_allocator`: A `no_std` crate that implements a global allocator for bare-metal
//   Rust. It uses an intrusive linked list to store free blocks in the unallocated memory space
//   itself, meaning it has zero metadata overhead.
// - Our implementation uses `HashSet` to manage free lists for educational simplicity and memory
//   safety, which consumes external memory (metadata overhead).
//
// Missing vs. Production:
// - No intrinsic headers: We require `size` on deallocation, whereas production allocators store
//   headers in memory.
// - Concurrency: Global allocators must be thread-safe (wrapped in Mutex/Spinlock).
// - Intrusive linked lists: As noted, production allocators don't allocate vectors and hash sets
//   to keep track of memory; they write pointers directly into the free memory blocks.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initialization() {
        let allocator = BuddyAllocator::new(1024, 16);
        assert_eq!(allocator.total_size, 1024);
        assert_eq!(allocator.min_block_size, 16);
        assert_eq!(allocator.max_order, 6); // 1024 / 16 = 64 = 2^6

        // Only the max order should have a free block
        for i in 0..6 {
            assert!(allocator.free_lists[i].is_empty());
        }
        assert_eq!(allocator.free_lists[6].len(), 1);
        assert!(allocator.free_lists[6].contains(&0));
    }

    #[test]
    fn test_basic_allocation() {
        let mut allocator = BuddyAllocator::new(1024, 16);

        // Allocate 16 bytes. It should split the 1024 block all the way down to order 0.
        let offset = allocator.allocate(16).unwrap();
        assert_eq!(offset, 0);

        // Check the state of the free lists.
        // Order 0 (16b): block at 16
        // Order 1 (32b): block at 32
        // Order 2 (64b): block at 64
        // Order 3 (128b): block at 128
        // Order 4 (256b): block at 256
        // Order 5 (512b): block at 512
        // Order 6 (1024b): empty
        assert_eq!(allocator.free_lists[0].len(), 1);
        assert!(allocator.free_lists[0].contains(&16));

        assert_eq!(allocator.free_lists[1].len(), 1);
        assert!(allocator.free_lists[1].contains(&32));

        assert_eq!(allocator.free_lists[5].len(), 1);
        assert!(allocator.free_lists[5].contains(&512));

        assert!(allocator.free_lists[6].is_empty());
    }

    #[test]
    fn test_allocation_rounding() {
        let mut allocator = BuddyAllocator::new(1024, 16);

        // Requesting 20 bytes should round up to the next power of 2 (32 bytes).
        let offset = allocator.allocate(20).unwrap();
        assert_eq!(offset, 0);

        // Check that order 0 (16 bytes) is empty because we consumed a 32-byte block directly.
        assert!(allocator.free_lists[0].is_empty());

        // The remaining 32-byte buddy should be in order 1.
        assert_eq!(allocator.free_lists[1].len(), 1);
        assert!(allocator.free_lists[1].contains(&32));
    }

    #[test]
    fn test_multiple_allocations() {
        let mut allocator = BuddyAllocator::new(128, 16);

        let mut offsets = vec![];
        for _ in 0..8 {
            offsets.push(allocator.allocate(16).unwrap());
        }

        // Must sort because HashSet iteration order is non-deterministic,
        // so the actual offsets returned might come in a random order from the tree.
        offsets.sort_unstable();

        assert_eq!(offsets, vec![0, 16, 32, 48, 64, 80, 96, 112]);

        // The allocator should now be full
        assert!(allocator.allocate(16).is_none());

        for i in 0..=allocator.max_order {
            assert!(allocator.free_lists[i].is_empty());
        }
    }

    #[test]
    fn test_deallocation_and_coalescing() {
        let mut allocator = BuddyAllocator::new(64, 16);

        let o1 = allocator.allocate(16).unwrap();
        let o2 = allocator.allocate(16).unwrap();
        let o3 = allocator.allocate(16).unwrap();
        let o4 = allocator.allocate(16).unwrap();

        assert!(allocator.allocate(16).is_none());

        // Deallocate o1 (0) and o2 (16). They are buddies, so they should coalesce into a 32-byte block.
        allocator.deallocate(o1, 16);
        allocator.deallocate(o2, 16);

        // We should now have a 32-byte block free.
        assert_eq!(allocator.free_lists[1].len(), 1);

        // Deallocate o3 and o4. They should coalesce to 32, and then the two 32s should coalesce to 64.
        allocator.deallocate(o3, 16);
        allocator.deallocate(o4, 16);

        assert!(allocator.free_lists[0].is_empty());
        assert!(allocator.free_lists[1].is_empty());
        assert_eq!(allocator.free_lists[2].len(), 1);
        assert!(allocator.free_lists[2].contains(&0));
    }
}
// Next Steps:
// 1. Port this logic to a `no_std` environment using raw pointers and `core::alloc::GlobalAlloc`.
// 2. Implement a Slab Allocator on top of this Buddy Allocator to manage small, fixed-size objects (e.g., `< 64 bytes`).
//
// Benchmarking Note:
// To benchmark this implementation, you would typically use Criterion.rs or `std::time::Instant` around
// a tight loop of random `allocate()` and `deallocate()` calls of varying sizes, comparing the throughput
// against the system allocator (`std::alloc::System`) or another custom allocator crate.
