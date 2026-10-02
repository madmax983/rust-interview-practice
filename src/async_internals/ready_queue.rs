//! # Ready Queue Drill
//!
//! The heart of every executor: a FIFO of *task ids that are ready to be polled*.
//! A waker does not poll anything - it just pushes its task id onto the ready queue.
//! The executor pops ids and polls the matching futures.
//!
//! This single-threaded executor practices the details real runtimes get right:
//!
//! - **Wake de-duplication**: a `scheduled` flag per task means waking a task ten
//!   times before it runs enqueues it once.
//! - **Wake during poll**: the flag is cleared *before* polling, so a task that wakes
//!   itself (e.g. `yield_now`) is re-queued instead of lost.
//! - **Generational ids**: a stale waker for a finished task cannot poll a new task
//!   that reused the same slot.
//! - **Re-entrancy**: futures are taken out of their slot while polled, so a task may
//!   spawn new tasks through a [`Spawner`].
//! - **`JoinHandle`**: awaitable result, plus [`JoinHandle::abort`] (cancellation by drop).
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::async_internals::ready_queue::Executor;
//! use rust_interview_practice::async_internals::future_polling::yield_now;
//!
//! let executor = Executor::new();
//! let a = executor.spawn(async { yield_now().await; 1 });
//! let b = executor.spawn(async { 2 });
//! executor.run_until_stalled();
//! assert_eq!(a.try_take(), Some(Ok(1)));
//! assert_eq!(b.try_take(), Some(Ok(2)));
//! ```
//!
//! ## Architecture
//!
//! ```text
//!   spawn(fut) ──► slots[index] = Task { future, waker }  ──► ready.push(id)
//!
//!   run_until_stalled:
//!     while let Some(id) = ready.pop()
//!        slot generation != id.generation ? ──► skip (stale wake)
//!        scheduled = false                      (wakes during poll re-enqueue)
//!        aborted?  ──► drop future, free slot
//!        poll(future, Waker(TaskWaker{id}))
//!           Ready   ──► free slot (generation += 1)
//!           Pending ──► put future back
//!
//!   TaskWaker::wake: if !scheduled.swap(true) { ready.push(id) }
//! ```
//!
//! Invariant: a task id appears in the ready queue at most once while its `scheduled`
//! flag is set, so the queue length never exceeds the number of live tasks.
//!
//! | Operation         | Time  | Space |
//! |-------------------|-------|-------|
//! | spawn             | O(1)* | O(1)  |
//! | wake              | O(1)  | O(1)  |
//! | poll one task     | O(1)+ | O(1)  |
//!
//! \* amortised (slot vector growth). + plus the future's own `poll` cost.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::task::{Context, Poll, Wake, Waker};

type LocalBoxFuture = Pin<Box<dyn Future<Output = ()>>>;

/// Generational task identifier: `index` into the slot table plus the slot's
/// `generation` at spawn time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TaskId {
    index: usize,
    generation: u64,
}

/// Why a [`JoinHandle`] produced no value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JoinError {
    /// The task was aborted before it completed; its future was dropped.
    Aborted,
}

impl fmt::Display for JoinError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Aborted => f.write_str("task was aborted"),
        }
    }
}

impl std::error::Error for JoinError {}

// =========================================================================================
// Ready queue + waker
// =========================================================================================

/// The shared FIFO. Wakers must be `Send + Sync`, so this is a `Mutex` even though the
/// executor itself is single-threaded.
#[derive(Debug, Default)]
struct ReadyQueue {
    ids: Mutex<VecDeque<TaskId>>,
}

impl ReadyQueue {
    fn push(&self, id: TaskId) {
        self.ids
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push_back(id);
    }

    fn pop(&self) -> Option<TaskId> {
        self.ids
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .pop_front()
    }

    fn len(&self) -> usize {
        self.ids
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .len()
    }
}

/// Per-task waker state.
#[derive(Debug)]
struct TaskWaker {
    id: TaskId,
    /// `true` while the id sits in the ready queue (or is about to).
    scheduled: AtomicBool,
    /// Set by [`JoinHandle::abort`]; checked by the executor before polling.
    aborted: AtomicBool,
    ready: Arc<ReadyQueue>,
}

