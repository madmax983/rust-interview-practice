//! # Lock-Free Single-Producer Single-Consumer (SPSC) Ring Buffer
//!
//! A high-performance, thread-safe, lock-free queue for one producer and one consumer.
//!
//! **Replaces Crates:** `rigtorp-spsc`, `heapless::spsc`, `crossbeam-queue` (ArrayQueue)
//!
//! **Real-world Usage:**
//! - Audio processing callbacks (real-time thread to UI thread).
//! - Network packet processing (NIC interrupt handler to userspace).
//! - High-frequency trading (market data feed to strategy engine).
//! - Logging (worker thread to disk writer).
//!
//! **Why build it yourself?**
//! This is the "Hello World" of lock-free programming. It teaches you:
//! 1.  Memory Ordering (`Acquire` / `Release`).
//! 2.  The need for cache-line padding to prevent "False Sharing".
//! 3.  Safe usage of `UnsafeCell` and `MaybeUninit`.
//! 4.  The "Shadow Head/Tail" optimization to reduce atomic contention.

use std::cell::UnsafeCell;
use std::mem::MaybeUninit;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

// =========================================================================================
// Architecture
// =========================================================================================
//
// Layout:
// [ Head (Atomic) ] -- [ Padding ] -- [ Tail (Atomic) ] -- [ Padding ] -- [ Buffer Ptr ]
//
// Shared State (SpscRingBuffer):
// - `buffer`: Box<[UnsafeCell<MaybeUninit<T>>]>. Storage.
//   We use UnsafeCell per element to allow safe concurrent access to distinct elements.
// - `head`: AtomicUsize. Index of the next item to read. Updated by Consumer. Read by Producer.
// - `tail`: AtomicUsize. Index of the next slot to write. Updated by Producer. Read by Consumer.
// - `capacity`: Constant.
//
// Local State (Producer):
// - `local_tail`: usize. Tracks `tail` without atomic loads (since Producer owns tail).
// - `shadow_head`: usize. Cached copy of `head` to avoid reading the atomic `head` constantly.
//
// Local State (Consumer):
// - `local_head`: usize. Tracks `head` without atomic loads (since Consumer owns head).
// - `shadow_tail`: usize. Cached copy of `tail` to avoid reading the atomic `tail` constantly.
//
// Invariants:
// 1. `head` and `tail` are always < 2*capacity (if using power-of-two wrap) or just mod capacity.
//    We'll use strict modulus: `idx % capacity`.
// 2. Buffer full condition: `(tail + 1) % capacity == head`.
// 3. Buffer empty condition: `head == tail`.
// 4. Producer is the ONLY thread writing to `tail` and `buffer[tail]`.
// 5. Consumer is the ONLY thread writing to `head` and reading `buffer[head]`.
//
// Complexity:
// ┌───────────────┬─────────────┬─────────────┐
// │ Operation     │ Time        │ Space       │
// ├───────────────┼─────────────┼─────────────┤
// │ Push          │ O(1)        │ O(1)        │
// │ Pop           │ O(1)        │ O(1)        │
// └───────────────┴─────────────┴─────────────┘

// Cache line size constant (typical for x86/ARM)
const CACHE_LINE_SIZE: usize = 64;

/// Internal shared state.
// RUST INSIGHT:
// #[repr(C)] ensures that the compiler respects the field order and padding.
// Without it, Rust is free to reorder fields to minimize padding, potentially
// defeating our cache line separation strategy (false sharing prevention).
#[repr(C)]
struct Shared<T> {
    // Padding before head to prevent adjacent allocations sharing cache line
    _pad1: [u8; CACHE_LINE_SIZE],

    // Head index (Consumer writes, Producer reads)
    head: AtomicUsize,

    // Padding between head and tail to prevent False Sharing
    _pad2: [u8; CACHE_LINE_SIZE],

    // Tail index (Producer writes, Consumer reads)
    tail: AtomicUsize,

    // Padding after tail
    _pad3: [u8; CACHE_LINE_SIZE],

    // The buffer.
    // Box<[T]> is a fat pointer (ptr + len).
    // UnsafeCell wraps EACH element to allow obtaining mutable pointers to distinct elements
    // concurrently without violating aliasing rules on the whole slice.
    buffer: Box<[UnsafeCell<MaybeUninit<T>>]>,
    capacity: usize,
}

