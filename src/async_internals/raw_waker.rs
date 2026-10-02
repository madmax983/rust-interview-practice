//! # `RawWaker` Drills
//!
//! A [`Waker`] is how a future says "poll me again later". Underneath it is a
//! [`RawWaker`]: a type-erased data pointer plus a hand-written vtable of four
//! functions (`clone`, `wake`, `wake_by_ref`, `drop`). Every executor builds one.
//!
//! This drill builds three wakers from scratch:
//!
//! 1. [`noop_waker`] - a static vtable that does nothing (for manual polling).
//! 2. [`counting_waker`] - an `Arc`-backed vtable that counts wakes, written with raw
//!    pointers so the reference counting is visible.
//! 3. [`ThreadWaker`] - the safe [`std::task::Wake`] route, used by [`block_on`] to
//!    park/unpark the current thread.
//!
//! ## Examples
//!
//! ```
//! use std::sync::Arc;
//! use rust_interview_practice::async_internals::raw_waker::{WakeCounter, counting_waker};
//!
//! let counter = Arc::new(WakeCounter::default());
//! let waker = counting_waker(Arc::clone(&counter));
//! waker.wake_by_ref();
//! waker.clone().wake();
//! drop(waker);
//!
//! assert_eq!(counter.wakes(), 2);
//! assert_eq!(Arc::strong_count(&counter), 1); // every clone was released
//! ```
//!
//! ## Vtable contract
//!
//! ```text
//!   clone(data)       -> RawWaker   must produce an independent owner (+1 ref)
//!   wake(data)                      consumes the waker              (-1 ref)
//!   wake_by_ref(data)               wakes, keeps ownership          (+0 ref)
//!   drop(data)                      releases the waker              (-1 ref)
//! ```
//!
//! Invariant: for an `Arc`-backed waker, the strong count contributed by wakers equals
//! the number of live `Waker` values. Breaking it either leaks the task or frees it while
//! a waker still points at it (use-after-free).

use std::future::Future;
use std::pin::pin;
use std::ptr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::task::{Context, Poll, RawWaker, RawWakerVTable, Wake, Waker};
use std::thread::{self, Thread};

// =========================================================================================
// 1. No-op waker: static data, static vtable
// =========================================================================================

/// Vtable whose functions ignore their data pointer entirely.
static NOOP_VTABLE: RawWakerVTable =
    RawWakerVTable::new(noop_clone, noop_wake, noop_wake, noop_wake);

const fn noop_raw_waker() -> RawWaker {
    RawWaker::new(ptr::null(), &NOOP_VTABLE)
}

const unsafe fn noop_clone(_data: *const ()) -> RawWaker {
    noop_raw_waker()
}

const unsafe fn noop_wake(_data: *const ()) {}

/// Builds a waker that does nothing when woken.
///
/// Useful for manually polling futures that wake themselves or that you poll in a loop.
#[must_use]
pub const fn noop_waker() -> Waker {
    // SAFETY: every vtable function ignores the (null) data pointer, so the RawWaker
    // contract (thread-safe, clone yields an equivalent waker) holds trivially.
    unsafe { Waker::from_raw(noop_raw_waker()) }
}

// =========================================================================================
// 2. Counting waker: Arc<WakeCounter> smuggled through *const ()
// =========================================================================================

/// Shared state behind [`counting_waker`]: how many times it has been woken.
#[derive(Debug, Default)]
pub struct WakeCounter {
    wakes: AtomicUsize,
}

impl WakeCounter {
    /// Number of `wake`/`wake_by_ref` calls observed so far.
    #[must_use]
    pub fn wakes(&self) -> usize {
        self.wakes.load(Ordering::Acquire)
    }

    fn record(&self) {
        self.wakes.fetch_add(1, Ordering::AcqRel);
    }
}

static COUNTING_VTABLE: RawWakerVTable = RawWakerVTable::new(
    counting_clone,
    counting_wake,
    counting_wake_by_ref,
    counting_drop,
);

