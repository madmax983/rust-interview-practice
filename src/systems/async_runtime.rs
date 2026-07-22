//! Async Runtime
//!
//! # What this implements and what it replaces
//! This implements a minimal asynchronous executor, spawner, and timer-based reactor from scratch.
//! It replaces foundational async runtimes like `tokio`, `async-std`, and `smol`.
//!
//! # Real-world systems that use this
//! Asynchronous runtimes are the backbone of high-performance Rust networking crates like `hyper`,
//! web frameworks like `actix-web` and `axum`, and concurrent database drivers. They enable massive
//! concurrency without the overhead of native OS threads.
//!
//! # Why build it yourself?
//! Rust's `async/await` syntax relies on the compiler generating state machines, but it doesn't provide
//! a built-in runtime. Building an executor demystifies the `Future` trait, the `Context` / `Waker` API,
//! and how tasks are actually driven to completion. You learn how "awaiting" translates into non-blocking I/O.
//!
//! # Architecture
//!
//! ```text
//!  +-----------+           +-------------+
//!  |           | send task |             |
//!  |  Spawner  |---------->|   Executor  |
//!  |           |           |             |
//!  +-----------+           +-------------+
//!                               |  ^
//!                     poll task |  | wake (enqueue task)
//!                               v  |
//!                          +-------------+
//!                          |             |
//!                          |    Task     | (wraps Future)
//!                          |             |
//!                          +-------------+
//!                               |  ^
//!                 register waker|  | fire (timer expires)
//!                               v  |
//!                          +-------------+
//!                          |             |
//!                          |   Reactor   |
//!                          |   (Timer)   |
//!                          +-------------+
//! ```
//!
//! ## Core Components
//! 1. **Executor**: A loop that receives tasks from a channel and calls `.poll()` on their internal futures.
//! 2. **Spawner**: A handle to clone and push new tasks onto the executor's channel.
//! 3. **Task**: A wrapper around a boxed future that implements `alloc::task::Wake`, allowing it to re-enqueue itself when woken.
//! 4. **Reactor**: A separate background thread that tracks timers. When a timer expires, it calls `.wake()` on the task's `Waker`.
//!
//! ## Invariants
//! - **Non-blocking Poll**: A future's `poll` method must never block the thread. It must return `Poll::Pending` if not ready.
//! - **Guaranteed Wakeup**: If a future returns `Poll::Pending`, it *must* ensure its `Waker` is called eventually.
//!
//! ## Complexity
//! - **Task Spawning**: `O(1)` - Channel send.
//! - **Task Polling**: `O(1)` - Channel receive + virtual call.
//! - **Timer Registration**: `O(log N)` - Inserting into the reactor's min-heap.
//! - **Timer Firing**: `O(log N)` - Extracting from the reactor's min-heap.
//!
//! ## Benchmarking Note
//! Benchmarking an executor requires comparing task throughput and latency against `tokio` or `async-std`. This can be done using `criterion` with workloads simulating I/O pauses (`sleep`) and heavy CPU-bound tasks inside `spawn`. The primary metrics are task dispatch time and the overhead of the MPSC channel and waker allocations.
//!
//! # Tradeoffs vs. Alternatives
//! - **Single-threaded vs. Multi-threaded**: This is a single-threaded executor. Production runtimes like `tokio` use work-stealing thread pools for CPU parallelism.
//! - **Timer only vs. Full I/O**: We only implement a timer reactor. Real runtimes use `epoll`/`kqueue`/`io_uring` to wait on network sockets and files.

use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::future::Future;
use std::pin::Pin;
use std::sync::mpsc::{Receiver, SyncSender, sync_channel};
use std::sync::{Arc, Mutex, OnceLock};
use std::task::{Context, Poll, Waker};
use std::thread;
use std::time::{Duration, Instant};

/// A type alias for a dynamically dispatched Future that can be sent across threads.
type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

// =========================================================================================
// Executor & Spawner
// =========================================================================================

/// A trait defining how tasks are spawned. This allows swappable executor strategies.
pub trait Spawn {
    /// Spawns a new future onto the executor.
    fn spawn(&self, future: impl Future<Output = ()> + Send + 'static);
}

/// A trait defining how an executor runs tasks.
pub trait Run {
    /// Runs the executor, polling tasks until completion.
    fn run(&self);
}

/// A task wrapper that pairs a future with a channel sender to reschedule itself.
struct Task {
    /// The future being executed. It is protected by a Mutex because tasks are accessed via `Arc` when waking.
    // RUST INSIGHT:
    // We use a `Mutex<Option<...>>` to ensure we can take ownership of the Future when it completes,
    // and because `Task` implements `Wake`, meaning it's shared (`Arc<Task>`) and requires interior mutability
    // to mutate the Future during polling.
    future: Mutex<Option<BoxFuture<'static, ()>>>,

