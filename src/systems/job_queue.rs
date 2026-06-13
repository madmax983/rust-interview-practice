//! # Background Job Queue Implementation
//!
//! Implements a robust, thread-safe, in-memory background job queue from scratch.
//! This implementation features configurable worker pools, delayed job execution,
//! automatic retries with exponential backoff, and dead-letter queues.
//!
//! **Replaces Crates:** `fang`, `faktory`, `celery` (basic functionality), `sidekiq`
//!
//! **Real-world Usage:**
//! - Sending emails asynchronously.
//! - Generating reports (PDF/Excel) out-of-band.
//! - Processing webhooks or third-party API callbacks.
//! - Video encoding and heavy background processing.
//!
//! **Why build it yourself?**
//! Job queues sit at the heart of distributed system reliability. Building one teaches you
//! about state transitions (Pending -> Processing -> Completed/Failed), concurrent worker coordination,
//! signaling with condition variables, and how to safely isolate failing tasks so they don't
//! crash the main event loop.

use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, VecDeque};
use std::error::Error;
use std::fmt;
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, Instant};

// =========================================================================================
// Architecture
// =========================================================================================
//
// Data Structure:
//
//      JobQueue (Arc<Mutex<State>>)
//      ├── Queues (HashMap<String, VecDeque<Job>>) -> FIFO per named queue
//      ├── Scheduled (BinaryHeap<ScheduledJob>)    -> Min-heap by execution time
//      ├── DeadLetters (Vec<Job>)                  -> Jobs that exhausted retries
//      └── Condvar                                 -> Wakes idle workers
//
// Invariants:
// 1. A job is only in ONE state at a time (Queued, Scheduled, Processing, DeadLetter).
// 2. Scheduled jobs are moved to the active queue ONLY when their execution time is reached.
// 3. Workers block efficiently using the Condvar when no jobs are available.
// 4. Failing jobs are retried up to `max_retries` times with exponential backoff.
//
// Complexity:
// ┌───────────────┬─────────────┬─────────────┐
// │ Operation     │ Time        │ Space       │
// ├───────────────┼─────────────┼─────────────┤
// │ enqueue       │ O(1)        │ O(1)        │
// │ schedule      │ O(log N)    │ O(1)        │
// │ dequeue       │ O(1)*       │ O(1)        │
// └───────────────┴─────────────┴─────────────┘
// * Dequeue might take O(N log N) if it needs to poll the scheduled heap, but typically O(1) for FIFO.
//
// Design Decisions:
// - **In-Memory**: Used for simplicity. A production system (like Redis/Postgres) ensures durability.
// - **Trait-based Tasks**: The `Job` payload is boxed `dyn Runnable` to support heterogeneous workloads.
// - **Condvar over Channels**: Channels are 1-way. We use `Condvar` because we need complex
//   condition checks (e.g., checking both the scheduled heap and active queues) before sleeping.

/// Represents the status of a job.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobStatus {
    Pending,
    Processing,
    Completed,
    Failed,
}

/// A unique identifier for a job.
pub type JobId = String;

/// The trait that all background jobs must implement.
///
/// # RUST INSIGHT: Send + Sync + 'static
/// Since jobs are executed on background threads, they must be safely transferable (`Send`).
/// Since they are stored in a shared queue, they need a `'static` lifetime (no borrowed data
/// referencing the stack that enqueued them).
///
/// # RUST INSIGHT: Immutable `&self` for Retries
/// The `run` method takes `&self` rather than `&mut self` or `Box<Self>`. Because jobs
/// might fail and need to be retried, we cannot safely consume (`self`) or easily allow
/// mutable state without risking partial-state leaks between retries. If a job requires
/// mutable state, the implementer is forced to use interior mutability (e.g., `Mutex`, `Atomic`).
pub trait Runnable: Send + 'static {
    fn run(&self) -> Result<(), Box<dyn Error + Send + Sync>>;
}

/// A Job envelope containing metadata and the executable payload.
pub struct Job {
    pub id: JobId,
    pub queue: String,
    pub payload: Box<dyn Runnable>,
    pub max_retries: u32,
    pub attempts: u32,
    pub status: JobStatus,
}

