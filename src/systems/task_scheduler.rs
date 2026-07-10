//! # Task Scheduler Implementation
//!
//! Implements a Cron-like in-memory task scheduler using a Priority Queue.
//! It executes tasks concurrently when their scheduled time arrives.
//!
//! **Replaces Crates:** `clokwerk`, `tokio-cron-scheduler`, `delay_timer`
//!
//! **Real-world Usage:**
//! - Background job processing (e.g., sending emails).
//! - Maintenance tasks like database vacuuming or log rotation.
//! - Periodic cache invalidation.
//!
//! **Why build it yourself?**
//! Building a task scheduler teaches you about precise time management, concurrency,
//! and thread coordination. You will learn how to use a `BinaryHeap` as a Priority Queue
//! to efficiently determine the next task to run, and how to use `Condvar` to efficiently
//! put a worker thread to sleep and wake it up only when necessary.

// The scheduler lock is intentionally held across the wait/peek/pop worker-loop critical sections.
#![allow(clippy::significant_drop_tightening)]

use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, Instant};

// =========================================================================================
// Architecture
// =========================================================================================
//
// Flow:
//
//     Client                     Task Scheduler
//       │                              │
//       ▼                              ▼
//   schedule() ──► [ Mutex<BinaryHeap<Task>> ] ◄── Background Thread
//                          │                   (Sleeps via Condvar until
//                          │                    next task is due)
//                          │                           │
//                          ▼                           ▼
//                  Wakes up thread if          Pops due tasks and
//                  new task is sooner          spawns worker threads
//                  than current sleep.         to execute them.
//
// Invariants:
// 1. The `BinaryHeap` always keeps the task with the *earliest* execution time at the top.
// 2. The background thread must sleep only as long as the time remaining until the next task.
// 3. The background thread must be woken up early if a *new* task is scheduled before the current next task.
//
// Complexity:
// ┌───────────────┬─────────────┬─────────────┐
// │ Operation     │ Time        │ Space       │
// ├───────────────┼─────────────┼─────────────┤
// │ Schedule      │ O(log N)    │ O(1)        │
// │ Execute Next  │ O(log N)    │ O(1)        │
// └───────────────┴─────────────┴─────────────┘
//
// Design Decisions:
// - **Storage**: `BinaryHeap` provides O(1) peek and O(log N) insert/pop. We wrap it in a `Mutex` for thread safety.
// - **Concurrency**: `Condvar` is used to sleep the background thread. `wait_timeout` handles waking up for the next task or earlier if notified.
// - **Task Execution**: Tasks are executed in spawned threads to prevent blocking the scheduler thread. In a real system, a thread pool would be better to avoid unbounded thread creation.
// - **Task State**: We store tasks as `Box<dyn FnOnce() + Send + 'static>`.

/// Represents a scheduled task.
struct Task {
    /// The absolute time when the task should run.
    run_at: Instant,
    /// The closure to execute.
    /// Needs to be Boxed because it's a trait object, and Send + 'static
    /// so it can be moved to another thread to execute safely.
    action: Box<dyn FnOnce() + Send + 'static>,
}

impl Task {
    fn new<F>(run_at: Instant, action: F) -> Self
    where
        F: FnOnce() + Send + 'static,
    {
        Self {
            run_at,
            action: Box::new(action),
        }
    }
}

// RUST INSIGHT: We must implement `Ord` and `PartialOrd` for `Task` to use it in `BinaryHeap`.
// `BinaryHeap` is a max-heap by default, but we want a min-heap (earliest time first).
// We invert the comparison of `Instant` to achieve this.

impl PartialEq for Task {
    fn eq(&self, other: &Self) -> bool {
        self.run_at == other.run_at
    }
}

impl Eq for Task {}

