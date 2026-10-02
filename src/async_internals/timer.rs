//! # Timer Drill
//!
//! How `sleep` works inside a runtime: a [`Sleep`] future registers
//! `(deadline, waker)` with a timer; the runtime, when it has no ready tasks, jumps
//! (or blocks) until the earliest deadline, fires every due timer by waking its
//! waker, and goes back to polling the ready queue.
//!
//! The clock here is **virtual**: [`Timer::advance`] moves time forward explicitly, which
//! makes every test deterministic and instant. Swap the virtual clock for
//! `std::time::Instant` plus `thread::park_timeout` and you have a real runtime timer.
//!
//! ## Examples
//!
//! ```
//! use std::time::Duration;
//! use rust_interview_practice::async_internals::ready_queue::Executor;
//! use rust_interview_practice::async_internals::timer::{Timer, run_with_timer};
//!
//! let executor = Executor::new();
//! let timer = Timer::new();
//! let t = timer.clone();
//! let handle = executor.spawn(async move {
//!     t.sleep(Duration::from_millis(250)).await;
//!     t.now()
//! });
//! run_with_timer(&executor, &timer);
//! assert_eq!(handle.try_take().unwrap().unwrap().as_millis(), 250);
//! ```
//!
//! ## Design
//!
//! ```text
//!   heap:    BinaryHeap<Reverse<(deadline, TimerId)>>   earliest deadline on top
//!   entries: HashMap<TimerId, Waker>                     live registrations only
//!
//!   Sleep::poll   now >= deadline ? Ready : register/refresh waker, Pending
//!   Sleep::drop   entries.remove(id)            (heap entry becomes a tombstone)
//!   advance_to(t) pop heap while top <= t; wake entries still present
//! ```
//!
//! Cancelling a `Sleep` is O(1) (remove from the map); tombstones are skipped lazily
//! when they reach the top of the heap. Ties fire in registration order.
//!
//! | Operation      | Time            | Space |
//! |----------------|-----------------|-------|
//! | register       | O(log n)        | O(1)  |
//! | cancel (drop)  | O(1)            | O(0)  |
//! | fire k timers  | O((k + t) log n)| O(k)  |
//!
//! `n` = heap size, `t` = tombstones popped along the way.

use std::cell::RefCell;
use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::task::{Context, Poll, Waker};
use std::time::Duration;

use super::ready_queue::Executor;

/// A point on the virtual clock, in milliseconds since the timer was created.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Instant(u64);

impl Instant {
    /// Milliseconds since the timer's epoch.
    #[must_use]
    pub const fn as_millis(self) -> u64 {
        self.0
    }

    /// `self + duration`, saturating at the end of time.
    #[must_use]
    pub fn saturating_add(self, duration: Duration) -> Self {
        let millis = u64::try_from(duration.as_millis()).unwrap_or(u64::MAX);
        Self(self.0.saturating_add(millis))
    }
}

/// Registration id; monotonically increasing so equal deadlines fire FIFO.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct TimerId(u64);

#[derive(Default)]
struct TimerState {
    now: Instant,
    next_id: u64,
    heap: BinaryHeap<Reverse<(Instant, TimerId)>>,
    entries: HashMap<TimerId, Waker>,
}

impl TimerState {
    /// Pushes `deadline` onto the heap; the caller inserts the waker into `entries`.
    fn schedule(&mut self, deadline: Instant) -> TimerId {
        let id = TimerId(self.next_id);
        self.next_id += 1;
        self.heap.push(Reverse((deadline, id)));
        id
    }

    /// Drops tombstones from the top of the heap and returns the earliest live deadline.
    fn next_deadline(&mut self) -> Option<Instant> {
        while let Some(&Reverse((deadline, id))) = self.heap.peek() {
            if self.entries.contains_key(&id) {
                return Some(deadline);
            }
            self.heap.pop();
        }
        None
    }
}

/// Handle to a shared virtual-clock timer. Clones share the same clock.
#[derive(Clone, Default)]
pub struct Timer {
    state: Rc<RefCell<TimerState>>,
}

