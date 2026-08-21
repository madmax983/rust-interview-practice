//! # Async Runtime
//!
//! What this implements and what crate(s) it replaces:
//! This implements a minimal asynchronous runtime (Executor and Reactor) from scratch.
//! It replaces crates like `tokio`, `async-std`, `smol`, and `pollster`.
//!
//! Real-world systems that use this:
//! - Highly concurrent network servers (e.g., HTTP servers like Axum/Actix).
//! - Database drivers handling thousands of connections multiplexed over a few threads.
//! - Background task schedulers and timers in distributed systems.
//!
//! Why build it yourself?
//! Asynchronous Rust can seem like "magic" because the standard library only provides
//! the interfaces (`Future`, `Waker`, `Context`) but no implementation. Building a
//! runtime demystifies how `async/.await` maps to a state machine, how tasks are scheduled
//! on a thread pool, and how non-blocking I/O or timers notify the executor to resume execution.
//!
//! # Architecture
//!
//! - **Executor**: A loop that receives tasks ready to make progress from a channel and polls them.
//! - **Spawner**: A handle to enqueue new tasks onto the executor's channel.
//! - **Task**: A wrapper around a boxed future that implements `std::task::Wake` to re-enqueue itself.
//! - **Reactor**: The external event source (represented here by `TimerFuture` threads) that wakes the task.
//!
//! ```text
//!  +-----------+                  +-----------+
//!  |           |     spawn()      |           |
//!  |  Spawner  | ---------------->| Channel   |
//!  |           |                  |           |
//!  +-----------+                  +-----------+
//!                                       |
//!                                     recv()
//!                                       v
//!  +-----------+      wake()      +-----------+
//!  |           | <----------------|           |
//!  |  Reactor  |                  | Executor  |
//!  | (Timers)  | ---------------->|           |
//!  +-----------+    poll()        +-----------+
//! ```
//!
//! **Invariants:**
//! - A `Task` must only be queued once at a time. The bounded channel and `take()` mechanism ensure a task isn't double-polled.
//! - The `Waker` must be thread-safe (`Send + Sync`) because the reactor (timer thread) signals the executor thread asynchronously.
//! - A `poll` implementation must never block the executing thread; it must return `Poll::Pending` immediately if work isn't ready.
//!
//! | Operation | Time Complexity | Space Complexity |
//! |-----------|-----------------|------------------|
//! | `spawn`   | O(1)            | O(1)             |
//! | `poll`    | O(1)*           | O(1)             |
//! | `wake`    | O(1)            | O(1)             |
//!
//! *Time complexity of `poll` depends on the underlying future's progress before yielding.
//!
//! # Design Decisions and Tradeoffs
//! - **Channel-based Scheduling**: We use an `std::sync::mpsc::sync_channel` to schedule tasks.
//!   This is simple and thread-safe but less performant than a lock-free work-stealing queue
//!   (like Tokio's `crossbeam`-inspired scheduler).
//! - **Thread-per-Timer**: For simplicity, our `TimerFuture` spawns a thread to sleep and wake.
//!   A real reactor uses a single thread with a timer wheel and `epoll`/`kqueue`/`io_uring`
//!   to handle thousands of events concurrently.
//! - **Boxed Futures**: We allocate each task on the heap via `Box::pin`. This avoids complex
//!   intrusive linked list scheduling but incurs allocation overhead.

use std::future::Future;
use std::pin::Pin;
use std::sync::mpsc::{Receiver, SyncSender, sync_channel};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Wake, Waker};
use std::thread;
use std::time::Duration;

/// A future that resolves after a specified duration.
pub struct TimerFuture {
    shared_state: Arc<Mutex<SharedState>>,
}

/// Shared state between the future and the waiting thread
struct SharedState {
    completed: bool,
    waker: Option<Waker>,
}

impl Future for TimerFuture {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // RUST INSIGHT: The runtime enforces that `poll` is fast and non-blocking.
        // We only check if the condition is met (lock is just for shared state protection).
        let mut shared_state = self.shared_state.lock().expect("lock poisoned");

        if shared_state.completed {
            Poll::Ready(())
        } else {
            // RUST INSIGHT: The Waker is cloned and stored. The reactor will use this
            // to notify the executor when the state changes, safely bridging the
            // synchronous reactor world and asynchronous executor world.
            // GOTCHA: We must clone the waker on *every* pending poll, because the task
            // might have been moved to a different executor or the waker might have been replaced.
            shared_state.waker = Some(cx.waker().clone());
            Poll::Pending
        }
    }
}

