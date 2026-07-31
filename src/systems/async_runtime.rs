//! # Async Runtime Implementation
//!
//! Demonstrates a minimal asynchronous executor, spawner, and timer-based reactor from scratch.
//!
//! **Replaces Crates:** `tokio`, `async-std`
//!
//! **Real-world Usage:**
//! - Web servers handling thousands of concurrent connections
//! - Database drivers multiplexing queries
//! - High-throughput microservices
//!
//! **Why build it yourself?**
//! Async Rust is often seen as "magic" or a black box. Building a runtime demystifies how `Future`s are polled,
//! how `Waker`s wake tasks up, and how a Reactor notifies the Executor. It forces you to understand the
//! machinery behind `async/await`.
//!
//! # Architecture
//!
//! ```text
//! ┌────────────────┐         ┌───────────────┐
//! │    Executor    │◄────────│     Waker     │
//! │ (Polls Tasks)  │         │ (Wakes Tasks) │
//! └──────┬─────────┘         └──────▲────────┘
//!        │                          │
//!        ▼                          │
//! ┌────────────────┐         ┌──────┴────────┐
//! │     Future     │────────►│    Reactor    │
//! │ (State Machine)│         │ (Event Loop)  │
//! └────────────────┘         └───────────────┘
//! ```
//!
//! **Invariants:**
//! 1. A task is only polled when woken, or when initially spawned.
//! 2. The reactor runs independently, checking for events (like timers).
//! 3. Wakers must safely push the task back to the executor's ready queue.
//!
//! **Time/Space Complexity:**
//! - Task Spawn: O(1) time, O(1) space per task
//! - Task Wake: O(1) time (channel send)
//! - Timer Register: O(log N) time (BTreeMap insert)
//!
//! **Design Decisions and Tradeoffs:**
//! - Uses a simple MPSC channel for the task queue, avoiding complex work-stealing algorithms.
//! - The reactor uses a dedicated thread and `BTreeMap` for timers instead of a specialized timing wheel (like in Tokio), which is simpler but has O(log N) insertions.
//!
//! # Comparison to Canonical Crates
//! - **tokio**: Uses a complex work-stealing thread pool, epoll/kqueue for I/O, and a highly optimized timer wheel.
//! - **Missing here**: I/O polling, work-stealing, cancellation safety, graceful shutdown.
//! - **Next Steps**: Add TCP/epoll integration to the reactor.

// PRODUCTION NOTE: To benchmark context switching overhead, measure the time between
// waking a task and the task actually executing using `std::time::Instant` and `black_box`.

use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, SyncSender, sync_channel};
use std::sync::{Arc, Mutex, OnceLock};
use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};
use std::thread;
use std::time::{Duration, Instant};

// =========================================================================================
// Reactor (Timer)
// =========================================================================================

/// Global reactor instance
static REACTOR: OnceLock<&'static TimerReactor> = OnceLock::new();

pub struct TimerReactor {
    /// BTreeMap of timers, keyed by (Instant, id) to disambiguate identical Instants.
    timers: Mutex<BTreeMap<(Instant, usize), Waker>>,
    timer_id: AtomicUsize,
}

