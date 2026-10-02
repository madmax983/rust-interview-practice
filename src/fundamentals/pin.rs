//! # Pin, Unpin, and Pin Projection
//!
//! `Pin<P>` is a promise about the value behind the pointer `P`: it will never move
//! again, and its memory will not be reused until its destructor has run. If the value
//! is `Unpin`, the promise is empty and `Pin` is just a thin wrapper around `P`.
//!
//! Drills in this module:
//!
//! 1. **`Unpin` types** — `Pin::new`, `Pin::get_mut`, `Pin::into_inner`: no unsafe needed.
//! 2. **`!Unpin` types** — `PhantomPinned`, `Box::pin`, `pin!`, and `Pin::set`.
//! 3. **Self-referential structs** — sound only because the value is pinned.
//! 4. **Pin projection** — structural (`Pin<&mut Field>`) vs. non-structural (`&mut Field`).
//! 5. **Enum projection** — `MaybeDone` and a hand-written two-way `join`.
//! 6. **The drop guarantee** — pinned memory is dropped in place before it is reused.
//! 7. **`Unpin` escape hatches** — `Pin<Box<T>>` is always `Unpin`.
//!
//! ## Example
//!
//! ```
//! use rust_interview_practice::fundamentals::pin::{Map, YieldNow, block_on, join};
//!
//! // `YieldNow` is `!Unpin`; `Map` projects to it with `Pin<&mut YieldNow<_>>`.
//! let doubled = Map::new(YieldNow::new(3, 21), |n| n * 2);
//! assert_eq!(block_on(doubled), 42);
//!
//! // `join` drives two pinned futures inside one pinned struct.
//! assert_eq!(block_on(join(YieldNow::new(1, 'a'), YieldNow::new(4, 'b'))), ('a', 'b'));
//! ```
//!
//! ## The four rules of structural pinning
//!
//! A field is *structurally pinned* when `Pin<&mut Struct>` projects to `Pin<&mut Field>`.
//! Doing that by hand (what `pin-project` generates) is sound only if:
//!
//! 1. The struct is `Unpin` only when every structurally pinned field is `Unpin`.
//! 2. The struct's `Drop` never moves out of a structurally pinned field.
//! 3. No method moves out of a structurally pinned field while the struct is pinned
//!    (e.g. `Option::take` on a pinned `Option<Fut>` is forbidden).
//! 4. The struct is not `#[repr(packed)]` (the compiler may move fields to align them).

use std::cell::RefCell;
use std::collections::HashSet;
use std::future::Future;
use std::marker::PhantomPinned;
use std::pin::{Pin, pin};
use std::ptr::NonNull;
use std::rc::Rc;
use std::task::{Context, Poll, Waker, ready};

// ============================================================================
// 1. Unpin types: Pin is a no-op wrapper
// ============================================================================

/// Swaps the value behind a pinned reference and returns the old one.
///
/// Only legal because `T: Unpin`: `Pin::get_mut` hands back a plain `&mut T`,
/// so moving the old value out with `mem::replace` breaks no promise.
pub const fn replace_unpin<T: Unpin>(pinned: Pin<&mut T>, value: T) -> T {
    std::mem::replace(Pin::get_mut(pinned), value)
}

/// Pins a `Box` of an `Unpin` value and unpins it again — both directions are safe.
///
/// `Pin::new` requires the pointee to be `Unpin`; `Pin::into_inner` gives the box back.
#[must_use]
pub const fn unpin_round_trip<T: Unpin>(boxed: Box<T>) -> Box<T> {
    let pinned: Pin<Box<T>> = Pin::new(boxed);
    Pin::into_inner(pinned)
}

// ============================================================================
// 2. !Unpin types: PhantomPinned, Box::pin, pin!, Pin::set
// ============================================================================

/// A value that opts out of `Unpin` with a `PhantomPinned` marker.
///
/// Once pinned it can never be moved, so its address is a stable identity.
#[derive(Debug)]
pub struct Pinned {
    value: u32,
    _pin: PhantomPinned,
}

