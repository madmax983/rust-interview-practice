//! # Async Runtime
//!
//! An educational implementation of a minimal asynchronous executor, spawner, and timer-based reactor from scratch.
//! This replaces crates like `tokio` or `async-std` to demonstrate how `Future`s are polled, tasks are scheduled,
//! and I/O (or time) events are reactor-driven.
//!
//! Real-world usage: Understanding the core loop of asynchronous I/O and task dispatching.
//! Why build it: To demystify the "magic" behind `async/await` and see exactly how a `Waker` interacts with a `Reactor`.
//!
//! ## Architecture
//!
//! ```text
//! +-------------+      (spawn)     +-------------+
//! |             | ---------------->|             |
//! |   Spawner   |                  |   Executor  |
//! |             |                  |             |
//! +-------------+                  +------+------+
//!                                         | (poll)
//!                                         v
//! +-------------+      (wake)      +-------------+
//! |             | <----------------|             |
//! |   Reactor   |                  |   Future    |
//! |             |                  |             |
//! +-------------+                  +-------------+
//! ```
//!
//! - **Invariants:** The executor polls tasks until they are `Pending`. A `Waker` must put the task back onto the executor's queue.
//! - **Complexity:** Polling is O(1) per task, but scheduling overhead exists.
//! - **Tradeoffs:** This educational runtime uses a simple global reactor and lacks features like thread-stealing or fair scheduling.

use std::{
    collections::BTreeMap,
    future::Future,
    pin::Pin,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicUsize, Ordering},
        mpsc::{Receiver, SyncSender, sync_channel},
    },
    task::{Context, Poll, Waker},
    thread,
    time::{Duration, Instant},
};

// RUST INSIGHT: We use a global `OnceLock` with `Box::leak` to store a static reference to the reactor.
// This allows any `Future` to register with the reactor without needing to explicitly pass reactor state around.
// Note the explicit `&'static TimerReactor` type parameter.
static REACTOR: OnceLock<&'static TimerReactor> = OnceLock::new();

/// The `Reactor` is responsible for tracking I/O or time events and waking tasks when they are ready.
pub struct TimerReactor {
    // GOTCHA: Timers can have identical instants. We disambiguate them using an atomic counter
    // so we don't accidentally overwrite an existing timer in the BTreeMap.
    timers: Mutex<BTreeMap<(Instant, usize), Waker>>,
    counter: AtomicUsize,
}

impl Default for TimerReactor {
    fn default() -> Self {
        Self::new()
    }
}

impl TimerReactor {
    #[must_use]
    pub fn new() -> Self {
        Self {
            timers: Mutex::new(BTreeMap::new()),
            counter: AtomicUsize::new(0),
        }
    }

    /// Registers a waker to be called at the given instant.
    pub fn register_timer(&self, at: Instant, waker: Waker) {
        let mut timers = self.timers.lock().unwrap();
        let id = self.counter.fetch_add(1, Ordering::SeqCst);
        timers.insert((at, id), waker);
        // UNSAFE JUSTIFICATION: Safe because we immediately drop the lock guard after use to prevent deadlocks,
        // addressing `clippy::significant_drop_tightening`.
        drop(timers);
    }

    /// Reactor loop that blocks and waits for timers to expire.
    // PRODUCTION NOTE: A real reactor (like Mio) would use `epoll` or `kqueue`.
    pub fn run(&self) {
        loop {
            let now = Instant::now();
            let mut timers = self.timers.lock().unwrap();

            // Extract all expired timers.
            // In Rust 1.53+, `BTreeMap::split_off` could be used or `extract_if`.
            // We do it manually to ensure we drop the lock during wake to prevent deadlocks.
            let mut to_wake = Vec::new();
            let mut _remaining = BTreeMap::<Instant, Waker>::new();

            for (_key, waker) in timers.drain_filter(|(k, _), _| *k <= now) {
                to_wake.push(waker);
            }
            drop(timers);

            for waker in to_wake {
                waker.wake();
            }

            thread::sleep(Duration::from_millis(10));
        }
    }
}

// Support manual draining for stable rust where drain_filter is nightly.
trait BTreeMapExt<K, V> {
    fn drain_filter<F>(&mut self, f: F) -> Vec<(K, V)>
    where
        F: FnMut(&K, &mut V) -> bool;
}

