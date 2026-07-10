//! # Lock-Free Single-Producer Single-Consumer (SPSC) Ring Buffer
//!
//! A high-performance, thread-safe, lock-free queue for one producer and one consumer.
//!
//! **Replaces Crates:** `rigtorp-spsc`, `heapless::spsc`, `crossbeam-queue` (`ArrayQueue`)
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
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

// Cache line size constant (typical for x86/ARM)
const CACHE_LINE_SIZE: usize = 64;

/// Internal shared state.
/// We use specific padding to ensure `head` and `tail` are on different cache lines
/// to prevent false sharing between the producer (writing tail) and consumer (writing head).
#[repr(C)]
struct Shared<T> {
    // Padding before head
    _pad1: [u8; CACHE_LINE_SIZE],

    // Head index (Consumer writes, Producer reads)
    head: AtomicUsize,

    // Padding between head and tail
    _pad2: [u8; CACHE_LINE_SIZE],

    // Tail index (Producer writes, Consumer reads)
    tail: AtomicUsize,

    // Padding after tail
    _pad3: [u8; CACHE_LINE_SIZE],

    // The buffer storage.
    // Box<[UnsafeCell<MaybeUninit<T>>]> ensures stable address and correct alignment.
    buffer: Box<[UnsafeCell<MaybeUninit<T>>]>,

    // Capacity of the buffer (allocated size).
    // Note: Usable capacity is `capacity - 1` to distinguish full from empty.
    capacity: usize,
}

// UNSAFE JUSTIFICATION:
// - Sync: The SpscRingBuffer logic guarantees exclusive access:
//   - Producer owns `tail` writes and `buffer[tail]` writes.
//   - Consumer owns `head` writes and `buffer[head]` reads.
//   - Proper memory ordering (Release/Acquire) ensures visibility.
// - Send: `T` must be Send because it is moved between threads via the buffer.
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
///
/// # Panics
/// Panics if `capacity` is 0.
#[must_use]
pub fn channel<T>(capacity: usize) -> (Producer<T>, Consumer<T>) {
    assert!(capacity > 0, "Capacity must be greater than 0");

    // Allocate buffer with uninitialized data wrapped in UnsafeCell
    // We need capacity + 1 slots to distinguish full state (head == tail + 1) from empty (head == tail)
    let internal_capacity = capacity + 1;
    let mut buffer = Vec::with_capacity(internal_capacity);
    for _ in 0..internal_capacity {
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
        capacity: internal_capacity,
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
    ///
    /// # Errors
    /// Returns `Err(Full(item))` (handing the item back) if the queue is full.
    pub fn push(&mut self, item: T) -> Result<(), Full<T>> {
        let current_tail = self.local_tail;
        let next_tail = (current_tail + 1) % self.shared.capacity;

        // Check if full.
        // We check against shadow_head first to avoid loading the atomic head which causes cache coherence traffic.
        if next_tail == self.shadow_head {
            // Shadow says potentially full. Check the real head.
            let real_head = self.shared.head.load(Ordering::Acquire);
            self.shadow_head = real_head;

            if next_tail == real_head {
                return Err(Full(item));
            }
        }

        // Write the item.
        // SAFETY:
        // 1. We checked that next_tail != head, so this slot is guaranteed free.
        // 2. We are the exclusive producer for `tail`.
        unsafe {
            let slot_ptr = self.shared.buffer[current_tail].get();
            slot_ptr.write(MaybeUninit::new(item));
        }

        // Update tail.
        // Release ordering ensures the consumer sees the written item BEFORE seeing the new tail index.
        self.shared.tail.store(next_tail, Ordering::Release);
        self.local_tail = next_tail;

        Ok(())
    }

    /// Returns the capacity of the queue.
    #[must_use]
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
        // Check shadow_tail first.
        if current_head == self.shadow_tail {
            // Shadow says potentially empty. Check real tail.
            let real_tail = self.shared.tail.load(Ordering::Acquire);
            self.shadow_tail = real_tail;

            if current_head == real_tail {
                return None;
            }
        }

        // Read the item.
        // SAFETY:
        // 1. We checked head != tail, so this slot has valid data.
        // 2. We are the exclusive consumer for `head`.
        // 3. Acquire load on `tail` ensured we see the data written by producer.
        let item = unsafe {
            let slot_ptr = self.shared.buffer[current_head].get();
            (*slot_ptr).assume_init_read()
        };

        // Advance head.
        let next_head = (current_head + 1) % self.shared.capacity;

        // Update head.
        // Release ordering ensures the producer sees that we've read the item (and slot is free)
        // only AFTER we have actually read it.
        self.shared.head.store(next_head, Ordering::Release);
        self.local_head = next_head;

        Some(item)
    }
}

impl<T> Drop for Shared<T> {
    fn drop(&mut self) {
        // Drop all items currently in the buffer.
        // We have exclusive access in Drop.

        // Use Relaxed loads as no contention is possible.
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

// Send implementation logic:
// Producer can be sent to another thread if T is Send.
// Consumer can be sent to another thread if T is Send.
unsafe impl<T: Send> Send for Producer<T> {}
unsafe impl<T: Send> Send for Consumer<T> {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_simple_push_pop() {
        let (mut p, mut c) = channel(2);

        assert_eq!(p.push(1), Ok(()));
        assert_eq!(p.push(2), Ok(()));
        assert_eq!(p.push(3), Err(Full(3))); // Full (cap is 2)

        assert_eq!(c.pop(), Some(1));
        assert_eq!(c.pop(), Some(2));
        assert_eq!(c.pop(), None);
    }

    #[test]
    fn test_wrap_around() {
        let (mut p, mut c) = channel(3);

        // Fill
        p.push(1).unwrap();
        p.push(2).unwrap();
        p.push(3).unwrap();
        assert!(p.push(4).is_err());

        // Drain 2
        assert_eq!(c.pop(), Some(1));
        assert_eq!(c.pop(), Some(2));

        // Fill 2 more (wrap around)
        p.push(4).unwrap();
        p.push(5).unwrap();
        assert!(p.push(6).is_err());

        // Drain all
        assert_eq!(c.pop(), Some(3));
        assert_eq!(c.pop(), Some(4));
        assert_eq!(c.pop(), Some(5));
        assert_eq!(c.pop(), None);
    }

    #[test]
    fn test_concurrent() {
        const COUNT: usize = 100_000;
        let (mut p, mut c) = channel(128);

        let producer = thread::spawn(move || {
            for i in 0..COUNT {
                while p.push(i).is_err() {
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
    fn test_drop_safety() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static DROP_COUNT: AtomicUsize = AtomicUsize::new(0);

        #[derive(Debug)]
        struct Dropper;
        impl Drop for Dropper {
            fn drop(&mut self) {
                DROP_COUNT.fetch_add(1, Ordering::SeqCst);
            }
        }

        {
            let (mut p, _c) = channel(10);
            p.push(Dropper).unwrap();
            p.push(Dropper).unwrap();
            p.push(Dropper).unwrap();
        } // Both p and c dropped, so Shared dropped.

        assert_eq!(DROP_COUNT.load(Ordering::SeqCst), 3);
    }
}