impl TimerFuture {
    /// Creates a new `TimerFuture` which will resolve after the provided duration.
    ///
    /// # Panics
    /// Panics if the internal thread spawning fails or mutex lock is poisoned.
    #[must_use]
    pub fn new(duration: Duration) -> Self {
        let shared_state = Arc::new(Mutex::new(SharedState {
            completed: false,
            waker: None,
        }));

        let thread_shared_state = Arc::clone(&shared_state);

        // PRODUCTION NOTE: Spawning a thread per timer is highly inefficient and just
        // for demonstration. A production reactor (like Tokio's) uses a single background thread
        // managing an epoll/kqueue instance or a timer wheel.
        thread::spawn(move || {
            thread::sleep(duration);
            let mut shared_state = thread_shared_state.lock().expect("lock poisoned");

            // Mark as completed
            shared_state.completed = true;

            // Wake up the task if a waker was registered
            if let Some(waker) = shared_state.waker.take() {
                // GOTCHA: We must wake the task *after* setting completed to true,
                // otherwise the executor might poll the future before it's marked
                // as completed, causing it to go to sleep forever!
                waker.wake();
            }
        });

        Self { shared_state }
    }
}

/// Task execution engine that polls futures until completion.
pub struct Executor {
    ready_queue: Receiver<Arc<Task>>,
}

impl Executor {
    /// Runs the executor, polling tasks from the ready queue.
    /// This method will block until all task senders (Spawners and Tasks) are dropped.
    ///
    /// # Panics
    /// Panics if a task's internal mutex is poisoned.
    pub fn run(&self) {
        // We continuously receive tasks that are ready to be polled.
        while let Ok(task) = self.ready_queue.recv() {
            let mut future_slot = task.future.lock().expect("lock poisoned");

            // Take the future. If it is Some, it means it hasn't completed yet.
            if let Some(mut future) = future_slot.take() {
                // RUST INSIGHT: We construct a Waker from an Arc<Task>.
                // `Task` implements the `Wake` trait, which allows the standard library
                // to construct a Waker seamlessly.
                let waker = Waker::from(Arc::clone(&task));
                let mut context = Context::from_waker(&waker);

                // RUST INSIGHT: We use `as_mut` to get a Pin<&mut Box<dyn Future...>>
                // without moving or consuming the actual boxed future.
                if future.as_mut().poll(&mut context).is_pending() {
                    // Future isn't done, put it back in the slot so it can be
                    // polled again when awoken.
                    *future_slot = Some(future);
                }
            }
        }
    }
}

/// Handle to spawn new asynchronous tasks.
#[derive(Clone)]
pub struct Spawner {
    task_sender: SyncSender<Arc<Task>>,
}

impl Spawner {
    /// Spawns a new future onto the executor.
    ///
    /// # Panics
    /// Panics if the executor channel is disconnected or the queue is full.
    pub fn spawn(&self, future: impl Future<Output = ()> + 'static + Send) {
        let future = Box::pin(future);
        let task = Arc::new(Task {
            future: Mutex::new(Some(future)),
            task_sender: self.task_sender.clone(),
        });

        // Push the task to the ready queue so the executor starts polling it.
        self.task_sender.send(task).expect("too many tasks queued");
    }
}

/// A wrapper around a spawned future that can re-schedule itself.
pub struct Task {
    /// The actual future. Wrapped in a Mutex so it can be mutated by the Executor
    /// while being shared via Arc with the reactor.
    #[allow(clippy::type_complexity)]
    future: Mutex<Option<Pin<Box<dyn Future<Output = ()> + Send + 'static>>>>,
    /// Channel to send itself back to the executor when woken.
    task_sender: SyncSender<Arc<Self>>,
}

impl Wake for Task {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        // RUST INSIGHT: When a task is woken, it simply clones its Arc and
        // sends itself back to the executor's channel. The Executor will then
        // pull it off the channel and poll it again.
        self.task_sender
            .send(Arc::clone(self))
            .expect("too many tasks queued");
    }
}

