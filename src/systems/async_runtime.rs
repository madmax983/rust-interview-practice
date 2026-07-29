//! # Async Runtime Implementation
//!
//! Implements a minimal asynchronous runtime from scratch, including an executor,
//! a task spawner, and a timer-based reactor.
//!
//! **Replaces Crates:** `tokio`, `async-std`, `smol`
//!
//! **Real-world Usage:**
//! - High-concurrency network servers
//! - I/O bound microservices
//! - Client libraries for databases and web APIs
//!
//! **Why build it yourself?**
//! Rust's async/await syntax is just syntactic sugar for state machines that yield `Future`s.
//! Unlike Go or Erlang, Rust does not provide a built-in runtime. Building one teaches you
//! the critical components: the **Executor** (which polls futures), the **Reactor**
//! (which wakes futures when I/O or timers complete), and the **Waker** mechanism that bridges them.
//!
//! # Architecture
//!
//! ```text
//!    async/await (Tasks)
//!         │
//!         ▼
//! ┌──────────────────────┐
//! │       Executor       │◄───────┐ (Waker pushes Task back to Queue)
//! │  (Polls Futures)     │        │
//! └──────────────────────┘        │
//!         │                       │
//!   (Returns Pending)             │
//!         ▼                       │
//! ┌──────────────────────┐        │
//! │       Reactor        │        │
//! │ (Waits for Events)   │────────┘
//! └──────────────────────┘
//! ```
//!
//! **Invariants:**
//! 1. A `Future` must not be polled again after it returns `Poll::Ready`.
//! 2. The `Reactor` must call the `Waker` when the underlying event completes.
//! 3. The `Executor` must run tasks until the queue is empty (or sleep if waiting).
//!
//! **Time/Space Complexity:**
//! - **Reactor Tick:** Time: O(K log N) where K is number of expired timers, N is total timers. Space: O(1).
//! - **Task Spawning:** Time: O(1) amortized. Space: O(1) per task in queue.
//!
//! **Benchmarking Notes:**
//! Benchmarking an async runtime involves measuring task spawn overhead and reactor wake latency.
//! A simple microbenchmark would spawn 10,000 tasks that immediately resolve and measure total time.
//!
//! **Design Decisions and Tradeoffs:**
//! - **Reactor:** This implementation uses a simple `BTreeMap` for a timer-based reactor,
//!   running on a background thread. A production runtime uses `epoll`/`kqueue`/`io_uring`
//!   for network I/O.
//! - **Executor:** Uses a simple multi-producer, single-consumer (`mpsc`) channel as the task queue.
//!   A production runtime (like Tokio) uses a work-stealing scheduler across multiple threads.

use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::{
    Arc, Mutex, OnceLock,
    atomic::{AtomicUsize, Ordering},
    mpsc::{Receiver, SyncSender, sync_channel},
};
use std::task::{Context, Poll, Waker};
use std::thread;
use std::time::{Duration, Instant};

// =========================================================================================
// Reactor Implementation
// =========================================================================================

/// Global Reactor Instance
/// RUST INSIGHT: We use `OnceLock` to safely initialize a global static reference.
/// We leak a `Box` to get a `&'static TimerReactor` because the reactor lives for the lifetime
/// of the program, and this avoids the overhead of `Arc` for global access.
static REACTOR: OnceLock<&'static TimerReactor> = OnceLock::new();

/// The Reactor tracks timers and wakes up associated tasks when they expire.
pub struct TimerReactor {
    // We use a BTreeMap ordered by Instant to efficiently find expired timers.
    // We include a usize ID to disambiguate timers that expire at the exact same Instant.
    timers: Mutex<BTreeMap<(Instant, usize), Waker>>,
    next_id: AtomicUsize,
}

impl TimerReactor {
    const fn new() -> Self {
        Self {
            timers: Mutex::new(BTreeMap::new()),
            next_id: AtomicUsize::new(0),
        }
    }

    /// Registers a new timer and returns a unique ID (if needed by advanced logic).
    fn register_timer(&self, expiration: Instant, waker: Waker) {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let mut timers = self.timers.lock().unwrap();
        timers.insert((expiration, id), waker);
        // clippy::significant_drop_tightening
        drop(timers);
    }