impl<K: Ord + Clone, V> BTreeMapExt<K, V> for BTreeMap<K, V> {
    fn drain_filter<F>(&mut self, mut f: F) -> Vec<(K, V)>
    where
        F: FnMut(&K, &mut V) -> bool,
    {
        let mut to_remove = Vec::new();
        for (k, v) in self.iter_mut() {
            if f(k, v) {
                to_remove.push(k.clone());
            }
        }

        let mut removed = Vec::new();
        for k in to_remove {
            if let Some(v) = self.remove(&k) {
                removed.push((k, v));
            }
        }
        removed
    }
}

/// A Future that resolves after a specified duration.
pub struct SleepFuture {
    until: Instant,
    registered: bool,
}

impl SleepFuture {
    #[must_use]
    pub fn new(duration: Duration) -> Self {
        Self {
            until: Instant::now().checked_add(duration).expect("Overflow"), // Will be evaluated at runtime, `Instant::now()` isn't const, so we cheat
            registered: false,
        }
    }

    #[must_use]
    pub fn after(duration: Duration) -> Self {
        Self {
            until: Instant::now() + duration,
            registered: false,
        }
    }
}

impl Future for SleepFuture {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if Instant::now() >= self.until {
            Poll::Ready(())
        } else {
            if !self.registered {
                let reactor = REACTOR.get().expect("Reactor not initialized");
                reactor.register_timer(self.until, cx.waker().clone());
                self.registered = true;
            }
            Poll::Pending
        }
    }
}

/// Task represents a single, spawned Future.
pub struct Task {
    future: Mutex<Pin<Box<dyn Future<Output = ()> + Send + 'static>>>,
    task_sender: SyncSender<Arc<Task>>,
}

impl arc_wake::ArcWake for Task {
    fn wake_by_ref(arc_self: &Arc<Self>) {
        let cloned = arc_self.clone();
        arc_self
            .task_sender
            .send(cloned)
            .expect("Too many tasks queued");
    }
}

/// The Spawner provides a channel to send tasks to the Executor.
#[derive(Clone)]
pub struct Spawner {
    task_sender: SyncSender<Arc<Task>>,
}

impl Spawner {
    pub fn spawn(&self, future: impl Future<Output = ()> + Send + 'static) {
        let future = Box::pin(future);
        let task = Arc::new(Task {
            future: Mutex::new(future),
            task_sender: self.task_sender.clone(),
        });
        self.task_sender.send(task).expect("Too many tasks queued");
    }
}

/// The Executor pulls tasks off a channel and polls them.
pub struct Executor {
    ready_queue: Receiver<Arc<Task>>,
}

impl Executor {
    pub fn run(&self) {
        while let Ok(task) = self.ready_queue.recv() {
            let mut future_slot = task.future.lock().unwrap();
            let waker = arc_wake::waker_ref(&task);
            let context = &mut Context::from_waker(&waker);

            // Poll the future
            let _ = future_slot.as_mut().poll(context);
            drop(future_slot);
        }
    }
}

/// Creates a new asynchronous runtime, returning a `Spawner` and `Executor`.
#[must_use]
pub fn new_runtime() -> (Spawner, Executor) {
    let (task_sender, ready_queue) = sync_channel(10000);

    // Initialize the global reactor
    let reactor = TimerReactor::new();
    let leaked: &'static TimerReactor = Box::leak(Box::new(reactor));

    let _ = REACTOR.set(leaked);

    // Spawn the reactor thread
    // RUST INSIGHT: We explicitly type the variable as an immutable reference
    // to avoid borrow checker move errors.
    let ref_reactor: &'static TimerReactor = leaked;
    thread::spawn(move || {
        ref_reactor.run();
    });

    (Spawner { task_sender }, Executor { ready_queue })
}

// -----------------------------------------------------------------------------
// Boilerplate arc_wake crate stub for educational purposes (avoids external dep)
// -----------------------------------------------------------------------------
mod arc_wake {
    use std::sync::Arc;
    use std::task::{RawWaker, RawWakerVTable, Waker};