impl TimerReactor {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            timers: Mutex::new(BTreeMap::new()),
            timer_id: AtomicUsize::new(0),
        }
    }

    /// Register a new timer for the given duration.
    pub fn register(&self, duration: Duration, waker: Waker) {
        let when = Instant::now() + duration;
        let id = self.timer_id.fetch_add(1, Ordering::Relaxed);
        let mut timers = self.timers.lock().unwrap();
        timers.insert((when, id), waker);
        // clippy::significant_drop_tightening
        drop(timers);
    }

    /// Background loop to wake expired timers.
    pub fn run_loop(&self) {
        loop {
            let now = Instant::now();
            let mut timers = self.timers.lock().unwrap();

            let mut expired = Vec::new();
            for (&key, _waker) in timers.iter() {
                if key.0 <= now {
                    expired.push(key);
                } else {
                    break;
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
    }

    /// Initialize the global reactor and start its thread.
    pub fn init() {
        if REACTOR.get().is_none() {
            let reactor = Self::new();
            let ref_reactor: &'static Self = Box::leak(Box::new(reactor));
            if REACTOR.set(ref_reactor).is_ok() {
                thread::spawn(move || {
                    ref_reactor.run_loop();
                });
            }
        }
    }
}

impl Default for TimerReactor {
    fn default() -> Self {
        Self::new()
    }
}

// =========================================================================================
// Waker (VTable Implementation)
// =========================================================================================

static VTABLE: RawWakerVTable =
    RawWakerVTable::new(clone_waker, wake_waker, wake_by_ref_waker, drop_waker);

fn clone_waker(ptr: *const ()) -> RawWaker {
    let arc = unsafe { Arc::from_raw(ptr as *const Task) };
    // RUST INSIGHT: We clone the Arc to increase the reference count, but then immediately
    // turn both back into raw pointers to avoid dropping them.
    let _ = Arc::into_raw(arc.clone());
    let ptr2 = Arc::into_raw(arc);
    RawWaker::new(ptr2 as *const (), &VTABLE)
}

fn wake_waker(ptr: *const ()) {
    let arc = unsafe { Arc::from_raw(ptr as *const Task) };
    arc.schedule();
}

fn wake_by_ref_waker(ptr: *const ()) {
    let arc = unsafe { Arc::from_raw(ptr as *const Task) };
    arc.schedule();
    let _ = Arc::into_raw(arc);
}

fn drop_waker(ptr: *const ()) {
    let _arc = unsafe { Arc::from_raw(ptr as *const Task) };
}

// =========================================================================================
// Executor & Task System
// =========================================================================================

struct Task {
    future: Mutex<Option<Pin<Box<dyn Future<Output = ()> + Send + 'static>>>>,
    task_sender: SyncSender<Arc<Task>>,
}

impl Task {
    fn schedule(self: &Arc<Self>) {
        let _ = self.task_sender.send(Arc::clone(self));
    }
}

pub struct Executor {
    task_receiver: Receiver<Arc<Task>>,
}

impl Executor {
    pub fn run(&self) {
        while let Ok(task) = self.task_receiver.recv() {
            let mut future_slot = task.future.lock().unwrap();
            if let Some(mut future) = future_slot.take() {
                let ptr = Arc::into_raw(Arc::clone(&task)) as *const ();
                let raw_waker = RawWaker::new(ptr, &VTABLE);
                // UNSAFE JUSTIFICATION: We constructed the RawWaker safely with our VTABLE,
                // which correctly manages the Arc reference count.
                let waker = unsafe { Waker::from_raw(raw_waker) };
                let mut cx = Context::from_waker(&waker);

                match future.as_mut().poll(&mut cx) {
                    Poll::Ready(()) => {}
                    Poll::Pending => {
                        *future_slot = Some(future);
                    }
                }
            }
        }
    }
}

pub struct Spawner {
    task_sender: SyncSender<Arc<Task>>,
}

impl Spawner {
    pub fn spawn<F>(&self, future: F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        let task = Arc::new(Task {
            future: Mutex::new(Some(Box::pin(future))),
            task_sender: self.task_sender.clone(),
        });
        task.schedule();
    }
}

#[must_use]
pub fn new_executor_and_spawner() -> (Executor, Spawner) {
    let (task_sender, task_receiver) = sync_channel(1000);
    (Executor { task_receiver }, Spawner { task_sender })
}

// =========================================================================================
// Sleep Future
// =========================================================================================

pub struct Sleep {
    duration: Duration,
    waker_registered: bool,
}

impl Sleep {
    #[must_use]
    pub const fn new(duration: Duration) -> Self {
        Self {
            duration,
            waker_registered: false,
        }
    }
}

impl Future for Sleep {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.waker_registered {
            Poll::Ready(())
        } else {
            self.waker_registered = true;
            // PRODUCTION NOTE: a production-grade future should store and update its waker (e.g., using `AtomicWaker` and `cx.waker().will_wake(stored_waker)`) in case it is polled by a different executor/task.
            let waker = cx.waker().clone();

            TimerReactor::init();
            let reactor = REACTOR.get().expect("Reactor should be initialized");
            reactor.register(self.duration, waker);

            Poll::Pending
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};

    #[test]
    fn test_async_runtime() {
        let (executor, spawner) = new_executor_and_spawner();
        let flag = Arc::new(AtomicBool::new(false));
        let flag_clone = Arc::clone(&flag);

        let start = Instant::now();
        spawner.spawn(async move {
            Sleep::new(Duration::from_millis(50)).await;
            flag_clone.store(true, Ordering::SeqCst);
        });

        drop(spawner);
        executor.run();

        assert!(flag.load(Ordering::SeqCst));
        assert!(start.elapsed() >= Duration::from_millis(50));
    }
}
