//! # Async Runtime Implementation
//!
//! A minimal, educational implementation of an asynchronous executor and timer-based reactor from scratch.
//!
//! **Replaces Crates:** `tokio`, `async-std`, `smol` (partial)
//!
//! **Real-world Usage:**
//! - High-concurrency network servers (e.g., hyper, actix-web)
//! - Highly concurrent I/O operations and database access
//! - Scheduled task running and job scheduling systems
//!
//! **Why build it yourself?**
//! Building an async runtime demystifies Rust's `Future` trait and the async/await machinery.
//! It teaches you exactly how executors poll futures, how Wakers tell the executor to poll again,
//! and how a reactor interfaces with system resources (like timers or epoll) to wake up tasks.
//! You will learn about `Pin`, `Context`, `Waker`, and custom arc-based waker implementations.
//!
//! # Architecture
//!
//! **Benchmarking Note:**
//! In a real scenario, you would benchmark this runtime by spawning N tasks that each sleep for M milliseconds and measuring total execution time vs `tokio` using `criterion`.
//!
//! **Design decisions and tradeoffs vs. alternatives:**
//! - **BTreeMap vs. Timer Wheel:** We chose `BTreeMap` for the timer queue for its simplicity, trading off the O(1) amortized performance of a hashed timer wheel for O(log N) inserts.
//! - **Mutex vs. Lock-free queues:** The task future is wrapped in a `Mutex` to satisfy `Sync` for the Waker, and we use a basic channel for the ready queue instead of a lock-free work-stealing deque.
//!
//! **Components:**
//! 1. **Executor**: Runs spawned futures by continually calling their `poll` method when woken.
//! 2. **Spawner**: Creates tasks and pushes them into the executor's task queue.
//! 3. **Task**: Wraps a `Future` alongside a mechanism to schedule it (push back to the queue) when woken.
//! 4. **TimerReactor**: Manages `Sleep` futures, keeping track of when they should complete using a thread and a `BTreeMap`.
//!
//! **Diagram:**
//! ```text
//!    Spawner
//!       │
//!       ▼ (Spawn Task)
//!    Task Queue ◄────────┐
//!       │                │
//!       ▼                │ (Waker calls wake())
//!    Executor            │
//!       │                │
//!    (poll) ────────┐    │
//!       │           │    │
//!       ▼           ▼    │
//!    Future 1    Sleep Future ───► TimerReactor (Thread)
//!                                    │
//!                                (Time elapses)
//! ```
//!
//! **Invariants:**
//! - A Task's future must be `Pin<Box<dyn Future>>` because futures can be self-referential.
//! - The `TimerReactor` must uniquely identify timers (using `(Instant, usize)` instead of just `Instant`) to avoid dropping concurrent timers.
//! - The `Waker` must be thread-safe (Send + Sync) so that background threads (reactor) can wake tasks running on the executor thread.
//!
//! **Complexity:**
//! - **Spawn**: O(1)
//! - **Executor Loop Iteration**: O(1) per ready task
//! - **Timer Insert (Reactor)**: O(log N)
//! - **Timer Fire (Reactor)**: O(1) to check, O(log N) per fired timer to remove

use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex, OnceLock};
use std::task::{Context, Poll, Wake, Waker};
use std::thread;
use std::time::{Duration, Instant};

// =========================================================================================
// Reactor (Timer)
// =========================================================================================

/// Global singleton for the timer reactor.
static TIMER_REACTOR: OnceLock<&'static TimerReactor> = OnceLock::new();

/// Manages active sleep futures and wakes them when their time has elapsed.
pub struct TimerReactor {
    // RUST INSIGHT: We use a BTreeMap ordered by Instant to efficiently find expired timers.
    // GOTCHA: If multiple timers are set for the exact same Instant, a pure BTreeMap<Instant, Waker>
    // would overwrite the previous waker. We use a tuple `(Instant, usize)` with an atomic counter
    // to disambiguate identical Instants.
    timers: Mutex<BTreeMap<(Instant, usize), Waker>>,
    id_generator: AtomicUsize,
}

impl TimerReactor {
    /// Creates and starts a new TimerReactor on a background thread.
    #[must_use]
    pub fn new() -> Self {
        Self {
            timers: Mutex::new(BTreeMap::new()),
            id_generator: AtomicUsize::new(0),
        }
    }

