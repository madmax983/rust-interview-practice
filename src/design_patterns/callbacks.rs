//! # Closure-Based Callbacks & Higher-Order Functions
//!
//! Replaces: **Strategy Pattern** (OOP), **Delegates** (C#), **Lambdas** (Java 8+)
//!
//! Real Rust usage: `Iterator::map`, `Option::and_then`, `std::thread::spawn`, `tokio::spawn`
//!
//! ## Why this pattern exists in Rust
//! Rust closures are anonymous functions that can capture their environment. Unlike languages where
//! closures always allocate (GC'd languages), Rust closures are unboxed by default and come in three flavors
//! based on how they capture variables:
//! 1. `FnOnce`: Consumes captured variables (moves them). Callable once.
//! 2. `FnMut`: Mutates captured variables. Callable multiple times.
//! 3. `Fn`: Reads captured variables (immutable borrow). Callable multiple times concurrently.
//!
//! ## Architecture
//!
//! ```text
//! fn retry<F, T, E>(n: usize, op: F) -> Result<T, E>
//! where
//!     F: FnMut() -> Result<T, E>  // Strategy injected as a closure
//! { ... }
//! ```
//!
//! **Invariants:**
//! - The caller decides what state is captured and how (move vs borrow).
//! - The callee defines the interface (`Fn*` trait bounds).
//! - Zero overhead: Generic closures are monomorphized (inlined).
//!
//! ## When to use
//! - When passing behavior as an argument (e.g., sort comparators, event handlers).
//! - To delay execution (lazy evaluation).
//! - To customize algorithms without creating new classes/structs.

use std::collections::HashMap;

// ============================================================================
// Pattern 1: The Retry Logic (FnMut)
// ============================================================================

/// Retries an operation `n` times.
///
/// **OWNERSHIP INSIGHT:** We use `FnMut` because the operation might need to mutate
/// its own state on each attempt (e.g., a counter, or an RNG).
/// If we used `Fn`, it couldn't mutate state.
/// If we used `FnOnce`, we could only call it once (no retry!).
pub fn retry<F, T, E>(mut n: usize, mut op: F) -> Result<T, E>
where
    F: FnMut() -> Result<T, E>,
{
    loop {
        match op() {
            Ok(val) => return Ok(val),
            Err(e) => {
                if n == 0 {
                    return Err(e);
                }
                n -= 1;
                // In real life: std::thread::sleep(...)
            }
        }
    }
}

// ============================================================================
// Pattern 2: Event Emitter (Box<dyn FnMut>)
// ============================================================================

// Type alias for our callback
// We use FnMut to allow listeners to mutate their own state (e.g. counters).
// Note: FnMut is not thread-safe if shared across threads (requires Mutex),
// but here we are in a single-threaded context for simplicity, or we would need
// `Box<dyn Fn(...) + Send + Sync>` and `Arc<Mutex<...>>` for state.
//
// However, the prompt asked for "Closure-Based Callbacks" and mentioned "Event Emitter".
// A strictly threaded emitter usually needs `Fn` or `Arc<Mutex<FnMut>>`.
// Let's stick to `FnMut` as it is more flexible for single-threaded usage (common in UI/game loops),
// but we'll mark it as non-thread-safe if we don't add Send+Sync.
type EventHandler = Box<dyn FnMut(&str) + Send + Sync>;

/// A simple event emitter that stores callbacks.
///
/// **TRADEOFF:** Because we store multiple callbacks in a collection (heterogeneous types),
/// we must box them (`Box<dyn FnMut...>`) and use dynamic dispatch.
///
/// **Thread Safety:** We require `Send + Sync` so the Emitter itself is thread-safe,
/// but since we use `FnMut`, calling `emit` requires `&mut self` (exclusive access).
pub struct EventEmitter {
    // Map event name to list of handlers
    handlers: HashMap<String, Vec<EventHandler>>,
}

impl Default for EventEmitter {
    fn default() -> Self {
        Self::new()
    }
}

