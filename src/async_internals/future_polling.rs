//! # Future Polling Drills
//!
//! A [`Future`] is a state machine with one method: `poll`. Each call either finishes
//! (`Poll::Ready(value)`) or reports `Poll::Pending` *after* arranging for
//! `cx.waker()` to be woken when progress is possible. Nothing happens unless something
//! polls - futures are lazy.
//!
//! This drill hand-writes the futures that `async`/`.await` normally generate:
//!
//! - [`Ready`] - completes on the first poll.
//! - [`YieldNow`] - returns `Pending` once, waking itself (cooperative yield).
//! - [`Countdown`] - returns `Pending` `n` times.
//! - [`Map`] - a combinator with **unsafe structural pin projection**.
//! - [`Join`] - polls two futures until both finish (the `MaybeDone` pattern).
//! - [`AddOneAfterYield`] - the enum the compiler generates for a small `async fn`.
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::async_internals::future_polling::{countdown, poll_to_completion};
//!
//! // Pending 3 times, then Ready on the 4th poll.
//! assert_eq!(poll_to_completion(countdown(3), 10), Some(((), 4)));
//! ```
//!
//! ## Poll contract
//!
//! ```text
//!   poll(cx) ──► Ready(v)   done; polling again is a contract violation (may panic)
//!          └──► Pending    MUST have stored/used cx.waker() so someone wakes us later
//! ```

use std::future::Future;
use std::mem;
use std::pin::{Pin, pin};
use std::task::{Context, Poll};

use super::raw_waker::noop_waker;

// =========================================================================================
// Driving futures by hand
// =========================================================================================

/// Polls `future` once with a no-op waker.
pub fn poll_once<F: Future>(future: Pin<&mut F>) -> Poll<F::Output> {
    let waker = noop_waker();
    let mut cx = Context::from_waker(&waker);
    future.poll(&mut cx)
}

/// Busy-polls `future` with a no-op waker, returning `(output, polls)` or `None` if it is
/// still pending after `max_polls` polls.
///
/// Only meaningful for futures that make progress on every poll (self-waking futures);
/// a future waiting on I/O would just spin.
///
/// Time: O(`max_polls`) polls. Space: O(1) beyond the pinned future.
pub fn poll_to_completion<F: Future>(future: F, max_polls: usize) -> Option<(F::Output, usize)> {
    let mut future = pin!(future);
    let waker = noop_waker();
    let mut cx = Context::from_waker(&waker);
    for polls in 1..=max_polls {
        if let Poll::Ready(output) = future.as_mut().poll(&mut cx) {
            return Some((output, polls));
        }
    }
    None
}

// =========================================================================================
// Leaf futures
// =========================================================================================

/// Completes immediately with the stored value (like `std::future::ready`).
#[derive(Debug)]
pub struct Ready<T>(Option<T>);

// The value is never pinned (we move it out), so `Ready` is `Unpin` for any `T`.
impl<T> Unpin for Ready<T> {}

/// Creates a future that resolves to `value` on its first poll.
pub const fn ready<T>(value: T) -> Ready<T> {
    Ready(Some(value))
}

impl<T> Future for Ready<T> {
    type Output = T;

    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<T> {
        Poll::Ready(
            self.get_mut()
                .0
                .take()
                .expect("`Ready` polled after completion"),
        )
    }
}

/// Returns `Pending` exactly once, waking itself first so the executor re-queues it.
#[derive(Debug, Default)]
pub struct YieldNow {
    yielded: bool,
}

/// Creates a future that yields control back to the executor once.
#[must_use]
pub const fn yield_now() -> YieldNow {
    YieldNow { yielded: false }
}

impl Future for YieldNow {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        if self.yielded {
            return Poll::Ready(());
        }
        self.yielded = true;
        // Without this wake the task would never be polled again (a "lost wakeup").
        cx.waker().wake_by_ref();
        Poll::Pending
    }
}

/// Returns `Pending` `remaining` times (waking itself each time), then `Ready(())`.
#[derive(Debug)]
pub struct Countdown {
    remaining: u32,
}

/// Creates a future that needs `n + 1` polls to complete.
#[must_use]
pub const fn countdown(n: u32) -> Countdown {
    Countdown { remaining: n }
}

impl Future for Countdown {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        if self.remaining == 0 {
            return Poll::Ready(());
        }
        self.remaining -= 1;
        cx.waker().wake_by_ref();
        Poll::Pending
    }
}

// =========================================================================================
// Combinators
// =========================================================================================

/// Applies `f` to the output of `future` (like `FutureExt::map`).
///
/// `future` is *structurally pinned*: once `Map` is pinned, `future` never moves.
/// `f` is not pinned: it is moved out with `Option::take`.
#[derive(Debug)]
pub struct Map<Fut, F> {
    future: Fut,
    f: Option<F>,
}

/// Creates a future that maps `future`'s output through `f`.
pub const fn map<Fut, F, T>(future: Fut, f: F) -> Map<Fut, F>
where
    Fut: Future,
    F: FnOnce(Fut::Output) -> T,
{
    Map { future, f: Some(f) }
}