impl Wake for TaskWaker {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        // Only the caller that flips false -> true enqueues: de-duplication.
        if !self.scheduled.swap(true, Ordering::AcqRel) {
            self.ready.push(self.id);
        }
    }
}

// =========================================================================================
// Slot table
// =========================================================================================

struct Task {
    /// `None` while the executor is polling it (taken out for re-entrancy).
    future: Option<LocalBoxFuture>,
    waker: Arc<TaskWaker>,
}

#[derive(Default)]
struct Slot {
    generation: u64,
    task: Option<Task>,
}

#[derive(Default)]
struct Slots {
    entries: Vec<Slot>,
    free: Vec<usize>,
    live: usize,
}

impl Slots {
    fn insert(&mut self, ready: &Arc<ReadyQueue>, future: LocalBoxFuture) -> Arc<TaskWaker> {
        let index = self.free.pop().unwrap_or_else(|| {
            self.entries.push(Slot::default());
            self.entries.len() - 1
        });
        let slot = &mut self.entries[index];
        let waker = Arc::new(TaskWaker {
            id: TaskId {
                index,
                generation: slot.generation,
            },
            scheduled: AtomicBool::new(false),
            aborted: AtomicBool::new(false),
            ready: Arc::clone(ready),
        });
        slot.task = Some(Task {
            future: Some(future),
            waker: Arc::clone(&waker),
        });
        self.live += 1;
        waker
    }

    fn get_mut(&mut self, id: TaskId) -> Option<&mut Task> {
        self.entries
            .get_mut(id.index)
            .filter(|slot| slot.generation == id.generation)
            .and_then(|slot| slot.task.as_mut())
    }

    /// Frees the slot; bumping the generation invalidates every outstanding `TaskId`.
    fn remove(&mut self, id: TaskId) -> Option<Task> {
        let slot = self
            .entries
            .get_mut(id.index)
            .filter(|slot| slot.generation == id.generation)?;
        let task = slot.task.take()?;
        slot.generation += 1;
        self.free.push(id.index);
        self.live -= 1;
        Some(task)
    }
}

// =========================================================================================
// Executor + Spawner
// =========================================================================================

/// Cloneable handle that can spawn tasks, including from inside a running task.
#[derive(Clone)]
pub struct Spawner {
    slots: Rc<RefCell<Slots>>,
    ready: Arc<ReadyQueue>,
}

impl fmt::Debug for Spawner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Spawner")
            .field("live_tasks", &self.slots.borrow().live)
            .finish_non_exhaustive()
    }
}

impl Spawner {
    /// Spawns `future` and schedules it for its first poll.
    pub fn spawn<F>(&self, future: F) -> JoinHandle<F::Output>
    where
        F: Future + 'static,
        F::Output: 'static,
    {
        let state = Rc::new(RefCell::new(JoinState::default()));
        let completer = Rc::clone(&state);
        let wrapped: LocalBoxFuture = Box::pin(async move {
            let output = future.await;
            completer.borrow_mut().complete(Ok(output));
        });
        let waker = self.slots.borrow_mut().insert(&self.ready, wrapped);
        waker.wake_by_ref();
        JoinHandle { state, waker }
    }
}

/// Single-threaded executor driven by a ready queue of [`TaskId`]s.
#[derive(Debug, Clone)]
pub struct Executor {
    spawner: Spawner,
}

impl Default for Executor {
    fn default() -> Self {
        Self::new()
    }
}

impl Executor {
    /// Creates an executor with no tasks.
    #[must_use]
    pub fn new() -> Self {
        Self {
            spawner: Spawner {
                slots: Rc::new(RefCell::new(Slots::default())),
                ready: Arc::new(ReadyQueue::default()),
            },
        }
    }

    /// Returns a handle for spawning tasks from inside other tasks.
    #[must_use]
    pub fn spawner(&self) -> Spawner {
        self.spawner.clone()
    }

    /// Spawns `future` onto this executor.
    pub fn spawn<F>(&self, future: F) -> JoinHandle<F::Output>
    where
        F: Future + 'static,
        F::Output: 'static,
    {
        self.spawner.spawn(future)
    }

    /// Number of tasks spawned but not yet finished or aborted.
    #[must_use]
    pub fn live_tasks(&self) -> usize {
        self.spawner.slots.borrow().live
    }

    /// Number of ids waiting in the ready queue (may include stale ids).
    #[must_use]
    pub fn ready_len(&self) -> usize {
        self.spawner.ready.len()
    }

