//! # Background Job Queue Implementation
//!
//! Implements an in-memory background job processing queue with thread-safe execution,
//! worker pools, and automatic retries for failed jobs.
//!
//! **Replaces Crates:** `fang`, `sidekiq`, `obok`
//!
//! **Real-world Usage:**
//! - Sending emails asynchronously in web applications.
//! - Processing video uploads or generating thumbnails.
//! - Scheduled background maintenance tasks.
//! - Synchronizing data with third-party APIs.
//!
//! **Why build it yourself?**
//! Building a background job queue from scratch teaches you how to orchestrate multiple worker threads
//! competing for work using `Mutex` and `Condvar`. You will learn the importance of using immutable `&self`
//! for state preservation during retries and why `notify_all()` is crucial when multiple types of workers
//! might be waiting on a shared lock, preventing deadlocks or stalled tasks.

use std::collections::VecDeque;
use std::sync::{Arc, Condvar, Mutex};
use std::thread;

// =========================================================================================
// Architecture
// =========================================================================================
//
// Data Structure:
//
//        [ Producer Threads ]
//                 │
//                 ▼ (enqueue)
//    ┌───────────────────────────┐
//    │   Shared State (Mutex)    │
//    │ ┌───────────────────────┐ │
//    │ │ Queue (VecDeque<Job>) │ │
//    │ └───────────────────────┘ │
//    └────────────┬──────────────┘
//                 │ (Condvar wake)
//                 ▼
//        [ Worker Threads ]
//
// Invariants:
// 1. Jobs are executed in FIFO order within their priority/queue.
// 2. A job is retried up to its maximum retry count if it fails.
// 3. Worker threads sleep on a Condvar when the queue is empty, consuming zero CPU.
//
// Complexity:
// ┌───────────┬────────┬────────┐
// │ Operation │ Time   │ Space  │
// ├───────────┼────────┼────────┤
// │ enqueue   │ O(1)   │ O(1)   │
// │ execute   │ O(1)   │ O(1)   │
// └───────────┴────────┴────────┘
//
// Design Decisions:
// - **Immutable Job Trait**: The `run` method takes `&self` rather than `&mut self` or `self`. This is critical.
//   If `run` took `self`, we could not retry the job if it failed because ownership would be consumed.
//   If it took `&mut self`, we would need exclusive access which complicates concurrent scheduling.
// - **notify_all over notify_one**: We use `notify_all()` to ensure all workers wake up and check the queue.
//   This explicitly prevents stalled tasks when multiple queues or different kinds of workers share a single lock.

/// Represents a background job that can be executed.
pub trait Job: Send + Sync {
    /// Executes the job.
    ///
    /// RUST INSIGHT: We take `&self` rather than `self` or `&mut self`.
    /// Taking `self` would consume the job, making it impossible to retry on failure.
    /// Taking `&self` forces jobs to use interior mutability (e.g., `AtomicUsize`, `Mutex`)
    /// if they need to mutate state, which is safer for concurrent retries.
    fn run(&self) -> Result<(), String>;

    /// The maximum number of times to retry this job if it fails.
    fn max_retries(&self) -> usize {
        3
    }
}

/// A wrapper around a job that tracks its retry attempts.
struct JobEntry {
    job: Box<dyn Job>,
    attempts: usize,
}

/// The internal state shared across all workers and producers.
struct QueueState {
    jobs: VecDeque<JobEntry>,
    shutting_down: bool,
}

/// A Background Job Queue.
pub struct JobQueue {
    state: Arc<Mutex<QueueState>>,
    condvar: Arc<Condvar>,
    workers: Vec<thread::JoinHandle<()>>,
}

impl JobQueue {
    /// Creates a new job queue and spawns `worker_count` background threads.
    #[must_use]
    pub fn new(worker_count: usize) -> Self {
        let state = Arc::new(Mutex::new(QueueState {
            jobs: VecDeque::new(),
            shutting_down: false,
        }));
        let condvar = Arc::new(Condvar::new());
        let mut workers = Vec::with_capacity(worker_count);

        for _ in 0..worker_count {
            let state_clone = Arc::clone(&state);
            let condvar_clone = Arc::clone(&condvar);

            workers.push(thread::spawn(move || {
                Self::worker_loop(state_clone, condvar_clone);
            }));
        }

        Self {
            state,
            condvar,
            workers,
        }
    }

    /// Enqueues a job for background processing.
    pub fn enqueue<J: Job + 'static>(&self, job: J) {
        let mut state = self.state.lock().unwrap();
        state.jobs.push_back(JobEntry {
            job: Box::new(job),
            attempts: 0,
        });

