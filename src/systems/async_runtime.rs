//! # Async Runtime
//!
//! What this implements and what crate(s) it replaces:
//! This is a minimal asynchronous runtime featuring a basic executor, spawner, and a timer-based reactor.
//! It replaces the fundamental core of crates like `tokio` and `async-std`.
//!
//! Real-world systems that use this:
//! Rust's async ecosystem relies on runtimes (like Tokio) to poll futures to completion, handling I/O, timers, and task scheduling.
//!
//! Why build it yourself?
//! Understanding the `Future` trait, `Waker` mechanics, and how runtimes coordinate with reactors is critical.
//! Building a mini runtime demystifies how `async`/`await` actually compiles down to state machines driven by a polling executor.
//!
//! ## Architecture
//!
//! ```text
//! +---------------+          +---------------+           +---------------+
//! |   Spawner     | --Task-> |  MiniExecutor | <-Wakes-- | TimerReactor  |
//! +---------------+          +---------------+           +---------------+
//!        ^                           |                           |
//!        |                           v                           |
//!        +----Wakes itself----- [Task (Future)] ---Registers-----+
//! ```
//!
//! ## Invariants
//! - **Wakeup Guarantees**: A future must ensure it will be woken up if it returns `Poll::Pending`.
//! - **Reactor Thread**: The reactor must run continuously to process expired timers and wake their associated futures.
//!
//! ## Complexity
//! - **Spawn**: `O(1)` - Just sends a task to the channel.
//! - **Poll**: `O(1)` per ready task.
//! - **Timer Registration**: `O(log N)` - Uses a `BTreeMap` for sorting timers.
//! - **Timer Wakeup**: `O(K)` where `K` is the number of expired timers.
//!
//! ## Design Decisions and Tradeoffs
//! - **Sync Channel**: We use `std::sync::mpsc::sync_channel` with a capacity to limit the number of queued tasks, applying backpressure.
//! - **Global Reactor**: A `OnceLock` singleton is used for the reactor to easily create and register timers from anywhere without passing the reactor explicitly.
//!
//! ## Alternative Approaches
//! - A real production runtime like `tokio` uses lock-free queues, epoll/kqueue/io_uring for I/O, and specialized wheel timers for `O(1)` timer overhead.
//!
//! ## Missing Features
//! - No I/O reactor (only timers).
//! - No work-stealing thread pool (single-threaded executor).
//!
//! ## Suggested Next Steps / Extensions
//! - Implement an I/O reactor using `mio` or `epoll` directly.
//! - Add a multithreaded work-stealing executor.
//! - Build a `JoinHandle` to return results from spawned tasks.
//!
//! ## Benchmarking Note
//! To benchmark this runtime against `tokio`, use `criterion` to spawn and await 10,000 short sleep tasks in a loop, measuring the overhead of task creation and waking vs Tokio's optimized wheel timer.

use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};
use std::sync::{Arc, Mutex, OnceLock};
use std::task::{Context, Poll, Wake, Waker};
use std::thread;
use std::time::{Duration, Instant};

/// The Executor trait defines the interface for running a runtime.
pub trait Executor {
    /// Runs the executor, polling tasks until completion or exhaustion.
    fn run(&self);
}

/// A task is a boxed future that can be spawned and polled.
pub struct Task {
    /// The future to be polled. It is wrapped in a `Mutex` to ensure thread safety
    /// when the executor polls it, and `Option` so we can take it out to poll.
    future: Mutex<Option<Pin<Box<dyn Future<Output = ()> + Send + 'static>>>>,
    /// The sender back to the executor's ready queue.
    task_sender: SyncSender<Arc<Self>>,
}

impl Wake for Task {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        // Send the task back to the executor to be polled again.
        let _ = self.task_sender.send(self.clone());
    }
}

/// A Spawner allows pushing new tasks onto the executor's queue.
#[derive(Clone)]
pub struct Spawner {
    task_sender: SyncSender<Arc<Task>>,
}

impl Spawner {
    /// Spawns a new asynchronous task.
    pub fn spawn(&self, future: impl Future<Output = ()> + Send + 'static) {
        let task = Arc::new(Task {
            future: Mutex::new(Some(Box::pin(future))),
            task_sender: self.task_sender.clone(),
        });
        let _ = self.task_sender.send(task);
    }
}

/// A minimal single-threaded executor.
pub struct MiniExecutor {
    ready_queue: Receiver<Arc<Task>>,
}

impl MiniExecutor {
    /// Creates a new executor and spawner pair.
    #[must_use]
    pub fn new() -> (Self, Spawner) {
        // PRODUCTION NOTE: Tokio uses a highly optimized lock-free queue (like crossbeam's).
        // We use the standard library's bounded channel for simplicity and backpressure.
        let (task_sender, ready_queue) = sync_channel(10_000);
        (
            Self { ready_queue },
            Spawner { task_sender },
        )
    }
}

impl Executor for MiniExecutor {
    /// # Panics
    ///
    /// Panics if the `Mutex` lock on the task's future is poisoned.
    fn run(&self) {
        // RUST INSIGHT: The loop only terminates when all `Spawner` instances (including
        // the ones inside the tasks themselves) are dropped.
        while let Ok(task) = self.ready_queue.recv() {
            let mut future_slot = task.future.lock().unwrap();
            if let Some(mut future) = future_slot.take() {
                // Construct a Waker from the task using the standard `Wake` trait.
                let waker = Waker::from(task.clone());
                let mut context = Context::from_waker(&waker);

                if future.as_mut().poll(&mut context).is_pending() {
                    // We're not done yet, put it back.
                    *future_slot = Some(future);
                }

                drop(future_slot); // clippy::significant_drop_tightening
            }
        }
    }
}

