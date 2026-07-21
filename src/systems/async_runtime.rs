//! # Async Runtime
//!
//! What this implements and what crate(s) it replaces:
//! This implements a minimal asynchronous task executor and spawner. It serves as a
//! foundational replacement for the core mechanics found in crates like `tokio`,
//! `async-std`, and `smol`.
//!
//! Real-world systems that use this:
//! Asynchronous runtimes are the engine behind high-concurrency systems like web servers
//! (Axum, Actix-web), network proxies (Linkerd), and async database drivers. They enable
//! multiplexing thousands of I/O tasks onto a few OS threads.
//!
//! Why build it yourself?
//! Building an executor demystifies the magic of `async`/`await`. It provides a concrete
//! understanding of how `Future::poll` is driven, how `Waker`s notify the executor to
//! reschedule tasks, and why blocking an async task stalls the entire runtime.
//!
//! ## Architecture
//!
//! ```text
//!  [ Spawner ] ----- pushes ----> [ Task Queue (Channel) ] <---- pushes ---- [ Waker ]
//!                                           |
//!                                         pops
//!                                           |
//!                                           v
//!                                    [ Executor ]
//!                                  polls each Task
//! ```
//!
//! **Invariants:**
//! 1. A task is only polled when it is first spawned or after its `Waker` has been invoked.
//! 2. The executor blocks waiting for tasks if the queue is empty but active spawners exist.
//! 3. The executor shuts down when all spawners are dropped and the queue is empty.
//!
//! **Time/Space Complexity:**
//! - Spawn Task: `O(1)` time, `O(1)` space (channel send).
//! - Wake Task: `O(1)` time, `O(1)` space (channel send).
//! - Task Allocation: `O(1)` heap allocation per task (`Box::pin`).
//!
//! **Design Decisions and Tradeoffs vs. Alternatives:**
//! - **Single-threaded vs. Multi-threaded:** This implementation is single-threaded but uses thread-safe primitives (`Arc`, `Mutex`, crossbeam-style channels conceptually via `mpsc`) so `Waker`s can be moved to background threads (like a timer). Production runtimes use complex work-stealing algorithms.
//! - **Dynamic Dispatch:** We use `Pin<Box<dyn Future...>>`. This causes a heap allocation per task and relies on dynamic dispatch, adding slight overhead, but heavily simplifies executor design since all tasks share the same type.
//!
//! // RUST INSIGHT:
//! // Rust's async model is "pull-based". Futures do nothing unless actively polled.
//! // The `Waker` is the mechanism for a future to say "I'm ready to be polled again".

use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex, mpsc};
use std::task::{Context, Poll, Wake, Waker};
use std::thread;
use std::time::Duration;

/// A dynamically dispatched, pinned future that produces no output.
type BoxFuture = Pin<Box<dyn Future<Output = ()> + Send + 'static>>;

/// A task submitted to the executor.
///
/// It wraps a future and a channel to reschedule itself when woken.
// PRODUCTION NOTE:
// Real executors often avoid `Arc<Mutex<BoxFuture>>` by using intrusive linked lists,
// lock-free data structures, or raw pointers (like Tokio's `Task` struct) to eliminate
// allocation overhead and lock contention.
pub struct Task {
    future: Mutex<Option<BoxFuture>>,
    task_sender: mpsc::SyncSender<Arc<Task>>,
}

impl Wake for Task {
    fn wake(self: Arc<Self>) {
        // GOTCHA:
        // When a task is woken, we send a clone of the `Arc<Task>` back into the task channel.
        // If the channel is full, this could block or panic depending on `SyncSender` usage.
        // A production executor usually uses an unbounded queue or a specialized bounded
        // queue that avoids blocking the waking thread.
        let _ = self.task_sender.send(self.clone());
    }
}

/// Spawns new tasks onto the executor.
#[derive(Clone)]
pub struct Spawner {
    task_sender: mpsc::SyncSender<Arc<Task>>,
}

impl Spawner {
    /// Spawns a future onto the executor.
    pub fn spawn(&self, future: impl Future<Output = ()> + Send + 'static) {
        let future = Box::pin(future);
        let task = Arc::new(Task {
            future: Mutex::new(Some(future)),
            task_sender: self.task_sender.clone(),
        });

        // Push the task into the queue for its first poll.
        let _ = self.task_sender.send(task);
    }
}

/// Executes tasks by polling them when they are ready.
pub struct Executor {
    ready_queue: mpsc::Receiver<Arc<Task>>,
}

