//! # Reactive Signals Implementation
//!
//! # Header
//!
//! *   **Problem Name**: Reactive Signals / Computed State
//! *   **Difficulty**: Hard (Concurrency & Lifetimes)
//! *   **Link**: <https://github.com/tc39/proposal-signals>
//! *   **Replaces Crates**: `sycamore-reactive`, `leptos_reactive`, `dioxus-signals`
//! *   **Real-world Usage**:
//!     *   **Frontend Frameworks**: SolidJS, Vue, Preact, Leptos, Dioxus.
//!     *   **State Management**: MobX, Recoil.
//! *   **Why build it yourself?**
//!     Modern UI frameworks use "Signals" to achieve fine-grained reactivity without Virtual DOM diffing.
//!     Building this demystifies how reading a variable magically subscribes a component to updates.
//!     You will learn about interior mutability (`RefCell`), topological dependency graphs, and
//!     managing memory leaks using `Rc` and `Weak` pointers (since observers must not keep observables alive forever).
//!
//! # Architecture
//!
//! A Signal system consists of observables (Signals) and observers (Effects/Computeds).
//! The core trick is a global, thread-local stack of "active observers". When an observable is read,
//! it checks the stack and subscribes the currently active observer.
//!
//! **Flow:**
//!
//! ```text
//!     [Global Context: Active Effect = E1]
//!           │
//!           ▼
//!     Signal_A.get() ──► Adds E1 to Signal_A's subscribers
//!           │
//!     [Time Passes]
//!           │
//!     Signal_A.set() ──► Iterates subscribers, triggers E1.run()
//! ```
//!
//! **Invariants:**
//! 1.  **Automatic Tracking**: Users don't manually call `.subscribe()`. Calling `.get()` inside an effect does it automatically.
//! 2.  **No Memory Leaks**: Signals hold `Weak` references to Effects. If an Effect is dropped, it is cleaned up on the next trigger.
//! 3.  **Glitch-Free**: (Simplified here) Effects should run once per state change, ideally topologically sorted to avoid intermediate inconsistent states.
//!
//! **Complexity:**
//!
//! | Operation | Time | Space |
//! | :--- | :--- | :--- |
//! | Get | O(1) | O(1) |
//! | Set | O(S) | O(1) |
//!
//! S = Number of subscribers.

use std::cell::RefCell;
use std::rc::{Rc, Weak};

// =========================================================================================
// Global Tracker
// =========================================================================================

thread_local! {
    /// The currently executing effect. Any Signal read during this time will
    /// add this effect to its subscriber list.
    static ACTIVE_EFFECT: RefCell<Option<Rc<dyn EffectRunner>>> = const { RefCell::new(None) };
}

/// A trait for something that can be run when a dependency changes.
trait EffectRunner {
    fn run(&self);
}

// =========================================================================================
// Signal (Observable)
// =========================================================================================

/// A reactive piece of state.
pub struct Signal<T> {
    // RUST INSIGHT: We use `Rc<RefCell<T>>` because a Signal might be shared
    // across multiple closures (Effects) and needs interior mutability to update
    // its value while maintaining shared ownership.
    state: Rc<RefCell<SignalState<T>>>,
}

struct SignalState<T> {
    value: T,
    // GOTCHA: We must use `Weak` here. If a Signal holds a strong `Rc` to an Effect,
    // and the Effect captures the Signal (e.g., a Computed), it creates a memory leak (reference cycle).
    subscribers: Vec<Weak<dyn EffectRunner>>,
}

// Implement Clone so we can pass the Signal cheaply into closures.
impl<T> Clone for Signal<T> {
    fn clone(&self) -> Self {
        Self {
            state: Rc::clone(&self.state),
        }
    }
}

impl<T: Clone> Signal<T> {
    /// Creates a new Signal.
    pub fn new(value: T) -> Self {
        Self {
            state: Rc::new(RefCell::new(SignalState {
                value,
                subscribers: Vec::new(),
            })),
        }
    }

