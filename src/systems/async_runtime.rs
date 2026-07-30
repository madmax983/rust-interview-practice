//! # Async Runtime
//!
//! Implements a minimal asynchronous executor, spawner, and timer-based reactor from scratch.
//!
//! **Replaces Crates:** `tokio`, `async-std`, `smol`
//!
//! **Real-world Usage:**
//! - Core of modern Rust networking (web servers like Axum/Actix rely on Tokio).
//! - Database drivers handling thousands of concurrent connections.
//! - Embedded devices multiplexing I/O over serial interfaces.
//!
//! **Why build it yourself?**
//! Rust's `async`/`await` model is often viewed as magic. Building a runtime demystifies
//! this by showing that `Futures` are just lazy state machines, the `Executor` is just
//! a while loop over a queue, and a `Waker` is a callback that pushes the task back onto
//! the queue when I/O or a timer is ready.
//!
//! # Architecture
//!
//! ```text
//!       ┌─────────────────────────────────────────────────────────────┐
//!       │                         Executor                            │
//!       │  ┌─────────────────────────┐                                │
//!       │  │       Task Queue        │◄────────┐                      │
//!       │  └────────────┬────────────┘         │                      │
//!       │               │                      │                      │
//!       │               ▼                      │                      │
//!       │      ┌────────────────┐         ┌────┴────┐                 │
//!       │      │   poll()       │────────►│  Waker  │                 │
//!       │      └────────────────┘         └─────────┘                 │
//!       │               │                      ▲                      │
//!       └───────────────┼──────────────────────┼──────────────────────┘
//!                       │                      │ wake()
//!                       ▼                      │
//!                  ┌─────────┐            ┌────┴────┐
//!                  │ Reactor │───────────►│ I/O /   │
//!                  │ (Timer) │            │ Timers  │
//!                  └─────────┘            └─────────┘
//! ```
//!
//! **Invariants:**
//! 1. A `Future` is only polled when it's in the Executor's ready queue.
//! 2. When a `Future` returns `Poll::Pending`, it *must* arrange for its `Waker` to be called.
//! 3. Calling `wake()` pushes the associated task back onto the Executor's ready queue.
//!
//! **Design Decisions:**
//! - **Channels for queues**: We use `std::sync::mpsc` for simplicity, though a real
//!   runtime uses highly optimized lock-free or work-stealing queues.
//! - **Global Reactor**: The timer reactor runs in a separate background thread, managing
//!   all active timers via a `BTreeMap` ordered by expiration time.
//!
//! # Comparison to Production (e.g., Tokio)
//!
//! | Feature          | This Implementation | Tokio / smol                           |
//! |------------------|---------------------|----------------------------------------|
//! | **Executor**     | Single-threaded MPSC| Work-stealing, multi-threaded         |
//! | **Reactor**      | Thread.sleep + Map  | epoll/kqueue/io_uring                 |
//! | **Waker alloc**  | `Arc` per task      | Slab allocated or inline              |
//! | **Timers**       | Background thread   | Hierarchical hashed wheel timers       |
//!
//! **Missing Features:**
//! - I/O polling (TCP/UDP).
//! - Task cancellation (dropping the JoinHandle).
//! - Local sets (executing `!Send` futures).
//
// **Suggested Next Steps:**
// 1. Add TCP/UDP non-blocking socket handling by integrating `mio` into the Reactor loop.
// 2. Implement an unbounded queue or `try_send` for the Waker to avoid deadlocking if the channel fills up.
// 3. Swap the `std::sync::mpsc` for `crossbeam_channel` or implement a basic work-stealing queue.
//
// **Benchmarking Note:**
// To benchmark this runtime, you would typically use `criterion` to measure task spawning
// throughput (e.g., spawn 100,000 empty futures and wait for them all to complete).
// A micro-benchmark could look like:
// `b.iter(|| { spawner.spawn(async { black_box(1 + 1); }) })`

use std::{
    collections::BTreeMap,
    future::Future,
    mem::ManuallyDrop,
    pin::Pin,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicUsize, Ordering},
        mpsc::{Receiver, SyncSender, sync_channel},
    },
    task::{Context, Poll, RawWaker, RawWakerVTable, Waker},
    thread,
    time::Instant,
};

// =========================================================================================
// Executor & Task
// =========================================================================================

