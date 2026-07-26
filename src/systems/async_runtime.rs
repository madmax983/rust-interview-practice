//! Async Runtime
//!
//! What this implements and what crate(s) it replaces:
//! This is a minimal, educational async runtime that implements its own `Executor`, `Spawner`,
//! and a timer-based `Reactor`. It replaces heavy-duty async runtimes like `tokio`, `async-std`,
//! and `smol` to demonstrate how futures are actually polled, how wakers work, and how the
//! reactor pattern handles asynchronous events like timers.
//!
//! Real-world systems that use this:
//! - Web servers (e.g., Hyper, Actix) use async runtimes to handle thousands of concurrent
//!   connections without dedicating an OS thread to each.
//! - Networked microservices use them to parallelize I/O-bound tasks.
//!
//! Why build it yourself?
//! Understanding the async runtime demystifies the magic of `async`/`.await`. You learn exactly
//! what a `Waker` is, how tasks are scheduled on an executor, and how a reactor notifies the
//! executor when I/O or a timer is ready.
//!
//! Architecture:
//! - `Executor`: Runs the `Futures` by pulling tasks from a channel and calling `poll()`.
//! - `Spawner`: Pushes new tasks onto the channel.
//! - `Task`: Wraps a `Future` and provides a `Waker` that pushes the task back onto the channel
//!   when awoken.
//! - `Reactor`: A background thread that sleeps until the next timer expires, then wakes the
//!   associated task.
//!
//! Invariants:
//! - A waker must schedule the exact same task for polling again.
//! - The reactor must handle identical expirations (same `Instant`) by disambiguating them.
//!
//! Time/Space Complexity:
//! - Spawning a task: O(1) time, O(1) space per task on the heap.
//! - Polling: Depends on the Future, overhead is O(1).
//! - Timer Reactor Insertion: O(log N) time (`BTreeMap`).
//! - Timer Reactor Expiration: O(K) where K is the number of expired timers.
//!
//! Footer:
//! - Comparison to Canonical Crates: Crates like `tokio` or `async-std` use sophisticated
//!   `epoll/kqueue/io_uring` backends for I/O readiness, whereas our minimal reactor only
//!   handles timers on a separate sleeping thread. They also use work-stealing multithreaded
//!   executors, whereas ours is a simple single-threaded MPSC queue.
//! - Missing vs Production: We don't handle I/O (sockets, files), we don't have task cancellation
//!   (Drop semantics for unpolled futures), and our channel is an MPSC blocking queue instead of
//!   a lock-free concurrent queue. We also sleep in the reactor instead of blocking on an eventfd.
//! - Next Steps: Implement a simple TCP listener reactor using `epoll` via the `libc` crate,
//!   and upgrade the executor to a multi-threaded work-stealing pool using `crossbeam`.

use std::{
    collections::BTreeMap,
    future::Future,
    pin::Pin,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicUsize, Ordering},
        mpsc::{Receiver, SyncSender, sync_channel},
    },
    task::{Context, Poll, Wake, Waker},
    thread,
    time::{Duration, Instant},
};

// GOTCHA:
// We need a global reactor for our `TimerFuture` to easily register itself without passing a reference
// down through the entire call stack.
static REACTOR: OnceLock<&'static TimerReactor> = OnceLock::new();

/// Initialize the global reactor.
fn get_reactor() -> &'static TimerReactor {
    REACTOR.get_or_init(|| {
        // RUST INSIGHT:
        // We use `Box::leak` to intentionally create a `'static` reference that lives forever.
        // This is safe because the reactor is meant to run for the lifetime of the program.
        let reactor_ref: &'static TimerReactor = Box::leak(Box::new(TimerReactor::new()));

        let reactor_clone = reactor_ref;
        // Start the reactor background thread.
        thread::spawn(move || {
            reactor_clone.run();
        });

        reactor_ref
    })
}

/// A background reactor that manages timers.
pub struct TimerReactor {
    // We use `(Instant, usize)` to disambiguate identical timer limits.
    // The `usize` is an atomic counter.
    timers: Mutex<BTreeMap<(Instant, usize), Waker>>,
    next_id: AtomicUsize,
}

impl Default for TimerReactor {
    fn default() -> Self {
        Self::new()
    }
}

impl TimerReactor {
    /// Creates a new `TimerReactor`.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            timers: Mutex::new(BTreeMap::new()),
            next_id: AtomicUsize::new(0),
        }
    }

    /// Register a waker to be called at `when`.
    ///
    /// # Panics
    ///
    /// Panics if the internal `timers` mutex is poisoned.
    pub fn register_timer(&self, when: Instant, waker: Waker) {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let mut timers = self.timers.lock().unwrap();
        timers.insert((when, id), waker);
        drop(timers);
    }

    /// Run the reactor loop, waking expired timers.
    ///
    /// # Panics
    ///
    /// Panics if the internal `timers` mutex is poisoned.
    pub fn run(&self) {
        loop {
            let now = Instant::now();
            let mut timers = self.timers.lock().unwrap();

            // PRODUCTION NOTE:
            // A production reactor would use `epoll` or `kqueue` to wait for I/O events, and
            // use a `Condvar` or eventfd for thread synchronization rather than sleeping.

            let mut to_remove = Vec::new();
            let mut expired = Vec::new();

            for (key, waker) in timers.iter() {
                if key.0 <= now {
                    expired.push(waker.clone());
                    to_remove.push(*key);
                } else {
                    break;
                }
            }

            for key in to_remove {
                timers.remove(&key);
            }

            // explicitly invoking drop(guard) on Mutex or RwLock guards
            drop(timers);

            for waker in expired {
                waker.wake();
            }

            // Sleep briefly to prevent 100% CPU usage.
            thread::sleep(Duration::from_millis(1));
        }
    }
}