/// A reactor that processes sleep timers.
pub struct TimerReactor {
    /// Timers sorted by their expiration `Instant`. The `usize` disambiguates identical instants.
    timers: Mutex<BTreeMap<(Instant, usize), Waker>>,
    id_counter: AtomicUsize,
}

impl TimerReactor {
    /// Creates a new timer reactor.
    #[must_use]
    pub fn new() -> Self {
        Self {
            timers: Mutex::new(BTreeMap::new()),
            id_counter: AtomicUsize::new(0),
        }
    }
}

impl Default for TimerReactor {
    fn default() -> Self {
        Self::new()
    }
}

static REACTOR: OnceLock<&'static TimerReactor> = OnceLock::new();

/// Gets the global timer reactor instance, initializing it and its background thread if necessary.
#[must_use]
pub fn get_reactor() -> &'static TimerReactor {
    REACTOR.get_or_init(|| {
        let reactor = Box::new(TimerReactor::default());
        // Explicitly type the reference to avoid borrow checker move errors when passing it to thread::spawn.
        let reactor_ref: &'static TimerReactor = Box::leak(reactor);

        thread::spawn(move || {
            loop {
                let now = Instant::now();
                let mut timers_to_wake = Vec::new();

                {
                    let mut timers = reactor_ref.timers.lock().unwrap();
                    // ⚡ BOLT OPTIMIZATION: Using `BTreeMap::split_off` allows us to extract all expired timers in O(log N) time,
                    // rather than iterating over all timers and removing them one by one in O(N) time.
                    // Split the tree at current time.
                    // split_off returns all keys >= the given key.
                    // So `later` contains all timers in the future.
                    let mut later = timers.split_off(&(now, usize::MAX));

                    // Swap to keep `later` (the future timers) in the reactor,
                    // and take the current timers (which are <= now and ready to fire).
                    std::mem::swap(&mut *timers, &mut later);
                    let ready = later; // later now holds the old timers that are ready.

                    for (_key, waker) in ready {
                        timers_to_wake.push(waker);
                    }
                    drop(timers); // clippy::significant_drop_tightening
                }

                for waker in timers_to_wake {
                    // GOTCHA: Calling wake() might execute arbitrary user code or enqueue tasks.
                    // It's critical that we do not hold the timers mutex while waking, or we could deadlock.
                    waker.wake();
                }

                // Sleep to prevent hot loop.
                thread::sleep(Duration::from_millis(1));
            }
        });

        reactor_ref
    })
}

/// A future that completes after a specified duration.
pub struct Sleep {
    until: Instant,
    id: usize,
}

impl Sleep {
    /// Creates a new sleep future.
    #[must_use]
    pub fn new(duration: Duration) -> Self {
        let reactor = get_reactor();
        let id = reactor.id_counter.fetch_add(1, Ordering::Relaxed);
        Self {
            until: Instant::now() + duration,
            id,
        }
    }
}

impl Future for Sleep {
    type Output = ();

    /// # Panics
    ///
    /// Panics if the `Mutex` lock on `timers` is poisoned.
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let now = Instant::now();
        if now >= self.until {
            Poll::Ready(())
        } else {
            let reactor = get_reactor();
            let mut timers = reactor.timers.lock().unwrap();
            timers.insert((self.until, self.id), cx.waker().clone());
            drop(timers); // clippy::significant_drop_tightening

            Poll::Pending
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};

    #[test]
    fn test_executor_spawns_and_runs() {
        let (executor, spawner) = MiniExecutor::new();
        let flag = Arc::new(AtomicBool::new(false));

        let flag_clone = Arc::clone(&flag);
        spawner.spawn(async move {
            flag_clone.store(true, Ordering::SeqCst);
        });

        // Drop the spawner so the executor knows it can shut down when the queue is empty.
        drop(spawner);
        executor.run();

        assert!(flag.load(Ordering::SeqCst));
    }

    #[test]
    fn test_sleep_future_wakes_up() {
        let (executor, spawner) = MiniExecutor::new();
        let flag = Arc::new(AtomicBool::new(false));

        let flag_clone = Arc::clone(&flag);
        spawner.spawn(async move {
            let sleep = Sleep::new(Duration::from_millis(10));
            sleep.await;
            flag_clone.store(true, Ordering::SeqCst);
        });

        drop(spawner);

        let start = Instant::now();
        executor.run();
        let elapsed = start.elapsed();

        assert!(flag.load(Ordering::SeqCst));
        assert!(elapsed >= Duration::from_millis(10));
    }

    #[test]
    fn test_concurrent_tasks_and_timers() {
        let (executor, spawner) = MiniExecutor::new();
        let counter = Arc::new(AtomicUsize::new(0));

        for i in 0..5 {
            let counter_clone = Arc::clone(&counter);
            spawner.spawn(async move {
                // Variable timeout to test out-of-order completion
                let sleep = Sleep::new(Duration::from_millis(10 + i as u64 * 2));
                sleep.await;
                counter_clone.fetch_add(1, Ordering::SeqCst);
            });
        }

        drop(spawner);
        executor.run();

        assert_eq!(counter.load(Ordering::SeqCst), 5);
        let _empty_vec = Vec::<i32>::new();
    }
}
