//! # Cancellation Drills
//!
//! In Rust async, **cancellation is dropping**: a future that is never polled again
//! and then dropped simply stops at its last `.await`, running destructors for every
//! live local. There is no exception or unwinding, so code between two `.await`s
//! always runs to the next suspension point.
//!
//! This drill builds the cooperative pieces runtimes add on top of that:
//!
//! - [`CancellationToken`] - shareable, thread-safe "please stop" flag whose
//!   [`cancelled`](CancellationToken::cancelled) future wakes every waiter, with
//!   parent → child propagation and an RAII [`DropGuard`].
//! - [`with_cancellation`] - race a future against a token (biased towards the token);
//!   on cancel the inner future is dropped *eagerly*.
//! - [`timeout`] - race a future against a [`Sleep`]: cancellation by deadline.
//!
//! See also [`JoinHandle::abort`](super::ready_queue::JoinHandle::abort) for
//! executor-level cancellation.
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::async_internals::cancellation::{
//!     CancellationToken, CancelledError, with_cancellation,
//! };
//! use rust_interview_practice::async_internals::raw_waker::block_on;
//!
//! let token = CancellationToken::new();
//! let child = token.child_token();
//! token.cancel();
//!
//! let result = block_on(with_cancellation(&child, std::future::pending::<()>()));
//! assert_eq!(result, Err(CancelledError));
//! ```
//!
//! ## Cancel safety
//!
//! A future is *cancel-safe* if dropping it at any `.await` loses no data. A loop that
//! accumulates into a local buffer across awaits is **not**: the buffer is dropped with
//! the future. Keep state that must survive cancellation outside the cancelled future.

use std::collections::HashMap;
use std::fmt;
use std::future::Future;
use std::mem;
use std::pin::Pin;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, Weak};
use std::task::{Context, Poll, Waker};
use std::time::Duration;

use super::timer::{Sleep, Timer};

// =========================================================================================
// Errors
// =========================================================================================

/// The operation was cancelled via a [`CancellationToken`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CancelledError;

impl fmt::Display for CancelledError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("operation was cancelled")
    }
}

impl std::error::Error for CancelledError {}

/// The deadline passed before the inner future completed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Elapsed;

impl fmt::Display for Elapsed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("deadline has elapsed")
    }
}

impl std::error::Error for Elapsed {}

// =========================================================================================
// CancellationToken
// =========================================================================================

#[derive(Default)]
struct TokenState {
    cancelled: bool,
    next_waiter: u64,
    waiters: HashMap<u64, Waker>,
    children: Vec<Weak<TokenInner>>,
}

#[derive(Default)]
struct TokenInner {
    state: Mutex<TokenState>,
}

impl TokenInner {
    fn lock(&self) -> MutexGuard<'_, TokenState> {
        // A panic while holding the lock cannot leave `TokenState` half-updated in a way
        // that matters (every field write is a single assignment), so recover the guard.
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Thread-safe cancellation signal. Clones share the same state.
///
/// Invariants:
/// - `cancelled` only ever goes `false -> true`.
/// - Once cancelled, `waiters` and `children` are empty: everyone was notified exactly once.
#[derive(Clone, Default)]
pub struct CancellationToken {
    inner: Arc<TokenInner>,
}

impl fmt::Debug for CancellationToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CancellationToken")
            .field("cancelled", &self.is_cancelled())
            .finish_non_exhaustive()
    }
}

impl CancellationToken {
    /// Creates an uncancelled token.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether [`cancel`](Self::cancel) has been called on this token or an ancestor.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.inner.lock().cancelled
    }

    /// Cancels this token and all descendants, waking every waiter. Idempotent.
    ///
    /// Time: O(w + c) for `w` waiters and `c` descendant tokens.
    pub fn cancel(&self) {
        let (waiters, children) = {
            let mut state = self.inner.lock();
            if state.cancelled {
                return;
            }
            state.cancelled = true;
            (
                mem::take(&mut state.waiters),
                mem::take(&mut state.children),
            )
        };
        // Wake and recurse outside the lock: wakers may run arbitrary code.
        for waker in waiters.into_values() {
            waker.wake();
        }
        for child in children.iter().filter_map(Weak::upgrade) {
            Self { inner: child }.cancel();
        }
    }

    /// Creates a token that is cancelled when `self` is, but can also be cancelled on
    /// its own without affecting `self`.
    #[must_use]
    pub fn child_token(&self) -> Self {
        let child = Self::new();
        let mut state = self.inner.lock();
        if state.cancelled {
            drop(state);
            child.inner.lock().cancelled = true;
        } else {
            // Prune children that were dropped so the list doesn't grow forever.
            state.children.retain(|weak| weak.strong_count() > 0);
            state.children.push(Arc::downgrade(&child.inner));
        }
        child
    }

    /// A future that completes once this token is cancelled.
    pub fn cancelled(&self) -> WaitForCancellation {
        WaitForCancellation {
            token: self.clone(),
            waiter: None,
        }
    }

    /// Wraps the token in a guard that cancels it on drop (unless disarmed).
    #[must_use]
    pub const fn drop_guard(self) -> DropGuard {
        DropGuard {
            token: self,
            armed: true,
        }
    }

    #[cfg(test)]
    fn waiter_count(&self) -> usize {
        self.inner.lock().waiters.len()
    }

    #[cfg(test)]
    fn child_count(&self) -> usize {
        self.inner.lock().children.len()
    }
}

