//! # Async Runtime Implementation
//!
//! Implements a minimal asynchronous executor, spawner, and timer-based reactor from scratch.
//!
//! **Replaces Crates:** `tokio`, `async-std`, `smol`
//!
//! **Real-world Usage:**
//! - Core event loops in high-performance web servers (Axum, Actix, Nginx-like architectures).
//! - Managing thousands of concurrent I/O connections without OS thread overhead.
//! - Background task scheduling systems and multiplexers.
//!
//! **Why build it yourself?**
//! Async Rust can seem like magic. Building a runtime forces you to understand `Future`, `Waker`, and `Context`.
//! You learn that an executor is just a loop pulling tasks from a queue and calling `poll()`, and a reactor
//! is an event loop that wakes up tasks when their I/O or timers complete. You confront the difference
//! between "leaf futures" (like our timer) that interact with the reactor, and "combinators" that just chain state.

// Allow drop on MutexGuard directly if we want, or tighten it.
#![allow(clippy::significant_drop_tightening)]

use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, SyncSender, sync_channel};
use std::sync::{Arc, Mutex, OnceLock};
use std::task::{Context, Poll, Wake, Waker};
use std::thread;
use std::time::{Duration, Instant};

// =========================================================================================
// Architecture
// =========================================================================================
//
// Data Structure:
//
//      Executor (Thread)
//       │
//       ▼
//  ┌───────────┐       ┌─────────────────┐       ┌────────────┐
//  │ TaskQueue │ ◄───► │ Spawner (Clone) │ ───►  │ Reactor    │
//  └───────────┘       └─────────────────┘       └────────────┘
//       ▲                      │                       │
//       │ Wakes                │ Spawns                │ Sleeps/Wakes
//       └──────────────────────┴───────────────────────┘
//
// Invariants:
// 1. `Executor` blocks on a channel waiting for runnable tasks.
// 2. `Spawner` pushes tasks into the channel.
// 3. `Waker` implementations must push the task back to the executor's channel when woken.
// 4. `Reactor` maintains a priority queue (BTreeMap) of timers.
// 5. Timers with identical timestamps are disambiguated by a monotonic `usize` ID to prevent overwriting.
//
// Complexity:
// ┌───────────────┬─────────────┬─────────────┐
// │ Operation     │ Time        │ Space       │
// ├───────────────┼─────────────┼─────────────┤
// │ Spawn Task    │ O(1)        │ O(TaskSize) │
// │ Wake Task     │ O(1)        │ O(1)        │
// │ Register Timer│ O(log N)    │ O(1)        │
// │ Poll Reactor  │ O(1) amort. │ O(1)        │
// └───────────────┴─────────────┴─────────────┘
//
// Design Decisions:
// - **Task Queue**: We use `std::sync::mpsc::sync_channel` as a simple bounded channel for our task queue.
// - **Reactor**: A dedicated background thread sleeping until the next timer expires, or awakened by a condvar/channel if a sooner timer is added. For simplicity, our basic reactor wakes periodically or blocks.
// - **Timers**: `BTreeMap<(Instant, usize), Waker>`.
//   - *Alternative*: Binary heap. BTreeMap is easier to manage removal and iteration.
// - **Global Reactor**: We use `OnceLock` and `Box::leak` to create a global, static reference to the reactor. This mimics how Tokio's driver implicitly works for user code without explicitly passing the reactor everywhere.

// -----------------------------------------------------------------------------------------
// Global Reactor Initialization
// -----------------------------------------------------------------------------------------

static REACTOR: OnceLock<&'static TimerReactor> = OnceLock::new();

/// Initializes the global timer reactor background thread.
fn init_reactor() {
    REACTOR.get_or_init(|| {
        let reactor = TimerReactor::new();
        // RUST INSIGHT: We use `Box::leak` to create a `'static` reference from a heap allocation.
        // This is safe because a global reactor is intended to live for the duration of the program.
        let ref_reactor: &'static TimerReactor = Box::leak(Box::new(reactor));

        let reactor_clone = ref_reactor;

        thread::spawn(move || {
            reactor_clone.run();
        });

        ref_reactor
    });
}

/// Helper to get the global reactor instance.
/// # Panics
/// Panics if the reactor fails to initialize or is not set up correctly.
fn get_reactor() -> &'static TimerReactor {
    REACTOR.get().expect("Reactor must be initialized")
}

// -----------------------------------------------------------------------------------------
// The Reactor
// -----------------------------------------------------------------------------------------