impl PartialOrd for Task {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Task {
    fn cmp(&self, other: &Self) -> Ordering {
        // Reverse ordering so the earliest `run_at` is treated as the "greatest"
        // in the max-heap context, putting it at the top of the heap.
        other.run_at.cmp(&self.run_at)
    }
}

/// Shared state of the scheduler.
struct SchedulerState {
    tasks: BinaryHeap<Task>,
    running: bool,
}

/// A trait defining the operations of a task scheduler.
/// This allows swapping out implementations (e.g., in-memory vs. Redis-backed).
pub trait Scheduler: Send + Sync {
    /// Schedules a task to run after a specified duration.
    fn schedule_in<F>(&self, delay: Duration, action: F)
    where
        F: FnOnce() + Send + 'static;

    /// Schedules a task to run at a specific absolute time.
    fn schedule_at<F>(&self, run_at: Instant, action: F)
    where
        F: FnOnce() + Send + 'static;

    /// Gracefully stops the scheduler.
    fn stop(&mut self);
}

/// A background task scheduler using a `BinaryHeap`.
pub struct TaskScheduler {
    state: Arc<Mutex<SchedulerState>>,
    condvar: Arc<Condvar>,
    worker_handle: Option<thread::JoinHandle<()>>,
}

impl TaskScheduler {
    /// Creates a new `TaskScheduler` and spawns its background thread.
    #[must_use]
    pub fn new() -> Self {
        let state = Arc::new(Mutex::new(SchedulerState {
            tasks: BinaryHeap::new(),
            running: true,
        }));
        let condvar = Arc::new(Condvar::new());

        let state_clone = Arc::clone(&state);
        let condvar_clone = Arc::clone(&condvar);

        // Spawn the background worker thread
        let worker_handle = thread::spawn(move || {
            Self::worker_loop(state_clone, condvar_clone);
        });

        Self {
            state,
            condvar,
            worker_handle: Some(worker_handle),
        }
    }

    /// The core loop of the background thread.
    // The worker thread owns these Arc handles for its entire lifetime, so it takes them by value.
    #[allow(clippy::needless_pass_by_value)]
    fn worker_loop(state_arc: Arc<Mutex<SchedulerState>>, condvar: Arc<Condvar>) {
        let mut state = state_arc.lock().unwrap();

        while state.running {
            let now = Instant::now();

            if let Some(top) = state.tasks.peek() {
                if top.run_at <= now {
                    // Task is due! Pop it and execute.
                    // GOTCHA: We must `pop` before dropping the Mutex to execute,
                    // otherwise another thread could pop it too (if we had multiple workers).
                    let task = state.tasks.pop().unwrap();

                    // Drop the lock while executing the task to avoid deadlocking the scheduler
                    // if the task itself tries to schedule another task.
                    drop(state);

                    // PRODUCTION NOTE: We spawn a new thread for each task to prevent blocking the scheduler.
                    // A production crate would submit this to a thread pool instead.
                    thread::spawn(move || {
                        (task.action)();
                    });

                    // Re-acquire lock to continue looping
                    state = state_arc.lock().unwrap();
                } else {
                    // Top task is not due yet.
                    let delay = top.run_at - now;
                    // Sleep until the task is due.
                    // If a new task is inserted that is earlier, `wait_timeout` will be interrupted early
                    // by the `condvar.notify_one()` in `schedule_at`.
                    let (new_state, _timeout_result) = condvar.wait_timeout(state, delay).unwrap();
                    state = new_state;
                }
            } else {
                // No tasks. Wait indefinitely until one is scheduled.
                state = condvar.wait(state).unwrap();
            }
        }
    }
}

impl Scheduler for TaskScheduler {
    fn schedule_in<F>(&self, delay: Duration, action: F)
    where
        F: FnOnce() + Send + 'static,
    {
        let run_at = Instant::now() + delay;
        self.schedule_at(run_at, action);
    }

    fn schedule_at<F>(&self, run_at: Instant, action: F)
    where
        F: FnOnce() + Send + 'static,
    {
        let mut state = self.state.lock().unwrap();

        // Check if the newly scheduled task is sooner than all existing tasks.
        // If it is, we need to wake up the worker thread so it can adjust its sleep timeout.
        let is_earliest = state.tasks.peek().is_none_or(|top| run_at < top.run_at);

        state.tasks.push(Task::new(run_at, action));

        if is_earliest {
            // Wake up the worker thread
            self.condvar.notify_one();
        }
    }