// UNSAFE JUSTIFICATION:
// - Sync: The SpscRingBuffer coordinates access.
//   - Producer strictly owns `tail` writes and `buffer[tail]` writes.
//   - Consumer strictly owns `head` writes and `buffer[head]` reads.
//   - Memory ordering (Release/Acquire) ensures visibility.
// - Send: `T` must be Send because it is moved between threads.
unsafe impl<T: Send> Sync for Shared<T> {}
unsafe impl<T: Send> Send for Shared<T> {}

/// The Producer handle.
pub struct Producer<T> {
    shared: Arc<Shared<T>>,
    local_tail: usize,
    shadow_head: usize,
}

/// The Consumer handle.
pub struct Consumer<T> {
    shared: Arc<Shared<T>>,
    local_head: usize,
    shadow_tail: usize,
}

/// Creates a new SPSC Ring Buffer.
/// Returns a (Producer, Consumer) pair.
pub fn channel<T>(capacity: usize) -> (Producer<T>, Consumer<T>) {
    assert!(capacity > 0, "Capacity must be greater than 0");

    // Allocate buffer with uninitialized data wrapped in UnsafeCell
    let mut buffer = Vec::with_capacity(capacity + 1); // +1 for the "always empty slot" strategy
    for _ in 0..capacity + 1 {
        buffer.push(UnsafeCell::new(MaybeUninit::uninit()));
    }
    let buffer = buffer.into_boxed_slice();

    let shared = Arc::new(Shared {
        _pad1: [0; CACHE_LINE_SIZE],
        head: AtomicUsize::new(0),
        _pad2: [0; CACHE_LINE_SIZE],
        tail: AtomicUsize::new(0),
        _pad3: [0; CACHE_LINE_SIZE],
        buffer,
        capacity: capacity + 1, // Real internal capacity includes the empty slot
    });

    (
        Producer {
            shared: shared.clone(),
            local_tail: 0,
            shadow_head: 0,
        },
        Consumer {
            shared,
            local_head: 0,
            shadow_tail: 0,
        },
    )
}

// Error types
#[derive(Debug, PartialEq, Eq)]
pub struct Full<T>(pub T);

#[derive(Debug, PartialEq, Eq)]
pub struct Empty;

impl<T> Producer<T> {
    /// Pushes an item into the queue.
    /// Returns `Err(item)` if the queue is full.
    pub fn push(&mut self, item: T) -> Result<(), Full<T>> {
        let current_tail = self.local_tail;
        let next_tail = (current_tail + 1) % self.shared.capacity;

        // Check if full.
        // We use shadow_head to avoid loading the atomic head if possible.
        if next_tail == self.shadow_head {
            // Shadow says full. Check the real head.
            let real_head = self.shared.head.load(Ordering::Acquire);
            self.shadow_head = real_head;

            if next_tail == real_head {
                return Err(Full(item));
            }
        }

        // Write the item.
        // SAFETY:
        // 1. We checked that next_tail != head, so this slot is free.
        // 2. We are the only producer.
        // 3. We use UnsafeCell to get a mutable pointer to THIS specific slot.
        unsafe {
            let slot_ptr = self.shared.buffer[current_tail].get();
            slot_ptr.write(MaybeUninit::new(item));
        }

        // Commit the write by updating tail.
        // Release ordering ensures the consumer sees the written item BEFORE seeing the new tail index.
        self.shared.tail.store(next_tail, Ordering::Release);
        self.local_tail = next_tail;

        Ok(())
    }

    /// Returns the capacity of the queue.
    pub fn capacity(&self) -> usize {
        self.shared.capacity - 1
    }
}

impl<T> Consumer<T> {
    /// Pops an item from the queue.
    /// Returns `None` if the queue is empty.
    pub fn pop(&mut self) -> Option<T> {
        let current_head = self.local_head;

        // Check if empty.
        // Use shadow_tail to avoid atomic load.
        if current_head == self.shadow_tail {
            // Shadow says empty. Check real tail.
            let real_tail = self.shared.tail.load(Ordering::Acquire);
            self.shadow_tail = real_tail;

            if current_head == real_tail {
                return None;
            }
        }

        // Read the item.
        // SAFETY:
        // 1. We checked head != tail, so this slot has valid data.
        // 2. We are the only consumer.
        // 3. Acquire load on tail ensured we see the data written by producer.
        let item = unsafe {
            let slot_ptr = self.shared.buffer[current_head].get();
            (*slot_ptr).assume_init_read()
        };

        // Advance head.
        let next_head = (current_head + 1) % self.shared.capacity;

        // Release ordering ensures the producer sees that we've read the item (and the slot is free)
        // only AFTER we have actually read it.
        self.shared.head.store(next_head, Ordering::Release);
        self.local_head = next_head;

        Some(item)
    }
}