impl Pinned {
    /// Creates an unpinned `Pinned`; pin it with `Box::pin` or `pin!` before use.
    #[must_use]
    pub const fn new(value: u32) -> Self {
        Self {
            value,
            _pin: PhantomPinned,
        }
    }

    /// Reads through `Pin<&Self>` — `Pin<&T>` derefs to `&T` with no unsafe.
    #[must_use]
    pub fn value(self: Pin<&Self>) -> u32 {
        self.value
    }

    /// Writes a field in place. `Pin<&mut T>` does not deref mutably for `!Unpin` `T`,
    /// so we need `get_unchecked_mut`.
    pub const fn set_value(self: Pin<&mut Self>, value: u32) {
        // SAFETY: we only overwrite a `Copy` field in place; `self` is never moved.
        unsafe { self.get_unchecked_mut() }.value = value;
    }

    /// The address of the pinned value. Stable for as long as the value lives.
    #[must_use]
    pub fn address(self: Pin<&Self>) -> usize {
        std::ptr::from_ref(self.get_ref()).addr()
    }
}

/// Replaces a pinned `!Unpin` value in place with `Pin::set`.
///
/// `Pin::set` is safe even for `!Unpin` types: it drops the old value *in place* and
/// writes the new one into the same memory, so nothing pinned is ever moved.
pub fn replace_pinned(mut slot: Pin<&mut Pinned>, value: u32) {
    slot.set(Pinned::new(value));
}

// ============================================================================
// 3. Self-referential structs
// ============================================================================

/// A struct holding a pointer to one of its own fields.
///
/// Moving it would leave `self_ptr` dangling, so it is `!Unpin` and the pointer is only
/// created *after* the value is pinned (`init`). This mirrors the `Unmovable` example in
/// the `std::pin` docs.
#[derive(Debug)]
pub struct SelfRef {
    data: String,
    /// Points at `self.data` once initialised. Only valid while pinned.
    self_ptr: Option<NonNull<String>>,
    _pin: PhantomPinned,
}

impl SelfRef {
    /// Heap-pins a new `SelfRef` and initialises its self-pointer.
    #[must_use]
    pub fn new(data: impl Into<String>) -> Pin<Box<Self>> {
        let mut boxed = Box::pin(Self::unpinned(data));
        boxed.as_mut().init();
        boxed
    }

    /// Creates an uninitialised `SelfRef` (no self-pointer yet), e.g. for `pin!`.
    #[must_use]
    pub fn unpinned(data: impl Into<String>) -> Self {
        Self {
            data: data.into(),
            self_ptr: None,
            _pin: PhantomPinned,
        }
    }

    /// Points `self_ptr` at `self.data`. Requires a pin, so the address is final.
    pub fn init(self: Pin<&mut Self>) {
        // SAFETY: we write a pointer to our own field and never move `self` out.
        let this = unsafe { self.get_unchecked_mut() };
        this.self_ptr = Some(NonNull::from(&this.data));
    }

    /// Reads the data directly.
    #[must_use]
    pub fn data(self: Pin<&Self>) -> &str {
        &self.get_ref().data
    }

    /// Reads the data through the self-pointer, or `None` before `init`.
    #[must_use]
    pub fn data_via_self_ptr(self: Pin<&Self>) -> Option<&str> {
        let ptr = self.get_ref().self_ptr?;
        // SAFETY: `ptr` was created from `&self.data` in `init`, while `self` was already
        // pinned. Pinning means `self` has not moved since, so `ptr` still points at a live
        // `String`, and we only hand out a shared borrow tied to `&self`.
        Some(unsafe { ptr.as_ref() }.as_str())
    }

    /// True when `self_ptr` still points at `self.data` — the invariant pinning protects.
    #[must_use]
    pub fn is_self_ptr_valid(self: Pin<&Self>) -> bool {
        self.self_ptr == Some(NonNull::from(&self.data))
    }
}

// ============================================================================
// Leaf future + executor used by the projection drills
// ============================================================================