/// Builds a waker backed by `counter`, transferring the `Arc`'s ownership into the waker.
///
/// Time: O(1) for every vtable operation. Space: O(1); clones share the counter.
#[must_use]
pub fn counting_waker(counter: Arc<WakeCounter>) -> Waker {
    let data = Arc::into_raw(counter).cast::<()>();
    // SAFETY: `data` came from `Arc::into_raw` and the vtable below treats it as an
    // owned `Arc<WakeCounter>` reference; `WakeCounter` is `Send + Sync`.
    unsafe { Waker::from_raw(RawWaker::new(data, &COUNTING_VTABLE)) }
}

unsafe fn counting_clone(data: *const ()) -> RawWaker {
    // SAFETY: `data` is a live `Arc<WakeCounter>` pointer owned by the waker being cloned.
    // Bump the count without materialising (and later dropping) a second `Arc`.
    unsafe { Arc::increment_strong_count(data.cast::<WakeCounter>()) };
    RawWaker::new(data, &COUNTING_VTABLE)
}

unsafe fn counting_wake(data: *const ()) {
    // SAFETY: `wake` consumes the waker, so we take back the reference it owned.
    let counter = unsafe { Arc::from_raw(data.cast::<WakeCounter>()) };
    counter.record();
    // `counter` drops here: -1 ref, matching the consumed waker.
}

unsafe fn counting_wake_by_ref(data: *const ()) {
    // SAFETY: the waker still owns its reference; we only borrow through it.
    let counter = unsafe { &*data.cast::<WakeCounter>() };
    counter.record();
}

unsafe fn counting_drop(data: *const ()) {
    // SAFETY: releases exactly the one reference this waker owned.
    unsafe { Arc::decrement_strong_count(data.cast::<WakeCounter>()) };
}

// =========================================================================================
// 3. Thread waker via the safe `Wake` trait + `block_on`
// =========================================================================================

/// A waker that unparks a thread. Built with [`std::task::Wake`], which generates the
/// same `Arc` vtable as above without any `unsafe`.
#[derive(Debug)]
pub struct ThreadWaker {
    thread: Thread,
    notified: AtomicBool,
}

impl ThreadWaker {
    /// Creates a waker that unparks the calling thread.
    #[must_use]
    pub fn current() -> Arc<Self> {
        Arc::new(Self {
            thread: thread::current(),
            notified: AtomicBool::new(false),
        })
    }

    /// Consumes a pending notification, returning whether one was present.
    fn take_notification(&self) -> bool {
        self.notified.swap(false, Ordering::AcqRel)
    }
}

impl Wake for ThreadWaker {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        // Set the flag *before* unparking so the parked thread observes it on return.
        self.notified.store(true, Ordering::Release);
        self.thread.unpark();
    }
}

