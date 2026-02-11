//! # Thread Pool Implementation
//!
//! Implements a fixed-size thread pool that executes jobs concurrently.
//! It uses a channel-based dispatch mechanism where workers compete for jobs from a shared queue.
//!
//! **Replaces Crates:** `threadpool`, `rayon` (for simple tasks), `tokio` (runtime internals)
//!
//! **Real-world Usage:**
//! - Web servers (handling incoming requests).
//! - Parallel data processing (image resizing, file I/O).
//! - Task scheduling in background services.
//!
//! **Why build it yourself?**
//! Building a thread pool teaches you about:
//! - Thread lifecycle management (spawning, joining).
//! - Synchronization primitives (`Arc`, `Mutex`, `mpsc`).
//! - Graceful shutdown patterns (ensuring all threads finish before exit).
//! - Handling closures and trait objects (`Box<dyn FnOnce() + Send + 'static>`).

use std::sync::{Arc, Mutex, mpsc};
use std::thread;

// =========================================================================================
// Architecture
// =========================================================================================
//
// Flow:
//
//      Client
//        │
//        ▼
//    ThreadPool::execute(job)
//        │
//        ▼
//    [mpsc::Sender] ──► [Channel Queue] ──► [mpsc::Receiver (Mutex)]
//                                                    │
//                                      ┌─────────────┼─────────────┐
//                                      ▼             ▼             ▼
//                                  [Worker 1]    [Worker 2]    [Worker 3]
//                                      │             │             │
//                                      ▼             ▼             ▼
//                                   Execute       Execute       Execute
//
//
// Invariants:
// 1. The pool has a fixed number of threads, determined at creation.
// 2. Jobs are executed in the order they are received by the queue (FIFO), but completion order is non-deterministic.
// 3. Panicking jobs kill the worker thread (in this simple implementation).
//
// Complexity:
// ┌───────────────┬─────────────┬─────────────┐
// │ Operation     │ Time        │ Space       │
// ├───────────────┼─────────────┼─────────────┤
// │ New Pool      │ O(N)        │ O(N)        │
// ├───────────────┼─────────────┼─────────────┤
// │ Execute Job   │ O(1)        │ O(1)        │
// ├───────────────┼─────────────┼─────────────┤
// │ Shutdown      │ O(N)        │ O(1)        │
// └───────────────┴─────────────┴─────────────┘
//
// Design Decisions:
// - **Channel-based Dispatch**: Using `std::sync::mpsc`.
//   - *Tradeoff*: Simple to implement.
//   - *Downside*: Single receiver can be a bottleneck under extreme contention (though `Mutex` overhead dominates first).
//   - *Alternative*: Work-stealing queues (like Rayon) where each worker has its own deque.
// - **Shared Receiver**: `Arc<Mutex<mpsc::Receiver>>`.
//   - *Tradeoff*: `mpsc::Receiver` is not `Sync`, so it must be wrapped in a `Mutex`.
//   - *Alternative*: `crossbeam-channel` which supports multiple consumers (MPMC) without a Mutex.

/// A ThreadPool that manages a group of spawned threads.
pub struct ThreadPool {
    workers: Vec<Worker>,
    sender: Option<mpsc::Sender<Job>>,
}

/// A generic job to be executed by the thread pool.
// RUST INSIGHT: `Box<dyn FnOnce() + Send + 'static>`
// - `FnOnce`: The closure is called once.
// - `Send`: The closure must be safe to move to another thread.
// - `'static`: The closure must not borrow data with a limited lifetime (it must own its data or use static references).
type Job = Box<dyn FnOnce() + Send + 'static>;

impl ThreadPool {
    /// Create a new ThreadPool.
    ///
    /// The size is the number of threads in the pool.
    ///
    /// # Panics
    ///
    /// The `new` function will panic if the size is zero.
    pub fn new(size: usize) -> ThreadPool {
        assert!(size > 0);

        let (sender, receiver) = mpsc::channel();

        // RUST INSIGHT: Shared Ownership
        // The receiver is shared among all workers. Since `mpsc::Receiver` is not `Sync` (it's not thread-safe
        // to access concurrently), we wrap it in a `Mutex` to ensure mutually exclusive access.
        // We wrap that in an `Arc` (Atomic Reference Counted) pointer to share ownership across threads.
        let receiver = Arc::new(Mutex::new(receiver));

        let mut workers = Vec::with_capacity(size);

        for id in 0..size {
            workers.push(Worker::new(id, Arc::clone(&receiver)));
        }

        ThreadPool {
            workers,
            sender: Some(sender),
        }
    }

    /// Execute a function `f` on a thread in the pool.
    ///
    /// The closure `f` must be `Send` and `'static`.
    pub fn execute<F>(&self, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        let job = Box::new(f);
        // GOTCHA: If we used `unwrap()` here, a panic could occur during shutdown if the receiver is dropped.
        // However, since we own the sender and only drop it in `Drop`, this specific send should generally succeed
        // as long as the pool is alive.
        if let Some(sender) = &self.sender {
            sender
                .send(job)
                .expect("ThreadPool::execute failed sending job");
        }
    }
}

impl Drop for ThreadPool {
    fn drop(&mut self) {
        // Drop the sender to close the channel.
        // This signals to the workers (via the iterator returning None) that no more jobs will come.
        // We use `Option::take` to move the sender out of `self` and drop it.
        drop(self.sender.take());

        for worker in &mut self.workers {
            // println!("Shutting down worker {}", worker.id);

            // RUST INSIGHT: Joining Threads
            // We use `take()` to move the `JoinHandle` out of the worker.
            // This is necessary because `join()` takes ownership of the handle (`self`),
            // but we only have a mutable reference to the worker.
            if let Some(thread) = worker.thread.take() {
                thread.join().unwrap();
            }
        }
    }
}