impl Job {
    #[must_use]
    pub fn new(queue: impl Into<String>, payload: Box<dyn Runnable>) -> Self {
        Self {
            id: uuid(), // Generate unique ID
            queue: queue.into(),
            payload,
            max_retries: 3,
            attempts: 0,
            status: JobStatus::Pending,
        }
    }

    #[must_use]
    pub fn with_retries(mut self, retries: u32) -> Self {
        self.max_retries = retries;
        self
    }
}

// Custom Debug implementation to omit the Runnable payload.
impl fmt::Debug for Job {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Job")
            .field("id", &self.id)
            .field("queue", &self.queue)
            .field("attempts", &self.attempts)
            .field("status", &self.status)
            .finish()
    }
}

/// A wrapper for jobs scheduled to run in the future.
/// Implements Ord/PartialOrd to act as a Min-Heap based on `execute_at`.
struct ScheduledJob {
    execute_at: Instant,
    job: Job,
}

impl PartialEq for ScheduledJob {
    fn eq(&self, other: &Self) -> bool {
        self.execute_at == other.execute_at
    }
}

impl Eq for ScheduledJob {}

impl PartialOrd for ScheduledJob {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ScheduledJob {
    fn cmp(&self, other: &Self) -> Ordering {
        // Reverse ordering so BinaryHeap acts as a Min-Heap.
        other.execute_at.cmp(&self.execute_at)
    }
}

/// The internal state of the Job Queue.
struct QueueState {
    queues: HashMap<String, VecDeque<Job>>,
    scheduled: BinaryHeap<ScheduledJob>,
    dead_letters: Vec<Job>,
    is_shutting_down: bool,
}

impl QueueState {
    fn new() -> Self {
        Self {
            queues: HashMap::new(),
            scheduled: BinaryHeap::new(),
            dead_letters: Vec::new(),
            is_shutting_down: false,
        }
    }

