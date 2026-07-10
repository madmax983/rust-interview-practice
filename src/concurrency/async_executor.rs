//! # Minimal Async Executor Implementation
//!
//! Implements a cooperative multitasking runtime for executing asynchronous tasks (Futures).
//! It demonstrates the core mechanics of Rust's async/await model: polling, waking, and task scheduling.
//!
//! **Replaces Crates:** `tokio` (runtime), `async-std`
//!
//! **Real-world Usage:**
//! - High-concurrency network servers (web servers, database proxies).
//! - UI event loops.
//! - Embedded systems with cooperative multitasking.
//!
//! **Why build it yourself?**
//! Understanding executors demystifies "magic" like `#[tokio::main]` and `async/await`.
//! You learn how:
//! - Futures are lazy state machines that must be polled to advance.
//! - `Waker` tells the runtime *when* to poll a future again.
//! - Thread-local storage or explicit context passing manages the current task.

use std::future::Future;
use std::mem::ManuallyDrop;
use std::pin::Pin;
use std::sync::mpsc::{Receiver, SyncSender, sync_channel};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};
use std::thread;
use std::time::Duration;

// =========================================================================================
// Architecture
// =========================================================================================
//
// Flow:
//
//      Client
//        │
//        ▼
//    Spawner::spawn(future)
//        │
//        ▼
//    [SyncSender] ──► [Task Queue] ──► [Executor (Receiver)]
//                                            │
//                                            ▼
//                                        pop task
//                                            │
//                                            ▼
//                                        task.poll() ──► [Future::poll]
//                                            │               │
//                                            │          (Returns Poll::Pending)
//                                            ▼               │
//                                        [Waker] ◄───────────┘
//                                            │
//                                            ▼
//                                   wake() pushes task back
//                                     to [Task Queue]
//
//
// Invariants:
// 1. A task is only polled when it is in the `Executor`'s ready queue.
// 2. Waking a task puts it back into the ready queue.
// 3. Futures must be `Send` to be moved into the task queue.
//
// Complexity:
// ┌───────────────┬─────────────┬─────────────┐
// │ Operation     │ Time        │ Space       │
// ├───────────────┼─────────────┼─────────────┤
// │ Spawn         │ O(1)        │ O(1)        │
// ├───────────────┼─────────────┼─────────────┤
// │ Poll Task     │ O(1)*       │ O(1)        │
// └───────────────┴─────────────┴─────────────┘
// * Depends on the Future's implementation.

/// A future that can be boxed and pinned.
type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// The Task struct represents a unit of work.
struct Task {
    /// The future representing the computation.
    future: Mutex<Option<BoxFuture<'static, ()>>>,

    /// The channel sender to put this task back into the ready queue when woken.
    task_sender: SyncSender<Arc<Self>>,
}

/// Spawner spawns new tasks onto the executor.
#[derive(Clone)]
pub struct Spawner {
    task_sender: SyncSender<Arc<Task>>,
}

impl Spawner {
    pub fn spawn(&self, future: impl Future<Output = ()> + Send + 'static) {
        let future = Box::pin(future);
        let task = Arc::new(Task {
            future: Mutex::new(Some(future)),
            task_sender: self.task_sender.clone(),
        });
        self.task_sender.send(task).expect("too many tasks queued");
    }
}

/// Executor runs tasks from the queue.
pub struct Executor {
    ready_queue: Receiver<Arc<Task>>,
}

impl Executor {
    pub fn run(&self) {
        while let Ok(task) = self.ready_queue.recv() {
            let mut future_slot = task.future.lock().unwrap();
            if let Some(mut future) = future_slot.take() {
                let waker = waker_ref(&task);
                let context = &mut Context::from_waker(&waker);

                match future.as_mut().poll(context) {
                    Poll::Pending => {
                        *future_slot = Some(future);
                    }
                    Poll::Ready(()) => {
                        // Future complete.
                    }
                }
            }
        }
    }
}

#[must_use] 
pub fn new_executor_and_spawner() -> (Executor, Spawner) {
    let (task_sender, ready_queue) = sync_channel(10_000);
    (Executor { ready_queue }, Spawner { task_sender })
}

