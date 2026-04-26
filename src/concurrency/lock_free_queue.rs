//! # Lock-Free Bounded MPMC Queue Implementation
//!
//! A thread-safe, lock-free, bounded Multi-Producer Multi-Consumer (MPMC) queue.
//!
//! **Replaces Crates:** `crossbeam-queue` (specifically `ArrayQueue`)
//!
//! **Real-world Usage:**
//! - High-throughput thread pools (task scheduling).
//! - Low-latency audio or network packet processing pipelines.
//! - Passing messages between threads without OS-level context switches (Mutex blocking).
//!
//! **Why build it yourself?**
//! Lock-free programming is considered dark magic by many engineers. Building a lock-free queue
//! demystifies atomics, memory ordering (`Relaxed`, `Acquire`, `Release`), and hardware concepts
//! like "false sharing". You'll learn how to safely coordinate multiple threads modifying a shared
//! ring buffer without using a single `Mutex`.

use std::cell::UnsafeCell;
use std::mem::MaybeUninit;
use std::sync::atomic::{AtomicUsize, Ordering};

// =========================================================================================
// Architecture
// =========================================================================================
//
// Data Structure:
//
//   Head (Dequeue)                         Tail (Enqueue)
//     │                                      │
//     ▼                                      ▼
//   ┌─────┬─────┬─────┬─────┬─────┬─────┬─────┬─────┐
//   │ Seq │ Seq │ Seq │ Seq │ Seq │ Seq │ Seq │ Seq │  <-- Sequence Array
//   ├─────┼─────┼─────┼─────┼─────┼─────┼─────┼─────┤
//   │Data │Data │Data │Data │Data │Data │Data │Data │  <-- Data Array (UnsafeCell)
//   └─────┴─────┴─────┴─────┴─────┴─────┴─────┴─────┘
//
// Invariants:
// 1. **Sequence Array**: Each slot in the buffer has an atomic `sequence` number.
//    - For a slot at index `i`, it is ready for **enqueue** if `sequence == i`.
//    - It is ready for **dequeue** if `sequence == i + 1`.
// 2. **Head/Tail Counters**: Atomic indices that continuously increment. They are mapped to buffer
//    indices via modulo (`& (capacity - 1)` if power of two).
// 3. **Capacity**: Must be a power of two to allow fast modulo via bitwise AND.
//
// Complexity:
// ┌───────────┬──────────────┬────────┐
// │ Operation │ Time         │ Space  │
// ├───────────┼──────────────┼────────┤
// │ push      │ O(1) expected│ O(N)   │
// │ pop       │ O(1) expected│ O(1)   │
// └───────────┴──────────────┴────────┘
//
// Design Decisions & Tradeoffs:
// - **Bounded Array**: Bounded lock-free queues avoid ABA problems inherent in linked-list approaches
//   because nodes aren't recycled in arbitrary order. We avoid memory allocation during runtime.
// - **Sequence Counters**: We use sequence counters per slot rather than a generic Compare-And-Swap (CAS)
//   loop on the head/tail pointers alone. This is Dimitry Vyukov's MPMC queue design, heavily minimizing
//   CAS contention by letting threads independently acquire slots.
// - **Cache Line Padding**: To prevent false sharing, `head` and `tail` should be padded so they fall
//   on different CPU cache lines.

/// A slot inside the queue.
struct Slot<T> {
    /// The atomic sequence number used to coordinate accesses to this slot.
    sequence: AtomicUsize,
    /// The actual data. We use `UnsafeCell<MaybeUninit<T>>` to safely handle uninitialized memory
    /// and interior mutability without locks.
    data: UnsafeCell<MaybeUninit<T>>,
}

// RUST INSIGHT: `UnsafeCell` is the only legal way to obtain a mutable reference (`&mut`)
// to data through a shared reference (`&`). It tells the compiler to opt-out of certain
// aliasing optimizations.

// PRODUCTION NOTE: Cache Line Padding
// In a real `crossbeam-queue`, `head` and `tail` are wrapped in a `CachePadded` struct.
// CPUs load memory in chunks called cache lines (typically 64 bytes). If `head` and `tail`
// share the same cache line, threads pushing (modifying `tail`) and popping (modifying `head`)
// will continuously invalidate each other's L1 caches, causing severe "false sharing" performance degradation.
// We omit it here for simplicity, but it is critical for production performance.
// e.g., `#[repr(align(64))] struct CachePadded<T>(T);`

/// A trait defining a basic concurrent queue.
pub trait Queue<T> {
    /// Attempts to push an item into the queue.
    fn push(&self, item: T) -> Result<(), PushError<T>>;
    /// Attempts to pop an item from the queue.
    fn pop(&self) -> Option<T>;
}