    /// Moves jobs from the scheduled heap to the active queues if their time has arrived.
    fn promote_scheduled_jobs(&mut self) -> Option<Instant> {
        let now = Instant::now();

        while let Some(scheduled) = self.scheduled.peek() {
            if scheduled.execute_at <= now {
                let scheduled_job = self.scheduled.pop().unwrap();
                self.queues
                    .entry(scheduled_job.job.queue.clone())
                    .or_default()
                    .push_back(scheduled_job.job);
            } else {
                return Some(scheduled.execute_at);
            }
        }
        None
    }
}

/// The main Job Queue Manager.
#[derive(Clone)]
pub struct JobQueue {
    state: Arc<Mutex<QueueState>>,
    cvar: Arc<Condvar>,
}

impl JobQueue {
    /// Creates a new, empty Job Queue.
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(QueueState::new())),
            cvar: Arc::new(Condvar::new()),
        }
    }

    /// Enqueues a job for immediate execution.
    pub fn enqueue(&self, job: Job) {
        // PRODUCTION NOTE: In a real system (Redis/Postgres), enqueueing is an atomic transaction
        // that ensures durability before signaling workers.
        let mut state = self.state.lock().unwrap();
        state
            .queues
            .entry(job.queue.clone())
            .or_default()
            .push_back(job);

        // GOTCHA: We must use `notify_all()` instead of `notify_one()`. Because workers
        // are bound to specific queue names, `notify_one()` might wake a worker for a different
        // queue, causing a lost wakeup and leaving our job stalled forever.
        self.cvar.notify_all();
    }

    /// Schedules a job to be executed after a specific delay.
    pub fn enqueue_in(&self, job: Job, delay: Duration) {
        let execute_at = Instant::now() + delay;
        let mut state = self.state.lock().unwrap();

        state.scheduled.push(ScheduledJob { execute_at, job });

        // Notify in case this job is now the soonest to execute.
        self.cvar.notify_all();
    }

    /// Fetches the next available job from the specified queue.
    /// Blocks if no jobs are available.
    fn dequeue(&self, queue_name: &str) -> Option<Job> {
        let mut state = self.state.lock().unwrap();

        loop {
            if state.is_shutting_down {
                return None;
            }

            // Promote any scheduled jobs that are ready.
            let next_scheduled_time = state.promote_scheduled_jobs();

            // Try to pop a job from the active queue.
            if let Some(queue) = state.queues.get_mut(queue_name) {
                if let Some(mut job) = queue.pop_front() {
                    job.status = JobStatus::Processing;
                    return Some(job);
                }
            }

            // No jobs available. We need to sleep.
            if let Some(wake_at) = next_scheduled_time {
                let now = Instant::now();
                if wake_at > now {
                    let timeout = wake_at - now;
                    // RUST INSIGHT: `wait_timeout` releases the Mutex and puts the thread to sleep.
                    // It re-acquires the Mutex when awoken by a Condvar notification or timeout.
                    let (new_state, _timeout_result) =
                        self.cvar.wait_timeout(state, timeout).unwrap();
                    state = new_state;
                }
            } else {
                // No scheduled jobs, sleep indefinitely until notified.
                state = self.cvar.wait(state).unwrap();
            }
        }
    }

    /// Handles job completion or failure.
    fn finalize_job(&self, mut job: Job, success: bool) {
        let mut state = self.state.lock().unwrap();

        if success {
            // Job completed successfully. It can be safely dropped.
            return;
        }

        job.attempts += 1;

        if job.attempts <= job.max_retries {
            // Re-schedule with exponential backoff: (attempts^2) seconds.
            let backoff_secs = (job.attempts * job.attempts) as u64;
            let execute_at = Instant::now() + Duration::from_secs(backoff_secs);

            job.status = JobStatus::Pending;
            state.scheduled.push(ScheduledJob { execute_at, job });
            self.cvar.notify_all();
        } else {
            // Exhausted retries. Move to dead letter queue.
            job.status = JobStatus::Failed;
            state.dead_letters.push(job);
        }
    }

    /// Signals all workers to shut down.
    pub fn shutdown(&self) {
        let mut state = self.state.lock().unwrap();
        state.is_shutting_down = true;
        self.cvar.notify_all();
    }

    /// Returns the number of jobs currently in the dead letter queue.
    #[must_use]
    pub fn dead_letter_count(&self) -> usize {
        self.state.lock().unwrap().dead_letters.len()
    }
}

impl Default for JobQueue {
    fn default() -> Self {
        Self::new()
    }
}

/// A worker that polls the queue and executes jobs.
pub struct Worker {
    id: usize,
    queue_name: String,
    job_queue: JobQueue,
    handle: Option<thread::JoinHandle<()>>,
}