    /// Polls ready tasks until the ready queue is empty. Returns the number of polls.
    ///
    /// Tasks that are pending without having been woken stay parked; something external
    /// (a timer, another task, a thread) must wake them.
    #[allow(clippy::must_use_candidate)] // side-effecting; the count is informational
    pub fn run_until_stalled(&self) -> usize {
        let mut polls = 0;
        while let Some(id) = self.spawner.ready.pop() {
            if self.run_task(id) {
                polls += 1;
            }
        }
        polls
    }

    /// Handles one popped id. Returns whether a poll happened.
    fn run_task(&self, id: TaskId) -> bool {
        let slots = &self.spawner.slots;
        let Some((mut future, task_waker)) = slots.borrow_mut().get_mut(id).and_then(|task| {
            // `None` future means this task is mid-poll further up the stack; skip.
            task.future
                .take()
                .map(|future| (future, Arc::clone(&task.waker)))
        }) else {
            return false; // stale id: task finished and slot was reused or freed
        };

        // Clear before polling: a wake during `poll` must re-enqueue the task.
        task_waker.scheduled.store(false, Ordering::Release);

        if task_waker.aborted.load(Ordering::Acquire) {
            let removed = slots.borrow_mut().remove(id);
            // Drop outside the borrow: destructors may spawn or abort other tasks.
            drop(future);
            drop(removed);
            return false;
        }

        let waker = Waker::from(Arc::clone(&task_waker));
        let mut cx = Context::from_waker(&waker);
        match future.as_mut().poll(&mut cx) {
            Poll::Ready(()) => {
                let removed = slots.borrow_mut().remove(id);
                drop(removed);
            }
            Poll::Pending => {
                if let Some(task) = slots.borrow_mut().get_mut(id) {
                    task.future = Some(future);
                }
            }
        }
        true
    }
}

// =========================================================================================
// JoinHandle
// =========================================================================================

struct JoinState<T> {
    result: Option<Result<T, JoinError>>,
    finished: bool,
    joiner: Option<Waker>,
}

impl<T> Default for JoinState<T> {
    fn default() -> Self {
        Self {
            result: None,
            finished: false,
            joiner: None,
        }
    }
}

impl<T> JoinState<T> {
    /// Records the first outcome only: an abort racing a completion keeps whichever won.
    fn complete(&mut self, result: Result<T, JoinError>) {
        if self.finished {
            return;
        }
        self.finished = true;
        self.result = Some(result);
        if let Some(joiner) = self.joiner.take() {
            joiner.wake();
        }
    }
}

/// Handle to a spawned task's output. Awaiting it yields `Ok(output)` or
/// `Err(JoinError::Aborted)`. Dropping it detaches the task (it keeps running).
pub struct JoinHandle<T> {
    state: Rc<RefCell<JoinState<T>>>,
    waker: Arc<TaskWaker>,
}

impl<T> fmt::Debug for JoinHandle<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("JoinHandle")
            .field("id", &self.waker.id)
            .field("finished", &self.is_finished())
            .finish_non_exhaustive()
    }
}

impl<T> JoinHandle<T> {
    /// The spawned task's id.
    #[must_use]
    pub fn id(&self) -> TaskId {
        self.waker.id
    }

    /// Whether the task has completed or been aborted.
    #[must_use]
    pub fn is_finished(&self) -> bool {
        self.state.borrow().finished
    }

    /// Takes the result if the task has finished (non-blocking, single-shot).
    #[must_use]
    pub fn try_take(&self) -> Option<Result<T, JoinError>> {
        self.state.borrow_mut().result.take()
    }

    /// Requests cancellation. The executor drops the task's future the next time it
    /// pops the id, running its destructors. No-op if the task already finished.
    pub fn abort(&self) {
        if self.is_finished() {
            return;
        }
        self.waker.aborted.store(true, Ordering::Release);
        self.state.borrow_mut().complete(Err(JoinError::Aborted));
        // Make sure the executor visits the task so it can drop the future.
        self.waker.wake_by_ref();
    }
}