/// A `!Unpin` leaf future: returns `Pending` `remaining` times, then `Ready(value)`.
///
/// It wakes itself on every `Pending`, so `block_on` always makes progress.
///
/// # Panics
///
/// Polling after it has returned `Ready` panics, per the `Future` contract.
#[derive(Debug)]
pub struct YieldNow<T> {
    remaining: u32,
    value: Option<T>,
    _pin: PhantomPinned,
}

impl<T> YieldNow<T> {
    /// Creates a future that yields `remaining` times before producing `value`.
    #[must_use]
    pub const fn new(remaining: u32, value: T) -> Self {
        Self {
            remaining,
            value: Some(value),
            _pin: PhantomPinned,
        }
    }
}

impl<T> Future for YieldNow<T> {
    type Output = T;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<T> {
        // SAFETY: neither field is structurally pinned; we never move `self` itself.
        let this = unsafe { self.get_unchecked_mut() };
        if this.remaining == 0 {
            // Polling a completed future is a caller bug; panicking is the std convention.
            Poll::Ready(this.value.take().expect("YieldNow polled after completion"))
        } else {
            this.remaining -= 1;
            cx.waker().wake_by_ref();
            Poll::Pending
        }
    }
}

/// Drives a future to completion on the current thread with a no-op waker.
///
/// `pin!` stack-pins the future, which is all `poll` needs. This spins, so it is only
/// suitable for futures that make progress on every poll (like [`YieldNow`]).
pub fn block_on<F: Future>(fut: F) -> F::Output {
    let mut fut = pin!(fut);
    let mut cx = Context::from_waker(Waker::noop());
    loop {
        if let Poll::Ready(out) = fut.as_mut().poll(&mut cx) {
            return out;
        }
    }
}

// ============================================================================
// 4. Pin projection by hand (what pin-project generates)
// ============================================================================

/// `future.map(f)`: `future` is structurally pinned, `f` is not.
#[derive(Debug)]
pub struct Map<Fut, F> {
    future: Fut,
    f: Option<F>,
}

/// The projection of `Pin<&mut Map>`: a pinned view of `future`, a plain `&mut` of `f`.
struct MapProj<'a, Fut, F> {
    future: Pin<&'a mut Fut>,
    f: &'a mut Option<F>,
}

impl<Fut, F> Map<Fut, F> {
    /// Wraps `future`, applying `f` to its output.
    #[must_use]
    pub const fn new(future: Fut, f: F) -> Self {
        Self { future, f: Some(f) }
    }

    const fn project(self: Pin<&mut Self>) -> MapProj<'_, Fut, F> {
        // SAFETY: `future` is structurally pinned and the four rules hold — `Map` is `Unpin`
        // only when `Fut: Unpin` (impl below), it has no `Drop`, nothing moves `future` out,
        // and it is not `repr(packed)`. `f` is not structurally pinned, so `&mut` is fine.
        let this = unsafe { self.get_unchecked_mut() };
        MapProj {
            // SAFETY: see above; `this.future` is never moved while `self` is pinned.
            future: unsafe { Pin::new_unchecked(&mut this.future) },
            f: &mut this.f,
        }
    }
}

// Rule 1: `Unpin` depends only on structurally pinned fields. `F` is moved freely
// (`Option::take`), so `Map` stays `Unpin` even when the closure is not.
impl<Fut: Unpin, F> Unpin for Map<Fut, F> {}

impl<Fut, F, T> Future for Map<Fut, F>
where
    Fut: Future,
    F: FnOnce(Fut::Output) -> T,
{
    type Output = T;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<T> {
        let this = self.project();
        let output = ready!(this.future.poll(cx));
        // Moving out of `f` is allowed: it is not structurally pinned.
        let f = this.f.take().expect("Map polled after completion");
        Poll::Ready(f(output))
    }
}

/// Counts how many times the inner future was polled. Output: `(inner output, polls)`.
#[derive(Debug)]
pub struct Counted<Fut> {
    inner: Fut,
    polls: u32,
}

