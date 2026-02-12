//! Minimal Async Executor Implementation
//!
//! # What this implements
//! A simple MPSC-based async executor that can poll `Future`s to completion.
//! It demonstrates the core mechanics of how runtimes like Tokio or async-std work:
//! Task construction, Waker implementation using `RawWakerVTable`, and the polling loop.
//!
//! # Replaces
//! `tokio`, `async-std`, `smol` (core runtime part)
//!
//! # Real-world usage
//! - **Tokio**: Uses a much more complex work-stealing scheduler and IO-integrated reactor.
//! - **Embedded runtimes**: Often use similar simple executors for `no_std` environments.
//!
//! # Why build it yourself?
//! To demystify `async`/`await`. Magic happens only in `poll` and `wake`.
//! Understanding `Waker` construction and `Context` is crucial for writing custom futures.

use std::future::Future;
use std::mem;
use std::pin::Pin;
use std::sync::mpsc::{Receiver, SyncSender, sync_channel};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

/// A future that is pinned to the heap and thread-safe.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// The polling loop and task queue manager.
///
/// # Architecture
///
/// ```text
/// [ Spawner ] -> (Channel) -> [ Executor ]
///                                 |
///                                 v
///                              [ Task ] <---(wake)---+
///                                 |                  |
///                                 +------------------+
/// ```
///
/// 1. `Spawner` creates a `Task` wrapping the future and sends it to the channel.
/// 2. `Executor` receives the `Task`.
/// 3. `Executor` creates a `Waker` from the `Task` (via `RawWakerVTable`).
/// 4. `Executor` polls the future with a `Context` containing the `Waker`.
/// 5. If `Poll::Pending`, the future registers the waker with some reactor (e.g., timer).
/// 6. When the event occurs, `wake()` is called, pushing the `Task` back to the channel.
pub struct Executor {
    ready_queue: Receiver<Arc<Task>>,
}

/// Spawns new tasks onto the executor.
#[derive(Clone)]
pub struct Spawner {
    task_sender: SyncSender<Arc<Task>>,
}

/// A task that can be rescheduled.
/// It holds the future and the mechanism to reschedule itself (the sender).
struct Task {
    // RUST INSIGHT:
    // We use Mutex because polling requires mutable access (`Pin<&mut Future>`),
    // but `Arc<Task>` only gives shared access. The Waker requires `Arc` (shared),
    // so we need interior mutability.
    future: Mutex<Option<BoxFuture<'static, ()>>>,

    // The channel to send this task back to the executor when woken.
    task_sender: SyncSender<Arc<Task>>,
}

impl Spawner {
    /// Spawns a future onto the executor.
    pub fn spawn(&self, future: impl Future<Output = ()> + 'static + Send) {
        let future = Box::pin(future);
        let task = Arc::new(Task {
            future: Mutex::new(Some(future)),
            task_sender: self.task_sender.clone(),
        });

        // Ignore error if executor is dropped
        let _ = self.task_sender.send(task);
    }
}

impl Executor {
    pub fn new() -> (Self, Spawner) {
        const MAX_QUEUED_TASKS: usize = 10_000;
        let (sender, receiver) = sync_channel(MAX_QUEUED_TASKS);
        (
            Executor {
                ready_queue: receiver,
            },
            Spawner {
                task_sender: sender,
            },
        )
    }

    /// Runs the executor until the queue is empty.
    /// Note: This is different from `block_on` which waits for a specific future.
    /// This runs *all* spawned tasks.
    pub fn run(&self) {
        // RUST INSIGHT:
        // We use a simple loop. In a real reactor, we would `park` the thread
        // if the queue is empty but tasks are still pending (waiting on IO).
        // Here, `recv()` blocks, which is our "parking".
        while let Ok(task) = self.ready_queue.recv() {
            // Take the future, and if it's not yet completed (Some), poll it
            let mut future_slot = task.future.lock().unwrap();

            if let Some(mut future) = future_slot.take() {
                // Create a Waker from the Arc<Task>
                let waker = waker_ref(&task);
                let context = &mut Context::from_waker(&waker);

                if future.as_mut().poll(context).is_pending() {
                    // If pending, put it back in the slot so it can be polled again later.
                    // The `Waker` we passed in will handle rescheduling `task` when ready.
                    *future_slot = Some(future);
                }
                // If Ready, we don't put it back, effectively dropping it.
            }
        }
    }
}