/// Runs a future to completion on the current thread, parking between polls.
///
/// The `notified` flag guards against spurious unparks: the thread only re-polls after a
/// real wake. A wake that lands before `park` is not lost because `unpark` leaves a token.
///
/// Time: O(p) polls of `future`. Space: O(1) beyond the pinned future.
pub fn block_on<F: Future>(future: F) -> F::Output {
    let mut future = pin!(future);
    let thread_waker = ThreadWaker::current();
    let waker = Waker::from(Arc::clone(&thread_waker));
    let mut cx = Context::from_waker(&waker);

    loop {
        if let Poll::Ready(output) = future.as_mut().poll(&mut cx) {
            return output;
        }
        while !thread_waker.take_notification() {
            thread::park();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::Duration;

    /// Future that returns `Pending` until a flag is set, storing the latest waker.
    struct Flag {
        set: Arc<AtomicBool>,
        waker_slot: mpsc::Sender<Waker>,
    }

    impl Future for Flag {
        type Output = &'static str;

        fn poll(self: std::pin::Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            if self.set.load(Ordering::Acquire) {
                Poll::Ready("done")
            } else {
                let _ = self.waker_slot.send(cx.waker().clone());
                Poll::Pending
            }
        }
    }

    #[test]
    fn test_noop_waker_clone_and_wake_are_harmless() {
        let waker = noop_waker();
        let clone = waker.clone();
        waker.wake_by_ref();
        clone.wake();
        waker.wake();
    }

    #[test]
    fn test_noop_waker_polls_ready_future() {
        let waker = noop_waker();
        let mut cx = Context::from_waker(&waker);
        let mut fut = pin!(async { 7 });
        assert_eq!(fut.as_mut().poll(&mut cx), Poll::Ready(7));
    }

    #[test]
    fn test_counting_waker_counts_wake_by_ref() {
        let counter = Arc::new(WakeCounter::default());
        let waker = counting_waker(Arc::clone(&counter));
        waker.wake_by_ref();
        waker.wake_by_ref();
        assert_eq!(counter.wakes(), 2);
        assert_eq!(Arc::strong_count(&counter), 2);
    }

    #[test]
    fn test_counting_waker_clone_adds_reference() {
        let counter = Arc::new(WakeCounter::default());
        let waker = counting_waker(Arc::clone(&counter));
        let clones: Vec<Waker> = (0..3).map(|_| waker.clone()).collect();
        assert_eq!(Arc::strong_count(&counter), 5);
        drop(clones);
        assert_eq!(Arc::strong_count(&counter), 2);
        drop(waker);
        assert_eq!(Arc::strong_count(&counter), 1);
    }

    #[test]
    fn test_counting_waker_wake_consumes_reference() {
        let counter = Arc::new(WakeCounter::default());
        let waker = counting_waker(Arc::clone(&counter));
        #[allow(clippy::waker_clone_wake)] // deliberately exercises `wake` on a clone
        waker.clone().wake();
        assert_eq!(Arc::strong_count(&counter), 2);
        waker.wake();
        assert_eq!(Arc::strong_count(&counter), 1);
        assert_eq!(counter.wakes(), 2);
    }

    #[test]
    fn test_counting_waker_will_wake_same_data() {
        let counter = Arc::new(WakeCounter::default());
        let waker = counting_waker(Arc::clone(&counter));
        let clone = waker.clone();
        assert!(waker.will_wake(&clone));

        let other = counting_waker(Arc::new(WakeCounter::default()));
        assert!(!waker.will_wake(&other));
    }

    #[test]
    fn test_counting_waker_across_threads() {
        let counter = Arc::new(WakeCounter::default());
        let waker = counting_waker(Arc::clone(&counter));
        let handles: Vec<_> = (0..4)
            .map(|_| {
                let w = waker.clone();
                thread::spawn(move || w.wake())
            })
            .collect();
        for handle in handles {
            handle.join().expect("waker thread panicked");
        }
        drop(waker);
        assert_eq!(counter.wakes(), 4);
        assert_eq!(Arc::strong_count(&counter), 1);
    }

    #[test]
    fn test_block_on_ready_future() {
        assert_eq!(block_on(async { 40 + 2 }), 42);
    }

    #[test]
    fn test_block_on_woken_from_other_thread() {
        let set = Arc::new(AtomicBool::new(false));
        let (tx, rx) = mpsc::channel::<Waker>();
        let setter = Arc::clone(&set);
        let helper = thread::spawn(move || {
            let waker = rx.recv().expect("future never registered a waker");
            thread::sleep(Duration::from_millis(5));
            setter.store(true, Ordering::Release);
            waker.wake();
        });

        let out = block_on(Flag {
            set,
            waker_slot: tx,
        });
        helper.join().expect("helper panicked");
        assert_eq!(out, "done");
    }

    #[test]
    fn test_thread_waker_notification_is_consumed_once() {
        let waker = ThreadWaker::current();
        assert!(!waker.take_notification());
        Waker::from(Arc::clone(&waker)).wake_by_ref();
        assert!(waker.take_notification());
        assert!(!waker.take_notification());
    }
}