// SAFETY: Drop must handle items still in the queue.
impl<T> Drop for Shared<T> {
    fn drop(&mut self) {
        // We need to drop all items currently in the buffer.
        // Since we are in Drop, we have exclusive access.

        // Relaxed loads are fine here because no other threads can be accessing.
        let mut head = self.head.load(Ordering::Relaxed);
        let tail = self.tail.load(Ordering::Relaxed);
        let cap = self.capacity;

        while head != tail {
            unsafe {
                let slot_ptr = self.buffer[head].get();
                // Manually drop the item
                (*slot_ptr).assume_init_drop();
            }
            head = (head + 1) % cap;
        }
    }
}

// Send impls are derived from Shared<T> if T: Send.
// Producer is Send if T is Send.
// Consumer is Send if T is Send.
unsafe impl<T: Send> Send for Producer<T> {}
unsafe impl<T: Send> Send for Consumer<T> {}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `rigtorp-spsc`: Highly optimized C++ port. Very similar architecture.
// - `crossbeam-queue`: ArrayQueue is MPMC (Multi-Producer Multi-Consumer), which uses Compare-And-Swap (CAS) loops.
//   SPSC is faster because it only needs simple Loads/Stores, no CAS.
//
// Missing vs. Production:
// - **Batch Operations**: Production queues often support `push_slice` / `pop_slice` to amortize atomic costs.
// - **Huge Pages**: For extreme low latency, the buffer should be allocated on Huge Pages to reduce TLB misses.
// - **Cpu Hint**: `std::hint::spin_loop()` could be used in a blocking wrapper.

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_simple_push_pop() {
        let (mut p, mut c) = channel(2);

        assert_eq!(p.push(1), Ok(()));
        assert_eq!(p.push(2), Ok(()));
        assert_eq!(p.push(3), Err(Full(3))); // Full

        assert_eq!(c.pop(), Some(1));
        assert_eq!(c.pop(), Some(2));
        assert_eq!(c.pop(), None); // Empty
    }

    #[test]
    fn test_wrap_around() {
        let (mut p, mut c) = channel(3);

        // Fill
        p.push(1).unwrap();
        p.push(2).unwrap();
        p.push(3).unwrap();

        // Pop 2
        assert_eq!(c.pop(), Some(1));
        assert_eq!(c.pop(), Some(2));

        // Push 2 (wrapping)
        p.push(4).unwrap();
        p.push(5).unwrap();

        // Check Full
        assert!(p.push(6).is_err());

        // Drain
        assert_eq!(c.pop(), Some(3));
        assert_eq!(c.pop(), Some(4));
        assert_eq!(c.pop(), Some(5));
        assert_eq!(c.pop(), None);
    }

    #[test]
    fn test_concurrent() {
        let (mut p, mut c) = channel(100);
        const COUNT: usize = 1_000_000;

        let producer = thread::spawn(move || {
            for i in 0..COUNT {
                while let Err(_) = p.push(i) {
                    // Spin
                    std::hint::spin_loop();
                }
            }
        });

        let consumer = thread::spawn(move || {
            let mut received = 0;
            for i in 0..COUNT {
                loop {
                    if let Some(val) = c.pop() {
                        assert_eq!(val, i);
                        received += 1;
                        break;
                    }
                    std::hint::spin_loop();
                }
            }
            received
        });

        producer.join().unwrap();
        let received = consumer.join().unwrap();
        assert_eq!(received, COUNT);
    }

    #[test]
    fn test_drop_cleanup() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static DROP_COUNT: AtomicUsize = AtomicUsize::new(0);

        #[derive(Debug)]
        struct Droppable;
        impl Drop for Droppable {
            fn drop(&mut self) {
                DROP_COUNT.fetch_add(1, Ordering::SeqCst);
            }
        }

        {
            let (mut p, _c) = channel(10);
            p.push(Droppable).unwrap();
            p.push(Droppable).unwrap();
            p.push(Droppable).unwrap();
        } // Both dropped here

        assert_eq!(DROP_COUNT.load(Ordering::SeqCst), 3);
    }
}