/// Creates a new Executor and a paired Spawner.
#[must_use]
pub fn new_executor_and_spawner() -> (Executor, Spawner) {
    // PRODUCTION NOTE: A real runtime uses an unbounded queue or a lock-free work-stealing queue.
    // A bounded channel can deadlock if tasks spawn other tasks faster than they complete.
    const MAX_QUEUED_TASKS: usize = 10_000;
    let (task_sender, ready_queue) = sync_channel(MAX_QUEUED_TASKS);
    (Executor { ready_queue }, Spawner { task_sender })
}

// ==========================================
// FOOTER
// ==========================================
// How this compares to the canonical crate (Tokio, async-std):
// - Tokio uses a work-stealing scheduler with multiple threads, while this is single-threaded.
// - Tokio has an integrated I/O reactor (mio) and timer wheel. We use naive threads for timers.
// - Production runtimes use `std::task::RawWaker` with optimized memory layouts instead of `Arc` and `Mutex`.
//
// What's missing vs. production:
// - Non-blocking I/O support (TCP, UDP, Files).
// - Efficient timer management (Timer Wheel).
// - Work stealing (multi-threading).
// - Yielding (`yield_now`).
// - Local task sets (`spawn_local` for non-Send futures).
//
// Suggested next steps / extensions:
// 1. Build a mini epoll-based reactor and implement an `AsyncTcpStream`.
// 2. Replace the bounded sync_channel with a lock-free queue (like crossbeam).
// 3. Implement a `join!` macro to poll multiple futures concurrently.

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::{Duration, Instant};

    #[test]
    fn test_async_runtime_basic() {
        let (executor, spawner) = new_executor_and_spawner();
        let counter = Arc::new(AtomicUsize::new(0));

        let c = Arc::clone(&counter);
        spawner.spawn(async move {
            c.fetch_add(1, Ordering::SeqCst);
            TimerFuture::new(Duration::from_millis(10)).await;
            c.fetch_add(1, Ordering::SeqCst);
        });

        // Drop spawner so executor shuts down after tasks are complete
        drop(spawner);
        executor.run();

        assert_eq!(counter.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn test_multiple_tasks() {
        let (executor, spawner) = new_executor_and_spawner();
        let counter = Arc::new(AtomicUsize::new(0));

        for _ in 0..5 {
            let c = Arc::clone(&counter);
            spawner.spawn(async move {
                TimerFuture::new(Duration::from_millis(5)).await;
                c.fetch_add(1, Ordering::SeqCst);
            });
        }

        drop(spawner);
        executor.run();

        assert_eq!(counter.load(Ordering::SeqCst), 5);
    }

    #[test]
    fn test_nested_spawns() {
        let (executor, spawner) = new_executor_and_spawner();
        let counter = Arc::new(AtomicUsize::new(0));

        let c = Arc::clone(&counter);
        let s = spawner.clone();
        spawner.spawn(async move {
            TimerFuture::new(Duration::from_millis(5)).await;
            c.fetch_add(1, Ordering::SeqCst);

            let c2 = Arc::clone(&c);
            s.spawn(async move {
                TimerFuture::new(Duration::from_millis(5)).await;
                c2.fetch_add(1, Ordering::SeqCst);
            });
        });

        drop(spawner);
        executor.run();

        assert_eq!(counter.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn test_benchmark_note() {
        // RUST INSIGHT: To benchmark an async runtime, we would typically use Criterion
        // and measure the time it takes to spawn and join a large number of no-op tasks.
        // E.g., spawn 10,000 futures that immediately return `Poll::Ready`, and time the executor loop.
        let (executor, spawner) = new_executor_and_spawner();
        let start = Instant::now();
        for _ in 0..10_000 {
            spawner.spawn(async {});
        }
        drop(spawner);
        executor.run();

        let elapsed = start.elapsed();
        assert!(elapsed < Duration::from_secs(1)); // Should complete well under a second
    }

    #[test]
    fn test_timer_accuracy() {
        let (executor, spawner) = new_executor_and_spawner();
        let start = Instant::now();

        spawner.spawn(async move {
            TimerFuture::new(Duration::from_millis(50)).await;
        });

        drop(spawner);
        executor.run();

        let elapsed = start.elapsed();
        // Allow a small margin of error for thread scheduling
        assert!(elapsed >= Duration::from_millis(45));
    }
}