/// The Reactor manages timers and wakes tasks when their time has come.
struct TimerReactor {
    /// Disambiguates identical instants.
    next_id: AtomicUsize,
    /// The priority queue of wakeups.
    timers: Mutex<BTreeMap<(Instant, usize), Waker>>,
}

impl TimerReactor {
    fn new() -> Self {
        Self {
            next_id: AtomicUsize::new(0),
            timers: Mutex::new(BTreeMap::new()),
        }
    }

    /// Register a new waker to be called at `when`.
    fn register_timer(&self, when: Instant, waker: Waker) -> usize {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let mut timers = self.timers.lock().unwrap();
        timers.insert((when, id), waker);
        // Explicitly drop guard here if doing tight scope, though it's end of block anyway.
        drop(timers);
        id
    }

    /// Background thread loop that wakes expired timers.
    fn run(&self) {
        loop {
            let now = Instant::now();
            let mut wakers_to_wake = Vec::new();

            {
                let mut timers = self.timers.lock().unwrap();

                // RUST INSIGHT: BTreeMap iterates in sorted order by key.
                // We split off all timers that are strictly greater than `now`.
                // What remains in `timers` are those <= `now`. Wait, `split_off` returns keys >= `given`.
                // So we want to split off `(now, usize::MAX)`, keeping everything before it, and then swap.

                let future_timers = timers.split_off(&(now, usize::MAX));

                // `timers` now contains expired items. `future_timers` contains non-expired.
                // We swap them so `self.timers` has the future ones.
                let expired_timers = std::mem::replace(&mut *timers, future_timers);

                for (_, waker) in expired_timers {
                    wakers_to_wake.push(waker);
                }

                drop(timers);
            }

            for waker in wakers_to_wake {
                waker.wake();
            }

            // Sleep for a short duration to prevent busy waiting.
            // In a production reactor, this would wait on a Condvar, Epoll, or similar primitive.
            thread::sleep(Duration::from_millis(10));
        }
    }
}

// -----------------------------------------------------------------------------------------
// Leaf Future: Timer
// -----------------------------------------------------------------------------------------

/// A leaf future that resolves after a specified duration.
pub struct Sleep {
    when: Instant,
    id: Option<usize>,
}

impl Sleep {
    /// Creates a new `Sleep` future.
    #[must_use]
    pub fn new(duration: Duration) -> Self {
        init_reactor();
        Self {
            when: Instant::now() + duration,
            id: None,
        }
    }
}

impl Future for Sleep {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if Instant::now() >= self.when {
            Poll::Ready(())
        } else {
            // Register or update waker.
            // PRODUCTION NOTE: A production-grade future should store and update its waker
            // (e.g., using `AtomicWaker` and `cx.waker().will_wake(stored_waker)`) in case it is polled
            // by a different executor/task.

            let reactor = get_reactor();

            // We register every time it's polled and not ready, which works for this simple design,
            // but normally you only register once and update the waker if it changes.
            if self.id.is_none() {
                let waker = cx.waker().clone();
                self.id = Some(reactor.register_timer(self.when, waker));
            }

            Poll::Pending
        }
    }
}

/// Convenience function to create a sleep future.
#[must_use]
pub fn sleep(duration: Duration) -> Sleep {
    Sleep::new(duration)
}

// -----------------------------------------------------------------------------------------
// Task and Executor
// -----------------------------------------------------------------------------------------

/// Represents a single async task to be executed.
struct Task {
    /// The actual future being polled. Wrapped in a Mutex because `wake` only gives us a shared reference (`Arc<Task>`),
    /// but `poll` requires `Pin<&mut Future>`.
    future: Mutex<Option<BoxFuture<'static, ()>>>,
    /// Channel to send the task back to the executor when woken.
    task_sender: SyncSender<Arc<Task>>,
}

type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

impl Wake for Task {
    fn wake(self: Arc<Self>) {
        // RUST INSIGHT: `Wake` trait is implemented on `Arc<Task>`. When a waker wakes,
        // it clones the `Arc` and pushes it back into the executor's channel.
        let cloned = Arc::clone(&self);

        // If the channel is full or disconnected, we just drop the task.
        let _ = self.task_sender.try_send(cloned);
    }
}

/// Spawner allows spawning new tasks into the executor.
#[derive(Clone)]
pub struct Spawner {
    task_sender: SyncSender<Arc<Task>>,
}

impl Spawner {
    /// Spawns a new future onto the executor.
    pub fn spawn<F>(&self, future: F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        let future = Box::pin(future);
        let task = Arc::new(Task {
            future: Mutex::new(Some(future)),
            task_sender: self.task_sender.clone(),
        });

        let _ = self.task_sender.send(task);
    }
}