    pub trait ArcWake: Send + Sync {
        fn wake_by_ref(arc_self: &Arc<Self>);
        fn wake(arc_self: Arc<Self>) {
            Self::wake_by_ref(&arc_self);
        }
    }

    pub fn waker_ref<W: ArcWake + 'static>(wake: &Arc<W>) -> Waker {
        let ptr = Arc::into_raw(wake.clone()).cast::<()>();

        let vtable = &RawWakerVTable::new(
            clone_arc_raw::<W>,
            wake_arc_raw::<W>,
            wake_by_ref_arc_raw::<W>,
            drop_arc_raw::<W>,
        );

        let raw_waker = RawWaker::new(ptr, vtable);
        unsafe { Waker::from_raw(raw_waker) }
    }

    unsafe fn clone_arc_raw<W: ArcWake + 'static>(data: *const ()) -> RawWaker {
        let arc = unsafe { Arc::from_raw(data.cast::<W>()) };
        let cloned = arc.clone();
        let _ = Arc::into_raw(arc);

        let ptr = Arc::into_raw(cloned).cast::<()>();
        let vtable = &RawWakerVTable::new(
            clone_arc_raw::<W>,
            wake_arc_raw::<W>,
            wake_by_ref_arc_raw::<W>,
            drop_arc_raw::<W>,
        );
        RawWaker::new(ptr, vtable)
    }

    unsafe fn wake_arc_raw<W: ArcWake + 'static>(data: *const ()) {
        let arc = unsafe { Arc::from_raw(data.cast::<W>()) };
        ArcWake::wake(arc);
    }

    unsafe fn wake_by_ref_arc_raw<W: ArcWake + 'static>(data: *const ()) {
        let arc = unsafe { Arc::from_raw(data.cast::<W>()) };
        ArcWake::wake_by_ref(&arc);
        let _ = Arc::into_raw(arc);
    }

    unsafe fn drop_arc_raw<W: ArcWake + 'static>(data: *const ()) {
        drop(unsafe { Arc::from_raw(data.cast::<W>()) });
    }
}

// Footer:
// Missing features: Thread-pool based execution, I/O polling (epoll/kqueue), fair scheduling.
// Compare with: tokio, async-std. Next steps: Implement a multi-threaded work-stealing executor.

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};

    #[test]
    fn test_async_runtime_happy_path() {
        let (spawner, executor) = new_runtime();

        let flag = Arc::new(AtomicBool::new(false));
        let flag_clone = flag.clone();

        spawner.spawn(async move {
            SleepFuture::after(Duration::from_millis(10)).await;
            flag_clone.store(true, Ordering::SeqCst);
        });

        // Spawn a task that terminates the executor loop (mock behavior for test)
        let _spawner_clone = spawner.clone();
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(50));
            // Just dropping the sender doesn't stop our simple recv loop nicely,
            // but in a real test we'd have a shutdown mechanism.
            // For now, we spawn an executor on a thread and let the process end.
        });

        thread::spawn(move || {
            executor.run();
        });

        thread::sleep(Duration::from_millis(100));
        assert!(flag.load(Ordering::SeqCst));
    }

    #[test]
    fn test_async_runtime_edge_case_immediate_ready() {
        let (spawner, executor) = new_runtime();

        let flag = Arc::new(AtomicBool::new(false));
        let flag_clone = flag.clone();

        spawner.spawn(async move {
            // No sleep, completes immediately
            flag_clone.store(true, Ordering::SeqCst);
        });

        thread::spawn(move || {
            executor.run();
        });

        thread::sleep(Duration::from_millis(20));
        assert!(flag.load(Ordering::SeqCst));
    }

    #[test]
    fn test_async_runtime_stress_multiple_timers() {
        let (spawner, executor) = new_runtime();

        let counter = Arc::new(AtomicUsize::new(0));

        for _ in 0..10 {
            let counter_clone = counter.clone();
            spawner.spawn(async move {
                SleepFuture::after(Duration::from_millis(5)).await;
                counter_clone.fetch_add(1, Ordering::SeqCst);
            });
        }

        thread::spawn(move || {
            executor.run();
        });

        thread::sleep(Duration::from_millis(50));
        assert_eq!(counter.load(Ordering::SeqCst), 10);
    }
}