impl<Fut> Counted<Fut> {
    /// Wraps `inner` with a poll counter starting at zero.
    #[must_use]
    pub const fn new(inner: Fut) -> Self {
        Self { inner, polls: 0 }
    }

    const fn project(self: Pin<&mut Self>) -> (Pin<&mut Fut>, &mut u32) {
        // SAFETY: `inner` is structurally pinned (auto `Unpin` follows `Fut`, no `Drop`,
        // never moved, not packed); `polls` is a plain counter we only mutate in place.
        let this = unsafe { self.get_unchecked_mut() };
        // SAFETY: as above.
        (
            unsafe { Pin::new_unchecked(&mut this.inner) },
            &mut this.polls,
        )
    }
}

impl<Fut: Future> Future for Counted<Fut> {
    type Output = (Fut::Output, u32);

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let (inner, polls) = self.project();
        *polls += 1;
        let output = ready!(inner.poll(cx));
        Poll::Ready((output, *polls))
    }
}

// ============================================================================
// 5. Enum projection: MaybeDone + join
// ============================================================================

/// A future slot that remembers its output once complete (as in `futures-util`).
#[derive(Debug)]
pub enum MaybeDone<Fut: Future> {
    /// Still running; the future is structurally pinned.
    Future(Fut),
    /// Finished; the output is *not* structurally pinned and may be moved out.
    Done(Fut::Output),
    /// The output has been taken.
    Gone,
}

impl<Fut: Future> MaybeDone<Fut> {
    /// Polls the inner future; `Ready(())` once an output is stored.
    ///
    /// # Panics
    ///
    /// Panics if called after the output was taken.
    pub fn poll_done(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        // SAFETY: we only project to `Pin<&mut Fut>` in the `Future` variant and never
        // move the future out; it leaves the slot via `Pin::set`, which drops it in place.
        let fut = match unsafe { self.as_mut().get_unchecked_mut() } {
            // SAFETY: as above.
            Self::Future(fut) => unsafe { Pin::new_unchecked(fut) },
            Self::Done(_) => return Poll::Ready(()),
            Self::Gone => panic!("MaybeDone polled after output was taken"),
        };
        let output = ready!(fut.poll(cx));
        self.set(Self::Done(output));
        Poll::Ready(())
    }

    /// Moves the output out, leaving `Gone`. `None` if not finished (or already taken).
    pub fn take_output(self: Pin<&mut Self>) -> Option<Fut::Output> {
        // SAFETY: we only move the enum when it is `Done`, which holds no pinned data.
        let this = unsafe { self.get_unchecked_mut() };
        match this {
            Self::Done(_) => match std::mem::replace(this, Self::Gone) {
                Self::Done(output) => Some(output),
                Self::Future(_) | Self::Gone => None,
            },
            Self::Future(_) | Self::Gone => None,
        }
    }
}

/// Polls two futures concurrently and returns both outputs.
pub struct Join<A: Future, B: Future> {
    a: MaybeDone<A>,
    b: MaybeDone<B>,
}

// `#[derive(Debug)]` would only bound `A: Debug, B: Debug`, not their outputs.
impl<A: Future, B: Future> std::fmt::Debug for Join<A, B>
where
    MaybeDone<A>: std::fmt::Debug,
    MaybeDone<B>: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Join")
            .field("a", &self.a)
            .field("b", &self.b)
            .finish()
    }
}

/// Joins two futures: polls both on every wake-up until both are done.
#[must_use]
pub const fn join<A: Future, B: Future>(a: A, b: B) -> Join<A, B> {
    Join {
        a: MaybeDone::Future(a),
        b: MaybeDone::Future(b),
    }
}

