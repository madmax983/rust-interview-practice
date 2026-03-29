//! # Event Loop Implementation
//!
//! A single-threaded event loop for cooperative multitasking and I/O polling simulation.
//!
//! **Replaces Crates:** `mio`, `tokio` (runtime core), `glommio`
//!
//! **Real-world Usage:**
//! - Asynchronous network servers (e.g., Nginx, Redis, Node.js).
//! - GUI frameworks processing UI events.
//! - High-performance thread-per-core architectures.
//!
//! **Why build it yourself?**
//! Understanding an event loop demystifies how a single thread can handle thousands
//! of concurrent connections. It bridges the gap between hardware interrupts/OS events
//! (`epoll`/`kqueue`) and application-level callbacks or futures. You'll learn about
//! non-blocking I/O, readiness polling, and how the "Reactor" pattern works.

use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

// =========================================================================================
// Architecture
// =========================================================================================
//
// Components:
// 1. **EventLoop**: The core engine. It manages tasks, timers, and an I/O selector.
// 2. **Selector**: An abstraction over OS-level event notification (like `epoll` or `kqueue`).
//    Here, we simulate it to remain cross-platform and self-contained without `unsafe` OS calls.
// 3. **Tasks**: Closures or state machines waiting to be executed when an event occurs.
//
// Flow:
//
//      [Submit Task/Timer] ──┐
//                            ▼
//                     [Task Queue]
//                            │
//      [OS I/O Events] ──► [Selector] ──► [Ready Events]
//                                                │
//                                                ▼
//                                        [Execute Callbacks]
//
// The Loop (`run`):
//   1. Execute all immediately runnable tasks.
//   2. Check and execute expired timers.
//   3. Calculate sleep time (time until next timer).
//   4. Block on the `Selector` until an I/O event happens or sleep time expires.
//   5. Enqueue tasks for the ready I/O events.
//   6. Repeat.
//
// Invariants:
// 1. Tasks must not block the thread (cooperative multitasking).
// 2. The loop only sleeps if the task queue is empty.
//
// Complexity:
// ┌───────────────┬─────────────┬─────────────┐
// │ Operation     │ Time        │ Space       │
// ├───────────────┼─────────────┼─────────────┤
// │ Add Task      │ O(1)        │ O(1)        │
// │ Add Timer     │ O(log N)*   │ O(N)        │
// │ Poll I/O      │ O(K)**      │ O(K)        │
// └───────────────┴─────────────┴─────────────┘
// * With a BinaryHeap (not fully implemented here for simplicity; using a sorted Vec or O(N) scan).
// ** K is the number of ready events returned by the OS.
//
// Design Decisions:
// - **Timers**: We use a simple `Vec` and O(N) scan for expired timers to keep code concise.
//   - *Production*: Uses a `BinaryHeap` or a Hashed Wheel Timer for O(1) expiration checks.
// - **Selector**: Simulated using time and explicit readiness toggles.
//   - *Production*: Uses `libc::epoll_wait` (Linux) or `libc::kevent` (macOS).

pub type TaskId = usize;
pub type Fd = i32; // File Descriptor

/// Represents an I/O event type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Interest {
    Readable,
    Writable,
}

/// Simulated Selector for I/O events.
/// In a real system, this would wrap `epoll` or `kqueue`.
struct SimulatedSelector {
    registered: HashMap<Fd, Interest>,
    ready: Vec<(Fd, Interest)>,
}

impl SimulatedSelector {
    fn new() -> Self {
        Self {
            registered: HashMap::new(),
            ready: Vec::new(),
        }
    }

    fn register(&mut self, fd: Fd, interest: Interest) {
        self.registered.insert(fd, interest);
    }

    fn unregister(&mut self, fd: Fd) {
        self.registered.remove(&fd);
        self.ready.retain(|(ready_fd, _)| *ready_fd != fd);
    }

    // Simulate an external event making a file descriptor ready
    fn simulate_ready(&mut self, fd: Fd, interest: Interest) {
        if self.registered.get(&fd) == Some(&interest) {
            self.ready.push((fd, interest));
        }
    }