// --- Waker Implementation ---

// Manual VTable implementation for constructing Waker from Arc<Task>
// This is what `ArcWake` trait in `futures` crate does under the hood.

// UNSAFE JUSTIFICATION:
// We are manually implementing the RawWakerVTable to convert `*const ()` back to `Arc<Task>`.
// This requires careful handling of reference counts using `Arc::from_raw` and `ManuallyDrop`.
// The `data` pointer is guaranteed to be a valid `Arc<Task>` pointer created by `Arc::into_raw`.

unsafe fn clone_arc(data: *const ()) -> RawWaker {
    let arc = mem::ManuallyDrop::new(Arc::from_raw(data as *const Task));
    // Increase the reference count for the new RawWaker
    let arc_clone = (*arc).clone();
    let ptr = Arc::into_raw(arc_clone);
    RawWaker::new(ptr as *const (), &VTABLE)
}

unsafe fn wake_arc(data: *const ()) {
    let arc = Arc::from_raw(data as *const Task);
    let sender = arc.task_sender.clone();
    // Send the task back to the executor
    let _ = sender.send(arc);
}

unsafe fn wake_by_ref_arc(data: *const ()) {
    // Clone the arc without consuming the raw pointer
    let arc = mem::ManuallyDrop::new(Arc::from_raw(data as *const Task));
    let _ = arc.task_sender.send(Arc::clone(&arc));
}

unsafe fn drop_arc(data: *const ()) {
    // Decrease ref count
    drop(Arc::from_raw(data as *const Task));
}

const VTABLE: RawWakerVTable = RawWakerVTable::new(clone_arc, wake_arc, wake_by_ref_arc, drop_arc);

fn waker_ref(task: &Arc<Task>) -> Waker {
    let ptr = Arc::into_raw(task.clone());
    let raw = RawWaker::new(ptr as *const (), &VTABLE);
    unsafe { Waker::from_raw(raw) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_simple_future() {
        let (executor, spawner) = Executor::new();
        let counter = Arc::new(AtomicUsize::new(0));
        let c = counter.clone();

        spawner.spawn(async move {
            c.fetch_add(1, Ordering::SeqCst);
        });

        // Drop spawner to allow executor to finish when queue is empty
        drop(spawner);
        executor.run();

        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_spawn_multiple() {
        let (executor, spawner) = Executor::new();
        let counter = Arc::new(AtomicUsize::new(0));

        for _ in 0..10 {
            let c = counter.clone();
            spawner.spawn(async move {
                c.fetch_add(1, Ordering::SeqCst);
            });
        }

        drop(spawner);
        executor.run();

        assert_eq!(counter.load(Ordering::SeqCst), 10);
    }

    // A mock timer future that yields once then completes
    struct YieldFuture {
        yielded: bool,
    }

    impl Future for YieldFuture {
        type Output = ();
        fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
            if self.yielded {
                Poll::Ready(())
            } else {
                self.yielded = true;
                cx.waker().wake_by_ref(); // Wake ourselves immediately
                Poll::Pending
            }
        }
    }

    #[test]
    fn test_yield() {
        let (executor, spawner) = Executor::new();
        let done = Arc::new(AtomicUsize::new(0));
        let d = done.clone();

        spawner.spawn(async move {
            YieldFuture { yielded: false }.await;
            d.store(1, Ordering::SeqCst);
        });

        drop(spawner);
        executor.run();

        assert_eq!(done.load(Ordering::SeqCst), 1);
    }
}