impl fmt::Debug for Timer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let state = self.state.borrow();
        f.debug_struct("Timer")
            .field("now", &state.now)
            .field("pending", &state.entries.len())
            .finish_non_exhaustive()
    }
}

impl Timer {
    /// Creates a timer whose clock starts at `Instant(0)`.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Current virtual time.
    #[must_use]
    pub fn now(&self) -> Instant {
        self.state.borrow().now
    }

    /// Number of live (not fired, not cancelled) registrations.
    #[must_use]
    pub fn pending_timers(&self) -> usize {
        self.state.borrow().entries.len()
    }

    /// Earliest deadline among live registrations.
    #[must_use]
    pub fn next_deadline(&self) -> Option<Instant> {
        self.state.borrow_mut().next_deadline()
    }

    /// A future that completes once the clock reaches `now + duration`.
    pub fn sleep(&self, duration: Duration) -> Sleep {
        let deadline = self.now().saturating_add(duration);
        self.sleep_until(deadline)
    }

    /// A future that completes once the clock reaches `deadline`.
    pub fn sleep_until(&self, deadline: Instant) -> Sleep {
        Sleep {
            timer: self.clone(),
            deadline,
            id: None,
        }
    }

    /// Moves the clock forward by `duration` and fires due timers.
    #[allow(clippy::must_use_candidate)] // side-effecting; the count is informational
    pub fn advance(&self, duration: Duration) -> usize {
        let target = self.now().saturating_add(duration);
        self.advance_to(target)
    }

    /// Moves the clock to `target` (never backwards) and wakes every timer whose
    /// deadline is `<= target`, in `(deadline, registration)` order. Returns how many
    /// timers fired.
    #[allow(clippy::must_use_candidate)] // side-effecting; the count is informational
    pub fn advance_to(&self, target: Instant) -> usize {
        let due = {
            let mut state = self.state.borrow_mut();
            state.now = state.now.max(target);
            let now = state.now;
            let mut due = Vec::new();
            while let Some(&Reverse((deadline, id))) = state.heap.peek() {
                if deadline > now {
                    break;
                }
                state.heap.pop();
                if let Some(waker) = state.entries.remove(&id) {
                    due.push(waker);
                }
            }
            due
        };
        // Wake outside the borrow: a waker may poll or register timers re-entrantly.
        let fired = due.len();
        for waker in due {
            waker.wake();
        }
        fired
    }
}

/// Future returned by [`Timer::sleep`]. Registers lazily on first poll and
/// deregisters on drop.
#[must_use = "futures do nothing unless polled"]
pub struct Sleep {
    timer: Timer,
    deadline: Instant,
    id: Option<TimerId>,
}

impl fmt::Debug for Sleep {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Sleep")
            .field("deadline", &self.deadline)
            .field("registered", &self.id.is_some())
            .finish_non_exhaustive()
    }
}

impl Sleep {
    /// When this sleep completes.
    #[must_use]
    pub const fn deadline(&self) -> Instant {
        self.deadline
    }

    /// Whether the deadline has passed on the timer's clock.
    #[must_use]
    pub fn is_elapsed(&self) -> bool {
        self.timer.now() >= self.deadline
    }
}

impl Future for Sleep {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        if self.is_elapsed() {
            if let Some(id) = self.id.take() {
                self.timer.state.borrow_mut().entries.remove(&id);
            }
            return Poll::Ready(());
        }
        let this = self.get_mut();
        let mut state = this.timer.state.borrow_mut();
        let id = *this.id.get_or_insert_with(|| state.schedule(this.deadline));
        // First poll inserts; re-polls (possibly from a different task) refresh the waker.
        // An entry is only removed when it fires, and then `is_elapsed` returned above.
        state
            .entries
            .entry(id)
            .and_modify(|stored| {
                if !stored.will_wake(cx.waker()) {
                    stored.clone_from(cx.waker());
                }
            })
            .or_insert_with(|| cx.waker().clone());
        drop(state);
        Poll::Pending
    }
}

