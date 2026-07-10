//! # Concurrency Patterns
//!
//! Thread-safe patterns for concurrent programming in Rust.
//! Essential for systems programming interviews and parallel algorithms.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex, RwLock};
use std::thread;
use std::time::Duration;

// ============================================================================
// Thread Spawning and Joining
// ============================================================================

#[allow(dead_code)]
fn demonstrate_basic_threads() {
    // Spawn a thread
    let handle = thread::spawn(|| {
        println!("Hello from thread!");
        42
    });

    // Join to wait for completion and get return value
    let result = handle.join().unwrap();
    println!("Thread returned: {result}");

    // Multiple threads
    let mut handles = vec![];

    for i in 0..5 {
        let handle = thread::spawn(move || {
            println!("Thread {i} running");
            i * 2
        });
        handles.push(handle);
    }

    // Collect results
    for handle in handles {
        let result = handle.join().unwrap();
        println!("Result: {result}");
    }
}

/// Scoped threads allow borrowing from parent scope.
#[allow(dead_code)]
fn demonstrate_scoped_threads() {
    let data = vec![1, 2, 3, 4, 5];

    // Scoped threads can borrow 'data' safely
    thread::scope(|s| {
        // Spawn threads that borrow data
        s.spawn(|| {
            let sum: i32 = data.iter().sum();
            println!("Sum: {sum}");
        });

        s.spawn(|| {
            let max = data.iter().max();
            println!("Max: {max:?}");
        });

        // All scoped threads complete before scope ends
    });

    // data is still valid here
    println!("Data: {data:?}");
}

// ============================================================================
// Arc<T> - Atomic Reference Counting
// ============================================================================