impl Executor {
    /// Runs the executor, continuously pulling tasks from the queue and polling them.
    /// Exits when all `Spawner`s (and the executor's internal references) are dropped
    /// and the queue is empty.
    pub fn run(&self) {
        while let Ok(task) = self.ready_queue.recv() {
            // Take the future out of the task. We need to lock it briefly.
            let mut future_slot = task.future.lock().unwrap();

            if let Some(mut future) = future_slot.take() {
                // Create a Waker from the task itself.
                let waker = Waker::from(task.clone());
                let mut context = Context::from_waker(&waker);

                // Poll the future.
                match future.as_mut().poll(&mut context) {
                    Poll::Pending => {
                        // If it's pending, put the future back so it can be polled again
                        // when the waker pushes the task back onto the queue.
                        *future_slot = Some(future);
                    }
                    Poll::Ready(()) => {
                        // Task is complete, do nothing. It gets dropped.
                    }
                }
            }
        }
    }
}

/// Creates a new executor and spawner pair.
#[must_use]
pub fn new_executor_and_spawner() -> (Executor, Spawner) {
    // We use a SyncSender to provide backpressure, though 10,000 is generous.
    let (task_sender, ready_queue) = mpsc::sync_channel(10_000);
    (Executor { ready_queue }, Spawner { task_sender })
}

/// A simple future that waits for a specific duration.
/// This demonstrates how a Reactor interacts with the Executor.
pub struct TimerFuture {
    state: Arc<Mutex<TimerState>>,
}

struct TimerState {
    completed: bool,
    waker: Option<Waker>,
}

impl TimerFuture {
    /// Creates a new `TimerFuture` that completes after `duration`.
    #[must_use]
    pub fn new(duration: Duration) -> Self {
        let state = Arc::new(Mutex::new(TimerState {
            completed: false,
            waker: None,
        }));

        let state_clone = Arc::clone(&state);

        // UNSAFE JUSTIFICATION: No unsafe is used here. We simply spawn an OS thread
        // to act as our "reactor", simulating an I/O completion event.
        // In a real runtime (like `tokio`), epoll/kqueue/IOCP is used instead of
        // spawning a thread per timer.
        thread::spawn(move || {
            thread::sleep(duration);
            let mut state = state_clone.lock().unwrap();
            state.completed = true;
            if let Some(waker) = state.waker.take() {
                waker.wake();
            }
        });

        Self { state }
    }
}

impl Future for TimerFuture {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut state = self.state.lock().unwrap();
        if state.completed {
            Poll::Ready(())
        } else {
            // Update the waker in case it has changed between polls.
            state.waker = Some(cx.waker().clone());
            Poll::Pending
        }
    }
}

// =========================================================================================
// Footer
// =========================================================================================
// Comparison to Canonical Crates:
// - `tokio`: Features a multi-threaded, work-stealing scheduler with lock-free queues, an
//   advanced I/O reactor (epoll/kqueue), and integrated timers (hashed timing wheel).
// - `async-std`: Similar high-performance goals as Tokio but attempts to mimic the standard
//   library API surface more closely.
//
// Missing vs. Production:
// - **Multi-threading:** This executor is single-threaded and does not steal work.
// - **I/O Reactor:** We implement a naive timer that spawns a thread per delay. Real runtimes
//   use a centralized event loop (reactor) that monitors OS primitives.
// - **Lock-Free Fast Paths:** We use `Mutex` and standard channels. Real runtimes avoid locks
//   for task submission and execution fast paths.
//
// Suggested Next Steps / Extensions:
// 1. Implement a rudimentary I/O reactor using the `mio` crate for non-blocking sockets.
// 2. Extend the Spawner to support JoinHandles that can return values from completed tasks.
// 3. Build a multi-threaded thread-pool executor.

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Instant;

    #[test]
    fn test_simple_async_execution() {
        let (executor, spawner) = new_executor_and_spawner();

        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = Arc::clone(&counter);

        spawner.spawn(async move {
            counter_clone.fetch_add(1, Ordering::SeqCst);
        });

        // Drop the spawner so the executor will terminate when the queue is empty.
        drop(spawner);
        executor.run();

        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_timer_future() {
        let (executor, spawner) = new_executor_and_spawner();

        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = Arc::clone(&counter);

        let start = Instant::now();

        spawner.spawn(async move {
            TimerFuture::new(Duration::from_millis(50)).await;
            counter_clone.fetch_add(1, Ordering::SeqCst);
        });

        drop(spawner);
        executor.run();

        assert!(start.elapsed() >= Duration::from_millis(50));
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_multiple_spawns_and_interleaving() {
        let (executor, spawner) = new_executor_and_spawner();

        let results = Arc::new(Mutex::new(Vec::new()));

        let r1 = Arc::clone(&results);
        spawner.spawn(async move {
            TimerFuture::new(Duration::from_millis(100)).await;
            r1.lock().unwrap().push("slow");
        });

        let r2 = Arc::clone(&results);
        spawner.spawn(async move {
            TimerFuture::new(Duration::from_millis(10)).await;
            r2.lock().unwrap().push("fast");
        });

        drop(spawner);
        executor.run();

        let final_results = results.lock().unwrap();
        assert_eq!(final_results.len(), 2);
        // "fast" should complete before "slow"
        assert_eq!(final_results[0], "fast");
        assert_eq!(final_results[1], "slow");
    }
}