        // GOTCHA: We must use `notify_all()` instead of `notify_one()` in scenarios where
        // multiple queues share a single lock or workers have specific capabilities.
        // It prevents stalled tasks if the woken worker cannot process the task.
        self.condvar.notify_all();
    }

    /// Shuts down the queue and waits for all workers to finish their current jobs.
    pub fn stop(&mut self) {
        {
            let mut state = self.state.lock().unwrap();
            state.shutting_down = true;
        }
        self.condvar.notify_all();

        // Drain workers so we can join them safely
        for worker in self.workers.drain(..) {
            let _ = worker.join();
        }
    }

    /// The internal loop run by each worker thread.
    fn worker_loop(state_arc: Arc<Mutex<QueueState>>, condvar: Arc<Condvar>) {
        loop {
            let mut job_entry = {
                let mut state = state_arc.lock().unwrap();

                loop {
                    if state.shutting_down && state.jobs.is_empty() {
                        return; // Exit thread
                    }

                    if let Some(entry) = state.jobs.pop_front() {
                        break entry;
                    }

                    // Sleep if queue is empty and not shutting down
                    state = condvar.wait(state).unwrap();
                }
            };

            // Execute the job without holding the lock!
            // PRODUCTION NOTE: If we held the lock here, only one worker could execute a job at a time,
            // effectively making our system single-threaded.
            let result = job_entry.job.run();

            if result.is_err() {
                job_entry.attempts += 1;
                if job_entry.attempts <= job_entry.job.max_retries() {
                    // Re-acquire lock to re-enqueue
                    let mut state = state_arc.lock().unwrap();
                    // Optional: A production system might use a backoff delay here instead of immediate push.
                    state.jobs.push_back(job_entry);
                    condvar.notify_all();
                } else {
                    // Job failed permanently. In production, move to a Dead Letter Queue (DLQ).
                }
            }
        }
    }
}

impl Default for JobQueue {
    fn default() -> Self {
        Self::new(4)
    }
}

impl Drop for JobQueue {
    fn drop(&mut self) {
        self.stop();
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `fang`: Uses PostgreSQL as a backing store instead of an in-memory `VecDeque`, providing durability.
// - `sidekiq` (Ruby) / `obok` (Rust): Uses Redis for distributed job processing and complex retry/backoff scheduling.
//
// Missing vs. Production:
// - **Durability:** If the process crashes, all jobs in the in-memory queue are lost.
// - **Exponential Backoff:** Retries happen immediately, which can hammer a failing external service.
// - **Dead Letter Queue (DLQ):** Permanently failed jobs just disappear; they should be stored for manual review.
// - **Job Cancellation:** No way to cancel a running or queued job.
//
// Benchmarking Note:
// To benchmark the enqueue performance, use `criterion` to measure the overhead of
// acquiring the `Mutex` and inserting into the `VecDeque` (O(1)).
// Ensure the worker threads are either stopped or sleep to prevent execution overhead
// from polluting the queueing benchmark.
// Example: `b.iter(|| queue.enqueue(black_box(TestJob)))`
//
// Next Steps:
// 1. Add delayed execution (e.g., `enqueue_in(Duration, job)`).
// 2. Implement an exponential backoff strategy for retries.
// 3. Move failed jobs after max retries into a separate `dead_letters` queue.
// 4. Wrap `job_entry.job.run()` in `std::panic::catch_unwind` to prevent worker thread death.

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    struct TestJob {
        counter: Arc<AtomicUsize>,
    }

    impl Job for TestJob {
        fn run(&self) -> Result<(), String> {
            self.counter.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    #[test]
    fn test_job_execution() {
        let queue = JobQueue::new(2);
        let counter = Arc::new(AtomicUsize::new(0));

        for _ in 0..10 {
            queue.enqueue(TestJob {
                counter: Arc::clone(&counter),
            });
        }

        // Give workers time to process
        thread::sleep(Duration::from_millis(100));

        assert_eq!(counter.load(Ordering::SeqCst), 10);
    }

    struct FailingJob {
        counter: Arc<AtomicUsize>,
    }

    impl Job for FailingJob {
        fn run(&self) -> Result<(), String> {
            self.counter.fetch_add(1, Ordering::SeqCst);
            Err("Failed".to_string())
        }

        fn max_retries(&self) -> usize {
            2
        }
    }

    #[test]
    fn test_job_retries() {
        let mut queue = JobQueue::new(1);
        let counter = Arc::new(AtomicUsize::new(0));

        queue.enqueue(FailingJob {
            counter: Arc::clone(&counter),
        });

        // Give worker time to process and retry
        thread::sleep(Duration::from_millis(100));

        // Initial execution (1) + retries (2) = 3 total attempts
        assert_eq!(counter.load(Ordering::SeqCst), 3);

        queue.stop();
    }

    struct SlowJob {
        counter: Arc<AtomicUsize>,
    }

    impl Job for SlowJob {
        fn run(&self) -> Result<(), String> {
            thread::sleep(Duration::from_millis(50));
            self.counter.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    #[test]
    fn test_graceful_shutdown() {
        let mut queue = JobQueue::new(2);
        let counter = Arc::new(AtomicUsize::new(0));

        queue.enqueue(SlowJob {
            counter: Arc::clone(&counter),
        });
        queue.enqueue(SlowJob {
            counter: Arc::clone(&counter),
        });

        // Shutdown immediately, before jobs finish
        queue.stop();

        // After stop returns, all workers have joined, meaning jobs are done.
        assert_eq!(counter.load(Ordering::SeqCst), 2);
    }
}