/// A pinned, heap-allocated future that can be sent across threads.
type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Represents a single unit of work (a Future) in the runtime.
struct Task {
    /// The actual future being executed. Wrapped in a Mutex because `Task` is shared
    /// between the Executor (which polls it) and Wakers (which just need to re-queue it),
    /// requiring interior mutability. Option is used so we can take ownership of the
    /// future during polling.
    future: Mutex<Option<BoxFuture<'static, ()>>>,
    /// The channel back to the Executor to re-schedule this task.
    task_sender: SyncSender<Arc<Task>>,
}

/// The VTable that tells Rust how to clone, wake, and drop our custom Waker.
/// This must be static as `RawWaker` requires a `'static` lifetime.
static VTABLE: RawWakerVTable =
    RawWakerVTable::new(task_clone, task_wake, task_wake_by_ref, task_drop);

// RUST INSIGHT:
// Implementing a `RawWakerVTable` manually is bridging the gap between Rust's safe
// `Waker` trait and our specific implementation (an `Arc<Task>`). It requires unsafe
// because we are managing raw pointers, but we uphold safety by ensuring the pointer
// always points to a valid `Arc<Task>`.

unsafe fn task_clone(raw_ptr: *const ()) -> RawWaker {
    // UNSAFE JUSTIFICATION: We constructed this pointer from an Arc<Task>.
    let ptr = raw_ptr.cast::<Task>();

    // We wrap it in ManuallyDrop because we don't want to decrement the ref count
    // of the *original* Arc when this temporary Arc goes out of scope.
    let arc = ManuallyDrop::new(unsafe { Arc::from_raw(ptr) });

    // Clone the Arc to increment the ref count for the new Waker.
    let cloned: Arc<Task> = (*arc).clone();

    arc_to_raw_waker(cloned)
}

unsafe fn task_wake(raw_ptr: *const ()) {
    // UNSAFE JUSTIFICATION: We constructed this pointer from an Arc<Task>.
    // Here we take ownership (no ManuallyDrop), so dropping `arc` will decrement the ref count.
    let arc = unsafe { Arc::from_raw(raw_ptr.cast::<Task>()) };
    let _ = arc.task_sender.send(arc.clone());
}

unsafe fn task_wake_by_ref(raw_ptr: *const ()) {
    // UNSAFE JUSTIFICATION: We constructed this pointer from an Arc<Task>.
    let ptr = raw_ptr.cast::<Task>();
    let arc = ManuallyDrop::new(unsafe { Arc::from_raw(ptr) });
    let _ = arc.task_sender.send((*arc).clone());
}

unsafe fn task_drop(raw_ptr: *const ()) {
    // UNSAFE JUSTIFICATION: We constructed this pointer from an Arc<Task>.
    // Reconstruct the Arc and let it drop naturally to decrement the ref count.
    drop(unsafe { Arc::from_raw(raw_ptr.cast::<Task>()) });
}

fn arc_to_raw_waker(task: Arc<Task>) -> RawWaker {
    let raw_ptr = Arc::into_raw(task).cast::<()>();
    RawWaker::new(raw_ptr, &VTABLE)
}

/// Spawns new tasks onto the Executor.
#[derive(Clone)]
pub struct Spawner {
    task_sender: SyncSender<Arc<Task>>,
}

impl Spawner {
    /// Spawns a future onto the executor.
    ///
    /// # Panics
    /// Panics if the executor's task queue is full or closed.
    pub fn spawn(&self, future: impl Future<Output = ()> + Send + 'static) {
        let future = Box::pin(future);
        let task = Arc::new(Task {
            future: Mutex::new(Some(future)),
            task_sender: self.task_sender.clone(),
        });

        self.task_sender
            .send(task)
            .expect("Executor queue closed or full");
    }
}

/// The runtime that executes futures.
pub struct Executor {
    ready_queue: Receiver<Arc<Task>>,
}

