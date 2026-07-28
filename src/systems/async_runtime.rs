//! # Async Runtime Implementation
//!
//! Implements a minimal asynchronous executor, spawner, and timer-based reactor from scratch.
//!
//! **Replaces Crates:** `tokio`, `async-std`
//!
//! **Real-world Usage:**
//! - High-concurrency web servers (`axum`, `actix-web`)
//! - Database drivers (`sqlx`)
//! - Network proxies and load balancers
//!
//! **Why build it yourself?**
//! Building an async runtime demystifies Rust's `Future` trait and the `Waker` API.
//! You'll learn how executors schedule tasks, how reactors handle I/O or timers, and
//! how to manage state across asynchronous boundaries without magic.

use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{
    Arc, Mutex, OnceLock,
    mpsc::{Receiver, SyncSender, sync_channel},
};
use std::task::{Context, Poll, Wake, Waker};
use std::thread;
use std::time::{Duration, Instant};

// =========================================================================================
// Architecture
// =========================================================================================
//
// Diagram:
//
//     ┌─────────┐      spawn      ┌──────────┐
//     │ Spawner │ ──────────────> │ Executor │
//     └─────────┘                 └──────────┘
//          ▲                           │
//          │ wake()                 poll()
//          │                           ▼
//     ┌─────────┐      register   ┌──────────┐
//     │ Reactor │ <────────────── │  Future  │
//     └─────────┘                 └──────────┘
//
// Invariants:
// 1. The Executor runs until all tasks are complete (or dropped).
// 2. The Reactor runs in a separate thread, managing timers and waking tasks.
// 3. Wakers safely put tasks back onto the Executor's queue.
//
// Complexity:
// ┌─────────────┬──────────────┬────────┐
// │ Operation   │ Time         │ Space  │
// ├─────────────┼──────────────┼────────┤
// │ spawn       │ O(1)         │ O(1)   │
// │ poll (exec) │ O(1)         │ O(N)   │
// │ add_timer   │ O(log N)     │ O(N)   │
// └─────────────┴──────────────┴────────┘
//
// Design Decisions:
// - **Reactor**: Uses a global `OnceLock` with `Box::leak` for easy access by futures.
// - **Timers**: Uses a `BTreeMap` ordered by `Instant`. Rapid timer creation can result
//   in identical keys. Disambiguated by an atomic counter (tuple `(Instant, usize)`).
// - **Channels**: Uses `sync_channel` with a fixed bound to limit memory and provide backpressure.

/// A trait for generic spawning of futures.
pub trait Spawner {
    /// Spawns a future on the runtime.
    fn spawn(&self, future: impl Future<Output = ()> + 'static + Send);
}

/// A task that can be scheduled on the executor.
struct Task {
    // RUST INSIGHT: We use `Mutex` to safely mutate the inner future.
    // The future must be `Pin<Box<dyn Future>>` because it might be self-referential
    // after polling starts, and `Box` moves it to the heap.
    future: Mutex<Option<Pin<Box<dyn Future<Output = ()> + Send + 'static>>>>,
    task_sender: SyncSender<Arc<Self>>,
}

// RUST INSIGHT: The `Wake` trait requires `Arc<Self>`. This is because a waker
// might outlive the task itself if it's stored in a reactor or epoll queue.
impl Wake for Task {
    fn wake(self: Arc<Self>) {
        // Send the task back to the executor to be polled again.
        // GOTCHA: If the channel is full, this will block the waking thread (e.g. reactor thread).
        // A production executor might use an unbounded queue or drop tasks under extreme load.
        let _ = self.task_sender.send(self.clone());
    }
}

/// The Executor that runs spawned tasks.
pub struct Executor {
    ready_queue: Receiver<Arc<Task>>,
}

impl Executor {
    /// Runs the executor until the spawner is dropped and the queue is empty.
    pub fn run(&self) {
        while let Ok(task) = self.ready_queue.recv() {
            let mut future_slot = task.future.lock().unwrap();
            if let Some(mut future) = future_slot.take() {
                // RUST INSIGHT: `Waker::from_wake` requires `Task` to implement `Wake`
                // and be wrapped in an `Arc`.
                let waker = Waker::from(task.clone());
                let mut context = Context::from_waker(&waker);

                match future.as_mut().poll(&mut context) {
                    Poll::Pending => {
                        // The future is not ready. Put it back in the slot.
                        // It will be awakened later.
                        *future_slot = Some(future);
                    }
                    Poll::Ready(()) => {
                        // Task is complete. Let it drop.
                    }
                }
            }
            drop(future_slot);
        }
    }
}

/// The Spawner that schedules new tasks.
#[derive(Clone)]
pub struct TaskSpawner {
    task_sender: SyncSender<Arc<Task>>,
}

impl TaskSpawner {
    /// Spawns a new future onto the executor.
    pub fn spawn_task(&self, future: impl Future<Output = ()> + 'static + Send) {
        let future = Box::pin(future);
        let task = Arc::new(Task {
            future: Mutex::new(Some(future)),
            task_sender: self.task_sender.clone(),
        });
        self.task_sender.send(task).expect("too many tasks queued");
    }
}

impl Spawner for TaskSpawner {
    fn spawn(&self, future: impl Future<Output = ()> + 'static + Send) {
        self.spawn_task(future);
    }
}

/// Creates a new Executor and Spawner pair.
#[must_use]
pub fn new_executor_and_spawner() -> (Executor, TaskSpawner) {
    const MAX_QUEUED_TASKS: usize = 10_000;
    let (task_sender, ready_queue) = sync_channel(MAX_QUEUED_TASKS);
    (Executor { ready_queue }, TaskSpawner { task_sender })
}

/// The Reactor that manages timers.
pub struct TimerReactor {
    // RUST INSIGHT: Rapid timer creation can result in identical keys.
    // Disambiguate them by using a tuple like `(Instant, usize)` where `usize` is an atomic counter
    // to prevent silent overwriting of timers.
    timers: Mutex<BTreeMap<(Instant, usize), Waker>>,
    counter: AtomicUsize,
}

impl TimerReactor {
    /// Creates a new timer reactor.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            timers: Mutex::new(BTreeMap::new()),
            counter: AtomicUsize::new(0),
        }
    }

    /// Registers a new timer.
    fn add_timer(&self, when: Instant, waker: Waker) {
        let id = self.counter.fetch_add(1, Ordering::SeqCst);
        let mut timers = self.timers.lock().unwrap();
        timers.insert((when, id), waker);
        drop(timers);
    }

    /// The background loop for the reactor.
    fn run(&self) {
        loop {
            let now = Instant::now();
            let expired;

            {
                let mut timers_guard = self.timers.lock().unwrap();
                let unexpired = timers_guard.split_off(&(now, usize::MAX));
                expired = std::mem::replace(&mut *timers_guard, unexpired);
                drop(timers_guard);
            }

            for (_, waker) in expired {
                waker.wake();
            }

            thread::sleep(Duration::from_millis(10));
        }
    }
}