impl<T> Future for JoinHandle<T> {
    type Output = Result<T, JoinError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut state = self.state.borrow_mut();
        if let Some(result) = state.result.take() {
            return Poll::Ready(result);
        }
        assert!(
            !state.finished,
            "`JoinHandle` polled after its result was taken"
        );
        state.joiner = Some(cx.waker().clone());
        Poll::Pending
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::async_internals::future_polling::{countdown, yield_now};
    use std::cell::Cell;

    /// Future that stays pending until woken externally, recording its waker.
    struct Parked {
        waker: WakerSlot,
        released: Rc<Cell<bool>>,
    }

    impl Future for Parked {
        type Output = ();

        fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
            if self.released.get() {
                Poll::Ready(())
            } else {
                *self.waker.borrow_mut() = Some(cx.waker().clone());
                Poll::Pending
            }
        }
    }

    type WakerSlot = Rc<RefCell<Option<Waker>>>;

    fn parked() -> (Parked, WakerSlot, Rc<Cell<bool>>) {
        let waker = Rc::new(RefCell::new(None));
        let released = Rc::new(Cell::new(false));
        (
            Parked {
                waker: Rc::clone(&waker),
                released: Rc::clone(&released),
            },
            waker,
            released,
        )
    }

    #[test]
    fn test_spawned_task_runs_once() {
        let executor = Executor::new();
        let handle = executor.spawn(async { "ok" });
        assert_eq!(executor.live_tasks(), 1);
        assert_eq!(executor.run_until_stalled(), 1);
        assert_eq!(handle.try_take(), Some(Ok("ok")));
        assert_eq!(executor.live_tasks(), 0);
    }

    #[test]
    fn test_empty_executor_does_nothing() {
        let executor = Executor::default();
        assert_eq!(executor.run_until_stalled(), 0);
        assert_eq!(executor.ready_len(), 0);
    }

    #[test]
    fn test_fifo_order_and_yield_interleaving() {
        let executor = Executor::new();
        let log = Rc::new(RefCell::new(Vec::new()));
        for name in ['a', 'b'] {
            let log = Rc::clone(&log);
            executor.spawn(async move {
                log.borrow_mut().push((name, 1));
                yield_now().await;
                log.borrow_mut().push((name, 2));
            });
        }
        assert_eq!(executor.run_until_stalled(), 4);
        assert_eq!(*log.borrow(), [('a', 1), ('b', 1), ('a', 2), ('b', 2)]);
    }

    #[test]
    fn test_self_wake_during_poll_is_not_lost() {
        let executor = Executor::new();
        let handle = executor.spawn(countdown(10));
        assert_eq!(executor.run_until_stalled(), 11);
        assert_eq!(handle.try_take(), Some(Ok(())));
    }

    #[test]
    fn test_repeated_wakes_are_deduplicated() {
        let executor = Executor::new();
        let (future, waker, released) = parked();
        executor.spawn(future);
        assert_eq!(executor.run_until_stalled(), 1);

        let waker = waker.borrow_mut().take().expect("waker registered");
        for _ in 0..10 {
            waker.wake_by_ref();
        }
        assert_eq!(executor.ready_len(), 1);
        released.set(true);
        assert_eq!(executor.run_until_stalled(), 1);
        assert_eq!(executor.live_tasks(), 0);
    }

    #[test]
    fn test_pending_task_without_wake_stays_parked() {
        let executor = Executor::new();
        let (future, _waker, _released) = parked();
        let handle = executor.spawn(future);
        assert_eq!(executor.run_until_stalled(), 1);
        assert_eq!(executor.run_until_stalled(), 0);
        assert!(!handle.is_finished());
        assert_eq!(executor.live_tasks(), 1);
    }

    #[test]
    fn test_stale_waker_cannot_poll_reused_slot() {
        let executor = Executor::new();
        let (future, waker, released) = parked();
        let first = executor.spawn(future);
        executor.run_until_stalled();
        let stale = waker.borrow_mut().take().expect("waker registered");
        released.set(true);
        stale.wake_by_ref();
        executor.run_until_stalled();
        assert!(first.is_finished());

        // New task reuses slot 0 with a bumped generation.
        let polls = Rc::new(Cell::new(0));
        let counted = Rc::clone(&polls);
        let (future2, _w2, _r2) = parked();
        let second = executor.spawn(async move {
            counted.set(counted.get() + 1);
            future2.await;
        });
        executor.run_until_stalled();
        assert_eq!(second.id().index, first.id().index);
        assert_ne!(second.id(), first.id());

        stale.wake(); // old generation: must be ignored
        assert_eq!(executor.run_until_stalled(), 0);
        assert_eq!(polls.get(), 1);
    }

    #[test]
    fn test_join_handle_is_awaitable_from_another_task() {
        let executor = Executor::new();
        let inner = executor.spawn(async {
            countdown(3).await;
            7
        });
        let outer = executor.spawn(async move { inner.await.map(|v| v * 6) });
        executor.run_until_stalled();
        assert_eq!(outer.try_take(), Some(Ok(Ok(42))));
    }

    #[test]
    fn test_spawn_from_inside_task() {
        let executor = Executor::new();
        let spawner = executor.spawner();
        let outer = executor.spawn(async move {
            let child = spawner.spawn(async { 5 });
            child.await.map(|v| v + 1)
        });
        executor.run_until_stalled();
        assert_eq!(outer.try_take(), Some(Ok(Ok(6))));
        assert_eq!(executor.live_tasks(), 0);
    }

    #[test]
    fn test_abort_drops_future_and_reports_error() {
        struct DropFlag(Rc<Cell<bool>>);
        impl Drop for DropFlag {
            fn drop(&mut self) {
                self.0.set(true);
            }
        }

        let executor = Executor::new();
        let dropped = Rc::new(Cell::new(false));
        let flag = DropFlag(Rc::clone(&dropped));
        let (future, _waker, _released) = parked();
        let handle = executor.spawn(async move {
            let _flag = flag;
            future.await;
            "never"
        });
        executor.run_until_stalled();
        assert!(!dropped.get());

        handle.abort();
        assert!(handle.is_finished());
        executor.run_until_stalled();
        assert!(dropped.get());
        assert_eq!(executor.live_tasks(), 0);
        assert_eq!(handle.try_take(), Some(Err(JoinError::Aborted)));
    }

    #[test]
    fn test_abort_before_first_poll_never_runs_body() {
        let executor = Executor::new();
        let ran = Rc::new(Cell::new(false));
        let ran2 = Rc::clone(&ran);
        let handle = executor.spawn(async move { ran2.set(true) });
        handle.abort();
        assert_eq!(executor.run_until_stalled(), 0);
        assert!(!ran.get());
        assert_eq!(executor.ready_len(), 0);
    }

    #[test]
    fn test_abort_after_completion_is_noop() {
        let executor = Executor::new();
        let handle = executor.spawn(async { 1 });
        executor.run_until_stalled();
        handle.abort();
        assert_eq!(handle.try_take(), Some(Ok(1)));
    }

    #[test]
    fn test_dropping_join_handle_detaches_task() {
        let executor = Executor::new();
        let ran = Rc::new(Cell::new(false));
        let ran2 = Rc::clone(&ran);
        drop(executor.spawn(async move {
            yield_now().await;
            ran2.set(true);
        }));
        executor.run_until_stalled();
        assert!(ran.get());
    }

    #[test]
    fn test_join_error_display() {
        assert_eq!(JoinError::Aborted.to_string(), "task was aborted");
    }

    /// Model-based check of the de-duplication invariant: pseudo-random wake storms
    /// never queue an id twice and every woken task is polled exactly once per drain.
    #[test]
    fn test_wake_storm_invariant() {
        let executor = Executor::new();
        let tasks: Vec<_> = (0..8).map(|_| parked()).collect();
        let mut wakers = Vec::new();
        for (future, waker, _) in tasks {
            executor.spawn(future);
            wakers.push(waker);
        }
        executor.run_until_stalled();
        let wakers: Vec<Waker> = wakers
            .iter()
            .map(|w| w.borrow().clone().expect("waker registered"))
            .collect();

        let mut seed: u64 = 0x9E37_79B9_7F4A_7C15;
        for _round in 0..50 {
            let mut woken = std::collections::HashSet::new();
            for _ in 0..20 {
                seed ^= seed << 13;
                seed ^= seed >> 7;
                seed ^= seed << 17;
                let i = usize::try_from(seed % 8).expect("fits in usize");
                wakers[i].wake_by_ref();
                woken.insert(i);
            }
            assert_eq!(executor.ready_len(), woken.len());
            assert_eq!(executor.run_until_stalled(), woken.len());
        }
        assert_eq!(executor.live_tasks(), 8);
    }
}