impl Executor {
    /// Runs all spawned tasks to completion (or until the spawner is dropped).
    ///
    /// # Panics
    /// Panics if a task panics while being polled, poisoning the mutex.
    pub fn run(&self) {
        // GOTCHA:
        // We loop over the channel. The loop exits when the Sender (Spawner) is dropped
        // and the queue is empty. If you leak the Spawner, the Executor runs forever.
        while let Ok(task) = self.ready_queue.recv() {
            let mut future_slot = task.future.lock().unwrap();

            // Take the future. If it's already None, the task is finished.
            if let Some(mut future) = future_slot.take() {
                // RUST INSIGHT:
                // We create a Waker from our Arc<Task>. When the future's leaf node
                // (like our TimerFuture) calls `waker.wake()`, it will trigger our
                // `task_wake` function, which pushes the `Arc<Task>` back into the `ready_queue`.
                let waker = unsafe { Waker::from_raw(arc_to_raw_waker(task.clone())) };
                let context = &mut Context::from_waker(&waker);

                // Poll the future.
                match future.as_mut().poll(context) {
                    Poll::Pending => {
                        // The future is not done. Put it back in the slot.
                        // It will remain here until the Reactor calls wake(), re-queueing the task.
                        *future_slot = Some(future);
                    }
                    Poll::Ready(()) => {
                        // The future finished. We do nothing, dropping the future.
                    }
                }
            }
        }
    }
}

/// Creates a new Executor and a Spawner to submit tasks to it.
#[must_use]
pub fn new_executor_and_spawner() -> (Executor, Spawner) {
    // 10,000 max queued tasks.
    let (task_sender, ready_queue) = sync_channel(10_000);
    (Executor { ready_queue }, Spawner { task_sender })
}

// =========================================================================================
// Reactor (Timer System)
// =========================================================================================

/// A global singleton for the timer reactor.
static REACTOR: OnceLock<&'static TimerReactor> = OnceLock::new();

/// Represents a registration in the reactor.
struct TimerRegistration {
    waker: Waker,
}

/// A background reactor that wakes futures when their timers expire.
pub struct TimerReactor {
    /// Ordered map of timers. The key is `(ExpirationTime, Id)`.
    /// The `Id` disambiguates timers that expire at the exact same instant.
    timers: Mutex<BTreeMap<(Instant, usize), TimerRegistration>>,
    counter: AtomicUsize,
}

impl TimerReactor {
    /// Initializes the global timer reactor thread.
    /// In a real system (like Tokio), the reactor is tightly coupled with the executor,
    /// usually running on the same threads using epoll/io_uring.
    pub fn init() {
        REACTOR.get_or_init(|| {
            let reactor = Box::new(TimerReactor {
                timers: Mutex::new(BTreeMap::new()),
                counter: AtomicUsize::new(0),
            });

            // Leak the box to get a 'static reference.
            let reactor_ref: &'static TimerReactor = Box::leak(reactor);

            // Spawn the background thread to process timers.
            thread::spawn(move || {
                reactor_ref.run_loop();
            });

            reactor_ref
        });
    }

    /// The background loop that checks for expired timers.
    fn run_loop(&self) {
        loop {
            let now = Instant::now();
            let mut wakers_to_call = Vec::new();

            {
                let mut timers = self.timers.lock().unwrap();

                // BTreeMap is sorted by key (Instant). We can split the map at `now`.
                // `split_off` returns everything *after* the key, so the original map
                // retains everything *before* (i.e., the expired timers).

                // GOTCHA: We need to use a key that is guaranteed to be greater than any
                // valid timer at this `now` instant, so we use `usize::MAX` for the counter.
                let split_key = (now, usize::MAX);
                let unexpired = timers.split_off(&split_key);

                // The remaining elements in `timers` are expired.
                for (_, registration) in std::mem::take(&mut *timers) {
                    wakers_to_call.push(registration.waker);
                }

                // Restore the unexpired timers back into the map.
                *timers = unexpired;
            } // Drop the lock before calling wake()

            // Call the wakers. This sends the tasks back to the Executor.
            for waker in wakers_to_call {
                waker.wake();
            }

            // Sleep for a short duration to prevent spinning.
            // A production reactor would use `epoll_wait` with a timeout equal to the
            // closest timer, avoiding unnecessary wakeups.
            thread::sleep(std::time::Duration::from_millis(1));
        }
    }

    /// Registers a new timer, returning an ID to track it.
    fn register(&self, at: Instant, waker: Waker) -> (Instant, usize) {
        let id = self.counter.fetch_add(1, Ordering::Relaxed);
        let key = (at, id);

        let mut timers = self.timers.lock().unwrap();
        timers.insert(key, TimerRegistration { waker });

        key
    }