    /// Starts the background thread that checks for expired timers.
    fn start(&'static self) {
        thread::spawn(move || {
            loop {
                let now = Instant::now();
                let mut timers = self.timers.lock().unwrap();

                // Extract all timers that have expired.
                // BTreeMap::split_off returns everything >= the key, so we need to
                // keep that and take the remaining (which are < the key).
                // We use a slightly hacky approach for simplicity: collect expired keys.
                let mut expired = Vec::new();
                for key in timers.keys() {
                    if key.0 <= now {
                        expired.push(*key);
                    } else {
                        break; // BTreeMap is ordered, so we can stop early
                    }
                }

                for key in expired {
                    if let Some(waker) = timers.remove(&key) {
                        waker.wake();
                    }
                }
                drop(timers);

                thread::sleep(Duration::from_millis(10));
            }
        });
    }

    /// Gets the global reactor, initializing it if necessary.
    fn get() -> &'static Self {
        REACTOR.get_or_init(|| {
            let reactor = Box::new(Self::new());
            // Leak the box to get a static reference
            let ref_reactor: &'static Self = Box::leak(reactor);
            ref_reactor.start();
            ref_reactor
        })
    }
}

// =========================================================================================
// Future Implementations
// =========================================================================================

/// A simple Future that resolves after a specified duration.
pub struct Delay {
    expiration: Instant,
    registered: bool,
}

impl Delay {
    #[must_use]
    pub fn new(duration: Duration) -> Self {
        Self {
            expiration: Instant::now() + duration,
            registered: false,
        }
    }
}

impl Future for Delay {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if Instant::now() >= self.expiration {
            Poll::Ready(())
        } else {
            if !self.registered {
                // Register with the reactor
                // PRODUCTION NOTE: A production-grade future should store and update its waker
                // (e.g., using `AtomicWaker` and `cx.waker().will_wake(stored_waker)`)
                // in case it is polled by a different executor/task before completing.
                let waker = cx.waker().clone();
                TimerReactor::get().register_timer(self.expiration, waker);
                self.registered = true;
            }
            Poll::Pending
        }
    }
}

// =========================================================================================
// Executor & Task Implementation
// =========================================================================================

/// A task is a Future that is ready to be polled.
struct Task {
    // We use a Boxed Future. Pinning is required because Futures can be self-referential
    // across await points.
    future: Mutex<Option<Pin<Box<dyn Future<Output = ()> + Send + 'static>>>>,
    task_sender: SyncSender<Arc<Self>>,
}

impl ArcWake for Task {
    fn wake_by_ref(arc_self: &Arc<Self>) {
        // When the waker is called, we push the task back into the channel
        // so the executor knows it's ready to be polled again.
        let cloned = Arc::clone(arc_self);
        // We ignore the error; if the channel is full or closed, the executor is shutting down.
        let _ = arc_self.task_sender.send(cloned);
    }
}

/// A custom trait similar to `alloc::task::Wake` to construct a Waker from an Arc.
trait ArcWake {
    fn wake_by_ref(arc_self: &Arc<Self>);
}

/// Helper function to create a standard library `Waker` from our `ArcWake` type.
/// UNSAFE JUSTIFICATION:
/// We are constructing a `RawWaker` using a vtable that defines how to clone, wake,
/// and drop the underlying `Arc<Task>`.
/// - `clone`: increments the Arc refcount.
/// - `wake`: consumes the Arc and calls `wake_by_ref`.
/// - `wake_by_ref`: borrows the Arc and calls `wake_by_ref`.
/// - `drop`: decrements the Arc refcount.
fn waker_into_waker(waker: Arc<Task>) -> Waker {
    use std::task::{RawWaker, RawWakerVTable};

    static VTABLE: RawWakerVTable = RawWakerVTable::new(
        |data| unsafe {
            let arc = Arc::from_raw(data.cast::<Task>());
            let clone = Arc::clone(&arc);
            // Forget the original to prevent dropping the refcount we just borrowed
            let _ = Arc::into_raw(arc);
            RawWaker::new(Arc::into_raw(clone).cast::<()>(), &VTABLE)
        },
        |data| unsafe {
            let arc = Arc::from_raw(data.cast::<Task>());
            Task::wake_by_ref(&arc);
        },
        |data| unsafe {
            let arc = Arc::from_raw(data.cast::<Task>());
            // We just borrowed it, so we don't want to consume the Arc here, just borrow
            Task::wake_by_ref(&arc);
            // Forget it so we don't drop the refcount
            let _ = Arc::into_raw(arc);
        },
        |data| unsafe {
            drop(Arc::from_raw(data.cast::<Task>()));
        },
    );

    let ptr = Arc::into_raw(waker).cast::<()>();
    let raw_waker = RawWaker::new(ptr, &VTABLE);
    unsafe { Waker::from_raw(raw_waker) }
}