impl Default for TimerReactor {
    fn default() -> Self {
        Self::new()
    }
}

// Global reactor instance
static REACTOR: OnceLock<&'static TimerReactor> = OnceLock::new();

/// Initializes the global reactor thread.
pub fn init_reactor() {
    REACTOR.get_or_init(|| {
        let reactor = TimerReactor::new();
        // RUST INSIGHT: When creating a global singleton using OnceLock and Box::leak
        // to store a static reference, ensure the OnceLock type parameter correctly specifies
        // &'static TimerReactor rather than just TimerReactor.
        let reactor_ref: &'static TimerReactor = Box::leak(Box::new(reactor));

        thread::spawn(move || {
            reactor_ref.run();
        });

        reactor_ref
    });
}

/// An educational Future that resolves after a given duration.
pub struct Sleep {
    when: Instant,
    waker_registered: bool,
}

impl Sleep {
    /// Creates a new sleeping future.
    #[must_use]
    pub fn new(duration: Duration) -> Self {
        Self {
            when: Instant::now() + duration,
            waker_registered: false,
        }
    }
}

impl Default for Sleep {
    fn default() -> Self {
        Self::new(Duration::from_millis(0))
    }
}

impl Future for Sleep {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if Instant::now() >= self.when {
            Poll::Ready(())
        } else {
            if !self.waker_registered {
                // RUST INSIGHT: To resolve clippy::explicit_auto_deref when retrieving a reference
                // from a collection or wrapper (like OnceLock<&'static T>::get()), rely on Rust's
                // auto-dereferencing instead of explicitly dereferencing the returned value.
                let reactor = REACTOR
                    .get()
                    .expect("Reactor must be initialized before using timers");
                reactor.add_timer(self.when, cx.waker().clone());
                self.waker_registered = true;

                // PRODUCTION NOTE: A production-grade future should store and update its waker
                // (e.g., using `AtomicWaker` and `cx.waker().will_wake(stored_waker)`) in case
                // it is polled by a different executor/task.
            }
            Poll::Pending
        }
    }
}

/// Asynchronous sleep function.
pub async fn sleep(duration: Duration) {
    Sleep::new(duration).await;
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `tokio`: Has a far more complex work-stealing executor and a highly optimized epoll/kqueue reactor.
// - `async-std`: Also has work-stealing, though architecturally closer to `smol`.
//
// Missing vs. Production:
// - **Work Stealing**: This executor is single-threaded. Production executors use thread pools and work stealing.
// - **Epoll/Kqueue**: Our reactor only handles timers via sleep. Production reactors handle network I/O via OS primitives.
// - **Waker Storage**: Our `Sleep` future doesn't update its waker if polled by a different task (violating the `Future` contract strictly speaking).
//
// Next Steps:
// 1. Add multithreaded worker threads.
// 2. Implement `Mio` integration for network I/O.
// 3. Add an `AtomicWaker` to `Sleep`.
//
// Benchmarking Note:
// To benchmark the executor overhead, one could measure the time taken to spawn and
// await a large number of immediately ready futures (e.g., `async {}`) using `Instant::now()`
// before and after the executor's `run` method, ensuring the number of tasks is sufficiently
// high (e.g., 100,000) to measure the channel dispatch and polling overhead.

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};

    #[test]
    fn test_async_runtime_basic() {
        init_reactor();
        let (executor, spawner) = new_executor_and_spawner();

        let flag = Arc::new(AtomicBool::new(false));
        let flag_clone = Arc::clone(&flag);

        spawner.spawn(async move {
            sleep(Duration::from_millis(50)).await;
            flag_clone.store(true, Ordering::SeqCst);
        });

        // Drop the spawner so the executor knows no more tasks will be added
        drop(spawner);

        executor.run();

        assert!(flag.load(Ordering::SeqCst));
    }

    #[test]
    fn test_multiple_spawns() {
        init_reactor();
        let (executor, spawner) = new_executor_and_spawner();

        let counter = Arc::new(AtomicUsize::new(0));

        for _ in 0..5 {
            let c = Arc::clone(&counter);
            spawner.spawn(async move {
                sleep(Duration::from_millis(10)).await;
                c.fetch_add(1, Ordering::SeqCst);
            });
        }

        drop(spawner);
        executor.run();

        assert_eq!(counter.load(Ordering::SeqCst), 5);
    }
}