/// A lock-free bounded MPMC queue.
pub struct ArrayQueue<T> {
    /// The array of slots.
    buffer: Box<[Slot<T>]>,
    /// Bitmask for fast modulo operations (capacity - 1).
    mask: usize,
    /// The index where the next element will be enqueued.
    tail: AtomicUsize,
    /// The index from where the next element will be dequeued.
    head: AtomicUsize,
}

// UNSAFE JUSTIFICATION:
// 1. `Send`: If `T` is `Send`, the queue is `Send` because we logically transfer ownership
//    of `T` across thread boundaries.
// 2. `Sync`: The queue is `Sync` because our lock-free coordination via `AtomicUsize` and
//    `UnsafeCell` guarantees that only one thread can ever access a specific `UnsafeCell`
//    for reading or writing at any given time.
unsafe impl<T: Send> Send for ArrayQueue<T> {}
unsafe impl<T: Send> Sync for ArrayQueue<T> {}

pub enum PushError<T> {
    Full(T),
}

impl<T> ArrayQueue<T> {
    /// Creates a new `ArrayQueue` with the specified capacity.
    /// Capacity must be a power of two.
    pub fn new(capacity: usize) -> Self {
        assert!(
            capacity > 0 && capacity.is_power_of_two(),
            "Capacity must be a power of two"
        );

        let mut buffer = Vec::with_capacity(capacity);
        for i in 0..capacity {
            buffer.push(Slot {
                sequence: AtomicUsize::new(i),
                data: UnsafeCell::new(MaybeUninit::uninit()),
            });
        }

        Self {
            buffer: buffer.into_boxed_slice(),
            mask: capacity - 1,
            tail: AtomicUsize::new(0),
            head: AtomicUsize::new(0),
        }
    }
}

impl<T> Queue<T> for ArrayQueue<T> {
    /// Attempts to push an item into the queue.
    /// Returns `Ok(())` if successful, or `Err(PushError::Full(item))` if the queue is full.
    fn push(&self, item: T) -> Result<(), PushError<T>> {
        let mut tail = self.tail.load(Ordering::Relaxed);

        loop {
            let slot = &self.buffer[tail & self.mask];
            let seq = slot.sequence.load(Ordering::Acquire);

            // GOTCHA: Using simple subtraction like `seq as isize - tail as isize`
            // can cause an overflow panic on 32-bit systems after billions of operations.
            // We must use `wrapping_sub` and cast to signed.
            let diff = seq.wrapping_sub(tail) as isize;

            if diff == 0 {
                // The slot is ready for us to enqueue.
                // Try to claim the tail index by moving it forward.
                match self.tail.compare_exchange_weak(
                    tail,
                    tail + 1,
                    Ordering::Relaxed,
                    Ordering::Relaxed,
                ) {
                    Ok(_) => {
                        // We successfully claimed this slot!
                        // UNSAFE JUSTIFICATION: We exclusively own this slot now because we
                        // successfully incremented the tail, and `seq == tail`. No other thread
                        // can write to or read from this slot until we update the sequence.
                        unsafe {
                            (*slot.data.get()).write(item);
                        }

                        // Release the slot to consumers.
                        slot.sequence.store(tail + 1, Ordering::Release);
                        return Ok(());
                    }
                    Err(actual_tail) => {
                        // Another thread claimed the tail before us. Update and try again.
                        tail = actual_tail;
                    }
                }
            } else if diff < 0 {
                // The queue is full. The slot's sequence is behind our current tail,
                // meaning a consumer hasn't reached it yet to reset it.
                return Err(PushError::Full(item));
            } else {
                // `diff > 0`. Another producer has already claimed this slot and incremented
                // the sequence, but we haven't seen the `tail` update yet. Reload tail.
                tail = self.tail.load(Ordering::Relaxed);
            }
        }
    }

    /// Attempts to pop an item from the queue.
    /// Returns `Some(T)` if successful, or `None` if the queue is empty.
    fn pop(&self) -> Option<T> {
        let mut head = self.head.load(Ordering::Relaxed);

        loop {
            let slot = &self.buffer[head & self.mask];
            let seq = slot.sequence.load(Ordering::Acquire);
            let diff = seq.wrapping_sub(head + 1) as isize;

            if diff == 0 {
                // The slot contains data and is ready for us to dequeue.
                // Try to claim the head index by moving it forward.
                match self.head.compare_exchange_weak(
                    head,
                    head + 1,
                    Ordering::Relaxed,
                    Ordering::Relaxed,
                ) {
                    Ok(_) => {
                        // We successfully claimed this slot!
                        // UNSAFE JUSTIFICATION: We exclusively own this slot now. We incremented
                        // the head, and `seq == head + 1`. No other thread can read or write
                        // to this slot until we reset the sequence.
                        let item = unsafe { (*slot.data.get()).assume_init_read() };

                        // Reset the slot's sequence so it can be used for enqueuing again.
                        // For a slot at `idx` that was at wrap `wrap`, the next time a producer
                        // needs it, its tail will be `head + capacity`.
                        slot.sequence.store(head + self.mask + 1, Ordering::Release);
                        return Some(item);
                    }
                    Err(actual_head) => {
                        // Another thread claimed the head before us. Update and try again.
                        head = actual_head;
                    }
                }
            } else if diff < 0 {
                // The queue is empty. The slot's sequence hasn't been incremented
                // by a producer yet.
                return None;
            } else {
                // `diff > 0`. Another consumer has already claimed this slot and incremented
                // the sequence, but we haven't seen the `head` update yet. Reload head.
                head = self.head.load(Ordering::Relaxed);
            }
        }
    }
}