/// Spawner allows spawning tasks onto the executor.
#[derive(Clone)]
pub struct Spawner {
    task_sender: SyncSender<Arc<Task>>,
}

impl Spawner {
    /// Spawns a future onto the executor.
    /// Spawns a future onto the executor.
    ///
    /// # Panics
    /// Panics if the internal queue is full or closed.
    pub fn spawn(&self, future: impl Future<Output = ()> + Send + 'static) {
        let future = Box::pin(future);
        let task = Arc::new(Task {
            future: Mutex::new(Some(future)),
            task_sender: self.task_sender.clone(),
        });
        self.task_sender.send(task).expect("too many tasks queued");
    }
}

/// Executor polls tasks when they are ready.
pub struct Executor {
    ready_queue: Receiver<Arc<Task>>,
}

impl Executor {
    /// Runs the executor, polling tasks until the queue is empty.
    /// In a real runtime, this would block indefinitely waiting for new tasks.
    /// Runs the executor, polling tasks until the queue is empty.
    /// In a real runtime, this would block indefinitely waiting for new tasks.
    ///
    /// # Panics
    /// Panics if the task's future mutex is poisoned.
    pub fn run(&self) {
        while let Ok(task) = self.ready_queue.recv() {
            let mut future_slot = task.future.lock().unwrap();
            if let Some(mut future) = future_slot.take() {
                // Create a Waker from the task itself
                let waker = waker_into_waker(Arc::clone(&task));
                let mut context = Context::from_waker(&waker);

                // Poll the future
                if future.as_mut().poll(&mut context).is_pending() {
                    // Not done, put it back in the slot so it can be polled again
                    *future_slot = Some(future);
                }
            }
        }
    }
}

/// Creates a new Executor and associated Spawner.
#[must_use]
pub fn new_executor_and_spawner() -> (Executor, Spawner) {
    const MAX_QUEUED_TASKS: usize = 10_000;
    let (task_sender, ready_queue) = sync_channel(MAX_QUEUED_TASKS);
    (Executor { ready_queue }, Spawner { task_sender })
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `tokio`: Features a multi-threaded work-stealing scheduler, integrates tightly with OS I/O
//   primitives (epoll/io_uring), handles blocking I/O on separate threads, and includes complex
//   timer wheels for efficient timeouts.
//
// What's missing vs. production:
// - **I/O Reactor:** We only implemented a Timer reactor. Real runtimes need I/O polling.
// - **Work Stealing:** Our executor is single-threaded. Production ones use multiple workers.
// - **Waker Safety:** Production runtimes meticulously manage `AtomicWaker`s to ensure futures
//   can be safely migrated between threads and awoken from interrupt handlers.
//
// Suggested next steps:
// 1. Implement a `TcpStream` wrapper that registers interest with an `epoll` reactor.
// 2. Add an `AtomicWaker` to the `Delay` future.

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};

    #[test]
    fn test_async_runtime_execution() {
        let (executor, spawner) = new_executor_and_spawner();
        let flag = Arc::new(AtomicBool::new(false));
        let flag_clone = Arc::clone(&flag);

        spawner.spawn(async move {
            Delay::new(Duration::from_millis(50)).await;
            flag_clone.store(true, Ordering::SeqCst);
        });

        // Drop spawner so the executor loop knows no more tasks are coming
        drop(spawner);

        executor.run();

        assert!(flag.load(Ordering::SeqCst));
    }

    #[test]
    fn test_async_runtime_multiple_tasks() {
        let (executor, spawner) = new_executor_and_spawner();
        let counter = Arc::new(AtomicUsize::new(0));

        for _ in 0..5 {
            let c = Arc::clone(&counter);
            spawner.spawn(async move {
                Delay::new(Duration::from_millis(10)).await;
                c.fetch_add(1, Ordering::SeqCst);
            });
        }

        drop(spawner);
        executor.run();

        assert_eq!(counter.load(Ordering::SeqCst), 5);
    }
}
