//! # Loom Models
//!
//! A stress test runs two threads a million times and hopes the scheduler hits
//! the bad interleaving. [Loom](https://docs.rs/loom) instead runs the test
//! under *every* interleaving (bounded), and models the C++11 memory model, so
//! it also catches orderings that are too weak — bugs x86 hardware hides.
//!
//! ## The pattern
//!
//! 1. Import atomics, `UnsafeCell`, `Arc` and `thread` from a **`sync` shim**
//!    that re-exports `loom::*` under `cfg(loom)` and `std::*` otherwise.
//! 2. Access interior data only through `UnsafeCell::with` / `with_mut`, so loom
//!    can check every access for data races.
//! 3. Write each concurrent **scenario once** as a plain function, then run it
//!    under `loom::model` (loom build) or in a repeated stress loop (normal build).
//!
//! ```bash
//! RUSTFLAGS="--cfg loom" cargo test --release --lib testing_craft::loom_model
//! ```
//!
//! `loom` is a `cfg(loom)`-only dependency (not a Cargo feature), so
//! `cargo build --all-features` never swaps std's primitives for loom's.
//!
//! ## Subjects under test
//!
//! | Type | Bug variant | What loom reports |
//! |------|-------------|-------------------|
//! | [`Counter`] | [`Counter::increment_racy_buggy`] (load + store) | lost update: final count 1, not 2 |
//! | [`SpinLock`] | [`SpinLock::new_relaxed_buggy`] (`Relaxed` acquire/release) | causality violation on the `UnsafeCell` |
//! | [`OneShot`] | — | proves exactly-once delivery, no leak, no double drop |

// loom's atomics and `UnsafeCell` have no `const fn new`, so constructors that
// must compile against both backends cannot be `const`.
#![allow(clippy::missing_const_for_fn)]

use std::mem::MaybeUninit;

use self::sync::atomic::{AtomicBool, AtomicU8, AtomicUsize, Ordering};
use self::sync::{UnsafeCell, spin_hint};

/// The shim: identical paths, two implementations.
#[cfg(loom)]
pub mod sync {
    pub use loom::cell::UnsafeCell;
    pub use loom::sync::{Arc, atomic};
    pub use loom::thread;

    /// In a spin loop, tell loom to schedule another thread; otherwise the
    /// model would explore an infinite spin.
    pub fn spin_hint() {
        loom::thread::yield_now();
    }
}

/// The shim: identical paths, two implementations.
#[cfg(not(loom))]
pub mod sync {
    pub use std::sync::{Arc, atomic};
    pub use std::thread;

    /// CPU spin-loop hint.
    pub fn spin_hint() {
        std::hint::spin_loop();
    }

    /// `std::cell::UnsafeCell` with loom's closure-scoped API, so the same
    /// code compiles against both.
    #[derive(Debug)]
    pub struct UnsafeCell<T>(std::cell::UnsafeCell<T>);

    impl<T> UnsafeCell<T> {
        /// Wraps `value`.
        pub const fn new(value: T) -> Self {
            Self(std::cell::UnsafeCell::new(value))
        }

        /// Runs `f` with a shared raw pointer to the contents.
        pub fn with<R>(&self, f: impl FnOnce(*const T) -> R) -> R {
            f(self.0.get())
        }

        /// Runs `f` with an exclusive raw pointer to the contents.
        pub fn with_mut<R>(&self, f: impl FnOnce(*mut T) -> R) -> R {
            f(self.0.get())
        }
    }
}

// ============================================================================
// Counter: the lost-update bug
// ============================================================================

/// A shared counter.
#[derive(Debug, Default)]
pub struct Counter {
    value: AtomicUsize,
}

impl Counter {
    /// Starts at zero.
    #[must_use]
    pub fn new() -> Self {
        Self {
            value: AtomicUsize::new(0),
        }
    }

    /// Correct: a single atomic read-modify-write.
    pub fn increment(&self) {
        self.value.fetch_add(1, Ordering::Relaxed);
    }

    /// BUGGY on purpose: two separate atomic operations. Two threads can both
    /// load 0 and both store 1. Each operation is atomic; the pair is not.
    pub fn increment_racy_buggy(&self) {
        let current = self.value.load(Ordering::SeqCst);
        self.value.store(current + 1, Ordering::SeqCst);
    }

    /// Current value.
    #[must_use]
    pub fn get(&self) -> usize {
        self.value.load(Ordering::SeqCst)
    }
}

// ============================================================================
// SpinLock: memory ordering matters
// ============================================================================

/// A test-and-test-and-set spin lock with a closure API.
///
/// Correct orderings: `Acquire` on lock (see the previous holder's writes),
/// `Release` on unlock (publish ours).
#[derive(Debug)]
pub struct SpinLock<T> {
    locked: AtomicBool,
    acquire: Ordering,
    release: Ordering,
    value: UnsafeCell<T>,
}

// SAFETY: the lock grants exclusive access to `value`, and `T` moves between
// threads only through that exclusive access, so `T: Send` suffices.
unsafe impl<T: Send> Sync for SpinLock<T> {}