impl<Fut, F, T> Future for Map<Fut, F>
where
    Fut: Future,
    F: FnOnce(Fut::Output) -> T,
{
    type Output = T;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<T> {
        // SAFETY: we never move `this.future` out of `this`, `Map` has no `Drop` impl,
        // and the auto `Unpin` impl only applies when `Fut: Unpin`. Those three rules make
        // projecting `Pin<&mut Map>` to `Pin<&mut Fut>` sound. `this.f` is only accessed
        // through `&mut` and never pinned.
        let this = unsafe { self.get_unchecked_mut() };
        let future = unsafe { Pin::new_unchecked(&mut this.future) };
        match future.poll(cx) {
            Poll::Ready(value) => {
                let f = this.f.take().expect("`Map` polled after completion");
                Poll::Ready(f(value))
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

/// Slot that holds a future until it finishes, then holds its output.
enum MaybeDone<F: Future> {
    Pending(F),
    Done(F::Output),
    Taken,
}

impl<F: Future> MaybeDone<F> {
    const fn label(&self) -> &'static str {
        match self {
            Self::Pending(_) => "pending",
            Self::Done(_) => "done",
            Self::Taken => "taken",
        }
    }
}

impl<F: Future + Unpin> MaybeDone<F> {
    /// Polls the inner future if still pending; returns whether an output is available.
    fn poll_slot(&mut self, cx: &mut Context<'_>) -> bool {
        let output = match self {
            Self::Pending(future) => match Pin::new(future).poll(cx) {
                Poll::Ready(output) => output,
                Poll::Pending => return false,
            },
            Self::Done(_) => return true,
            Self::Taken => return false,
        };
        *self = Self::Done(output);
        true
    }

    fn take(&mut self) -> F::Output {
        match mem::replace(self, Self::Taken) {
            Self::Done(output) => output,
            Self::Pending(_) | Self::Taken => unreachable!("take() called before Done"),
        }
    }
}

/// Polls two futures concurrently on one task and returns both outputs.
///
/// Both children share the parent's waker, so a wake from either re-polls the `Join`;
/// a finished child is never polled again.
pub struct Join<A: Future, B: Future> {
    a: MaybeDone<A>,
    b: MaybeDone<B>,
}

impl<A: Future, B: Future> std::fmt::Debug for Join<A, B> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Join")
            .field("a", &self.a.label())
            .field("b", &self.b.label())
            .finish()
    }
}

// Outputs are never pinned, so `Join` only needs its *futures* to be `Unpin`.
impl<A: Future + Unpin, B: Future + Unpin> Unpin for Join<A, B> {}

/// Creates a future that completes when both `a` and `b` complete.
///
/// Requires `Unpin` children; wrap others in `Box::pin`.
pub const fn join<A, B>(a: A, b: B) -> Join<A, B>
where
    A: Future + Unpin,
    B: Future + Unpin,
{
    Join {
        a: MaybeDone::Pending(a),
        b: MaybeDone::Pending(b),
    }
}

impl<A, B> Future for Join<A, B>
where
    A: Future + Unpin,
    B: Future + Unpin,
{
    type Output = (A::Output, B::Output);

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        // Poll both every time: polling only `a` until it finishes would serialise them.
        let a_done = this.a.poll_slot(cx);
        let b_done = this.b.poll_slot(cx);
        if a_done && b_done {
            Poll::Ready((this.a.take(), this.b.take()))
        } else {
            Poll::Pending
        }
    }
}

// =========================================================================================
// What `async fn` desugars to
// =========================================================================================

/// Hand-written equivalent of:
///
/// ```
/// # use rust_interview_practice::async_internals::future_polling::{
/// #     add_one_after_yield, poll_to_completion, yield_now,
/// # };
/// async fn add_one_after_yield_sugar(x: u32) -> u32 {
///     yield_now().await;
///     x + 1
/// }
///
/// // Same output, same number of polls.
/// assert_eq!(
///     poll_to_completion(add_one_after_yield(1), 10),
///     poll_to_completion(add_one_after_yield_sugar(1), 10),
/// );
/// ```
///
/// Each `.await` point becomes a variant holding the locals that are live across it.
#[derive(Debug)]
pub enum AddOneAfterYield {
    /// Created but never polled; holds the arguments.
    Start {
        /// The argument.
        x: u32,
    },
    /// Suspended at `yield_now().await`.
    Yielding {
        /// Live across the await point.
        x: u32,
        /// The future being awaited.
        yield_now: YieldNow,
    },
    /// Returned; polling again panics, like a compiler-generated future.
    Done,
}

/// Creates the hand-desugared state machine for `add_one_after_yield(x)`.
#[must_use]
pub const fn add_one_after_yield(x: u32) -> AddOneAfterYield {
    AddOneAfterYield::Start { x }
}