// RUST INSIGHT: We must manually implement `Drop` to ensure any items remaining in the
// queue are properly dropped, as `UnsafeCell<MaybeUninit<T>>` does not automatically
// drop its contents.
impl<T> Drop for ArrayQueue<T> {
    fn drop(&mut self) {
        // Repeatedly pop elements until the queue is empty.
        // Because `pop` transfers ownership of `T` to the local scope, the item
        // will be dropped immediately.
        while self.pop().is_some() {}
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `crossbeam-queue`: The `ArrayQueue` implementation in crossbeam is heavily optimized.
//   It utilizes cache line padding (`CachePadded`) to prevent false sharing. It handles
//   yielding to the OS scheduler (`std::thread::yield_now`) in heavy contention spin loops
//   using exponential backoff (e.g., `crossbeam_utils::Backoff`).
//
// Missing vs. Production:
// - **Cache Padding**: As mentioned, missing cache padding will cause performance degradation
//   under heavy multi-core load.
// - **Backoff Strategy**: Spinning endlessly on a CAS loop burns CPU cycles. Production
//   lock-free structures implement spin strategies (spin-loop hint -> yield -> sleep).
// - **Arbitrary Capacity**: Production queues often pad capacity to the nearest power of two
//   under the hood instead of panicking on creation.
//
// Next Steps:
// 1. Add a `CachePadded` wrapper around `head` and `tail`.
// 2. Implement an exponential backoff loop inside the `push` and `pop` CAS loops.
//
// Benchmark Notes:
// To benchmark this lock-free queue against a Mutex-based queue, use Criterion.
// Create an MPMC test where multiple producers `push` and consumers `pop` concurrently.
// Ensure you use `std::hint::black_box` inside your test loops so the compiler doesn't optimize it away.

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn test_push_pop_sequential() {
        let queue = ArrayQueue::new(4);
        assert!(queue.pop().is_none());

        assert!(queue.push(1).is_ok());
        assert!(queue.push(2).is_ok());
        assert!(queue.push(3).is_ok());
        assert!(queue.push(4).is_ok());

        // Queue is full
        assert!(matches!(queue.push(5), Err(PushError::Full(5))));

        assert_eq!(queue.pop(), Some(1));
        assert_eq!(queue.pop(), Some(2));

        // We can push again
        assert!(queue.push(5).is_ok());

        assert_eq!(queue.pop(), Some(3));
        assert_eq!(queue.pop(), Some(4));
        assert_eq!(queue.pop(), Some(5));
        assert!(queue.pop().is_none());
    }

    #[test]
    fn test_concurrent_mpmc() {
        let queue = Arc::new(ArrayQueue::new(1024));
        let num_threads = 4;
        let items_per_thread = 10_000;

        let mut producers = vec![];
        for _ in 0..num_threads {
            let q = Arc::clone(&queue);
            producers.push(thread::spawn(move || {
                for i in 0..items_per_thread {
                    // Spin until successful push
                    while let Err(PushError::Full(item)) = q.push(i) {
                        std::thread::yield_now();
                    }
                }
            }));
        }

        let mut consumers = vec![];
        for _ in 0..num_threads {
            let q = Arc::clone(&queue);
            consumers.push(thread::spawn(move || {
                let mut sum = 0;
                for _ in 0..items_per_thread {
                    loop {
                        if let Some(val) = q.pop() {
                            sum += val;
                            break;
                        }
                        std::thread::yield_now();
                    }
                }
                sum
            }));
        }

        for p in producers {
            p.join().unwrap();
        }

        let mut total_sum = 0;
        for c in consumers {
            total_sum += c.join().unwrap();
        }

        // Formula for sum of 0 to n-1 is n * (n - 1) / 2
        let expected_sum_per_thread = items_per_thread * (items_per_thread - 1) / 2;
        let expected_total = expected_sum_per_thread * num_threads;

        assert_eq!(total_sum, expected_total);
    }

    #[test]
    #[should_panic(expected = "Capacity must be a power of two")]
    fn test_invalid_capacity() {
        let _q: ArrayQueue<i32> = ArrayQueue::new(3);
    }
}