    /// Blocks until events are ready or the timeout expires.
    fn select(&mut self, timeout: Option<Duration>) -> Vec<(Fd, Interest)> {
        // If we have ready events, return them immediately
        if !self.ready.is_empty() {
            let events = self.ready.clone();
            self.ready.clear();
            return events;
        }

        // If no events and we have a timeout, we simulate blocking by sleeping.
        // In a real selector, the OS wakes us up early if an event arrives.
        // Because this is a simulation and no other thread is mutating `self.ready`
        // (we are single-threaded without signals), we just sleep the full timeout.
        if let Some(t) = timeout {
            std::thread::sleep(t);
        } else {
            // RUST INSIGHT: Deadlock prevention in single-threaded simulation.
            // If there's no timeout and no events are ready, a real epoll_wait would block forever.
            // Since we don't have another thread injecting events, we must panic to avoid hanging tests,
            // or just yield. We'll simulate a 10ms block to prevent 100% CPU spin if called in a loop.
            std::thread::sleep(Duration::from_millis(10));
        }

        // Return whatever is ready (likely nothing in this simulation unless populated before)
        let events = self.ready.clone();
        self.ready.clear();
        events
    }
}

/// A timer that triggers a callback when it expires.
struct Timer {
    expires_at: Instant,
    callback: Box<dyn FnOnce()>,
}

pub struct EventLoop {
    tasks: VecDeque<Box<dyn FnOnce()>>,
    timers: Vec<Timer>,
    selector: SimulatedSelector,
    io_callbacks: HashMap<(Fd, Interest), Box<dyn FnMut()>>,
    running: bool,
}

impl EventLoop {
    #[must_use]
    pub fn new() -> Self {
        Self {
            tasks: VecDeque::new(),
            timers: Vec::new(),
            selector: SimulatedSelector::new(),
            io_callbacks: HashMap::new(),
            running: true,
        }
    }

    /// Spawns a task to be executed immediately in the next iteration.
    pub fn spawn<F>(&mut self, f: F)
    where
        F: FnOnce() + 'static,
    {
        self.tasks.push_back(Box::new(f));
    }

    /// Schedules a task to be executed after a certain duration.
    pub fn set_timeout<F>(&mut self, duration: Duration, f: F)
    where
        F: FnOnce() + 'static,
    {
        let expires_at = Instant::now() + duration;
        self.timers.push(Timer {
            expires_at,
            callback: Box::new(f),
        });
    }

    /// Registers a callback to be executed when the specified file descriptor
    /// becomes ready for the given interest.
    pub fn register_io<F>(&mut self, fd: Fd, interest: Interest, f: F)
    where
        F: FnMut() + 'static,
    {
        self.selector.register(fd, interest);
        self.io_callbacks.insert((fd, interest), Box::new(f));
    }

    /// Deregisters an I/O callback.
    pub fn unregister_io(&mut self, fd: Fd, interest: Interest) {
        self.selector.unregister(fd);
        self.io_callbacks.remove(&(fd, interest));
    }

    /// RUST INSIGHT: Exposing internal state for testing/simulation.
    /// In a real loop, you wouldn't trigger events manually.
    pub fn simulate_io_event(&mut self, fd: Fd, interest: Interest) {
        self.selector.simulate_ready(fd, interest);
    }

    /// Stops the event loop.
    pub fn stop(&mut self) {
        self.running = false;
    }

    /// Runs the event loop until `stop()` is called or no tasks/timers/io are pending.
    pub fn run(&mut self) {
        while self.running {
            let has_tasks = !self.tasks.is_empty();
            let has_timers = !self.timers.is_empty();
            let has_io = !self.io_callbacks.is_empty();

            if !has_tasks && !has_timers && !has_io {
                break; // Nothing left to do
            }

            // 1. Execute all ready tasks
            // GOTCHA: We only process the tasks currently in the queue.
            // If a task spawns another task, it goes to the back and is processed
            // in the *next* tick, preventing starvation of I/O and timers.
            let tasks_to_run = self.tasks.len();
            for _ in 0..tasks_to_run {
                if let Some(task) = self.tasks.pop_front() {
                    task();
                }
            }

            // 2. Process expired timers
            let now = Instant::now();
            // Partition timers: expired ones go to the end
            let mut i = 0;
            while i < self.timers.len() {
                if self.timers[i].expires_at <= now {
                    let timer = self.timers.swap_remove(i);
                    // RUST INSIGHT: We execute the callback immediately here.
                    // Alternatively, we could push it to `self.tasks`.
                    (timer.callback)();
                } else {
                    i += 1;
                }
            }

            // If we stopped during task/timer execution, break early.
            if !self.running {
                break;
            }

            // 3. Calculate sleep timeout for the Selector
            // We only sleep if there are no immediate tasks queued.
            let timeout = if !self.tasks.is_empty() {
                Some(Duration::from_millis(0))
            } else {
                // Find the nearest timer
                self.timers
                    .iter()
                    .map(|t| t.expires_at.saturating_duration_since(Instant::now()))
                    .min()
            };

            // 4. Poll I/O
            // If there are no I/O callbacks, and we have tasks, we don't block.
            // If there's nothing at all, we break (handled at start of loop).
            if !self.io_callbacks.is_empty() {
                let ready_events = self.selector.select(timeout);

                // 5. Execute I/O callbacks
                for (fd, interest) in ready_events {
                    if let Some(callback) = self.io_callbacks.get_mut(&(fd, interest)) {
                        callback();
                    }
                }
            } else if let Some(t) = timeout {
                // If we only have timers, just sleep for the timeout duration
                std::thread::sleep(t);
            } else {
                // No IO callbacks, no tasks, no timers -> we should have broken out at the start.
                // But just in case, prevent 100% CPU usage.
                std::thread::yield_now();
            }
        }
    }
}