/// A future that resolves after a specified duration.
pub struct TimerFuture {
    expiration: Instant,
}

impl TimerFuture {
    /// Creates a new `TimerFuture` which resolves after the given duration.
    #[must_use]
    pub fn new(duration: Duration) -> Self {
        Self {
            expiration: Instant::now() + duration,
        }
    }
}

impl Future for TimerFuture {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if Instant::now() >= self.expiration {
            Poll::Ready(())
        } else {
            // PRODUCTION NOTE:
            // A production-grade future would store the waker inside the future itself (using
            // something like `AtomicWaker`) to check `cx.waker().will_wake(stored_waker)` and
            // update it if the future is polled by a different executor/task. For simplicity in this
            // educational reactor, we just register it with the global reactor once or re-register.

            // To properly handle waker changes, we would deregister the old and register the new.
            // Here, we'll just allow multiple registrations for the same future which is harmless
            // (it just wakes it twice) but guarantees the new waker is notified.
            let reactor = get_reactor();
            reactor.register_timer(self.expiration, cx.waker().clone());

            Poll::Pending
        }
    }
}

/// A task to be executed.
struct Task {
    future: Mutex<Option<BoxFuture<'static, ()>>>,
    task_sender: SyncSender<Arc<Self>>,
}

type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

impl Wake for Task {
    fn wake(self: Arc<Self>) {
        // GOTCHA: We must clone the Arc to send it through the channel.
        // This enqueues the task back onto the executor's queue.
        let _ = self.task_sender.send(Arc::clone(&self));
    }
}

/// Spawns new futures onto the executor.
#[derive(Clone)]
pub struct Spawner {
    task_sender: SyncSender<Arc<Task>>,
}

impl Spawner {
    /// Spawn a future onto the executor.
    pub fn spawn(&self, future: impl Future<Output = ()> + Send + 'static) {
        let future = Box::pin(future);
        let task = Arc::new(Task {
            future: Mutex::new(Some(future)),
            task_sender: self.task_sender.clone(),
        });
        let _ = self.task_sender.send(task);
    }
}

/// Executes futures until they complete.
pub struct Executor {
    ready_queue: Receiver<Arc<Task>>,
}

impl Executor {
    /// Run all spawned futures to completion.
    ///
    /// # Panics
    ///
    /// Panics if the internal task future mutex is poisoned.
    pub fn run(&self) {
        // Keep receiving tasks until the channel is empty and all senders are dropped.
        while let Ok(task) = self.ready_queue.recv() {
            let mut future_slot = task.future.lock().unwrap();
            let future_opt = future_slot.take();
            drop(future_slot);

            if let Some(mut future) = future_opt {
                // RUST INSIGHT:
                // `Waker::from` works because `Task` implements the `Wake` trait.
                let waker = Waker::from(Arc::clone(&task));
                let mut context = Context::from_waker(&waker);

                // Poll the future.
                if future.as_mut().poll(&mut context).is_pending() {
                    // We're not done processing the future, so put it back in its task.
                    let mut future_slot = task.future.lock().unwrap();
                    *future_slot = Some(future);
                    drop(future_slot);
                }
            }
        }
    }
}

/// Create a new executor and spawner pair.
#[must_use]
pub fn new_executor_and_spawner() -> (Executor, Spawner) {
    // 10,000 tasks max in queue.
    let (task_sender, ready_queue) = sync_channel(10000);
    (Executor { ready_queue }, Spawner { task_sender })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};

    #[test]
    fn test_executor_basic() {
        let (executor, spawner) = new_executor_and_spawner();
        let flag = Arc::new(AtomicBool::new(false));
        let flag_clone = Arc::clone(&flag);

        spawner.spawn(async move {
            flag_clone.store(true, Ordering::SeqCst);
        });

        drop(spawner);
        executor.run();

        assert!(flag.load(Ordering::SeqCst));
    }

    #[test]
    fn test_timer_future() {
        let (executor, spawner) = new_executor_and_spawner();
        let start = Instant::now();

        spawner.spawn(async {
            TimerFuture::new(Duration::from_millis(50)).await;
        });

        drop(spawner);
        executor.run();

        assert!(start.elapsed() >= Duration::from_millis(50));
    }

    #[test]
    fn test_multiple_timers() {
        let (executor, spawner) = new_executor_and_spawner();
        let count = Arc::new(AtomicUsize::new(0));

        for _ in 0..5 {
            let c = Arc::clone(&count);
            spawner.spawn(async move {
                TimerFuture::new(Duration::from_millis(10)).await;
                c.fetch_add(1, Ordering::SeqCst);
            });
        }

        drop(spawner);
        executor.run();

        assert_eq!(count.load(Ordering::SeqCst), 5);
    }

    // Benchmarking Note:
    // To properly benchmark this custom async runtime against `tokio`:
    // 1. Use the `criterion` crate.
    // 2. Set up a benchmark that spawns 10,000 tasks which simply yield to the executor
    //    a fixed number of times (using a custom `YieldNow` future).
    // 3. Measure the throughput (tasks per second) of `executor.run()` vs `tokio::runtime::Runtime::new().unwrap().block_on(...)`.
    // 4. Expect `tokio` to be significantly faster due to its multi-threaded work-stealing pool
    //    and lack of Mutex locking around every spawned future state, but our simple MPSC channel
    //    executor will surprisingly hold its own for purely CPU-bound yielding tasks on a single thread.
}
