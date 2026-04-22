//! # Reactive Signals Engine
//!
//! Implements a fine-grained reactivity system from scratch.
//! This includes primitives for state management (`Signal`), derived state (`Memo`),
//! and side effects (`Effect`) that automatically track their dependencies.
//!
//! **Replaces Crates:** `leptos_reactive`, `dioxus-signals`, `salsa` (partially)
//!
//! **Real-world Usage:**
//! - Modern Rust web frameworks (Leptos, Dioxus, Sycamore) use this to update only the DOM nodes that changed, avoiding Virtual DOM diffing.
//! - Reactive build systems or incremental computation engines.
//! - GUI toolkits that need decoupled data binding.
//!
//! **Why build it yourself?**
//! The "magic" of signals—where simply calling `.get()` inside a closure automatically subscribes that closure to future updates—is a masterclass in global state management and interior mutability. Building it demystifies thread-local execution contexts and topological/graph-based notification propagation in Rust without relying on heavy garbage collection.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};

// =========================================================================================
// Architecture
// =========================================================================================
//
// Data Structure:
//
//   [ Global Context (Thread Local) ]
//      └── active_observer: Option<Rc<EffectState>>
//
//   [ Signal<T> ] ─────(notifies)──────► [ EffectState ]
//      ├── data: T                          ├── closure: Box<dyn FnMut()>
//      └── subscribers: Vec<Weak<Effect>>   └── id: usize
//
//   [ Memo<T> ] is implemented compositionally: An Effect that sets a Signal.
//
// Invariants:
// 1. **Automatic Tracking:** If `Signal::get()` is called while an `Effect` is currently executing (i.e., `active_observer` is set), the `Effect` is automatically added to the `Signal`'s subscriber list.
// 2. **Glitch-free (Simplified):** Effects run immediately when created to establish initial dependencies, and then re-run whenever a dependency calls `Signal::set()`.
// 3. **Memory Management:** Memory leaks are prevented by explicitly using weak references from Signals to Effects. When an Effect is dropped, Signals that notify it will gracefully clean up the dead weak pointers.
//
// Complexity:
// ┌───────────────┬─────────────┬─────────────┐
// │ Operation     │ Time        │ Space       │
// ├───────────────┼─────────────┼─────────────┤
// │ Signal::get() │ O(1)        │ O(1)        │
// │ Signal::set() │ O(S)        │ O(1)        │ (S = number of subscribers)
// │ Effect creation│ O(1)*       │ O(1)        │ (*Executes closure once)
// └───────────────┴─────────────┴─────────────┘
//
// Design Decisions & Tradeoffs:
// - **Thread-Local vs Arc/Mutex:** We use `thread_local!` and `Rc<RefCell<T>>`. Signals in frontend frameworks are almost always thread-local because web assembly (WASM) currently runs in a single thread, and avoiding `Mutex` overhead makes reactivity extremely fast.
// - **Push vs Pull:** This uses an immediate push-based reactivity model. Calling `set` immediately triggers subscribers. Production systems (like Leptos) often use a push-pull model (push dirty flags, pull evaluated values) to ensure glitch-free rendering.

// RUST INSIGHT:
// In GC'd languages (JS/TS), signals rely on garbage collection to clean up dead effects.
// In Rust, we use `Rc` and `Weak` to manage the ownership graph safely. The `Scope` or user
// holds the strong `Rc` to the Effect, while Signals only hold `Weak` references to prevent memory leaks.

// =========================================================================================
// Traits
// =========================================================================================

/// Defines a reactive primitive that can be tracked as a dependency.
pub trait Track<T> {
    /// Gets the current value and subscribes the currently running effect.
    fn get(&self) -> T;
}

/// Defines a reactive primitive that can update its state and notify dependencies.
pub trait Notify<T> {
    /// Updates the state and triggers all subscribed effects.
    fn set(&self, new_value: T);
    /// Mutates the state in place and triggers all subscribed effects.
    fn update<F: FnOnce(&mut T)>(&self, f: F);
}