// =========================================================================================
// Waker Implementation
// =========================================================================================

fn waker_ref(task: &Arc<Task>) -> Waker {
    let raw = arc_to_raw_waker(task.clone());
    unsafe { Waker::from_raw(raw) }
}

fn arc_to_raw_waker(task: Arc<Task>) -> RawWaker {
    let raw_ptr = Arc::into_raw(task).cast::<()>();
    RawWaker::new(raw_ptr, &VTABLE)
}

const VTABLE: RawWakerVTable =
    RawWakerVTable::new(task_clone, task_wake, task_wake_by_ref, task_drop);

unsafe fn task_clone(raw_ptr: *const ()) -> RawWaker {
    let ptr = raw_ptr.cast::<Task>();
    let arc = ManuallyDrop::new(unsafe { Arc::from_raw(ptr) });
    // We want a new Arc that owns a reference count.
    // (*arc) gives us &Arc<Task>.
    // .clone() on &Arc<Task> returns a new Arc<Task>.
    let cloned: Arc<Task> = (*arc).clone();
    arc_to_raw_waker(cloned)
}

unsafe fn task_wake(raw_ptr: *const ()) {
    let ptr = raw_ptr.cast::<Task>();
    let task = unsafe { Arc::from_raw(ptr) }; // Take ownership
    let sender = task.task_sender.clone();
    let _ = sender.send(task);
}

unsafe fn task_wake_by_ref(raw_ptr: *const ()) {
    let ptr = raw_ptr.cast::<Task>();
    let arc = ManuallyDrop::new(unsafe { Arc::from_raw(ptr) });
    let task_to_send: Arc<Task> = (*arc).clone();
    let sender = task_to_send.task_sender.clone();
    let _ = sender.send(task_to_send);
}

unsafe fn task_drop(raw_ptr: *const ()) {
    let ptr = raw_ptr.cast::<Task>();
    drop(unsafe { Arc::from_raw(ptr) });
}

// =========================================================================================
// Example: Timer Future
// =========================================================================================

pub struct TimerFuture {
    shared_state: Arc<Mutex<SharedState>>,
}

struct SharedState {
    completed: bool,
    waker: Option<Waker>,
}

impl Future for TimerFuture {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut shared_state = self.shared_state.lock().unwrap();
        if shared_state.completed {
            Poll::Ready(())
        } else {
            shared_state.waker = Some(cx.waker().clone());
            Poll::Pending
        }
    }
}

impl TimerFuture {
    #[must_use] 
    pub fn new(duration: Duration) -> Self {
        let shared_state = Arc::new(Mutex::new(SharedState {
            completed: false,
            waker: None,
        }));

        let thread_shared_state = shared_state.clone();
        thread::spawn(move || {
            thread::sleep(duration);
            let mut shared_state = thread_shared_state.lock().unwrap();
            shared_state.completed = true;
            if let Some(waker) = shared_state.waker.take() {
                waker.wake();
            }
        });

        Self { shared_state }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn test_simple_task() {
        let (executor, spawner) = new_executor_and_spawner();

        let executed = Arc::new(AtomicUsize::new(0));
        let executed_clone = executed.clone();

        spawner.spawn(async move {
            executed_clone.fetch_add(1, Ordering::SeqCst);
        });

        drop(spawner);
        executor.run();

        assert_eq!(executed.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_timer() {
        let (executor, spawner) = new_executor_and_spawner();
        let start = std::time::Instant::now();

        spawner.spawn(async move {
            TimerFuture::new(Duration::from_millis(50)).await;
        });

        drop(spawner);
        executor.run();

        assert!(start.elapsed() >= Duration::from_millis(50));
    }

    #[test]
    fn test_multiple_tasks() {
        let (executor, spawner) = new_executor_and_spawner();
        let counter = Arc::new(AtomicUsize::new(0));

        for _ in 0..5 {
            let c = counter.clone();
            spawner.spawn(async move {
                c.fetch_add(1, Ordering::SeqCst);
            });
        }

        drop(spawner);
        executor.run();

        assert_eq!(counter.load(Ordering::SeqCst), 5);
    }
}
