//! # Ring Buffer (Circular Queue) Implementation
//!
//! A fixed-size, FIFO (First-In-First-Out) buffer that overwrites old data when full (optional) or blocks/fails.
//!
//! **Replaces Crates:** `circular-queue`, `ringbuf`
//!
//! **Real-world Usage:**
//! - Audio processing (buffering samples).
//! - Network drivers (DMA descriptors).
//! - Logging systems (keeping the last N logs in memory).
//!
//! **Why build it yourself?**
//! Understanding circular indexing logic `(i + 1) % N` is fundamental. You also learn about
//! the trade-offs between "one empty slot" vs "explicit count" for full/empty detection.

// =========================================================================================
// Architecture
// =========================================================================================
//
// Data Structure:
// - Vector of Option<T> with fixed capacity.
// - `read` index (head) and `write` index (tail).
// - `count` to track number of elements.
//
// [ None, Some(A), Some(B), None ]
//         ^ read            ^ write
//
// Invariants:
// 1. `read` and `write` are always < capacity.
// 2. `count` <= capacity.
// 3. If count == 0, read == write (usually).
//
// Complexity:
// ┌───────────────┬─────────────┬─────────────┐
// │ Operation     │ Time        │ Space       │
// ├───────────────┼─────────────┼─────────────┤
// │ Push          │ O(1)        │ O(1)        │
// │ Pop           │ O(1)        │ O(1)        │
// │ Peek          │ O(1)        │ O(1)        │
// └───────────────┴─────────────┴─────────────┘

#[derive(Debug, Clone)]
pub struct RingBuffer<T> {
    // RUST INSIGHT:
    // Using Option<T> allows us to distinguish empty slots without unsafe code or requiring T: Default.
    // This is safer than using MaybeUninit but adds a small tag byte overhead.
    buffer: Vec<Option<T>>,
    capacity: usize,
    read: usize,
    write: usize,
    count: usize,
}

impl<T> RingBuffer<T> {
    /// Creates a new `RingBuffer` with the specified capacity.
    ///
    /// # Panics
    /// Panics if `capacity` is 0.
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "Capacity must be greater than 0");
        let mut buffer = Vec::with_capacity(capacity);
        for _ in 0..capacity {
            buffer.push(None);
        }

        Self {
            buffer,
            capacity,
            read: 0,
            write: 0,
            count: 0,
        }
    }

    /// Pushes an item into the buffer.
    /// Returns `Err(item)` if the buffer is full.
    ///
    /// # Errors
    /// Returns `Err(item)` (handing the item back) if the buffer is full.
    pub fn push(&mut self, item: T) -> Result<(), T> {
        if self.count == self.capacity {
            return Err(item);
        }

        self.buffer[self.write] = Some(item);
        self.write = (self.write + 1) % self.capacity;
        self.count += 1;
        Ok(())
    }

    /// Pushes an item into the buffer, overwriting the oldest item if full.
    /// Returns the overwritten item, if any.
    pub fn push_overwrite(&mut self, item: T) -> Option<T> {
        if self.count < self.capacity {
            self.push(item).ok(); // Should always succeed
            None
        } else {
            // Buffer is full. Overwrite at write index.

            let old_val = self.buffer[self.write].take();
            self.buffer[self.write] = Some(item);

            // GOTCHA: Overwriting the write head when full means we also effectively overwrite
            // the read head (the oldest item), so read must advance to the next oldest.
            self.write = (self.write + 1) % self.capacity;
            self.read = (self.read + 1) % self.capacity;
            // count stays same (max)

            old_val
        }
    }

    /// Pops the oldest item from the buffer.
    pub fn pop(&mut self) -> Option<T> {
        if self.count == 0 {
            return None;
        }

        let item = self.buffer[self.read].take();
        self.read = (self.read + 1) % self.capacity;
        self.count -= 1;
        item
    }

    /// Peeks at the oldest item without removing it.
    #[must_use]
    pub fn peek(&self) -> Option<&T> {
        if self.count == 0 {
            None
        } else {
            self.buffer[self.read].as_ref()
        }
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.count == 0
    }

    #[must_use]
    pub const fn is_full(&self) -> bool {
        self.count == self.capacity
    }

    #[must_use]
    pub const fn len(&self) -> usize {
        self.count
    }

    #[must_use]
    pub const fn capacity(&self) -> usize {
        self.capacity
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `circular-queue`: Similar API.
// - `ringbuf`: Lock-free SPSC support.
//
// Missing vs. Production:
// - **Concurrency**: This implementation is not thread-safe. A production concurrent ring buffer
//   would use `AtomicUsize` for indices and `UnsafeCell` for storage to avoid `Mutex` overhead.
// - **Contiguous Slice**: Some ring buffers map memory twice to allow accessing the wrapped content
//   as a single contiguous slice `&[T]`. This requires unsafe memory mapping (VM magic).

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_ops() {
        let mut rb = RingBuffer::new(3);
        assert!(rb.is_empty());

        assert!(rb.push(1).is_ok());
        assert!(rb.push(2).is_ok());
        assert!(rb.push(3).is_ok());

        assert!(rb.is_full());
        assert!(rb.push(4).is_err()); // Full

        assert_eq!(rb.peek(), Some(&1));
        assert_eq!(rb.pop(), Some(1));
        assert_eq!(rb.pop(), Some(2));
        assert_eq!(rb.pop(), Some(3));
        assert_eq!(rb.pop(), None);
    }

    #[test]
    fn test_overwrite() {
        let mut rb = RingBuffer::new(2);
        rb.push(1).unwrap();
        rb.push(2).unwrap();

        // Overwrite 1 with 3
        let overwritten = rb.push_overwrite(3);
        assert_eq!(overwritten, Some(1));

        // Buffer: [3, 2] (logically 2 then 3)
        // read should be at 2

        assert_eq!(rb.pop(), Some(2));
        assert_eq!(rb.pop(), Some(3));
    }

    #[test]
    fn test_wrap_around() {
        let mut rb = RingBuffer::new(3);
        rb.push(1).unwrap();
        rb.push(2).unwrap();
        rb.pop(); // Remove 1. read=1, write=2
        rb.push(3).unwrap(); // write=0 (wrap)
        rb.push(4).unwrap(); // write=1

        // State: [4, None, 2, 3] -> wait, buffer indices:
        // push 1: [1, -, -] w=1
        // push 2: [1, 2, -] w=2
        // pop 1:  [-, 2, -] r=1
        // push 3: [-, 2, 3] w=0 (wrap)
        // push 4: [4, 2, 3] w=1

        assert!(rb.is_full());
        assert_eq!(rb.pop(), Some(2));
        assert_eq!(rb.pop(), Some(3));
        assert_eq!(rb.pop(), Some(4));
    }
}