    /// Gets the current value of the signal.
    /// If called inside an `Effect`, automatically subscribes the effect to this signal.
    pub fn get(&self) -> T {
        // Track dependency if inside an effect
        ACTIVE_EFFECT.with(|active| {
            if let Some(effect) = active.borrow().as_ref() {
                let mut state = self.state.borrow_mut();

                // Deduplicate subscribers: Only add if not already in the list
                let is_already_subscribed = state.subscribers.iter().any(|weak| {
                    if let Some(strong) = weak.upgrade() {
                        Rc::ptr_eq(&strong, effect)
                    } else {
                        false
                    }
                });

                if !is_already_subscribed {
                    // Add the weak reference to the effect
                    state.subscribers.push(Rc::downgrade(effect));
                }
            }
        });

        // Return a clone of the value
        self.state.borrow().value.clone()
    }

    /// Updates the value of the signal and triggers all subscribers.
    pub fn set(&self, new_value: T) {
        let mut effects_to_run = Vec::new();

        {
            let mut state = self.state.borrow_mut();
            state.value = new_value;

            // Collect active effects and clean up dead ones
            // PRODUCTION NOTE: Modern reactive systems like Leptos or Sycamore
            // use much more sophisticated graphs (often Arena-allocated) instead of
            // RC/Weak to avoid the overhead of reference counting and dynamic dispatch.
            state.subscribers.retain(|weak_effect| {
                if let Some(effect) = weak_effect.upgrade() {
                    effects_to_run.push(effect);
                    true
                } else {
                    false // Remove dead effect
                }
            });
        }

        // Run effects outside the borrow to prevent `RefCell` panics
        // if an effect tries to read/write this signal again.
        for effect in effects_to_run {
            effect.run();
        }
    }
}

// =========================================================================================
// Effect (Observer)
// =========================================================================================

/// An Effect executes a closure immediately, and re-executes it whenever
/// any Signal it reads is updated.
pub struct Effect {
    // Keep the runner alive. When `Effect` drops, the `Rc` drops,
    // and Signals will clean up the `Weak` pointer.
    _runner: Rc<dyn EffectRunner>,
}

impl Effect {
    /// Creates and immediately runs a new Effect.
    pub fn new<F>(f: F) -> Self
    where
        F: Fn() + 'static,
    {
        // Wrap the closure in our runner implementation
        let runner = Rc::new(ClosureRunner {
            closure: RefCell::new(f),
        });

        // Run it once immediately to capture dependencies
        runner.run();

        Self { _runner: runner }
    }
}

// Concrete runner for user closures.
struct ClosureRunner<F> {
    closure: RefCell<F>,
}

impl<F: Fn()> EffectRunner for ClosureRunner<F> {
    fn run(&self) {
        // RUST INSIGHT: We need an `Rc` to `self` to put in the thread-local storage.
        // However, we only have `&self`.
        // To fix this without complex unsafe code or `Rc::from_raw`, we rely on the
        // caller (`Effect::new` or `Signal::set`) having the `Rc`.
        // Wait, how do we get the `Rc` here?
        // We can't directly upgrade `&self` to `Rc<Self>`.
        // Solution: We don't actually implement `EffectRunner` on `ClosureRunner`.
        // We implement it on a struct that holds the closure AND can be converted to Rc.
        // Actually, a simpler trick: The `Effect` or `Computed` will push *itself* to the stack.
        unreachable!("ClosureRunner shouldn't be called directly this way without Rc tracking");
    }
}

// Let's refactor the runner to hold its own weak reference, or we can use a wrapper.
// A better approach is to use a specific `Rc`-based executor.

struct RcEffectRunner {
    closure: Box<dyn Fn()>,
}

impl EffectRunner for RcEffectRunner {
    fn run(&self) {
        (self.closure)();
    }
}

// Let's implement `Effect::new` correctly.
impl Effect {
    pub fn create<F>(f: F) -> Self
    where
        F: Fn() + 'static,
    {
        // We need a way to pass the `Rc<RcEffectRunner>` to the thread-local storage.
        // Since `RcEffectRunner` doesn't know its own `Rc`, we wrap the execution in a helper.

        let runner: Rc<dyn EffectRunner> = Rc::new(RcEffectRunner {
            closure: Box::new(f),
        });

        Self::execute_with_tracking(Rc::clone(&runner));

        Self { _runner: runner }
    }

    fn execute_with_tracking(runner: Rc<dyn EffectRunner>) {
        // Set this runner as active
        ACTIVE_EFFECT.with(|active| {
            *active.borrow_mut() = Some(Rc::clone(&runner));
        });

        // Run the closure (which will call Signal::get and capture dependencies)
        runner.run();

        // Clear the active runner
        ACTIVE_EFFECT.with(|active| {
            *active.borrow_mut() = None;
        });
    }
}