    fn stop(&mut self) {
        let mut state = self.state.lock().unwrap();
        state.running = false;
        self.condvar.notify_all();
    }
}

impl Default for TaskScheduler {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for TaskScheduler {
    fn drop(&mut self) {
        self.stop();

        // Wait for the worker thread to finish.
        if let Some(handle) = self.worker_handle.take() {
            let _ = handle.join();
        }
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `tokio-cron-scheduler`: Integrates tightly with Tokio's async ecosystem and supports parsing
//    Cron strings (e.g. `* * * * *`). This implementation relies purely on `std::time::Instant` and the
//    synchronous `Condvar`.
// - `clokwerk`: Follows a similar syntax and logic to this but has robust repeating task semantics
//   (e.g., `.every(1.day()).at("3:20 pm")`).
//
// Missing vs. Production:
// - **Repeating Tasks:** Currently, tasks only run once. Real schedulers support repeating execution.
// - **Cron String Parsing:** Supporting Unix Cron format requires writing a parsing engine.
// - **Thread Pool:** Spawning a new thread per task is expensive. Production schedulers use a ThreadPool.
// - **Task Cancellation:** Returning a task ID or handle that can be used to remove the task from the heap
//   is necessary for full control.
//
// Benchmarking Note:
// To benchmark the `schedule_at` performance, use `criterion` to measure the overhead of
// acquiring the `Mutex` and inserting into the `BinaryHeap` (O(log N)).
// Ensure the worker thread is either stopped or tasks are scheduled far into the future
// to prevent execution overhead from polluting the scheduling benchmark.
// Example: `b.iter(|| scheduler.schedule_at(black_box(future_instant), || {}))`
//
// Next Steps:
// 1. Add repeating task capabilities.
// 2. Refactor to submit actions to `src/concurrency/thread_pool.rs`.
// 3. Add task cancellation via handles.

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    #[test]
    fn test_schedule_in() {
        let scheduler = TaskScheduler::new();
        let executed = Arc::new(AtomicBool::new(false));
        let executed_clone = executed.clone();

        let start = Instant::now();
        scheduler.schedule_in(Duration::from_millis(50), move || {
            executed_clone.store(true, Ordering::SeqCst);
        });

        // Wait slightly longer than the delay
        thread::sleep(Duration::from_millis(100));

        assert!(executed.load(Ordering::SeqCst));
        assert!(start.elapsed() >= Duration::from_millis(50));
    }

    #[test]
    fn test_schedule_multiple_in_order() {
        let scheduler = TaskScheduler::new();
        let results = Arc::new(Mutex::new(Vec::new()));

        let r1 = results.clone();
        scheduler.schedule_in(Duration::from_millis(100), move || {
            r1.lock().unwrap().push(2);
        });

        let r2 = results.clone();
        scheduler.schedule_in(Duration::from_millis(20), move || {
            r2.lock().unwrap().push(1);
        });

        let r3 = results.clone();
        scheduler.schedule_in(Duration::from_millis(150), move || {
            r3.lock().unwrap().push(3);
        });

        thread::sleep(Duration::from_millis(200));

        let final_results = results.lock().unwrap();
        assert_eq!(*final_results, vec![1, 2, 3]);
    }

    #[test]
    fn test_concurrent_scheduling() {
        let scheduler = Arc::new(TaskScheduler::new());
        let counter = Arc::new(AtomicUsize::new(0));
        let mut handles = vec![];

        for _ in 0..10 {
            let sched_clone = scheduler.clone();
            let count_clone = counter.clone();
            handles.push(thread::spawn(move || {
                sched_clone.schedule_in(Duration::from_millis(10), move || {
                    count_clone.fetch_add(1, Ordering::SeqCst);
                });
            }));
        }

        for h in handles {
            h.join().unwrap();
        }

        // Wait for all 10 tasks to fire
        thread::sleep(Duration::from_millis(50));
        assert_eq!(counter.load(Ordering::SeqCst), 10);
    }
}