/// Future returned by [`CancellationToken::cancelled`]. Deregisters its waker on drop.
#[must_use = "futures do nothing unless polled"]
pub struct WaitForCancellation {
    token: CancellationToken,
    waiter: Option<u64>,
}

impl fmt::Debug for WaitForCancellation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WaitForCancellation")
            .field("token", &self.token)
            .finish_non_exhaustive()
    }
}

impl Future for WaitForCancellation {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        let this = self.get_mut();
        let mut state = this.token.inner.lock();
        if state.cancelled {
            drop(state);
            // `cancel` already drained the waiter map; just forget our key.
            this.waiter = None;
            return Poll::Ready(());
        }
        let key = *this.waiter.get_or_insert_with(|| {
            let key = state.next_waiter;
            state.next_waiter += 1;
            key
        });
        // First poll inserts; re-polls refresh the waker only if it would wake a different task.
        state
            .waiters
            .entry(key)
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

impl Drop for WaitForCancellation {
    fn drop(&mut self) {
        if let Some(key) = self.waiter.take() {
            self.token.inner.lock().waiters.remove(&key);
        }
    }
}

/// Cancels its token when dropped - ties cancellation to a scope.
#[derive(Debug)]
pub struct DropGuard {
    token: CancellationToken,
    armed: bool,
}

impl DropGuard {
    /// Defuses the guard, returning the token without cancelling it.
    #[must_use]
    pub fn disarm(mut self) -> CancellationToken {
        self.armed = false;
        self.token.clone()
    }
}

impl Drop for DropGuard {
    fn drop(&mut self) {
        if self.armed {
            self.token.cancel();
        }
    }
}

// =========================================================================================
// Racing a future against cancellation
// =========================================================================================

/// Future returned by [`with_cancellation`].
#[must_use = "futures do nothing unless polled"]
pub struct WithCancellation<F> {
    /// Boxed so the combinator is `Unpin` without unsafe pin projection.
    future: Option<Pin<Box<F>>>,
    cancelled: WaitForCancellation,
}

impl<F> fmt::Debug for WithCancellation<F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WithCancellation")
            .field("running", &self.future.is_some())
            .finish_non_exhaustive()
    }
}

/// Runs `future` until it completes or `token` is cancelled, whichever comes first.
///
/// Biased: if both are ready in the same poll, cancellation wins. On cancellation the
/// inner future is dropped immediately (not when the combinator is dropped).
pub fn with_cancellation<F: Future>(token: &CancellationToken, future: F) -> WithCancellation<F> {
    WithCancellation {
        future: Some(Box::pin(future)),
        cancelled: token.cancelled(),
    }
}

impl<F: Future> Future for WithCancellation<F> {
    type Output = Result<F::Output, CancelledError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        if Pin::new(&mut this.cancelled).poll(cx).is_ready() {
            this.future = None; // cancellation == drop
            return Poll::Ready(Err(CancelledError));
        }
        let future = this
            .future
            .as_mut()
            .expect("`WithCancellation` polled after completion");
        match future.as_mut().poll(cx) {
            Poll::Ready(output) => {
                this.future = None;
                Poll::Ready(Ok(output))
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

/// Future returned by [`timeout`].
#[must_use = "futures do nothing unless polled"]
pub struct Timeout<F> {
    future: Option<Pin<Box<F>>>,
    sleep: Sleep,
}

impl<F> fmt::Debug for Timeout<F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Timeout")
            .field("running", &self.future.is_some())
            .field("sleep", &self.sleep)
            .finish()
    }
}

/// Runs `future` for at most `duration` on `timer`'s clock.
///
/// The inner future is polled first, so a future that is ready exactly at the deadline
/// still succeeds. On timeout the inner future is dropped immediately.
pub fn timeout<F: Future>(timer: &Timer, duration: Duration, future: F) -> Timeout<F> {
    Timeout {
        future: Some(Box::pin(future)),
        sleep: timer.sleep(duration),
    }
}

impl<F: Future> Future for Timeout<F> {
    type Output = Result<F::Output, Elapsed>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        let future = this
            .future
            .as_mut()
            .expect("`Timeout` polled after completion");
        if let Poll::Ready(output) = future.as_mut().poll(cx) {
            this.future = None;
            return Poll::Ready(Ok(output));
        }
        if Pin::new(&mut this.sleep).poll(cx).is_ready() {
            this.future = None;
            return Poll::Ready(Err(Elapsed));
        }
        Poll::Pending
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::async_internals::future_polling::{poll_once, ready};
    use crate::async_internals::raw_waker::{WakeCounter, block_on, counting_waker};
    use crate::async_internals::ready_queue::Executor;
    use crate::async_internals::timer::{Instant, run_with_timer};
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;
    use std::thread;