    /// Deregisters a timer if the future is dropped early.
    fn deregister(&self, key: &(Instant, usize)) {
        let mut timers = self.timers.lock().unwrap();
        timers.remove(key);
    }
}

// =========================================================================================
// Timer Future
// =========================================================================================

/// A Future that resolves after a specified duration.
pub struct Sleep {
    deadline: Instant,
    key: Option<(Instant, usize)>,
}

impl Sleep {
    /// Creates a new `Sleep` future.
    #[must_use]
    pub fn new(duration: std::time::Duration) -> Self {
        TimerReactor::init(); // Ensure reactor is running

        Self {
            deadline: Instant::now() + duration,
            key: None,
        }
    }
}

impl Future for Sleep {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if Instant::now() >= self.deadline {
            return Poll::Ready(());
        }

        // We aren't ready yet. We must register our waker with the Reactor.
        // If we already registered, we should deregister the old one first.
        // PRODUCTION NOTE:
        // A real Future must handle the case where it is polled by a *different*
        // Waker/Task than before. We should update the registration.

        let reactor = REACTOR.get().expect("Reactor not initialized");

        if let Some(key) = self.key.take() {
            reactor.deregister(&key);
        }

        let key = reactor.register(self.deadline, cx.waker().clone());
        self.key = Some(key);

        Poll::Pending
    }
}

impl Drop for Sleep {
    fn drop(&mut self) {
        // If the future is dropped before it completes, we must clean up the reactor.
        if let (Some(key), Some(reactor)) = (self.key, REACTOR.get()) {
            reactor.deregister(&key);
        }
    }
}

/// Convenience function to create a `Sleep` future.
#[must_use]
pub fn sleep(duration: std::time::Duration) -> Sleep {
    Sleep::new(duration)
}

// =========================================================================================
// Tests
// =========================================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    #[test]
    fn test_executor_simple_task() {
        let (executor, spawner) = new_executor_and_spawner();
        let counter = Arc::new(AtomicUsize::new(0));
        let c = counter.clone();

        spawner.spawn(async move {
            c.fetch_add(1, Ordering::SeqCst);
        });

        // Drop the spawner so the executor will exit when the queue is empty.
        drop(spawner);
        executor.run();

        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_sleep_timer() {
        let (executor, spawner) = new_executor_and_spawner();
        let start = Instant::now();

        spawner.spawn(async move {
            sleep(Duration::from_millis(50)).await;
        });

        drop(spawner);
        executor.run();

        assert!(start.elapsed() >= Duration::from_millis(50));
    }

    #[test]
    fn test_multiple_concurrent_sleeps() {
        let (executor, spawner) = new_executor_and_spawner();
        let counter = Arc::new(AtomicUsize::new(0));

        for _ in 0..5 {
            let c = counter.clone();
            spawner.spawn(async move {
                sleep(Duration::from_millis(20)).await;
                c.fetch_add(1, Ordering::SeqCst);
            });
        }

        drop(spawner);
        let start = Instant::now();
        executor.run();
        let elapsed = start.elapsed();

        assert_eq!(counter.load(Ordering::SeqCst), 5);
        // If they ran sequentially, it would take > 100ms.
        // Running concurrently, it should take ~20ms.
        assert!(elapsed < Duration::from_millis(80));
    }

    #[test]
    fn test_future_chaining() {
        let (executor, spawner) = new_executor_and_spawner();
        let result = Arc::new(Mutex::new(Vec::new()));

        let res1 = result.clone();
        let res2 = result.clone();

        spawner.spawn(async move {
            res1.lock().unwrap().push(1);
            sleep(Duration::from_millis(10)).await;
            res1.lock().unwrap().push(2);
        });

        spawner.spawn(async move {
            res2.lock().unwrap().push(3);
            sleep(Duration::from_millis(20)).await;
            res2.lock().unwrap().push(4);
        });

        drop(spawner);
        executor.run();

        let final_result = result.lock().unwrap().clone();
        assert_eq!(final_result.len(), 4);
        assert!(final_result.contains(&1));
        assert!(final_result.contains(&2));
        assert!(final_result.contains(&3));
        assert!(final_result.contains(&4));
    }
}