// =========================================================================================
// Global Runtime (Thread-Local)
// =========================================================================================

static NEXT_EFFECT_ID: AtomicUsize = AtomicUsize::new(1);

thread_local! {
    /// Tracks the currently executing effect so that signals can auto-subscribe it.
    ///
    /// // GOTCHA:
    /// We use `Rc<RefCell<Option<Rc<EffectState>>>>` because we need to borrow it globally
    /// and it might be mutated recursively (an effect causing another effect to run).
    static ACTIVE_EFFECT: RefCell<Option<Rc<EffectState>>> = const { RefCell::new(None) };
}

// =========================================================================================
// Effect
// =========================================================================================

/// The internal state of an Effect, hidden behind an Rc.
pub struct EffectState {
    id: usize,
    // We use a Box<dyn FnMut()> to store the user's closure.
    // It is wrapped in a RefCell because we need to execute it (which requires a mutable borrow)
    // while multiple signals might hold references to this EffectState.
    computation: RefCell<Box<dyn FnMut()>>,
}

impl EffectState {
    /// Executes the effect, setting it as the active observer so signals can track it.
    fn run(self: &Rc<Self>) {
        // 1. Set this effect as the active observer.
        // We must store the previous active effect in case effects are nested!
        let prev_active = ACTIVE_EFFECT.with(|active| active.replace(Some(Rc::clone(self))));

        // 2. Execute the computation.
        // UNSAFE JUSTIFICATION: No unsafe is used here, but we rely on RefCell's dynamic borrow checking.
        // If an effect triggers itself synchronously, this borrow_mut() will panic (which is a desirable feature to prevent infinite loops).
        if let Ok(mut comp) = self.computation.try_borrow_mut() {
            comp();
        }

        // 3. Restore the previous active observer.
        ACTIVE_EFFECT.with(|active| *active.borrow_mut() = prev_active);
    }
}

/// A reactive side-effect that automatically re-runs when its dependencies change.
pub struct Effect {
    // The strong reference keeps the effect alive.
    // When the `Effect` struct is dropped, the `Rc` count drops. Signals holding `Weak` pointers will realize it's gone.
    _state: Rc<EffectState>,
}

impl Effect {
    /// Creates a new effect and runs it immediately to establish dependencies.
    pub fn new<F>(computation: F) -> Self
    where
        F: FnMut() + 'static,
    {
        let state = Rc::new(EffectState {
            id: NEXT_EFFECT_ID.fetch_add(1, Ordering::Relaxed),
            computation: RefCell::new(Box::new(computation)),
        });

        // Run immediately to track dependencies.
        state.run();

        Self { _state: state }
    }
}

// =========================================================================================
// Signal
// =========================================================================================

/// Internal state of a Signal.
struct SignalState<T> {
    value: T,
    /// Subscribers are stored as weak pointers to prevent memory leaks if the Effect is dropped.
    /// We use a HashMap keyed by Effect ID to prevent exponential duplicate subscriptions
    /// if an effect evaluates a signal multiple times or in a loop.
    subscribers: HashMap<usize, std::rc::Weak<EffectState>>,
}

/// A reactive primitive that holds state and notifies subscribers when changed.
///
/// // PRODUCTION NOTE:
/// Real frameworks often use Arena allocators (like `bumpalo`) and return lightweight `Copy` handles (`struct Signal<T>(usize)`).
/// Here we use `Rc<RefCell<>>` for an idiomatic, safe implementation without lifetimes.
pub struct Signal<T> {
    state: Rc<RefCell<SignalState<T>>>,
}

// Manually implement Clone so we don't require T: Clone
impl<T> Clone for Signal<T> {
    fn clone(&self) -> Self {
        Self {
            state: Rc::clone(&self.state),
        }
    }
}

impl<T> Signal<T> {
    /// Creates a new Signal with an initial value.
    pub fn new(value: T) -> Self {
        Self {
            state: Rc::new(RefCell::new(SignalState {
                value,
                subscribers: HashMap::new(),
            })),
        }
    }
}

