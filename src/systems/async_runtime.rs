//! # Async Runtime
//!
//! Difficulty: Hard
//!
//! Why this matters in Rust: Rust's async/await is famously "bring your own runtime".
//! This module demonstrates a minimal asynchronous executor, spawner, and timer-based reactor
//! from scratch, demystifying crates like `tokio` and `async-std`.
//!
//! It shows how to use `Future`, `Waker`, and `Context`, as well as how a reactor wakes
//! up sleeping tasks.

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

/// A task is a boxed future that can send itself back to the executor when woken.
struct Task {
    future: Mutex<Option<Pin<Box<dyn Future<Output = ()> + Send + 'static>>>>,
    task_sender: SyncSender<Arc<Task>>,
}

impl std::task::Wake for Task {
    fn wake(self: Arc<Self>) {
        // RUST INSIGHT: When the waker is invoked, it sends the Arc<Task> back to the channel.
        // This is how the executor knows the task is ready to make progress.
        let _ = self.task_sender.send(self.clone());
    }
}

/// Spawner for scheduling new tasks onto the executor.
#[derive(Clone)]
pub struct Spawner {
    task_sender: SyncSender<Arc<Task>>,
}

impl Spawner {
    /// Spawns a new asynchronous task.
    pub fn spawn(&self, future: impl Future<Output = ()> + Send + 'static) {
        let future = Box::pin(future);
        let task = Arc::new(Task {
            future: Mutex::new(Some(future)),
            task_sender: self.task_sender.clone(),
        });
        let _ = self.task_sender.send(task);
    }
}

/// The Executor pulls tasks off the channel and polls them.
pub struct Executor {
    ready_queue: Receiver<Arc<Task>>,
}

impl Executor {
    /// Runs the executor, polling tasks until the queue is empty.
    /// In a real runtime, this would block and wait for new tasks.
    pub fn run(&self) {
        while let Ok(task) = self.ready_queue.try_recv() {
            let mut future_slot = task.future.lock().unwrap();
            if let Some(mut future) = future_slot.take() {
                // Create a Waker from the Arc<Task> using the Wake trait.
                let waker = Waker::from(task.clone());
                let mut context = Context::from_waker(&waker);

                // Poll the future
                if future.as_mut().poll(&mut context).is_pending() {
                    // If it's still pending, put it back in its slot
                    *future_slot = Some(future);
                }
            }
        }
    }
}

/// Creates a new executor and a spawner linked to it.
#[must_use]
pub fn new_executor_and_spawner() -> (Executor, Spawner) {
    // A channel with capacity to hold up to 10,000 tasks simultaneously
    let (task_sender, ready_queue) = sync_channel(10000);
    (Executor { ready_queue }, Spawner { task_sender })
}

// --- Timer Reactor ---

/// A simple timer-based reactor that wakes tasks when their deadline is reached.
struct TimerReactor {
    // We use a BTreeMap ordered by Instant to efficiently find expired timers.
    // The tuple contains an atomic counter to disambiguate identical Instants.
    timers: Mutex<BTreeMap<(Instant, usize), Waker>>,
    counter: AtomicUsize,
}

impl TimerReactor {
    fn new() -> Self {
        Self {
            timers: Mutex::new(BTreeMap::new()),
            counter: AtomicUsize::new(0),
        }
    }

    /// Registers a waker to be called at a specific instant.
    fn register_timer(&self, at: Instant, waker: Waker) {
        let id = self.counter.fetch_add(1, Ordering::SeqCst);
        self.timers.lock().unwrap().insert((at, id), waker);
    }
}

static REACTOR: OnceLock<&'static TimerReactor> = OnceLock::new();

/// Starts the global background reactor thread.
fn start_reactor() {
    let _reactor = REACTOR.get_or_init(|| {
        let r = Box::new(TimerReactor::new());
        let ref_r: &'static TimerReactor = Box::leak(r);

        // Spawn the reactor thread that checks for expired timers
        thread::spawn(move || {
            loop {
                let now = Instant::now();

                let mut wakers = Vec::new();
                {
                    let mut timers = ref_r.timers.lock().unwrap();
                    let keys: Vec<_> = timers
                        .keys()
                        .take_while(|&(k, _)| *k <= now)
                        .copied()
                        .collect();
                    for k in keys {
                        if let Some(waker) = timers.remove(&k) {
                            wakers.push(waker);
                        }
                    }
                }

                for waker in wakers {
                    waker.wake();
                }

                thread::sleep(Duration::from_millis(10));
            }
        });

        ref_r
    });
}

/// A Future that resolves after a given duration.
pub struct Sleep {
    deadline: Instant,
    registered: bool,
}

impl Sleep {
    #[must_use]
    pub fn new(duration: Duration) -> Self {
        start_reactor();
        Self {
            deadline: Instant::now() + duration,
            registered: false,
        }
    }
}

impl Future for Sleep {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if Instant::now() >= self.deadline {
            Poll::Ready(())
        } else {
            if !self.registered {
                let reactor = REACTOR.get().expect("Reactor should be initialized");
                reactor.register_timer(self.deadline, cx.waker().clone());
                self.registered = true;
            }
            Poll::Pending
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};

    #[test]
    fn test_executor_basic() {
        let (executor, spawner) = new_executor_and_spawner();
        let flag = Arc::new(AtomicBool::new(false));
        let flag_clone = flag.clone();

        spawner.spawn(async move {
            flag_clone.store(true, Ordering::SeqCst);
        });

        executor.run();
        assert!(flag.load(Ordering::SeqCst));
    }

    #[test]
    fn test_executor_with_sleep() {
        let (executor, spawner) = new_executor_and_spawner();
        let flag = Arc::new(AtomicBool::new(false));
        let flag_clone = flag.clone();

        spawner.spawn(async move {
            Sleep::new(Duration::from_millis(50)).await;
            flag_clone.store(true, Ordering::SeqCst);
        });

        // The executor run() method here is non-blocking (try_recv).
        // Since the future goes to pending and is woken by the reactor thread,
        // we need a simple loop in the test to keep polling the executor.
        let start = Instant::now();
        while !flag.load(Ordering::SeqCst) && start.elapsed() < Duration::from_millis(500) {
            executor.run();
            thread::sleep(Duration::from_millis(10));
        }

        assert!(flag.load(Ordering::SeqCst));
    }
}