    struct DropFlag(Rc<Cell<bool>>);

    impl Drop for DropFlag {
        fn drop(&mut self) {
            self.0.set(true);
        }
    }

    const fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    #[test]
    fn test_token_starts_uncancelled_and_cancel_is_idempotent() {
        let token = CancellationToken::new();
        assert!(!token.is_cancelled());
        token.cancel();
        token.cancel();
        assert!(token.is_cancelled());
    }

    #[test]
    fn test_clones_share_state() {
        let token = CancellationToken::new();
        let clone = token.clone();
        clone.cancel();
        assert!(token.is_cancelled());
    }

    #[test]
    fn test_cancel_wakes_every_waiter_once() {
        let token = CancellationToken::new();
        let counter = Arc::new(WakeCounter::default());
        let waker = counting_waker(Arc::clone(&counter));
        let mut cx = Context::from_waker(&waker);

        let mut waiters: Vec<_> = (0..3).map(|_| token.cancelled()).collect();
        for w in &mut waiters {
            assert_eq!(Pin::new(w).poll(&mut cx), Poll::Pending);
        }
        assert_eq!(token.waiter_count(), 3);

        token.cancel();
        token.cancel();
        assert_eq!(counter.wakes(), 3);
        assert_eq!(token.waiter_count(), 0);
        for w in &mut waiters {
            assert_eq!(Pin::new(w).poll(&mut cx), Poll::Ready(()));
        }
    }

    #[test]
    fn test_repoll_does_not_duplicate_waiter() {
        let token = CancellationToken::new();
        let mut waiter = token.cancelled();
        for _ in 0..4 {
            assert!(poll_once(Pin::new(&mut waiter)).is_pending());
        }
        assert_eq!(token.waiter_count(), 1);
    }

    #[test]
    fn test_dropped_waiter_deregisters() {
        let token = CancellationToken::new();
        let mut waiter = token.cancelled();
        assert!(poll_once(Pin::new(&mut waiter)).is_pending());
        drop(waiter);
        assert_eq!(token.waiter_count(), 0);
    }

    #[test]
    fn test_parent_cancels_children_but_not_vice_versa() {
        let parent = CancellationToken::new();
        let child = parent.child_token();
        let grandchild = child.child_token();
        let sibling = parent.child_token();

        sibling.cancel();
        assert!(!parent.is_cancelled());
        assert!(!child.is_cancelled());

        parent.cancel();
        assert!(child.is_cancelled());
        assert!(grandchild.is_cancelled());
    }

    #[test]
    fn test_child_of_cancelled_parent_starts_cancelled() {
        let parent = CancellationToken::new();
        parent.cancel();
        assert!(parent.child_token().is_cancelled());
    }

    #[test]
    fn test_dropped_children_are_pruned() {
        let parent = CancellationToken::new();
        for _ in 0..10 {
            drop(parent.child_token());
        }
        let _kept = parent.child_token();
        assert_eq!(parent.child_count(), 1);
    }

    #[test]
    fn test_drop_guard_cancels_on_scope_exit() {
        let token = CancellationToken::new();
        {
            let _guard = token.clone().drop_guard();
            assert!(!token.is_cancelled());
        }
        assert!(token.is_cancelled());
    }

    #[test]
    fn test_disarmed_drop_guard_does_not_cancel() {
        let token = CancellationToken::new();
        let back = token.clone().drop_guard().disarm();
        assert!(!token.is_cancelled());
        assert!(!back.is_cancelled());
    }

    #[test]
    fn test_cancel_from_another_thread_wakes_block_on() {
        let token = CancellationToken::new();
        let remote = token.clone();
        let canceller = thread::spawn(move || {
            thread::sleep(ms(5));
            remote.cancel();
        });
        block_on(token.cancelled());
        canceller.join().expect("canceller panicked");
        assert!(token.is_cancelled());
    }