/// The Executor runs spawned tasks.
pub struct Executor {
    ready_queue: Receiver<Arc<Task>>,
}

impl Executor {
    /// Creates a new Executor and Spawner pair.
    #[must_use]
    pub fn new() -> (Self, Spawner) {
        init_reactor();

        // 10,000 tasks max in queue.
        let (task_sender, ready_queue) = sync_channel(10000);

        (Self { ready_queue }, Spawner { task_sender })
    }

    /// Runs the executor, pulling tasks from the queue and polling them.
    /// This method blocks until the queue is disconnected (all spawners dropped).
    pub fn run(&self) {
        // Loop over tasks received on the channel.
        while let Ok(task) = self.ready_queue.recv() {
            let mut future_slot = task.future.lock().unwrap();

            // Take the future out to poll it.
            if let Some(mut future) = future_slot.take() {
                // Create a Waker from the Arc<Task> itself.
                let waker = Waker::from(Arc::clone(&task));
                let mut context = Context::from_waker(&waker);

                // Poll the future.
                if future.as_mut().poll(&mut context) == Poll::Pending {
                    // Not done. Put it back so it can be polled again when woken.
                    *future_slot = Some(future);
                }
                // If it returned Poll::Ready, we do nothing. The future is dropped.
            }

            // Explicitly drop the guard here.
            drop(future_slot);
        }
    }
}

impl Default for Executor {
    fn default() -> Self {
        unimplemented!("Use Executor::new() which returns a pair of (Executor, Spawner)")
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `tokio`: Has a complex multi-threaded work-stealing scheduler. Uses OS-level event notification (epoll, kqueue) for I/O and precise timer wheels.
// - `async-std`: Also multi-threaded work-stealing, aims to mirror `std` but asynchronously.
//
// What's missing vs. production:
// - **Work Stealing**: Our executor is single-threaded. Real runtimes use M:N scheduling (M tasks on N OS threads).
// - **Efficient Reactor**: Our reactor uses a `thread::sleep` loop. A real reactor integrates with `epoll`/`kqueue`/`mio`.
// - **I/O Integration**: We only implemented timers. Real runtimes handle asynchronous File/Network I/O.
// - **Cancellation/Drop**: We don't gracefully handle canceling tasks or shutting down the reactor thread.
//
// Next Steps / Extensions:
// 1. Integrate `mio` into the reactor to wake tasks on TCP socket readability.
// 2. Make `Executor` multi-threaded by having multiple worker threads pull from a concurrent queue (like `crossbeam::channel`).

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::Instant;

    #[test]
    fn test_executor_spawner_timer() {
        let (executor, spawner) = Executor::new();

        let flag = Arc::new(AtomicBool::new(false));
        let flag_clone = Arc::clone(&flag);

        // Spawn a task that sleeps then sets a flag
        spawner.spawn(async move {
            sleep(Duration::from_millis(50)).await;
            flag_clone.store(true, Ordering::Relaxed);
        });

        // Drop spawner so executor run loop will terminate when queue is empty
        // Wait, the executor blocks on `recv()`. If we drop `spawner`, the channel
        // still has a clone inside the Task! But wait, when the task finishes, it drops
        // its internal clone of the sender. Once the last task finishes, the channel disconnects.
        drop(spawner);

        let start = Instant::now();
        executor.run();
        let elapsed = start.elapsed();

        assert!(flag.load(Ordering::Relaxed));
        assert!(elapsed >= Duration::from_millis(50));
    }

    #[test]
    fn test_multiple_timers() {
        let (executor, spawner) = Executor::new();

        let counter = Arc::new(AtomicUsize::new(0));

        for _ in 0..3 {
            let c = Arc::clone(&counter);
            spawner.spawn(async move {
                sleep(Duration::from_millis(20)).await;
                c.fetch_add(1, Ordering::Relaxed);
            });
        }

        drop(spawner);
        executor.run();

        assert_eq!(counter.load(Ordering::Relaxed), 3);
    }

    #[test]
    fn test_identical_timestamp_disambiguation() {
        let (executor, spawner) = Executor::new();
        let counter = Arc::new(AtomicUsize::new(0));

        // We can't guarantee identical Instant::now() due to CPU speed,
        // but the BTreeMap allows it via the `usize` ID anyway.
        for _ in 0..5 {
            let c = Arc::clone(&counter);
            spawner.spawn(async move {
                sleep(Duration::from_millis(10)).await;
                c.fetch_add(1, Ordering::Relaxed);
            });
        }

        drop(spawner);
        executor.run();

        assert_eq!(counter.load(Ordering::Relaxed), 5);
    }
}