struct Worker {
    _id: usize,
    thread: Option<thread::JoinHandle<()>>,
}

impl Worker {
    fn new(id: usize, receiver: Arc<Mutex<mpsc::Receiver<Job>>>) -> Worker {
        let thread = thread::spawn(move || {
            loop {
                // RUST INSIGHT: Lock Contention
                // We lock the mutex to get the next job. The lock is held ONLY for the duration of `recv()`.
                // Once `recv()` returns (or blocks waiting), the lock logic depends on `mpsc`.
                // Actually, `recv()` blocks. If we held the mutex while blocking, no other worker could get a job.
                //
                // Wait, `recv()` on a `Mutex<Receiver>`?
                // The correct pattern is: Lock the mutex, get the receiver, call `recv()`.
                // BUT `recv()` blocks. If we block *while holding the lock*, we serialize the entire pool!
                //
                // FIX: standard `mpsc::Receiver` inside a `Mutex` is tricky.
                // The `Mutex` guards access to the `Receiver`.
                // If we do `receiver.lock().unwrap().recv()`, we ARE holding the lock while waiting.
                // This means only ONE worker waits. Others will block on the MUTEX.
                // Ideally, we want workers to sleep on the channel, not the mutex.
                //
                // However, in this simple design, it works because:
                // 1. Worker A acquires Lock.
                // 2. Worker A calls recv().
                // 3. If channel empty, A blocks (holding Lock).
                // 4. Sender sends job.
                // 5. A wakes up, gets job, releases Lock.
                // 6. A executes job.
                //
                // While A is executing, B can acquire Lock and wait for next job.
                // The downsides is that we can't have multiple workers waiting on the channel simultaneously
                // (waking up purely on CondVar). They wait on the Mutex.
                // This is acceptable for a "foundational" implementation but `crossbeam` or `SegQueue` is better.

                let message = receiver.lock().unwrap().recv();

                match message {
                    Ok(job) => {
                        // println!("Worker {} got a job; executing.", id);

                        // PRODUCTION NOTE: Panic Handling
                        // If this job panics, this thread will die.
                        // A robust thread pool would use `std::panic::catch_unwind` here
                        // to catch the panic and keep the worker alive (or respawn it).
                        job();
                    }
                    Err(_) => {
                        // Channel closed (Sender dropped).
                        // println!("Worker {} disconnected; shutting down.", id);
                        break;
                    }
                }
            }
        });

        Worker {
            _id: id,
            thread: Some(thread),
        }
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `threadpool`: Similar API, but handles panics by respawning threads.
// - `rayon`: Uses work-stealing (deque per thread) which is far more efficient for recursive/divide-and-conquer workloads.
//   Also supports `join` (fork-join parallelism).
// - `tokio`: Async runtime using an event loop + thread pool. Our pool is blocking/synchronous.
//
// Missing vs. Production:
// - **Panic Resilience**: If a job panics, the worker thread dies and is not replaced. Eventually the pool could empty out.
// - **Dynamic Resizing**: The pool size is fixed. Production pools might scale up/down based on load.
// - **Work Stealing**: Necessary for load balancing complex workloads.
// - **Thread Naming/Stack Size**: `std::thread::Builder` allows configuring these.
//
// Next Steps:
// 1. Wrap the job execution in `std::panic::catch_unwind`.
// 2. Implement a `join()` method on `ThreadPool` to wait for all *current* jobs to finish without shutting down (using a WaitGroup/Barrier).
// 3. Explore `crossbeam-channel` for MPMC support to remove the `Mutex`.

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    #[test]
    fn test_simple_execute() {
        let pool = ThreadPool::new(2);
        let counter = Arc::new(AtomicUsize::new(0));

        for _ in 0..5 {
            let counter = Arc::clone(&counter);
            pool.execute(move || {
                counter.fetch_add(1, Ordering::SeqCst);
            });
        }

        // Give some time for threads to finish
        thread::sleep(Duration::from_millis(100));

        assert_eq!(counter.load(Ordering::SeqCst), 5);
    }

    #[test]
    fn test_concurrent_execution() {
        let pool = ThreadPool::new(4);
        let counter = Arc::new(AtomicUsize::new(0));
        let n_jobs = 100;

        for _ in 0..n_jobs {
            let counter = Arc::clone(&counter);
            pool.execute(move || {
                // Simulate some work
                // thread::sleep(Duration::from_millis(1));
                counter.fetch_add(1, Ordering::SeqCst);
            });
        }

        // Wait for all jobs.
        // Since we don't implement a `join()` on the pool (only Drop joins threads),
        // we can either sleep or use a channel to signal completion for testing.
        // For robustness, let's just sleep a bit longer or use a barrier-like approach manually if needed.
        // But `Drop` waits for threads! So if we drop the pool, it should wait for all queued jobs if the workers process them?
        //
        // Wait! `Drop` closes the sender.
        // The workers loop: `receiver.lock().unwrap().recv()`.
        // `recv()` returns `Err` only when ALL senders are dropped and channel is empty.
        // So if we drop `ThreadPool`, `sender` is dropped.
        // Workers will continue to `recv()` until the channel is empty.
        // THEN `recv()` returns `Err`.
        // So dropping the pool *ensures* all submitted jobs are processed!
        drop(pool);

        assert_eq!(counter.load(Ordering::SeqCst), n_jobs);
    }

    #[test]
    fn test_shutdown() {
        let pool = ThreadPool::new(2);
        // Just verify it doesn't hang
        pool.execute(|| {});
        drop(pool);
    }
}