impl<A: Future, B: Future> Join<A, B> {
    const fn project(self: Pin<&mut Self>) -> (Pin<&mut MaybeDone<A>>, Pin<&mut MaybeDone<B>>) {
        // SAFETY: both fields are structurally pinned; `Join` is auto-`Unpin` only when both
        // are, has no `Drop`, never moves them, and is not packed.
        let this = unsafe { self.get_unchecked_mut() };
        // SAFETY: as above.
        unsafe {
            (
                Pin::new_unchecked(&mut this.a),
                Pin::new_unchecked(&mut this.b),
            )
        }
    }
}

impl<A: Future, B: Future> Future for Join<A, B> {
    type Output = (A::Output, B::Output);

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let (mut a, mut b) = self.project();
        let a_ready = a.as_mut().poll_done(cx).is_ready();
        let b_ready = b.as_mut().poll_done(cx).is_ready();
        if !(a_ready && b_ready) {
            return Poll::Pending;
        }
        // Invariant: both slots reported `Ready` just above, so both hold `Done`.
        let a_out = a.take_output().expect("a is Done");
        let b_out = b.take_output().expect("b is Done");
        Poll::Ready((a_out, b_out))
    }
}

// ============================================================================
// 6. The drop guarantee
// ============================================================================

/// Addresses of live, pinned [`Waiter`]s — a stand-in for an intrusive wait list.
///
/// Storing raw addresses is only sound because of the drop guarantee: a pinned
/// `Waiter`'s memory cannot be freed or reused until its `Drop` has deregistered it.
#[derive(Debug, Clone, Default)]
pub struct Registry {
    live: Rc<RefCell<HashSet<usize>>>,
}

impl Registry {
    /// Creates an empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// True if a waiter at `address` is registered.
    #[must_use]
    pub fn contains(&self, address: usize) -> bool {
        self.live.borrow().contains(&address)
    }

    /// Number of registered waiters.
    #[must_use]
    pub fn len(&self) -> usize {
        self.live.borrow().len()
    }

    /// True if no waiters are registered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.live.borrow().is_empty()
    }
}

/// A node that registers its own address and deregisters it on drop.
#[derive(Debug)]
pub struct Waiter {
    registry: Registry,
    registered: bool,
    _pin: PhantomPinned,
}

impl Waiter {
    /// Creates an unregistered waiter attached to `registry`.
    #[must_use]
    pub fn new(registry: &Registry) -> Self {
        Self {
            registry: registry.clone(),
            registered: false,
            _pin: PhantomPinned,
        }
    }

    /// Registers this waiter's address. Requires a pin: the address must never change.
    #[allow(clippy::must_use_candidate)] // called for its side effect; the address is a bonus
    pub fn register(self: Pin<&mut Self>) -> usize {
        let address = self.as_ref().address();
        // SAFETY: we only flip a `bool` flag; `self` is never moved.
        let this = unsafe { self.get_unchecked_mut() };
        if !this.registered {
            this.registry.live.borrow_mut().insert(address);
            this.registered = true;
        }
        address
    }

    /// The (stable, because pinned) address of this waiter.
    #[must_use]
    pub fn address(self: Pin<&Self>) -> usize {
        std::ptr::from_ref(self.get_ref()).addr()
    }

    /// True once `register` has been called.
    #[must_use]
    pub const fn is_registered(&self) -> bool {
        self.registered
    }
}

impl Drop for Waiter {
    fn drop(&mut self) {
        // `drop` takes `&mut self` even for pinned types. Immediately re-wrap it so the
        // body cannot accidentally move anything (the pattern from the `std::pin` docs).
        fn inner_drop(this: Pin<&mut Waiter>) {
            if this.registered {
                let address = this.as_ref().address();
                this.registry.live.borrow_mut().remove(&address);
            }
        }
        // SAFETY: `self` is never used again after `drop`, so treating it as pinned
        // for the rest of its life is trivially upheld.
        inner_drop(unsafe { Pin::new_unchecked(self) });
    }
}

// ============================================================================
// 7. Unpin escape hatches
// ============================================================================

/// A type-erased, heap-pinned future. `Pin<Box<_>>` is always `Unpin`.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + 'a>>;