impl Worker {
    /// Starts a new worker in a background thread.
    #[must_use]
    pub fn start(id: usize, queue_name: impl Into<String>, job_queue: JobQueue) -> Self {
        let queue_name = queue_name.into();
        let queue_name_clone = queue_name.clone();
        let job_queue_clone = job_queue.clone();

        let handle = thread::spawn(move || {
            loop {
                let job_opt = job_queue_clone.dequeue(&queue_name_clone);

                match job_opt {
                    Some(job) => {
                        // GOTCHA: We execute the job outside the Mutex lock!
                        // If we held the lock here, no other workers could dequeue jobs.

                        // Catch panics inside the job to prevent the worker thread from dying.
                        let payload_result =
                            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                                job.payload.run()
                            }));

                        let success = match payload_result {
                            Ok(Ok(())) => true,
                            Ok(Err(_err)) => {
                                // Normal Error returned by run()
                                false
                            }
                            Err(_) => {
                                // Panic inside run()
                                false
                            }
                        };

                        job_queue_clone.finalize_job(job, success);
                    }
                    None => {
                        // Queue is shutting down.
                        break;
                    }
                }
            }
        });

        Self {
            id,
            queue_name,
            job_queue,
            handle: Some(handle),
        }
    }

    /// Waits for the worker thread to finish.
    pub fn join(&mut self) {
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

// Simple UUID generator for job IDs
fn uuid() -> String {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static COUNTER: AtomicUsize = AtomicUsize::new(1);
    format!("job-{}", COUNTER.fetch_add(1, Ordering::Relaxed))
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `celery` / `sidekiq`: Production queues are backed by Redis or RabbitMQ to ensure durability
//   across process restarts. This implementation is purely in-memory.
// - `fang`: Uses PostgreSQL as a backend for durability.
//
// Missing vs. Production:
// - **Durability**: If the process crashes, all queued jobs are lost.
// - **Fairness/Weights**: Cannot prioritize one queue over another dynamically.
// - **Visibility Timeout**: If a worker thread is killed mid-processing (e.g. OOM killer),
//   the job is permanently lost. Production queues use 'visibility timeouts' to re-enqueue
//   jobs if the worker dies.
//
// Next Steps:
// 1. Back the `QueueState` with SQLite or Redis.
// 2. Add visibility timeouts using a secondary processing set.
// 3. Add priority queues.
// 4. Benchmarking: To benchmark this queue, you would use `criterion`. Create a suite
//    that enqueues 100k empty jobs, starts a thread pool, and measures the throughput
//    (jobs per second) utilizing `Instant::now()` and `std::hint::black_box`.

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct TestJob {
        counter: Arc<AtomicUsize>,
        should_fail: bool,
    }

    impl Runnable for TestJob {
        fn run(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
            if self.should_fail {
                return Err("Intentional failure".into());
            }
            self.counter.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    #[test]
    fn test_job_queue_basic_execution() {
        let queue = JobQueue::new();
        let counter = Arc::new(AtomicUsize::new(0));

        let mut worker = Worker::start(1, "default", queue.clone());

        // Enqueue 5 jobs
        for _ in 0..5 {
            let payload = Box::new(TestJob {
                counter: counter.clone(),
                should_fail: false,
            });
            queue.enqueue(Job::new("default", payload));
        }

        // Wait a bit for processing
        thread::sleep(Duration::from_millis(50));

        queue.shutdown();
        worker.join();

        assert_eq!(counter.load(Ordering::SeqCst), 5);
    }

    #[test]
    fn test_job_queue_delayed_execution() {
        let queue = JobQueue::new();
        let counter = Arc::new(AtomicUsize::new(0));

        let mut worker = Worker::start(1, "default", queue.clone());

        let payload = Box::new(TestJob {
            counter: counter.clone(),
            should_fail: false,
        });

        // Enqueue with a delay
        queue.enqueue_in(Job::new("default", payload), Duration::from_millis(100));

        // Immediately, it should be 0
        thread::sleep(Duration::from_millis(10));
        assert_eq!(counter.load(Ordering::SeqCst), 0);

        // After delay, it should be processed
        thread::sleep(Duration::from_millis(150));
        assert_eq!(counter.load(Ordering::SeqCst), 1);

        queue.shutdown();
        worker.join();
    }

    #[test]
    fn test_job_queue_retries_and_dead_letter() {
        let queue = JobQueue::new();
        let counter = Arc::new(AtomicUsize::new(0));

        let mut worker = Worker::start(1, "default", queue.clone());

        // Enqueue a job that always fails, with max_retries = 1
        let payload = Box::new(TestJob {
            counter: counter.clone(),
            should_fail: true,
        });

        let job = Job::new("default", payload).with_retries(1);
        queue.enqueue(job);

        // First attempt (fails), backoff is 1^2 = 1 sec.
        // For tests to run fast, we need a way to mock time, but we use real time.
        // We'll just assert it goes to DLQ eventually, though it might take a second.

        // Let's speed up the backoff for testing by skipping the assertion of exact backoff time,
        // or accepting that this test will take ~1 second.

        thread::sleep(Duration::from_millis(1200));

        queue.shutdown();
        worker.join();

        // The job should have been attempted 1 initial + 1 retry = 2 times.
        // (Our TestJob doesn't increment on failure, but we can check the DLQ).
        assert_eq!(queue.dead_letter_count(), 1);
    }
}