    /// The channel used to send the task back to the executor when woken.
    task_sender: SyncSender<Arc<Task>>,
}

// RUST INSIGHT:
// By implementing `std::task::Wake` for our `Task`, we enable seamless conversion from `Arc<Task>`
// into a `std::task::Waker`. This standard library feature avoids manual unsafe virtual table (vtable) construction.
impl std::task::Wake for Task {
    fn wake(self: Arc<Self>) {
        // When the task is woken (e.g., by the reactor), it sends a clone of its `Arc`
        // back to the executor to be polled again.
        // GOTCHA:
        // We ignore the error here. If the executor's receiver is dropped, we just silently stop rescheduling.
        let _ = self.task_sender.send(self.clone());
    }
}

/// The spawner submits new tasks to the executor.
#[derive(Clone)]
pub struct Spawner {
    task_sender: SyncSender<Arc<Task>>,
}

impl Spawn for Spawner {
    fn spawn(&self, future: impl Future<Output = ()> + Send + 'static) {
        let future = Box::pin(future);
        let task = Arc::new(Task {
            future: Mutex::new(Some(future)),
            task_sender: self.task_sender.clone(),
        });

        // Push the initial task onto the queue.
        self.task_sender
            .send(task)
            .expect("Executor channel closed");
    }
}

/// The executor runs tasks by polling them until completion.
pub struct Executor {
    ready_queue: Receiver<Arc<Task>>,
}

impl Executor {
    /// Creates a new Executor/Spawner pair.
    #[must_use]
    pub fn new() -> (Self, Spawner) {
        // We use a sync_channel with a reasonable bound to prevent unbounded memory growth.
        // PRODUCTION NOTE:
        // A production executor (like `tokio`) often uses lock-free intrusive linked lists
        // or specialized queues (e.g., crossbeam's Chase-Lev deque) for task scheduling.
        let (task_sender, ready_queue) = sync_channel(10_000);
        (Self { ready_queue }, Spawner { task_sender })
    }
}

impl Run for Executor {
    fn run(&self) {
        // RUST INSIGHT:
        // The loop gracefully terminates when all `SyncSender`s are dropped, meaning `recv()` returns an error.
        // The main thread holding the original Spawner dropping it signals termination.
        while let Ok(task) = self.ready_queue.recv() {
            let mut future_slot = task.future.lock().unwrap();
            if let Some(mut future) = future_slot.take() {
                // Create a Waker from the Arc<Task>
                let waker = Waker::from(Arc::clone(&task));
                let mut context = Context::from_waker(&waker);

                // GOTCHA:
                // We must use `Pin::as_mut` because `future` is an owned `Pin<Box<dyn Future>>`.
                // `poll` requires a mutable pinned reference (`Pin<&mut dyn Future>`).
                match future.as_mut().poll(&mut context) {
                    Poll::Pending => {
                        // The future isn't ready. Put it back into the slot.
                        // It will be re-enqueued by whatever component currently holds the `Waker`.
                        *future_slot = Some(future);
                    }
                    Poll::Ready(()) => {
                        // The future completed! It is implicitly dropped here by not putting it back.
                    }
                }
            }
        }
    }
}

// =========================================================================================
// Reactor (Timer)
// =========================================================================================

/// An entry in the timer queue.
struct TimerEntry {
    at: Instant,
    waker: Waker,
}

// We implement Eq, PartialEq, Ord, PartialOrd for TimerEntry to be used in a BinaryHeap.
// We only care about comparing the `at` timestamp.
impl PartialEq for TimerEntry {
    fn eq(&self, other: &Self) -> bool {
        self.at == other.at
    }
}
impl Eq for TimerEntry {}

impl PartialOrd for TimerEntry {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TimerEntry {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.at.cmp(&other.at)
    }
}

/// A global timer reactor that tracks sleeping futures.
struct TimerReactor {
    /// A priority queue of timers. We use `Reverse` because `BinaryHeap` is a max-heap,
    /// and we want the earliest expiration time to be at the top (min-heap).
    timers: Mutex<BinaryHeap<Reverse<TimerEntry>>>,
}

impl TimerReactor {
    fn new() -> Self {
        Self {
            timers: Mutex::new(BinaryHeap::new()),
        }
    }