    #[test]
    fn test_with_cancellation_completes_when_not_cancelled() {
        let token = CancellationToken::new();
        assert_eq!(block_on(with_cancellation(&token, ready(5))), Ok(5));
        assert_eq!(
            token.waiter_count(),
            0,
            "waiter released with the combinator"
        );
    }

    #[test]
    fn test_with_cancellation_is_biased_towards_cancel() {
        let token = CancellationToken::new();
        token.cancel();
        assert_eq!(
            block_on(with_cancellation(&token, ready(5))),
            Err(CancelledError)
        );
    }

    #[test]
    fn test_cancellation_drops_inner_future_and_loses_partial_state() {
        let executor = Executor::new();
        let timer = Timer::new();
        let token = CancellationToken::new();
        let dropped = Rc::new(Cell::new(false));
        let committed = Rc::new(RefCell::new(Vec::new()));

        let (t, flag, sink) = (
            timer.clone(),
            DropFlag(Rc::clone(&dropped)),
            Rc::clone(&committed),
        );
        let worker = executor.spawn(with_cancellation(&token, async move {
            let _flag = flag;
            let mut batch = Vec::new(); // not cancel-safe: lost on drop
            for i in 0..10 {
                t.sleep(ms(100)).await;
                batch.push(i);
                if batch.len() == 3 {
                    sink.borrow_mut().append(&mut batch); // committed outside: survives
                }
            }
            batch
        }));

        let (t, killer) = (timer.clone(), token);
        executor.spawn(async move {
            t.sleep(ms(550)).await;
            killer.cancel();
        });

        let report = run_with_timer(&executor, &timer);
        assert_eq!(worker.try_take(), Some(Ok(Err(CancelledError))));
        assert!(dropped.get(), "inner future dropped on cancel");
        assert_eq!(*committed.borrow(), [0, 1, 2]); // items 3 and 4 were in `batch`
        assert_eq!(
            report.finished_at,
            Instant::default().saturating_add(ms(550))
        );
        assert_eq!(
            timer.pending_timers(),
            0,
            "sleep inside dropped future deregistered"
        );
    }

    #[test]
    fn test_timeout_ok_when_inner_finishes_first() {
        let executor = Executor::new();
        let timer = Timer::new();
        let t = timer.clone();
        let handle = executor.spawn(timeout(&timer, ms(100), async move {
            t.sleep(ms(40)).await;
            "fast"
        }));
        let report = run_with_timer(&executor, &timer);
        assert_eq!(handle.try_take(), Some(Ok(Ok("fast"))));
        assert_eq!(report.finished_at.as_millis(), 40);
        assert_eq!(timer.pending_timers(), 0);
    }

    #[test]
    fn test_timeout_elapses_and_drops_inner() {
        let executor = Executor::new();
        let timer = Timer::new();
        let dropped = Rc::new(Cell::new(false));
        let (t, flag) = (timer.clone(), DropFlag(Rc::clone(&dropped)));
        let handle = executor.spawn(timeout(&timer, ms(100), async move {
            let _flag = flag;
            t.sleep(ms(1_000)).await;
            "slow"
        }));
        let report = run_with_timer(&executor, &timer);
        assert_eq!(handle.try_take(), Some(Ok(Err(Elapsed))));
        assert!(dropped.get());
        assert_eq!(report.finished_at.as_millis(), 100);
        assert_eq!(timer.pending_timers(), 0);
    }

    #[test]
    fn test_timeout_inner_ready_at_deadline_wins() {
        let executor = Executor::new();
        let timer = Timer::new();
        let t = timer.clone();
        let handle = executor.spawn(timeout(&timer, ms(50), async move {
            t.sleep(ms(50)).await;
            1
        }));
        run_with_timer(&executor, &timer);
        assert_eq!(handle.try_take(), Some(Ok(Ok(1))));
    }

    #[test]
    fn test_error_display_and_debug() {
        assert_eq!(CancelledError.to_string(), "operation was cancelled");
        assert_eq!(Elapsed.to_string(), "deadline has elapsed");
        let token = CancellationToken::new();
        assert!(format!("{token:?}").contains("cancelled: false"));
        assert!(format!("{:?}", token.cancelled()).contains("WaitForCancellation"));
        assert!(format!("{:?}", with_cancellation(&token, ready(1))).contains("running: true"));
        let timer = Timer::new();
        assert!(format!("{:?}", timeout(&timer, ms(1), ready(1))).contains("Timeout"));
    }
}