    /// Starts the reactor thread that continuously checks for expired timers.
    pub fn start(self: Arc<Self>) {
        thread::spawn(move || {
            loop {
                let now = Instant::now();
                let mut timers_to_wake = Vec::new();

                // Scope for the mutex lock
                {
                    let mut timers = self.timers.lock().unwrap();

                    // Split the map at the current time
                    // All timers with keys < (now, 0) are expired.
                    let mut keys_to_remove = Vec::new();
                    for (key, waker) in timers.iter() {
                        if key.0 <= now {
                            // PRODUCTION NOTE: A real runtime like Tokio uses a hierarchical timer wheel
                            // for O(1) amortized inserts and fires, rather than a BTreeMap which is O(log N).
                            timers_to_wake.push(waker.clone());
                            keys_to_remove.push(*key);
                        } else {
                            // Since it's sorted, we can stop at the first non-expired timer.
                            break;
                        }
                    }

                    for key in keys_to_remove {
                        timers.remove(&key);
                    }
                }

                // Wake the futures outside the lock to avoid potential deadlocks if a waker does something complex.
                for waker in timers_to_wake {
                    waker.wake();
                }

                // Sleep briefly to prevent 100% CPU usage.
                thread::sleep(Duration::from_millis(1));
            }
        });
    }

    /// Registers a waker to be called when the specified `when` Instant is reached.
    pub fn register_timer(&self, when: Instant, waker: Waker) {
        let id = self.id_generator.fetch_add(1, Ordering::SeqCst);
        let mut timers = self.timers.lock().unwrap();
        timers.insert((when, id), waker);
    }

    /// Initializes the global timer reactor if not already initialized.
    pub fn get() -> &'static TimerReactor {
        TIMER_REACTOR.get_or_init(|| {
            let reactor = Box::new(TimerReactor::new());
            let static_reactor: &'static TimerReactor = Box::leak(reactor);

            // Start the background thread using the static reference
            thread::spawn(move || {
                loop {
                    let now = Instant::now();
                    let mut timers_to_wake = Vec::new();

                    {
                        let mut timers = static_reactor.timers.lock().unwrap();
                        let mut keys_to_remove = Vec::new();
                        for (key, waker) in timers.iter() {
                            if key.0 <= now {
                                timers_to_wake.push(waker.clone());
                                keys_to_remove.push(*key);
                            } else {
                                break;
                            }
                        }

                        for key in keys_to_remove {
                            timers.remove(&key);
                        }
                    }

                    for waker in timers_to_wake {
                        waker.wake();
                    }

                    thread::sleep(Duration::from_millis(1));
                }
            });

            static_reactor
        })
    }
}

impl Default for TimerReactor {
    fn default() -> Self {
        Self::new()
    }
}

// =========================================================================================
// Futures
// =========================================================================================

/// A future that completes after a given duration.
pub struct Sleep {
    when: Instant,
}

impl Sleep {
    /// Creates a new Sleep future.
    #[must_use]
    pub fn new(duration: Duration) -> Self {
        Self {
            when: Instant::now() + duration,
        }
    }
}

impl Future for Sleep {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if Instant::now() >= self.when {
            Poll::Ready(())
        } else {
            // RUST INSIGHT: We register the waker with the reactor. The reactor will call `wake()`
            // on this waker when the timer expires, which puts the task back on the executor's queue.
            TimerReactor::get().register_timer(self.when, cx.waker().clone());
            Poll::Pending
        }
    }
}

/// Helper function to sleep for a given duration.
#[must_use]
pub fn sleep(duration: Duration) -> Sleep {
    Sleep::new(duration)
}

// =========================================================================================
// Task and Waker
// =========================================================================================

/// A unit of work managed by the executor.
/// Contains the future and a channel sender to re-queue itself.
struct Task {
    /// The actual future being run. Needs Mutex because `wake` can be called from other threads,
    /// and `poll` needs mutable access. `Pin<Box>` ensures the future doesn't move in memory.
    future: Mutex<Option<Pin<Box<dyn Future<Output = ()> + Send + 'static>>>>,
    /// Channel to send the task back to the executor when woken.
    task_sender: Sender<Arc<Task>>,
}

impl Wake for Task {
    /// Called when the task is ready to make progress (e.g., timer expired).
    fn wake(self: Arc<Self>) {
        // Send the cloned Arc back to the executor's queue.
        // If the executor has shut down, this might fail, which is fine (we just ignore the error).
        let _ = self.task_sender.send(self.clone());
    }
}

// =========================================================================================
// Executor and Spawner
// =========================================================================================

/// Spawns new tasks onto the executor.
#[derive(Clone)]
pub struct Spawner {
    task_sender: Sender<Arc<Task>>,
}

impl Spawner {
    /// Spawns a future onto the executor.
    pub fn spawn(&self, future: impl Future<Output = ()> + Send + 'static) {
        let future = Box::pin(future);
        let task = Arc::new(Task {
            future: Mutex::new(Some(future)),
            task_sender: self.task_sender.clone(),
        });

        // Push the new task into the executor's queue.
        let _ = self.task_sender.send(task);
    }
}