/// Boxes any future (even a `!Unpin` one) into an `Unpin` [`BoxFuture`].
pub fn boxed<'a, F: Future + 'a>(fut: F) -> BoxFuture<'a, F::Output> {
    Box::pin(fut)
}

/// Polls an `Unpin` future through a plain `&mut` — no `unsafe`, no `pin!`.
pub fn poll_unpin<F: Future + Unpin>(fut: &mut F, cx: &mut Context<'_>) -> Poll<F::Output> {
    Pin::new(fut).poll(cx)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    /// Compiles only if `T: Unpin`.
    const fn assert_unpin<T: Unpin + ?Sized>() {}

    /// Compiles only if `$t: !Unpin` (the `static_assertions` ambiguity trick: with two
    /// applicable impls the `_` cannot be inferred and compilation fails).
    macro_rules! assert_not_unpin {
        ($t:ty) => {{
            trait AmbiguousIfUnpin<A> {
                fn some_item() {}
            }
            impl<T: ?Sized> AmbiguousIfUnpin<()> for T {}
            impl<T: ?Sized + Unpin> AmbiguousIfUnpin<u8> for T {}
            let _ = <$t as AmbiguousIfUnpin<_>>::some_item;
        }};
    }

    fn noop_cx() -> Context<'static> {
        Context::from_waker(Waker::noop())
    }

    /// A future that is ready immediately and records when it is dropped.
    struct DropFlag<'a> {
        dropped: &'a Cell<bool>,
    }

    impl Future for DropFlag<'_> {
        type Output = u8;
        fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<u8> {
            Poll::Ready(7)
        }
    }

    impl Drop for DropFlag<'_> {
        fn drop(&mut self) {
            self.dropped.set(true);
        }
    }

    // ---- 1. Unpin -----------------------------------------------------------

    #[test]
    fn test_replace_unpin_moves_old_value_out() {
        let mut s = String::from("old");
        let old = replace_unpin(Pin::new(&mut s), String::from("new"));
        assert_eq!(old, "old");
        assert_eq!(s, "new");
    }

    #[test]
    fn test_unpin_round_trip_preserves_value() {
        assert_eq!(*unpin_round_trip(Box::new(42)), 42);
    }

    #[test]
    fn test_unpin_and_not_unpin_classification() {
        assert_unpin::<String>();
        assert_unpin::<Pin<Box<SelfRef>>>();
        assert_unpin::<BoxFuture<'static, u8>>();
        assert_unpin::<Map<std::future::Ready<u8>, fn(u8) -> u8>>();
        assert_not_unpin!(Pinned);
        assert_not_unpin!(SelfRef);
        assert_not_unpin!(Waiter);
        assert_not_unpin!(YieldNow<u8>);
        assert_not_unpin!(Map<YieldNow<u8>, fn(u8) -> u8>);
        assert_not_unpin!(Join<YieldNow<u8>, std::future::Ready<u8>>);
    }

    // ---- 2. !Unpin ----------------------------------------------------------

    #[test]
    fn test_box_pin_address_is_stable_across_moves_of_the_box() {
        let mut pinned = Box::pin(Pinned::new(1));
        let before = pinned.as_ref().address();
        pinned.as_mut().set_value(2);
        let moved = [pinned]; // moves the Box, not the pointee
        assert_eq!(moved[0].as_ref().address(), before);
        assert_eq!(moved[0].as_ref().value(), 2);
    }

    #[test]
    fn test_stack_pin_macro() {
        let mut pinned = pin!(Pinned::new(5));
        pinned.as_mut().set_value(6);
        assert_eq!(pinned.as_ref().value(), 6);
    }

    #[test]
    fn test_pin_set_replaces_in_place() {
        let mut pinned = Box::pin(Pinned::new(1));
        let before = pinned.as_ref().address();
        replace_pinned(pinned.as_mut(), 9);
        assert_eq!(pinned.as_ref().value(), 9);
        assert_eq!(pinned.as_ref().address(), before);
    }

    // ---- 3. Self-referential ------------------------------------------------

    #[test]
    fn test_self_ref_heap_pinned_survives_moving_the_box() {
        let boxed = SelfRef::new("hello");
        let moved = Some(boxed);
        let pinned = moved.as_ref().expect("just set").as_ref();
        assert!(pinned.is_self_ptr_valid());
        assert_eq!(pinned.data_via_self_ptr(), Some("hello"));
        assert_eq!(pinned.data(), "hello");
    }

    #[test]
    fn test_self_ref_stack_pinned_requires_init() {
        let mut pinned = pin!(SelfRef::unpinned("stack"));
        assert_eq!(pinned.as_ref().data_via_self_ptr(), None);
        assert!(!pinned.as_ref().is_self_ptr_valid());
        pinned.as_mut().init();
        assert!(pinned.as_ref().is_self_ptr_valid());
        assert_eq!(pinned.as_ref().data_via_self_ptr(), Some("stack"));
    }

    #[test]
    fn test_self_ref_empty_string() {
        let boxed = SelfRef::new("");
        assert_eq!(boxed.as_ref().data_via_self_ptr(), Some(""));
    }

    // ---- 4. Projection ------------------------------------------------------

    #[test]
    fn test_yield_now_pending_then_ready() {
        let mut fut = pin!(YieldNow::new(2, "done"));
        let mut cx = noop_cx();
        assert!(fut.as_mut().poll(&mut cx).is_pending());
        assert!(fut.as_mut().poll(&mut cx).is_pending());
        assert_eq!(fut.as_mut().poll(&mut cx), Poll::Ready("done"));
    }

    #[test]
    #[should_panic(expected = "polled after completion")]
    fn test_yield_now_poll_after_completion_panics() {
        let mut fut = pin!(YieldNow::new(0, 1));
        let mut cx = noop_cx();
        let _ = fut.as_mut().poll(&mut cx);
        let _ = fut.as_mut().poll(&mut cx);
    }

    #[test]
    fn test_map_over_pinned_future() {
        assert_eq!(block_on(Map::new(YieldNow::new(3, 20), |n| n + 1)), 21);
    }

    #[test]
    fn test_map_zero_yields() {
        assert_eq!(block_on(Map::new(YieldNow::new(0, "a"), str::len)), 1);
    }

    #[test]
    fn test_map_is_unpin_over_unpin_future_and_polls_without_pin() {
        let mut fut = Map::new(std::future::ready(4), |n: i32| n * n);
        assert_eq!(poll_unpin(&mut fut, &mut noop_cx()), Poll::Ready(16));
    }

    #[test]
    fn test_counted_reports_number_of_polls() {
        assert_eq!(block_on(Counted::new(YieldNow::new(4, 'x'))), ('x', 5));
        assert_eq!(block_on(Counted::new(YieldNow::new(0, 'y'))), ('y', 1));
    }

    // ---- 5. Enum projection -------------------------------------------------

    #[test]
    fn test_maybe_done_lifecycle() {
        let mut slot = pin!(MaybeDone::Future(YieldNow::new(1, 10)));
        let mut cx = noop_cx();
        assert_eq!(slot.as_mut().take_output(), None);
        assert!(slot.as_mut().poll_done(&mut cx).is_pending());
        assert!(slot.as_mut().poll_done(&mut cx).is_ready());
        assert!(slot.as_mut().poll_done(&mut cx).is_ready()); // idempotent once Done
        assert_eq!(slot.as_mut().take_output(), Some(10));
        assert_eq!(slot.as_mut().take_output(), None);
        assert!(matches!(*slot, MaybeDone::Gone));
    }

    #[test]
    fn test_maybe_done_drops_future_in_place_when_done() {
        let dropped = Cell::new(false);
        let mut slot = pin!(MaybeDone::Future(DropFlag { dropped: &dropped }));
        assert!(!dropped.get());
        assert!(slot.as_mut().poll_done(&mut noop_cx()).is_ready());
        assert!(dropped.get(), "Pin::set must drop the finished future");
        assert_eq!(slot.as_mut().take_output(), Some(7));
    }

    #[test]
    fn test_join_returns_both_outputs() {
        assert_eq!(
            block_on(join(YieldNow::new(1, "a"), YieldNow::new(5, 2))),
            ("a", 2)
        );
    }

    #[test]
    fn test_join_polls_until_slowest_finishes() {
        let (outputs, polls) =
            block_on(Counted::new(join(YieldNow::new(6, 1), YieldNow::new(2, 2))));
        assert_eq!(outputs, (1, 2));
        assert_eq!(polls, 7);
    }

    #[test]
    fn test_join_with_ready_futures() {
        let fut = join(std::future::ready(1), std::future::ready(2));
        assert_eq!(block_on(Counted::new(fut)), ((1, 2), 1));
    }

    // ---- 6. Drop guarantee --------------------------------------------------

    #[test]
    fn test_waiter_registers_its_pinned_address() {
        let registry = Registry::new();
        let mut waiter = Box::pin(Waiter::new(&registry));
        assert!(!waiter.is_registered());
        let address = waiter.as_mut().register();
        assert!(waiter.is_registered());
        assert_eq!(address, waiter.as_ref().address());
        assert!(registry.contains(address));
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn test_waiter_register_is_idempotent() {
        let registry = Registry::new();
        let mut waiter = Box::pin(Waiter::new(&registry));
        let first = waiter.as_mut().register();
        let second = waiter.as_mut().register();
        assert_eq!(first, second);
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn test_drop_of_heap_pinned_waiter_deregisters() {
        let registry = Registry::new();
        let mut waiter = Box::pin(Waiter::new(&registry));
        let address = waiter.as_mut().register();
        drop(waiter);
        assert!(!registry.contains(address));
        assert!(registry.is_empty());
    }

    #[test]
    fn test_drop_of_stack_pinned_waiter_deregisters_at_scope_end() {
        let registry = Registry::new();
        let address = {
            let mut waiter = pin!(Waiter::new(&registry));
            let address = waiter.as_mut().register();
            assert!(registry.contains(address));
            address
        };
        assert!(!registry.contains(address));
    }

    #[test]
    fn test_pin_set_drops_old_waiter_before_reusing_its_memory() {
        let registry = Registry::new();
        let mut waiter = Box::pin(Waiter::new(&registry));
        let address = waiter.as_mut().register();
        waiter.set(Waiter::new(&registry));
        assert!(registry.is_empty(), "old waiter must be dropped in place");
        assert!(!waiter.is_registered());
        assert_eq!(waiter.as_mut().register(), address);
    }

    #[test]
    fn test_unregistered_waiter_drop_is_noop() {
        let registry = Registry::new();
        drop(Box::pin(Waiter::new(&registry)));
        assert!(registry.is_empty());
    }

    // ---- 7. Escape hatches --------------------------------------------------

    #[test]
    fn test_boxed_futures_are_unpin_and_heterogeneous() {
        let mut futures: Vec<BoxFuture<'static, u32>> = vec![
            boxed(YieldNow::new(1, 1)),
            boxed(std::future::ready(2)),
            boxed(Map::new(YieldNow::new(2, 3_u32), |n| n * 10)),
        ];
        let mut cx = noop_cx();
        let mut outputs = vec![None; futures.len()];
        while outputs.iter().any(Option::is_none) {
            for (fut, out) in futures.iter_mut().zip(outputs.iter_mut()) {
                if out.is_some() {
                    continue;
                }
                if let Poll::Ready(v) = poll_unpin(fut, &mut cx) {
                    *out = Some(v);
                }
            }
        }
        assert_eq!(outputs, vec![Some(1), Some(2), Some(30)]);
    }

    #[test]
    fn test_block_on_async_block_with_pinned_locals() {
        let fut = async {
            let a = YieldNow::new(2, 1).await;
            let b = Map::new(YieldNow::new(1, 2), |n| n + a).await;
            a + b
        };
        assert_eq!(block_on(fut), 4);
    }
}