impl Default for EventLoop {
    fn default() -> Self {
        Self::new()
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `mio`: Provides the actual low-level bindings to `epoll`/`kqueue`/`IOCP` with minimal overhead.
// - `tokio`: A massive, multi-threaded runtime that builds on `mio` to provide an executor for `Future`s.
//   It uses a work-stealing scheduler and complex reactor patterns.
//
// Missing vs. Production:
// - **Real I/O**: We simulate `epoll`. Real implementations use `libc` calls and unsafe blocks.
// - **Futures Integration**: This loop takes closures. Real runtimes use `Waker` and `Poll` to drive `Future`s.
// - **Efficient Timers**: We do a linear scan of timers. Production uses Time Wheels or Min-Heaps.
//
// Next Steps:
// 1. Integrate with `src/concurrency/async_executor.rs` to create a fully functioning async runtime.
// 2. Replace `SimulatedSelector` with real `mio::Poll`.
//
// Benchmarking Note:
// Use `criterion` to measure task dispatch latency and timer precision.
// Real event loops process millions of events per second.
// Benchmark the overhead of `select()` vs immediate task execution.
// Example: `std::hint::black_box(loop_engine.run())` with pre-scheduled tasks.

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    #[test]
    fn test_spawn_execution() {
        let mut loop_engine = EventLoop::new();
        let counter = Rc::new(RefCell::new(0));

        let c1 = counter.clone();
        loop_engine.spawn(move || {
            *c1.borrow_mut() += 1;
        });

        let c2 = counter.clone();
        loop_engine.spawn(move || {
            *c2.borrow_mut() += 2;
        });

        loop_engine.run();
        assert_eq!(*counter.borrow(), 3);
    }

    #[test]
    fn test_set_timeout() {
        let mut loop_engine = EventLoop::new();
        let counter = Rc::new(RefCell::new(0));
        let c = counter.clone();

        loop_engine.set_timeout(Duration::from_millis(50), move || {
            *c.borrow_mut() = 42;
        });

        assert_eq!(*counter.borrow(), 0);
        loop_engine.run();
        assert_eq!(*counter.borrow(), 42);
    }

    #[test]
    fn test_io_simulation() {
        let mut loop_engine = EventLoop::new();
        let counter = Rc::new(RefCell::new(0));
        let c = counter.clone();

        loop_engine.register_io(1, Interest::Readable, move || {
            *c.borrow_mut() += 1;
        });

        // Simulate an event arriving before we run the loop.
        loop_engine.simulate_io_event(1, Interest::Readable);

        // Schedule a timer that will unregister the I/O to let the loop terminate.
        // Because we cannot capture `loop_engine` directly, we cheat by using a shared flag
        // and manually stepping through a single iteration of logic to prove it works.
        // In this architecture, it's difficult to mutate the engine from its own callbacks without Rc<RefCell>.

        // Run a single manual iteration to verify I/O executes.
        let ready = loop_engine.selector.select(Some(Duration::from_millis(0)));
        assert_eq!(ready.len(), 1);

        for (fd, interest) in ready {
            if let Some(callback) = loop_engine.io_callbacks.get_mut(&(fd, interest)) {
                callback();
            }
        }

        assert_eq!(*counter.borrow(), 1);
    }

    #[test]
    fn test_execution_order() {
        let mut loop_engine = EventLoop::new();
        let log = Rc::new(RefCell::new(Vec::new()));

        let l1 = log.clone();
        loop_engine.spawn(move || {
            l1.borrow_mut().push("task1");
        });

        let l2 = log.clone();
        loop_engine.set_timeout(Duration::from_millis(10), move || {
            l2.borrow_mut().push("timer1");
        });

        let l3 = log.clone();
        loop_engine.spawn(move || {
            l3.borrow_mut().push("task2");
        });

        loop_engine.run();

        let l = log.borrow();
        assert_eq!(l.len(), 3);
        assert_eq!(l[0], "task1");
        assert_eq!(l[1], "task2");
        assert_eq!(l[2], "timer1");
    }
}