impl Future for AddOneAfterYield {
    type Output = u32;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<u32> {
        let this = self.get_mut();
        loop {
            match mem::replace(this, Self::Done) {
                Self::Start { x } => {
                    *this = Self::Yielding {
                        x,
                        yield_now: yield_now(),
                    };
                }
                Self::Yielding { x, mut yield_now } => match Pin::new(&mut yield_now).poll(cx) {
                    Poll::Pending => {
                        *this = Self::Yielding { x, yield_now };
                        return Poll::Pending;
                    }
                    Poll::Ready(()) => return Poll::Ready(x + 1),
                },
                Self::Done => panic!("`async fn` resumed after completion"),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::async_internals::raw_waker::{WakeCounter, counting_waker};
    use std::sync::Arc;

    #[test]
    fn test_ready_completes_on_first_poll() {
        assert_eq!(poll_to_completion(ready("hi"), 1), Some(("hi", 1)));
    }

    #[test]
    #[should_panic(expected = "polled after completion")]
    fn test_ready_panics_when_polled_twice() {
        let mut fut = ready(1);
        let _ = poll_once(Pin::new(&mut fut));
        let _ = poll_once(Pin::new(&mut fut));
    }

    #[test]
    fn test_yield_now_wakes_itself_before_pending() {
        let counter = Arc::new(WakeCounter::default());
        let waker = counting_waker(Arc::clone(&counter));
        let mut cx = Context::from_waker(&waker);
        let mut fut = yield_now();

        assert_eq!(Pin::new(&mut fut).poll(&mut cx), Poll::Pending);
        assert_eq!(counter.wakes(), 1);
        assert_eq!(Pin::new(&mut fut).poll(&mut cx), Poll::Ready(()));
        assert_eq!(counter.wakes(), 1);
    }

    #[test]
    fn test_countdown_zero_is_immediately_ready() {
        assert_eq!(poll_to_completion(countdown(0), 1), Some(((), 1)));
    }

    #[test]
    fn test_countdown_needs_n_plus_one_polls() {
        assert_eq!(poll_to_completion(countdown(5), 100), Some(((), 6)));
    }

    #[test]
    fn test_poll_to_completion_gives_up_after_budget() {
        assert_eq!(poll_to_completion(countdown(5), 5), None);
        assert_eq!(poll_to_completion(std::future::pending::<()>(), 3), None);
    }

    #[test]
    fn test_map_transforms_output() {
        let fut = map(countdown(2), |()| "mapped");
        assert_eq!(poll_to_completion(fut, 10), Some(("mapped", 3)));
    }

    #[test]
    fn test_map_over_not_unpin_async_block() {
        // `async` blocks are `!Unpin`; `Map` handles them via pin projection.
        let fut = map(
            async {
                yield_now().await;
                20
            },
            |x| x + 1,
        );
        assert_eq!(poll_to_completion(fut, 10), Some((21, 2)));
    }

    #[test]
    fn test_join_waits_for_slower_side() {
        let fut = join(countdown(1), map(countdown(3), |()| 'b'));
        assert_eq!(poll_to_completion(fut, 10), Some((((), 'b'), 4)));
    }

    #[test]
    fn test_join_both_ready() {
        assert_eq!(
            poll_to_completion(join(ready(1), ready(2)), 1),
            Some(((1, 2), 1))
        );
    }

    #[test]
    fn test_join_boxed_async_blocks() {
        let a = Box::pin(async {
            yield_now().await;
            "a"
        });
        let b = Box::pin(async { "b" });
        assert_eq!(poll_to_completion(join(a, b), 5), Some((("a", "b"), 2)));
    }

    #[test]
    fn test_desugared_state_machine_matches_async_fn() {
        async fn add_one_after_yield_sugar(x: u32) -> u32 {
            yield_now().await;
            x + 1
        }

        let manual = poll_to_completion(add_one_after_yield(41), 10);
        let sugar = poll_to_completion(add_one_after_yield_sugar(41), 10);
        assert_eq!(manual, Some((42, 2)));
        assert_eq!(manual, sugar);
    }

    #[test]
    fn test_desugared_state_machine_transitions() {
        let mut fut = add_one_after_yield(1);
        assert!(matches!(fut, AddOneAfterYield::Start { x: 1 }));
        assert_eq!(poll_once(Pin::new(&mut fut)), Poll::Pending);
        assert!(matches!(fut, AddOneAfterYield::Yielding { x: 1, .. }));
        assert_eq!(poll_once(Pin::new(&mut fut)), Poll::Ready(2));
        assert!(matches!(fut, AddOneAfterYield::Done));
    }

    #[test]
    #[should_panic(expected = "resumed after completion")]
    fn test_desugared_state_machine_panics_after_done() {
        let mut fut = add_one_after_yield(1);
        while poll_once(Pin::new(&mut fut)).is_pending() {}
        let _ = poll_once(Pin::new(&mut fut));
    }

    #[test]
    fn test_futures_are_lazy() {
        let mut ran = false;
        let fut = async {
            ran = true;
        };
        drop(fut);
        assert!(!ran);
    }
}