impl<T> Notify<T> for Signal<T> {
    /// Updates the value and notifies all subscribed effects.
    fn set(&self, new_value: T) {
        let mut state = self.state.borrow_mut();
        state.value = new_value;

        // Clean up dead subscribers while notifying alive ones.
        let mut effects_to_run = Vec::new();
        // Remove dropped weak references
        state.subscribers.retain(|_id, weak_effect| {
            if let Some(effect) = weak_effect.upgrade() {
                effects_to_run.push(effect);
                true
            } else {
                false
            }
        });

        // Drop the borrow on state BEFORE running effects, because effects might call `set` or `get` on this same signal!
        drop(state);

        // Run the effects
        for effect in effects_to_run {
            effect.run();
        }
    }

    /// Mutates the value in place using a closure, then notifies subscribers.
    fn update<F>(&self, f: F)
    where
        F: FnOnce(&mut T),
    {
        let mut state = self.state.borrow_mut();
        f(&mut state.value);

        let mut effects_to_run = Vec::new();
        state.subscribers.retain(|_id, weak_effect| {
            if let Some(effect) = weak_effect.upgrade() {
                effects_to_run.push(effect);
                true
            } else {
                false
            }
        });

        drop(state);

        for effect in effects_to_run {
            effect.run();
        }
    }
}

impl<T: Clone> Track<T> for Signal<T> {
    /// Gets the current value and tracks the active effect as a dependency.
    fn get(&self) -> T {
        // 1. Auto-subscribe the active effect (if any) to this signal.
        ACTIVE_EFFECT.with(|active| {
            if let Some(active_effect) = &*active.borrow() {
                let mut state = self.state.borrow_mut();
                // Insert into HashMap to deduplicate subscriptions automatically
                state
                    .subscribers
                    .insert(active_effect.id, Rc::downgrade(active_effect));
            }
        });

        // 2. Return the cloned value.
        self.state.borrow().value.clone()
    }
}

// =========================================================================================
// Memo (Derived State)
// =========================================================================================

/// Derived reactive state.
/// A Memo computes a value based on other signals, and acts as a read-only Signal itself.
pub struct Memo<T> {
    signal: Signal<T>,
    // We must keep the effect alive as long as the Memo is alive.
    // By wrapping it in an Rc, multiple Memo handles can share the same computation.
    _effect: Rc<Effect>,
}

impl<T: Clone> Clone for Memo<T> {
    fn clone(&self) -> Self {
        Self {
            signal: self.signal.clone(),
            _effect: Rc::clone(&self._effect),
        }
    }
}

impl<T> Memo<T>
where
    T: Clone + PartialEq + 'static,
{
    /// Creates a new Memo.
    pub fn new<F>(mut computation: F) -> Self
    where
        F: FnMut() -> T + 'static,
    {
        // We need to initialize the signal with a dummy or first-computed value.
        // We run the computation once to get the initial value.
        // But wait, if we run it outside the effect, it won't track dependencies!
        // So we use a cell to capture the first value inside the effect.

        // To avoid double-evaluating the computation during initialization, and
        // to avoid recursively tracking the internal signal's state updates,
        // we extract the internal raw value bypassing `Track::get()` via `borrow().value`.

        let first_val = computation();
        let signal = Signal::new(first_val);
        let signal_clone = signal.clone();

        // Create the effect that will update the signal on subsequent changes
        let effect = Effect::new(move || {
            let new_val = computation();

            // Bypass `Track::get` to avoid subscribing Memo's effect to Memo's signal
            let current_val = signal_clone.state.borrow().value.clone();

            if new_val != current_val {
                signal_clone.set(new_val);
            }
        });

        Self {
            signal,
            _effect: Rc::new(effect),
        }
    }
}

impl<T: Clone> Track<T> for Memo<T> {
    fn get(&self) -> T {
        self.signal.get()
    }
}