    /// Registers a waker to be called at the specified `Instant`.
    fn register_timer(&self, at: Instant, waker: Waker) {
        let mut timers = self.timers.lock().unwrap();
        timers.push(Reverse(TimerEntry { at, waker }));
    }

    /// The background thread loop that checks for expired timers.
    fn run_loop(&self) {
        loop {
            let now = Instant::now();
            let mut timers = self.timers.lock().unwrap();

            // Pop all timers that have expired
            while let Some(Reverse(entry)) = timers.peek() {
                if entry.at <= now {
                    // Extract and wake
                    let entry = timers.pop().unwrap().0;
                    entry.waker.wake();
                } else {
                    break;
                }
            }

            // Explicitly drop the lock before sleeping to avoid deadlocking `register_timer` calls.
            drop(timers);

            // PRODUCTION NOTE:
            // A real reactor does not use busy-wait or coarse sleeps (like thread::sleep(1ms)).
            // It blocks efficiently using epoll/kqueue (e.g., mio::Poll::poll) with a timeout
            // equal to the duration until the next timer expires.
            thread::sleep(Duration::from_millis(1));
        }
    }
}

/// Returns a global reference to the timer reactor, initializing it and spawning its thread on the first call.
fn global_reactor() -> &'static TimerReactor {
    static REACTOR: OnceLock<&'static TimerReactor> = OnceLock::new();
    REACTOR.get_or_init(|| {
        let reactor = TimerReactor::new();
        // Leaking the Arc to a static is acceptable here for a global singleton background thread.
        // In a real system, you might have explicit reactor teardown or thread-local reactors.
        let reactor_ref: &'static TimerReactor = Box::leak(Box::new(reactor));

        thread::spawn(move || {
            reactor_ref.run_loop();
        });

        reactor_ref
    })
}

// =========================================================================================
// Asynchronous Primitives
// =========================================================================================

/// A future that completes after a given duration.
pub struct Sleep {
    expires_at: Instant,
    registered: bool,
}

impl Future for Sleep {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if Instant::now() >= self.expires_at {
            Poll::Ready(())
        } else {
            // GOTCHA:
            // We must ensure the timer is registered with the reactor so it can wake this task.
            // A future might be polled multiple times (e.g., via `select!`), so we must only register once,
            // or re-register if the waker changes. For simplicity, we just register once.
            if !self.registered {
                global_reactor().register_timer(self.expires_at, cx.waker().clone());
                self.registered = true;
            }
            Poll::Pending
        }
    }
}

/// Suspends the current task for `duration`.
#[must_use]
pub fn sleep(duration: Duration) -> Sleep {
    Sleep {
        expires_at: Instant::now() + duration,
        registered: false,
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `tokio`: Features a multi-threaded work-stealing scheduler, integrated I/O polling via `mio`, and
//   an immense ecosystem. Our executor is purely single-threaded and lacks native I/O support.
// - `async-std`: Also provides a thread-pool-based executor and attempts to mirror the standard library's API asynchronously.
//
// Missing vs. Production:
// - **I/O Reactor**: We only support time-based sleeping. Real runtimes hook into OS polling APIs.
// - **Work Stealing**: Essential for scaling CPU-bound async workloads across multiple cores.
// - **Efficient Wakers**: Using `Arc<Task>` for wakers requires atomic reference counting per wake.
//   Optimized runtimes often use intrusive lists and raw pointers to bypass standard `Arc` overhead.
// - **Drop Handling**: This simple executor silently drops uncompleted tasks if the channel closes or errors.
//
// Next Steps:
// 1. Integrate `mio` or `epoll` directly to support non-blocking TCP streams.
// 2. Build a multi-threaded executor by spawning multiple worker threads that steal from a shared queue.

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn test_async_execution() {
        let (executor, spawner) = Executor::new();

        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = counter.clone();

        spawner.spawn(async move {
            counter_clone.fetch_add(1, Ordering::SeqCst);
        });

        // Drop the original spawner so the executor terminates when all tasks finish.
        drop(spawner);
        executor.run();

        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_async_sleep() {
        let (executor, spawner) = Executor::new();

        let start = Instant::now();
        spawner.spawn(async move {
            sleep(Duration::from_millis(50)).await;
        });

        drop(spawner);
        executor.run();

        let elapsed = start.elapsed();
        assert!(elapsed >= Duration::from_millis(50));
        assert!(elapsed < Duration::from_millis(500)); // Ensure it didn't take an unreasonably long time
    }

    #[test]
    fn test_concurrent_tasks() {
        let (executor, spawner) = Executor::new();

        let counter = Arc::new(AtomicUsize::new(0));

        for _ in 0..10 {
            let counter_clone = counter.clone();
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