impl EventEmitter {
    pub fn new() -> Self {
        EventEmitter {
            handlers: HashMap::new(),
        }
    }

    /// Registers a handler for an event.
    ///
    /// **LIFETIME INSIGHT:** The closure must be `'static` because we store it indefinitely.
    /// It cannot capture references to the stack frame where `on` is called.
    /// `move ||` is often required.
    pub fn on<F>(&mut self, event: &str, handler: F)
    where
        F: FnMut(&str) + Send + Sync + 'static,
    {
        self.handlers
            .entry(event.to_string())
            .or_default()
            .push(Box::new(handler));
    }

    /// Emits an event, calling all registered handlers.
    ///
    /// Note: Requires `&mut self` because the handlers are `FnMut`.
    pub fn emit(&mut self, event: &str, data: &str) {
        if let Some(listeners) = self.handlers.get_mut(event) {
            for listener in listeners {
                listener(data);
            }
        }
    }
}

// ============================================================================
// Pattern 3: Higher-Order Mapper (Fn)
// ============================================================================

/// Applies a function to a value if a condition is met.
///
/// **COMPILE-TIME WIN:** This function is generic over `F`. When you call it with a closure,
/// the compiler generates a specialized version of `conditional_map` just for that closure.
/// No function pointer overhead.
pub fn conditional_map<T, F>(val: T, condition: bool, mapper: F) -> T
where
    F: FnOnce(T) -> T,
{
    if condition {
        mapper(val)
    } else {
        val
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_retry_success() {
        let mut attempts = 0;
        let result = retry(3, || {
            attempts += 1;
            if attempts < 2 {
                Err("Fail")
            } else {
                Ok("Success")
            }
        });

        assert_eq!(result, Ok("Success"));
        assert_eq!(attempts, 2);
    }

    #[test]
    fn test_retry_fail() {
        let mut attempts = 0;
        let result: Result<&str, &str> = retry(2, || {
            attempts += 1;
            Err("Always Fail")
        });

        assert_eq!(result, Err("Always Fail"));
        assert_eq!(attempts, 3); // Initial + 2 retries
    }

    #[test]
    fn test_event_emitter_fn_mut() {
        let mut emitter = EventEmitter::new();

        let mut _count = 0; // Prefixed with _ to silence warning

        // We can capture `count` by mutable reference because `emit` takes `&mut emitter`
        // but we are registering the closure. The closure takes ownership of `count` if we `move`.
        // Wait, if we move `count` into the closure, we can't inspect it later easily outside.
        // We typically use internal state for the closure or Rc/Arc.
        // But for a simple test, we can use a simplified approach or verify side effects.

        // Let's use a shared counter for verification
        use std::sync::{Arc, Mutex};
        let shared_count = Arc::new(Mutex::new(0));
        let c = shared_count.clone();

        emitter.on("click", move |_| {
            let mut g = c.lock().unwrap();
            *g += 1;
        });

        emitter.emit("click", "btn1");
        emitter.emit("click", "btn2");

        assert_eq!(*shared_count.lock().unwrap(), 2);
    }

    #[test]
    fn test_event_emitter_stateful_closure() {
        // Demonstrating FnMut state within the closure
        let mut emitter = EventEmitter::new();

        // This closure has its own internal state `local_count`
        let mut local_count = 0;
        emitter.on("tick", move |_| {
            local_count += 1;
            // logic that depends on history
            if local_count > 1 {
                // do something
            }
        });

        emitter.emit("tick", ".");
        emitter.emit("tick", ".");
    }

    #[test]
    fn test_conditional_map() {
        let val = 10;
        // Closure captures nothing, so it's effectively a function pointer
        let res = conditional_map(val, true, |x| x * 2);
        assert_eq!(res, 20);

        let res2 = conditional_map(val, false, |x| x * 2);
        assert_eq!(res2, 10);
    }

    #[test]
    fn test_capture_environment() {
        let factor = 5;
        // Closure captures `factor` by reference (immutably)
        let res = conditional_map(10, true, |x| x * factor);
        assert_eq!(res, 50);
    }
}