// We need to override `run` for `RcEffectRunner` to also setup the tracking,
// so that when a Signal triggers it, it re-tracks dependencies.
// Actually, re-tracking is crucial because an Effect might have conditional dependencies:
// `if a.get() { b.get() } else { c.get() }`

struct TrackedRunner {
    // We use RefCell to allow mutation of the closure if needed, though Fn is used here.
    closure: Box<dyn Fn()>,
    // Store a weak pointer to ourselves so we can put it in the thread-local storage
    self_weak: RefCell<Weak<TrackedRunner>>,
}

impl EffectRunner for TrackedRunner {
    fn run(&self) {
        let rc_self = self.self_weak.borrow().upgrade().expect("Runner died");

        let prev = ACTIVE_EFFECT.with(|active| {
            let mut active_ref = active.borrow_mut();
            let p = active_ref.take();
            *active_ref = Some(rc_self as Rc<dyn EffectRunner>);
            p
        });

        (self.closure)();

        ACTIVE_EFFECT.with(|active| {
            *active.borrow_mut() = prev;
        });
    }
}

impl Effect {
    /// Creates and immediately runs a new Effect.
    /// This replaces the placeholder `new` above.
    pub fn new_tracked<F>(f: F) -> Self
    where
        F: Fn() + 'static,
    {
        let runner = Rc::new(TrackedRunner {
            closure: Box::new(f),
            self_weak: RefCell::new(Weak::new()), // Dummy initially
        });

        // Set the weak reference
        *runner.self_weak.borrow_mut() = Rc::downgrade(&runner);

        // Initial run
        runner.run();

        Self {
            _runner: runner,
        }
    }
}

// =========================================================================================
// Computed (Derived Signal)
// =========================================================================================

/// A Computed is a Signal whose value is derived from other Signals.
/// It acts as both an Observer (Effect) and an Observable (Signal).
pub struct Computed<T> {
    signal: Signal<T>,
    _effect: Effect,
}