/// Releases the lock on drop, so a panicking critical section cannot deadlock.
struct Unlock<'a> {
    locked: &'a AtomicBool,
    release: Ordering,
}

impl Drop for Unlock<'_> {
    fn drop(&mut self) {
        self.locked.store(false, self.release);
    }
}

impl<T> SpinLock<T> {
    /// Creates an unlocked lock with correct orderings.
    pub fn new(value: T) -> Self {
        Self::with_orderings(value, Ordering::Acquire, Ordering::Release)
    }

    /// BUGGY on purpose: `Relaxed` lock and unlock. Mutual exclusion still
    /// holds, but writes made inside one critical section are not guaranteed
    /// to be visible in the next. Loom reports a causality violation.
    pub fn new_relaxed_buggy(value: T) -> Self {
        Self::with_orderings(value, Ordering::Relaxed, Ordering::Relaxed)
    }

    fn with_orderings(value: T, acquire: Ordering, release: Ordering) -> Self {
        Self {
            locked: AtomicBool::new(false),
            acquire,
            release,
            value: UnsafeCell::new(value),
        }
    }

    fn acquire_lock(&self) -> Unlock<'_> {
        while self
            .locked
            .compare_exchange_weak(false, true, self.acquire, Ordering::Relaxed)
            .is_err()
        {
            // Spin on a plain load: cheaper than hammering the cache line with CAS.
            while self.locked.load(Ordering::Relaxed) {
                spin_hint();
            }
        }
        Unlock {
            locked: &self.locked,
            release: self.release,
        }
    }

    /// Runs `f` with exclusive access to the protected value.
    pub fn with_lock<R>(&self, f: impl FnOnce(&mut T) -> R) -> R {
        let _unlock = self.acquire_lock();
        // SAFETY: holding the lock gives exclusive access; the reference does
        // not escape `f`, and `_unlock` outlives the call.
        self.value.with_mut(|ptr| f(unsafe { &mut *ptr }))
    }
}

// ============================================================================
// OneShot: publish a value exactly once
// ============================================================================

const EMPTY: u8 = 0;
const WRITING: u8 = 1;
const READY: u8 = 2;
const TAKEN: u8 = 3;

/// A single-use slot: at most one `send` succeeds, at most one `try_recv`
/// returns the value. State machine:
///
/// ```text
/// EMPTY --send CAS--> WRITING --store(Release)--> READY --recv CAS(Acquire)--> TAKEN
/// ```
#[derive(Debug)]
pub struct OneShot<T> {
    state: AtomicU8,
    slot: UnsafeCell<MaybeUninit<T>>,
}

// SAFETY: the state machine hands the slot to exactly one writer and then to
// exactly one reader; `T` is moved, never shared, so `T: Send` suffices.
unsafe impl<T: Send> Sync for OneShot<T> {}

impl<T> OneShot<T> {
    /// An empty slot.
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: AtomicU8::new(EMPTY),
            slot: UnsafeCell::new(MaybeUninit::uninit()),
        }
    }

    /// Stores `value` if the slot was never written.
    ///
    /// # Errors
    ///
    /// Gives `value` back if another `send` already claimed the slot.
    pub fn send(&self, value: T) -> Result<(), T> {
        if self
            .state
            .compare_exchange(EMPTY, WRITING, Ordering::Relaxed, Ordering::Relaxed)
            .is_err()
        {
            return Err(value);
        }
        // SAFETY: winning the EMPTY->WRITING CAS makes us the only writer, and
        // no reader touches the slot before it observes READY.
        self.slot.with_mut(|ptr| unsafe { (*ptr).write(value) });
        self.state.store(READY, Ordering::Release);
        Ok(())
    }

    /// Takes the value if it has been published and not yet taken.
    pub fn try_recv(&self) -> Option<T> {
        self.state
            .compare_exchange(READY, TAKEN, Ordering::Acquire, Ordering::Relaxed)
            .ok()?;
        // SAFETY: READY means initialized (Acquire pairs with the sender's
        // Release); winning READY->TAKEN makes us the only reader.
        Some(self.slot.with(|ptr| unsafe { (*ptr).assume_init_read() }))
    }
}

impl<T> Default for OneShot<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Drop for OneShot<T> {
    fn drop(&mut self) {
        if self.state.load(Ordering::Acquire) == READY {
            // SAFETY: READY means initialized and never taken; `&mut self`
            // means no one else can observe the slot.
            self.slot
                .with_mut(|ptr| unsafe { (*ptr).assume_init_drop() });
        }
    }
}

// ============================================================================
// Scenarios: written once, run under loom or as std stress tests
// ============================================================================

#[cfg(test)]
mod scenarios {
    use super::sync::{Arc, thread};
    use super::{Counter, OneShot, SpinLock};

    pub fn counter_two_threads(racy: bool) {
        let counter = Arc::new(Counter::new());
        let handles: Vec<_> = (0..2)
            .map(|_| {
                let counter = Arc::clone(&counter);
                thread::spawn(move || {
                    if racy {
                        counter.increment_racy_buggy();
                    } else {
                        counter.increment();
                    }
                })
            })
            .collect();
        for handle in handles {
            handle.join().unwrap();
        }
        assert_eq!(counter.get(), 2, "lost update");
    }