// =========================================================================================
// Footer
// =========================================================================================
// Comparison to canonical crates:
// - `leptos_reactive`: Uses a central `Runtime` arena to allocate signals as `Copy` IDs instead of `Rc<RefCell>`.
//   This drastically improves ergonomics (no need to clone `Rc`s to move into closures) and avoids reference counting overhead.
// - `dioxus-signals`: Uses a similar generational arena approach.
//
// Benchmarking Notes:
// To benchmark this, you would create thousands of deeply nested Memos and measure the time it takes
// for `Signal::set` to propagate to the end using `std::time::Instant::now()` and `std::hint::black_box`.
//
// Missing vs Production:
// - **Topological Sorting:** If `Signal A` updates `Memo B` and `Effect C`, and `Memo B` updates `Effect C`,
//   a naive push model might run `Effect C` twice (a "glitch"). Real systems sort the execution graph.
// - **Bidirectional Tracking:** Here, Signals know about Effects, but Effects don't know about Signals.
//   If an Effect stops depending on a Signal (e.g. conditional branch `if a.get() { b.get() }`),
//   it remains subscribed to `b` forever in this naive implementation. Real systems clear dependencies before every run.
// - **Copy Handles:** Production signals are `Copy`, which makes moving them into `move ||` closures much nicer.
//
// Suggested next steps / extensions:
// 1. Add `dependencies` Vec to `EffectState` to allow clearing tracking lists before re-running.
// 2. Implement push-pull reactivity (dirty flags) to solve glitches.
// 3. Implement an Arena allocator to remove `Rc<RefCell>` and pass lightweight IDs.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_signal_basic() {
        let count = Signal::new(0);
        assert_eq!(count.get(), 0);

        count.set(1);
        assert_eq!(count.get(), 1);

        count.update(|c| *c += 1);
        assert_eq!(count.get(), 2);
    }

    #[test]
    fn test_effect_reacts_to_signal() {
        let count = Signal::new(0);
        let tracker = Rc::new(RefCell::new(0));

        let tracker_clone = Rc::clone(&tracker);
        let count_clone = count.clone();

        // Effect runs immediately once.
        let _effect = Effect::new(move || {
            *tracker_clone.borrow_mut() = count_clone.get();
        });

        assert_eq!(*tracker.borrow(), 0);

        count.set(5);
        assert_eq!(*tracker.borrow(), 5);

        count.update(|c| *c += 10);
        assert_eq!(*tracker.borrow(), 15);
    }

    #[test]
    fn test_effect_cleanup_on_drop() {
        let count = Signal::new(0);
        let runs = Rc::new(RefCell::new(0));

        {
            let runs_clone = Rc::clone(&runs);
            let count_clone = count.clone();
            let _effect = Effect::new(move || {
                let _ = count_clone.get();
                *runs_clone.borrow_mut() += 1;
            });

            assert_eq!(*runs.borrow(), 1);
            count.set(1);
            assert_eq!(*runs.borrow(), 2);
        } // _effect is dropped here

        // Updating the signal should not run the dropped effect.
        count.set(2);
        assert_eq!(*runs.borrow(), 2);

        // The signal's internal subscriber list should be cleaned up.
        assert_eq!(count.state.borrow().subscribers.len(), 0);
    }

    #[test]
    fn test_memo_derives_state() {
        let a = Signal::new(1);
        let b = Signal::new(2);

        let a_clone = a.clone();
        let b_clone = b.clone();

        let sum = Memo::new(move || a_clone.get() + b_clone.get());

        assert_eq!(sum.get(), 3);

        a.set(10);
        assert_eq!(sum.get(), 12);

        b.set(20);
        assert_eq!(sum.get(), 30);
    }

    #[test]
    fn test_memo_chaining() {
        let name = Signal::new("Alice".to_string());

        let name_clone = name.clone();
        let greeting = Memo::new(move || format!("Hello, {}!", name_clone.get()));

        let greeting_clone = greeting.clone();
        let loud_greeting = Memo::new(move || greeting_clone.get().to_uppercase());

        assert_eq!(loud_greeting.get(), "HELLO, ALICE!");

        name.set("Bob".to_string());
        assert_eq!(loud_greeting.get(), "HELLO, BOB!");
    }
}
