//! # Async Internals
//!
//! Drills that rebuild the machinery under `async`/`.await` by hand, on stable Rust and
//! without any runtime crate. Work through them in order; each builds on the previous:
//!
//! 1. [`raw_waker`] - `RawWaker`/`RawWakerVTable` from scratch, the `Wake` trait, `block_on`.
//! 2. [`future_polling`] - hand-written futures, pin projection, `join`, and the state
//!    machine an `async fn` desugars to.
//! 3. [`ready_queue`] - a single-threaded executor: ready queue, wake de-duplication,
//!    generational task ids, `JoinHandle`, abort.
//! 4. [`timer`] - `sleep` on a virtual clock: deadline heap, lazy registration,
//!    cancel-on-drop, and the runtime driver loop.
//! 5. [`cancellation`] - `CancellationToken`, `with_cancellation`, `timeout`, and
//!    cancel safety.
//!
//! Everything is deterministic (virtual time, FIFO scheduling), so the tests double as
//! executable specifications of each invariant.
//!
//! ```text
//!   Waker ──wake──► ready_queue ──pop──► Executor::poll(task)
//!     ▲                                       │
//!     │                                       ▼
//!   Timer::advance_to ◄── run_with_timer ◄── Sleep / WaitForCancellation register wakers
//! ```

pub mod cancellation;
pub mod future_polling;
pub mod raw_waker;
pub mod ready_queue;
pub mod timer;