    pub fn spinlock_two_threads(lock: SpinLock<usize>) {
        let lock = Arc::new(lock);
        let other = Arc::clone(&lock);
        let handle = thread::spawn(move || other.with_lock(|v| *v += 1));
        lock.with_lock(|v| *v += 1);
        handle.join().unwrap();
        assert_eq!(lock.with_lock(|v| *v), 2);
    }

    pub fn oneshot_send_recv() {
        let slot = Arc::new(OneShot::new());
        let tx = Arc::clone(&slot);
        let sender = thread::spawn(move || tx.send(String::from("hi")).is_ok());
        // Receiver races the sender: it may see nothing yet, never garbage.
        let early = slot.try_recv();
        assert!(sender.join().unwrap());
        let got = early.or_else(|| slot.try_recv());
        assert_eq!(got.as_deref(), Some("hi"));
        assert_eq!(slot.try_recv(), None, "delivered twice");
    }

    pub fn oneshot_racing_senders() {
        let slot = Arc::new(OneShot::new());
        let a = Arc::clone(&slot);
        let b = Arc::clone(&slot);
        let ta = thread::spawn(move || a.send(1).is_ok());
        let tb = thread::spawn(move || b.send(2).is_ok());
        let wins = usize::from(ta.join().unwrap()) + usize::from(tb.join().unwrap());
        assert_eq!(wins, 1, "exactly one sender wins");
        assert!(matches!(slot.try_recv(), Some(1 | 2)));
    }

    pub fn oneshot_dropped_unread() {
        // A value that is sent but never received must still be dropped.
        let witness = Arc::new(());
        let slot = OneShot::new();
        slot.send(Arc::clone(&witness)).unwrap();
        drop(slot);
        assert_eq!(Arc::strong_count(&witness), 1, "leaked");
    }
}

#[cfg(all(test, loom))]
mod loom_tests {
    use super::SpinLock;
    use super::scenarios;

    fn bounded(f: impl Fn() + Sync + Send + 'static) {
        let mut builder = loom::model::Builder::new();
        builder.preemption_bound = Some(3);
        builder.check(f);
    }

    #[test]
    fn loom_counter_fetch_add_is_correct() {
        loom::model(|| scenarios::counter_two_threads(false));
    }

    #[test]
    #[should_panic(expected = "lost update")]
    fn loom_finds_lost_update() {
        loom::model(|| scenarios::counter_two_threads(true));
    }

    #[test]
    fn loom_spinlock_is_correct() {
        bounded(|| scenarios::spinlock_two_threads(SpinLock::new(0)));
    }

    #[test]
    #[should_panic(expected = "Causality violation")]
    fn loom_finds_relaxed_spinlock_race() {
        bounded(|| scenarios::spinlock_two_threads(SpinLock::new_relaxed_buggy(0)));
    }

    #[test]
    fn loom_oneshot_send_recv() {
        loom::model(scenarios::oneshot_send_recv);
    }

    #[test]
    fn loom_oneshot_racing_senders() {
        loom::model(scenarios::oneshot_racing_senders);
    }

    #[test]
    fn loom_oneshot_dropped_unread() {
        loom::model(scenarios::oneshot_dropped_unread);
    }
}

#[cfg(all(test, not(loom)))]
mod std_tests {
    use super::{Counter, OneShot, SpinLock, scenarios};

    const ROUNDS: usize = 200;

    #[test]
    fn stress_counter() {
        for _ in 0..ROUNDS {
            scenarios::counter_two_threads(false);
        }
    }

    #[test]
    fn stress_spinlock() {
        for _ in 0..ROUNDS {
            scenarios::spinlock_two_threads(SpinLock::new(0));
        }
    }

    #[test]
    fn stress_oneshot() {
        for _ in 0..ROUNDS {
            scenarios::oneshot_send_recv();
            scenarios::oneshot_racing_senders();
        }
        scenarios::oneshot_dropped_unread();
    }

    #[test]
    fn test_single_threaded_behavior() {
        // Without contention even the buggy variants "work" — which is
        // exactly why they need loom rather than example tests.
        let counter = Counter::default();
        counter.increment_racy_buggy();
        counter.increment();
        assert_eq!(counter.get(), 2);

        let lock = SpinLock::new_relaxed_buggy(vec![1]);
        lock.with_lock(|v| v.push(2));
        assert_eq!(lock.with_lock(|v| v.len()), 2);

        let slot = OneShot::default();
        assert_eq!(slot.try_recv(), None);
        assert_eq!(slot.send(5), Ok(()));
        assert_eq!(slot.send(6), Err(6));
        assert_eq!(slot.try_recv(), Some(5));
        assert_eq!(slot.try_recv(), None);
    }

    #[test]
    fn test_spinlock_unlocks_after_panic() {
        let lock = SpinLock::new(0);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            lock.with_lock(|_| panic!("boom"));
        }));
        assert!(result.is_err());
        assert_eq!(lock.with_lock(|v| *v), 0, "lock was released by the guard");
    }
}