impl Drop for Sleep {
    fn drop(&mut self) {
        if let Some(id) = self.id.take() {
            // Cancellation: O(1) removal; the heap entry is now a tombstone.
            self.timer.state.borrow_mut().entries.remove(&id);
        }
    }
}

/// Outcome of [`run_with_timer`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RunReport {
    /// Total task polls performed.
    pub polls: usize,
    /// Number of times the clock jumped to the next deadline.
    pub clock_jumps: usize,
    /// Virtual time when the run went idle.
    pub finished_at: Instant,
}

/// The runtime driver loop: drain the ready queue, then jump the clock to the next
/// deadline and fire it; stop when nothing is ready and no timers are pending.
///
/// Tasks still parked on something other than the timer are left alive.
#[allow(clippy::must_use_candidate)] // side-effecting; the report is informational
pub fn run_with_timer(executor: &Executor, timer: &Timer) -> RunReport {
    let mut polls = 0;
    let mut clock_jumps = 0;
    loop {
        polls += executor.run_until_stalled();
        match timer.next_deadline() {
            Some(deadline) => {
                timer.advance_to(deadline);
                clock_jumps += 1;
            }
            None => break,
        }
    }
    RunReport {
        polls,
        clock_jumps,
        finished_at: timer.now(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::async_internals::raw_waker::{WakeCounter, counting_waker, noop_waker};
    use std::sync::Arc;

    const fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    #[test]
    fn test_sleep_zero_is_ready_immediately() {
        let timer = Timer::new();
        let waker = noop_waker();
        let mut cx = Context::from_waker(&waker);
        let mut sleep = timer.sleep(Duration::ZERO);
        assert_eq!(Pin::new(&mut sleep).poll(&mut cx), Poll::Ready(()));
        assert_eq!(timer.pending_timers(), 0);
    }

    #[test]
    fn test_sleep_registers_lazily_and_fires_on_deadline() {
        let timer = Timer::new();
        let counter = Arc::new(WakeCounter::default());
        let waker = counting_waker(Arc::clone(&counter));
        let mut cx = Context::from_waker(&waker);

        let mut sleep = timer.sleep(ms(100));
        assert_eq!(timer.pending_timers(), 0, "registration happens on poll");
        assert_eq!(Pin::new(&mut sleep).poll(&mut cx), Poll::Pending);
        assert_eq!(timer.pending_timers(), 1);

        assert_eq!(timer.advance(ms(99)), 0);
        assert_eq!(counter.wakes(), 0);
        assert_eq!(timer.advance(ms(1)), 1);
        assert_eq!(counter.wakes(), 1);
        assert_eq!(Pin::new(&mut sleep).poll(&mut cx), Poll::Ready(()));
    }

    #[test]
    fn test_repoll_does_not_double_register() {
        let timer = Timer::new();
        let waker = noop_waker();
        let mut cx = Context::from_waker(&waker);
        let mut sleep = timer.sleep(ms(10));
        for _ in 0..5 {
            assert_eq!(Pin::new(&mut sleep).poll(&mut cx), Poll::Pending);
        }
        assert_eq!(timer.pending_timers(), 1);
        assert_eq!(timer.advance(ms(10)), 1);
    }

    #[test]
    fn test_repoll_with_new_waker_updates_registration() {
        let timer = Timer::new();
        let first = Arc::new(WakeCounter::default());
        let second = Arc::new(WakeCounter::default());
        let mut sleep = timer.sleep(ms(5));

        let w1 = counting_waker(Arc::clone(&first));
        let _ = Pin::new(&mut sleep).poll(&mut Context::from_waker(&w1));
        let w2 = counting_waker(Arc::clone(&second));
        let _ = Pin::new(&mut sleep).poll(&mut Context::from_waker(&w2));

        timer.advance(ms(5));
        assert_eq!(first.wakes(), 0);
        assert_eq!(second.wakes(), 1);
    }

    #[test]
    fn test_dropping_sleep_cancels_timer() {
        let timer = Timer::new();
        let counter = Arc::new(WakeCounter::default());
        let waker = counting_waker(Arc::clone(&counter));
        let mut sleep = timer.sleep(ms(10));
        let _ = Pin::new(&mut sleep).poll(&mut Context::from_waker(&waker));
        drop(sleep);

        assert_eq!(timer.pending_timers(), 0);
        assert_eq!(timer.next_deadline(), None, "tombstone skipped");
        assert_eq!(timer.advance(ms(10)), 0);
        assert_eq!(counter.wakes(), 0);
    }

    #[test]
    fn test_clock_never_moves_backwards() {
        let timer = Timer::new();
        timer.advance_to(Instant(50));
        timer.advance_to(Instant(10));
        assert_eq!(timer.now(), Instant(50));
    }

    #[test]
    fn test_instant_saturating_add() {
        assert_eq!(
            Instant(u64::MAX - 1).saturating_add(ms(5)),
            Instant(u64::MAX)
        );
        assert_eq!(Instant(1).saturating_add(Duration::MAX), Instant(u64::MAX));
    }

    #[test]
    fn test_runtime_sleeps_run_concurrently() {
        let executor = Executor::new();
        let timer = Timer::new();
        let log = Rc::new(RefCell::new(Vec::new()));
        for (name, delay) in [("slow", 300), ("fast", 100), ("mid", 200)] {
            let (t, log) = (timer.clone(), Rc::clone(&log));
            executor.spawn(async move {
                t.sleep(ms(delay)).await;
                log.borrow_mut().push((name, t.now().as_millis()));
            });
        }
        let report = run_with_timer(&executor, &timer);
        assert_eq!(*log.borrow(), [("fast", 100), ("mid", 200), ("slow", 300)]);
        // Total time is the max, not the sum: the sleeps overlapped.
        assert_eq!(report.finished_at, Instant(300));
        assert_eq!(report.clock_jumps, 3);
        assert_eq!(report.polls, 6);
        assert_eq!(executor.live_tasks(), 0);
    }

    #[test]
    fn test_sequential_sleeps_add_up() {
        let executor = Executor::new();
        let timer = Timer::new();
        let t = timer.clone();
        let handle = executor.spawn(async move {
            for _ in 0..3 {
                t.sleep(ms(40)).await;
            }
            t.now()
        });
        run_with_timer(&executor, &timer);
        assert_eq!(handle.try_take(), Some(Ok(Instant(120))));
    }

    #[test]
    fn test_equal_deadlines_fire_in_registration_order() {
        let executor = Executor::new();
        let timer = Timer::new();
        let log = Rc::new(RefCell::new(Vec::new()));
        for i in 0..5 {
            let (t, log) = (timer.clone(), Rc::clone(&log));
            executor.spawn(async move {
                t.sleep(ms(10)).await;
                log.borrow_mut().push(i);
            });
        }
        run_with_timer(&executor, &timer);
        assert_eq!(*log.borrow(), [0, 1, 2, 3, 4]);
    }

    /// Model-based check: random sleep durations wake in sorted
    /// `(deadline, spawn order)` order, matching a simple sorted-vector model.
    #[test]
    fn test_fire_order_matches_sorted_model() {
        let executor = Executor::new();
        let timer = Timer::new();
        let log = Rc::new(RefCell::new(Vec::new()));
        let mut model = Vec::new();
        let mut seed: u64 = 0xDEAD_BEEF_CAFE_F00D;
        for i in 0..64 {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            let delay = seed % 50;
            model.push((delay, i));
            let (t, log) = (timer.clone(), Rc::clone(&log));
            executor.spawn(async move {
                t.sleep(ms(delay)).await;
                log.borrow_mut().push((t.now().as_millis(), i));
            });
        }
        model.sort_unstable();
        run_with_timer(&executor, &timer);
        assert_eq!(*log.borrow(), model);
        assert_eq!(timer.pending_timers(), 0);
    }

    #[test]
    fn test_debug_impls() {
        let timer = Timer::new();
        let sleep = timer.sleep(ms(1));
        assert!(format!("{timer:?}").contains("Timer"));
        assert!(format!("{sleep:?}").contains("registered: false"));
        assert_eq!(sleep.deadline(), Instant(1));
        assert!(!sleep.is_elapsed());
    }
}