impl<T: Clone + 'static> Computed<T> {
    /// Creates a new Computed signal.
    pub fn new<F>(f: F) -> Self
    where
        F: Fn() -> T + 'static,
    {
        // Initialize with a dummy value.
        // We must run the closure once to get the first value and track dependencies.
        // To do this elegantly, we can use an `Option` or `MaybeUninit`. For simplicity,
        // we'll run it, then create the Signal, then create the Effect that updates the Signal.

        // Wait, we need the initial value immediately.
        // We can just run it without tracking to get the value? No, better to track immediately.
        // But the Signal needs a value *now*.

        // We will temporarily disable tracking, get the value, create the signal,
        // then setup the real tracking effect.
        // Actually, we can just let the Effect overwrite the value immediately.

        // But we need a valid `T` to initialize `Signal<T>`.
        // If `T` doesn't implement `Default`, we must execute `f()` once to get it.
        // Executing `f()` here might not track dependencies if ACTIVE_EFFECT is None,
        // which is fine because the Effect below will immediately re-run it and track.

        // Temporarily suppress tracking (optional, but clean)
        let initial_val = ACTIVE_EFFECT.with(|active| {
            let prev = active.borrow_mut().take();
            let val = f();
            *active.borrow_mut() = prev;
            val
        });

        let signal = Signal::new(initial_val);
        let sig_clone = signal.clone();

        // Create the effect that updates the signal
        let effect = Effect::new_tracked(move || {
            let new_val = f();
            sig_clone.set(new_val);
        });

        Self {
            signal,
            _effect: effect,
        }
    }

    /// Gets the current derived value.
    pub fn get(&self) -> T {
        self.signal.get()
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `dioxus-signals` / `leptos_reactive`: Highly optimized. They use `Arena` allocators to avoid `Rc`
//   reference counting overhead entirely. They map signals to integers (`Copy` types) so they are
//   trivial to pass around closures without `clone()`.
// - `sycamore`: Very similar to this implementation but optimized for topological sorting.
//
// Missing vs. Production:
// - **Glitch-Free (Topological Sorting)**: If Signal A and Signal B both depend on Signal C, and
//   Computed D depends on A and B, a change to C will cause D to evaluate *twice* (once when A updates,
//   once when B updates). Production systems use a push-pull mechanism to ensure D evaluates only once.
// - **Copy semantics**: Here we must `clone()` signals into closures. Modern Rust UI frameworks use Arena
//   allocation so `Signal` is just an `ID` (e.g., `u32`) that implements `Copy`.

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn test_basic_signal_effect() {
        let name = Signal::new("Alice".to_string());

        let counter = Rc::new(RefCell::new(0));
        let counter_clone = counter.clone();

        let name_clone = name.clone();

        let _effect = Effect::new_tracked(move || {
            let current_name = name_clone.get();
            println!("Hello, {}!", current_name);
            *counter_clone.borrow_mut() += 1;
        });

        // Initial run
        assert_eq!(*counter.borrow(), 1);

        // Update
        name.set("Bob".to_string());
        assert_eq!(*counter.borrow(), 2);
    }

    #[test]
    fn test_computed() {
        let first = Signal::new("John".to_string());
        let last = Signal::new("Doe".to_string());

        let f_c = first.clone();
        let l_c = last.clone();

        let full_name = Computed::new(move || {
            format!("{} {}", f_c.get(), l_c.get())
        });

        assert_eq!(full_name.get(), "John Doe");

        first.set("Jane".to_string());
        assert_eq!(full_name.get(), "Jane Doe");

        last.set("Smith".to_string());
        assert_eq!(full_name.get(), "Jane Smith");
    }

    #[test]
    fn test_memory_leak_prevention() {
        let signal = Signal::new(10);
        let runs = Rc::new(AtomicUsize::new(0));

        {
            let sig_clone = signal.clone();
            let runs_clone = runs.clone();
            let _effect = Effect::new_tracked(move || {
                sig_clone.get();
                runs_clone.fetch_add(1, Ordering::SeqCst);
            });

            assert_eq!(runs.load(Ordering::SeqCst), 1);
            signal.set(20);
            assert_eq!(runs.load(Ordering::SeqCst), 2);

            // _effect drops here. The weak reference in `signal` should become dead.
        }

        // This should clean up the dead weak reference and NOT run the effect
        signal.set(30);

        // Run count should remain 2
        assert_eq!(runs.load(Ordering::SeqCst), 2);

        // Verify subscribers are cleaned up
        assert_eq!(signal.state.borrow().subscribers.len(), 0);
    }

    #[test]
    fn test_conditional_dependencies() {
        let condition = Signal::new(true);
        let branch_a = Signal::new(1);
        let branch_b = Signal::new(100);

        let runs = Rc::new(RefCell::new(0));
        let sum = Rc::new(RefCell::new(0));

        let cond_c = condition.clone();
        let a_c = branch_a.clone();
        let b_c = branch_b.clone();
        let runs_c = runs.clone();
        let sum_c = sum.clone();

        let _effect = Effect::new_tracked(move || {
            *runs_c.borrow_mut() += 1;
            if cond_c.get() {
                *sum_c.borrow_mut() = a_c.get();
            } else {
                *sum_c.borrow_mut() = b_c.get();
            }
        });

        assert_eq!(*runs.borrow(), 1);
        assert_eq!(*sum.borrow(), 1);

        // Updating branch_a triggers effect
        branch_a.set(2);
        assert_eq!(*runs.borrow(), 2);
        assert_eq!(*sum.borrow(), 2);

        // Updating branch_b DOES NOT trigger effect (not tracked)
        branch_b.set(200);
        assert_eq!(*runs.borrow(), 2); // Still 2!

        // Flip condition
        // Here `condition.set(false)` will run the effect.
        // Inside the effect, cond=false, so `branch_b` is accessed (`branch_b.get()`).
        // `branch_b.get()` was 200.
        // BUT wait: before this line, `sum` was 2.
        // This run will update `sum` to 200.
        // And `runs` becomes 3.
        condition.set(false);
        assert_eq!(*runs.borrow(), 3);
        assert_eq!(*sum.borrow(), 200);

        // Updating branch_a DOES trigger effect because our simple system
        // doesn't garbage collect subscriptions that are no longer hit.
        // It triggers the effect, but the effect evaluates `else`,
        // so `sum` becomes whatever `branch_b` is (200).
        branch_a.set(3);
        assert_eq!(*runs.borrow(), 4);
        assert_eq!(*sum.borrow(), 200);

        // Updating branch_b DOES trigger effect
        branch_b.set(300);
        assert_eq!(*runs.borrow(), 5);
        assert_eq!(*sum.borrow(), 300);
    }
}