/// Runs tasks to completion.
pub trait AsyncExecutor {
    /// Runs all tasks in the queue until the queue is closed and all tasks are completed.
    fn run(&self);
}

pub struct Executor {
    ready_queue: Receiver<Arc<Task>>,
}

impl AsyncExecutor for Executor {
    fn run(&self) {
        // RUST INSIGHT: The executor just loops over the ready queue.
        // It blocks until a task is available, then polls it.
        while let Ok(task) = self.ready_queue.recv() {
            let mut future_slot = task.future.lock().unwrap();

            // Take the future out to poll it. If it's already None, it means the task completed.
            if let Some(mut future) = future_slot.take() {
                // Create a Waker from the Arc<Task>
                // This uses the `Wake` trait implementation on `Task`.
                let waker = Waker::from(task.clone());
                let mut context = Context::from_waker(&waker);

                // Poll the future.
                match future.as_mut().poll(&mut context) {
                    Poll::Ready(()) => {
                        // Task is done, we don't put the future back.
                    }
                    Poll::Pending => {
                        // Task is not done. Put the future back in the task so it can be polled again
                        // when the waker wakes it up.
                        *future_slot = Some(future);
                    }
                }
            }
        }
    }
}

/// Creates a new executor and spawner pair.
#[must_use]
pub fn new_executor_and_spawner() -> (Executor, Spawner) {
    // We use an unbounded channel for simplicity, though a real runtime might use bounded
    // or work-stealing queues.
    let (task_sender, ready_queue) = channel();
    (Executor { ready_queue }, Spawner { task_sender })
}

// =========================================================================================
// Tests
// =========================================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::time::Instant;

    #[test]
    fn test_basic_spawn_and_run() {
        let (executor, spawner) = new_executor_and_spawner();
        let flag = Arc::new(AtomicBool::new(false));
        let flag_clone = Arc::clone(&flag);

        spawner.spawn(async move {
            flag_clone.store(true, Ordering::SeqCst);
        });

        // Drop the spawner so the executor knows no more tasks will be spawned
        // and the channel will close when all tasks are done.
        drop(spawner);

        executor.run();

        assert!(flag.load(Ordering::SeqCst));
    }

    #[test]
    fn test_sleep_future() {
        let (executor, spawner) = new_executor_and_spawner();
        let start = Instant::now();
        let completed = Arc::new(AtomicBool::new(false));
        let completed_clone = Arc::clone(&completed);

        spawner.spawn(async move {
            sleep(Duration::from_millis(50)).await;
            completed_clone.store(true, Ordering::SeqCst);
        });

        drop(spawner);
        executor.run();

        let elapsed = start.elapsed();
        assert!(completed.load(Ordering::SeqCst));
        assert!(elapsed >= Duration::from_millis(50));
    }

    #[test]
    fn test_multiple_concurrent_tasks() {
        let (executor, spawner) = new_executor_and_spawner();
        let counter = Arc::new(AtomicUsize::new(0));

        for _ in 0..10 {
            let counter_clone = Arc::clone(&counter);
            spawner.spawn(async move {
                sleep(Duration::from_millis(10)).await;
                counter_clone.fetch_add(1, Ordering::SeqCst);
            });
        }

        drop(spawner);
        executor.run();

        assert_eq!(counter.load(Ordering::SeqCst), 10);
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// **Comparison to Canonical Crates:**
// - `tokio`: This is a toy version of Tokio's current-thread executor. Tokio is vastly more complex,
//   using lock-free MPSC queues, epoll/kqueue for I/O readiness (not just timers), and a highly optimized
//   timer wheel.
// - `async-std`: Similar architecture but uses work-stealing across multiple threads.
//
// **What's missing vs Production:**
// - I/O Polling: We only handle time (`sleep`), not networking (`TcpStream`, `UdpSocket`) or file I/O.
// - Work Stealing: We only have a single executor thread. A real runtime uses multiple threads and work-stealing queues.
// - Timer Wheel: `BTreeMap` is O(log N). Production timers use hierarchical timer wheels for O(1) inserts/fires.
// - Waker Optimizations: Our `Task` wraps the future in a `Mutex` to satisfy `Sync` for the Waker,
//   which adds overhead. Real wakers use unsafe raw pointers to avoid this.
//
// **Next Steps:**
// - Implement a `TcpStream` future using `epoll` (Linux) or `kqueue` (macOS).
// - Add a thread pool to the executor and implement a basic work-stealing queue.