#[allow(dead_code)]
fn demonstrate_arc_basics() {
    // Arc allows sharing data across threads
    let data = Arc::new(vec![1, 2, 3, 4, 5]);

    let mut handles = vec![];

    for i in 0..3 {
        // Clone the Arc (not the data!) to share across threads
        let data_clone = Arc::clone(&data);

        let handle = thread::spawn(move || {
            println!("Thread {i}: {data_clone:?}");
            let sum: i32 = data_clone.iter().sum();
            println!("Thread {i} sum: {sum}");
        });

        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // Original Arc still valid
    println!("Main: {data:?}");
}

/// Arc vs Rc: Arc is thread-safe, Rc is not.
#[allow(dead_code)]
fn demonstrate_arc_vs_rc() {
    use std::rc::Rc;

    // Rc - single threaded reference counting
    let rc_data = Rc::new(vec![1, 2, 3]);
    let _rc_clone = Rc::clone(&rc_data);
    // Cannot send Rc across threads - won't compile!
    // thread::spawn(move || { println!("{:?}", rc_clone); });

    // Arc - thread-safe reference counting
    let arc_data = Arc::new(vec![1, 2, 3]);
    let arc_clone = Arc::clone(&arc_data);
    // Can send Arc across threads
    thread::spawn(move || {
        println!("{arc_clone:?}");
    })
    .join()
    .unwrap();
}

// ============================================================================
// Mutex<T> - Mutual Exclusion
// ============================================================================

#[allow(dead_code)]
fn demonstrate_mutex_basics() {
    // Create mutex-protected data
    let counter = Mutex::new(0);

    // Lock to access data
    {
        let mut num = counter.lock().unwrap();
        *num += 1;
        // Lock is automatically released when 'num' goes out of scope (RAII)
    }

    println!("Counter: {:?}", counter.lock().unwrap());

    // Handle lock poisoning
    match counter.lock() {
        Ok(guard) => println!("Locked: {guard}"),
        Err(poisoned) => {
            println!("Lock was poisoned!");
            let _guard = poisoned.into_inner(); // Recover the data
        }
    }
}

/// Arc<Mutex<T>> pattern for shared mutable state across threads.
#[allow(dead_code)]
fn demonstrate_arc_mutex() {
    let counter = Arc::new(Mutex::new(0));
    let mut handles = vec![];

    for _ in 0..10 {
        let counter_clone = Arc::clone(&counter);
        let handle = thread::spawn(move || {
            let mut num = counter_clone.lock().unwrap();
            *num += 1;
            // Lock released here
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    println!("Final counter: {}", *counter.lock().unwrap()); // 10
}

/// Avoiding deadlocks with lock ordering.
#[allow(dead_code)]
fn demonstrate_deadlock_prevention() {
    let mutex1 = Arc::new(Mutex::new(0));
    let mutex2 = Arc::new(Mutex::new(0));

    let m1_clone = Arc::clone(&mutex1);
    let m2_clone = Arc::clone(&mutex2);

    // Thread 1: locks mutex1 then mutex2
    let t1 = thread::spawn(move || {
        let _lock1 = m1_clone.lock().unwrap();
        thread::sleep(Duration::from_millis(10));
        let _lock2 = m2_clone.lock().unwrap();
        println!("Thread 1 acquired both locks");
    });

    let m1_clone = Arc::clone(&mutex1);
    let m2_clone = Arc::clone(&mutex2);

    // Thread 2: locks in SAME ORDER to prevent deadlock
    let t2 = thread::spawn(move || {
        let _lock1 = m1_clone.lock().unwrap(); // Same order as thread 1
        thread::sleep(Duration::from_millis(10));
        let _lock2 = m2_clone.lock().unwrap();
        println!("Thread 2 acquired both locks");
    });

    t1.join().unwrap();
    t2.join().unwrap();
}

// ============================================================================
// RwLock<T> - Reader-Writer Lock
// ============================================================================

#[allow(dead_code)]
fn demonstrate_rwlock() {
    let data = Arc::new(RwLock::new(vec![1, 2, 3]));

    // Multiple readers can access simultaneously
    let mut read_handles = vec![];
    for i in 0..3 {
        let data_clone = Arc::clone(&data);
        let handle = thread::spawn(move || {
            let reader = data_clone.read().unwrap();
            println!("Reader {i}: {reader:?}");
            thread::sleep(Duration::from_millis(100));
        });
        read_handles.push(handle);
    }

    // Wait for readers
    for handle in read_handles {
        handle.join().unwrap();
    }

    // Single writer has exclusive access
    let data_clone = Arc::clone(&data);
    let write_handle = thread::spawn(move || {
        let mut writer = data_clone.write().unwrap();
        writer.push(4);
        println!("Writer: {writer:?}");
    });

    write_handle.join().unwrap();

    println!("Final: {:?}", data.read().unwrap());
}

/// `RwLock` vs Mutex: Use `RwLock` for read-heavy workloads.
#[allow(dead_code)]
fn demonstrate_rwlock_vs_mutex() {
    println!("=== RwLock vs Mutex ===");
    println!("Mutex:");
    println!("  - One thread at a time (read or write)");
    println!("  - Simpler, less overhead");
    println!("  - Use for write-heavy workloads");
    println!();
    println!("RwLock:");
    println!("  - Multiple readers OR single writer");
    println!("  - More complex, more overhead");
    println!("  - Use for read-heavy workloads");
}

// ============================================================================
// Channels - Message Passing
// ============================================================================

#[allow(dead_code)]
fn demonstrate_mpsc_basics() {
    // Create channel (multiple producer, single consumer)
    let (tx, rx) = mpsc::channel();

    // Spawn thread that sends messages
    thread::spawn(move || {
        let messages = vec!["hello", "from", "thread"];
        for msg in messages {
            tx.send(msg).unwrap();
            thread::sleep(Duration::from_millis(100));
        }
    });

    // Receive messages
    for received in rx {
        println!("Received: {received}");
    }
}

/// Multiple producers, single consumer.
#[allow(dead_code)]
fn demonstrate_multiple_producers() {
    let (tx, rx) = mpsc::channel();

    // Create multiple senders
    for i in 0..3 {
        let tx_clone = tx.clone();
        thread::spawn(move || {
            tx_clone.send(format!("Message from thread {i}")).unwrap();
        });
    }

    // Drop original sender
    drop(tx);

    // Receive all messages
    for received in rx {
        println!("Received: {received}");
    }
}

/// Non-blocking receive with `try_recv`.
#[allow(dead_code)]
fn demonstrate_try_recv() {
    let (tx, rx) = mpsc::channel();

    thread::spawn(move || {
        thread::sleep(Duration::from_millis(500));
        tx.send("delayed message").unwrap();
    });

    // Try receiving without blocking
    loop {
        match rx.try_recv() {
            Ok(msg) => {
                println!("Received: {msg}");
                break;
            }
            Err(mpsc::TryRecvError::Empty) => {
                println!("No message yet, doing other work...");
                thread::sleep(Duration::from_millis(100));
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                println!("Sender disconnected");
                break;
            }
        }
    }
}

/// Bounded channel with `sync_channel`.
#[allow(dead_code)]
fn demonstrate_bounded_channel() {
    // Channel with capacity of 2
    let (tx, rx) = mpsc::sync_channel(2);

    thread::spawn(move || {
        for i in 0..5 {
            println!("Sending {i}");
            tx.send(i).unwrap(); // Blocks when buffer is full
            println!("Sent {i}");
        }
    });

    thread::sleep(Duration::from_millis(500));

    for received in rx {
        println!("Received: {received}");
        thread::sleep(Duration::from_millis(200));
    }
}

// ============================================================================
// Atomic Types
// ============================================================================

#[allow(dead_code)]
fn demonstrate_atomics() {
    // Atomic bool for flags
    let flag = Arc::new(AtomicBool::new(false));
    let flag_clone = Arc::clone(&flag);

    let handle = thread::spawn(move || {
        thread::sleep(Duration::from_millis(100));
        flag_clone.store(true, Ordering::SeqCst);
        println!("Flag set to true");
    });

    // Wait for flag
    while !flag.load(Ordering::SeqCst) {
        thread::sleep(Duration::from_millis(10));
    }
    println!("Flag is true!");

    handle.join().unwrap();

    // Atomic counter
    let counter = Arc::new(AtomicUsize::new(0));
    let mut handles = vec![];

    for _ in 0..10 {
        let counter_clone = Arc::clone(&counter);
        let handle = thread::spawn(move || {
            for _ in 0..100 {
                counter_clone.fetch_add(1, Ordering::SeqCst);
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    println!("Final count: {}", counter.load(Ordering::SeqCst)); // 1000
}

/// Memory ordering explanation.
#[allow(dead_code)]
fn demonstrate_memory_ordering() {
    println!("=== Memory Ordering ===");
    println!("Relaxed:");
    println!("  - No ordering guarantees");
    println!("  - Use for counters where exact order doesn't matter");
    println!();
    println!("Acquire/Release:");
    println!("  - Acquire: prevents reordering of loads after this");
    println!("  - Release: prevents reordering of stores before this");
    println!("  - Use for synchronization");
    println!();
    println!("SeqCst (Sequentially Consistent):");
    println!("  - Strongest guarantees, total global order");
    println!("  - Easiest to reason about, but slowest");
    println!("  - Use when unsure (default choice)");
}

/// Compare-and-swap pattern.
#[allow(dead_code)]
fn demonstrate_compare_exchange() {
    let atomic = AtomicUsize::new(0);

    // Try to set to 10 if current value is 0
    match atomic.compare_exchange(0, 10, Ordering::SeqCst, Ordering::SeqCst) {
        Ok(prev) => println!("Changed from {prev} to 10"),
        Err(actual) => println!("Failed, actual value was {actual}"),
    }

    println!("Final value: {}", atomic.load(Ordering::SeqCst));
}

// ============================================================================
// Common Concurrent Patterns
// ============================================================================

/// Pattern 1: Shared counter with Arc<Mutex<T>>.
#[allow(dead_code)]
fn pattern_shared_counter() {
    let counter = Arc::new(Mutex::new(0));
    let mut handles = vec![];

    for i in 0..5 {
        let counter = Arc::clone(&counter);
        let handle = thread::spawn(move || {
            for _ in 0..10 {
                let mut num = counter.lock().unwrap();
                *num += 1;
            }
            println!("Thread {i} done");
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    println!("Total: {}", *counter.lock().unwrap()); // 50
}

/// Pattern 2: Fan-out work distribution.
#[allow(dead_code)]
fn pattern_fan_out() {
    let data = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
    let chunk_size = 2;
    let mut handles = vec![];

    for chunk in data.chunks(chunk_size) {
        let chunk = chunk.to_vec();
        let handle = thread::spawn(move || {
            let sum: i32 = chunk.iter().sum();
            println!("Chunk sum: {sum}");
            sum
        });
        handles.push(handle);
    }

    let total: i32 = handles.into_iter().map(|h| h.join().unwrap()).sum();

    println!("Total sum: {total}"); // 55
}

/// Pattern 3: Parallel map-reduce.
#[allow(dead_code)]
fn pattern_map_reduce() {
    let numbers = [1, 2, 3, 4, 5, 6, 7, 8];
    let num_threads = 4;
    let chunk_size = numbers.len() / num_threads;

    let results = Arc::new(Mutex::new(Vec::new()));
    let mut handles = vec![];

    for chunk in numbers.chunks(chunk_size) {
        let chunk = chunk.to_vec();
        let results = Arc::clone(&results);

        let handle = thread::spawn(move || {
            // Map: square each number
            let mapped: Vec<i32> = chunk.iter().map(|&x| x * x).collect();

            // Reduce: sum the chunk
            let sum: i32 = mapped.iter().sum();

            // Store result
            results.lock().unwrap().push(sum);
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // Final reduce: sum all chunk sums
    let total: i32 = results.lock().unwrap().iter().sum();
    println!("Map-reduce total: {total}"); // 1+4+9+16+25+36+49+64 = 204
}

/// Pattern 4: Worker pool with channels.
#[allow(dead_code)]
fn pattern_worker_pool() {
    let num_workers = 3;
    let (job_tx, job_rx) = mpsc::channel();
    let job_rx = Arc::new(Mutex::new(job_rx));

    // Spawn workers
    let mut workers = vec![];
    for id in 0..num_workers {
        let job_rx = Arc::clone(&job_rx);

        let handle = thread::spawn(move || {
            loop {
                let job = job_rx.lock().unwrap().recv();
                if let Ok(job_id) = job {
                    println!("Worker {id} processing job {job_id}");
                    thread::sleep(Duration::from_millis(100));
                    println!("Worker {id} finished job {job_id}");
                } else {
                    println!("Worker {id} shutting down");
                    break;
                }
            }
        });
        workers.push(handle);
    }

    // Send jobs
    for i in 0..10 {
        job_tx.send(i).unwrap();
    }

    // Shutdown workers by dropping sender
    drop(job_tx);

    // Wait for workers
    for worker in workers {
        worker.join().unwrap();
    }
}

/// Pattern 5: Producer-Consumer.
#[allow(dead_code)]
fn pattern_producer_consumer() {
    let (tx, rx) = mpsc::sync_channel(5); // Bounded buffer

    // Producer thread
    let producer = thread::spawn(move || {
        for i in 0..10 {
            println!("Producing {i}");
            tx.send(i).unwrap();
            thread::sleep(Duration::from_millis(50));
        }
        println!("Producer done");
    });

    // Consumer thread
    let consumer = thread::spawn(move || {
        for item in rx {
            println!("Consuming {item}");
            thread::sleep(Duration::from_millis(100));
        }
        println!("Consumer done");
    });

    producer.join().unwrap();
    consumer.join().unwrap();
}

// ============================================================================
// Thread-Safe Data Structures
// ============================================================================

/// Simple thread-safe queue using Arc<Mutex<Vec<T>>>.
#[allow(dead_code)]
struct ThreadSafeQueue<T> {
    data: Arc<Mutex<Vec<T>>>,
}

impl<T> ThreadSafeQueue<T> {
    fn new() -> Self {
        Self {
            data: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn push(&self, item: T) {
        self.data.lock().unwrap().push(item);
    }

    fn pop(&self) -> Option<T> {
        self.data.lock().unwrap().pop()
    }

    fn len(&self) -> usize {
        self.data.lock().unwrap().len()
    }
}

impl<T> Clone for ThreadSafeQueue<T> {
    fn clone(&self) -> Self {
        Self {
            data: Arc::clone(&self.data),
        }
    }
}

#[allow(dead_code)]
fn demonstrate_thread_safe_queue() {
    let queue = ThreadSafeQueue::new();

    // Producer threads
    let mut handles = vec![];
    for i in 0..3 {
        let queue_clone = queue.clone();
        let handle = thread::spawn(move || {
            for j in 0..5 {
                queue_clone.push(i * 10 + j);
            }
        });
        handles.push(handle);
    }

    // Wait for producers
    for handle in handles {
        handle.join().unwrap();
    }

    println!("Queue length: {}", queue.len());

    // Consumer thread
    while let Some(item) = queue.pop() {
        println!("Popped: {item}");
    }
}
