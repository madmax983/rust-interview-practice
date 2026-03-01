//! # Observer Pattern (Rust-Style)
//!
//! Replaces: **Observer Pattern** (OOP), **Event Listeners** (Java/C#)
//!
//! Real Rust usage: `std::sync::mpsc`, `tokio::sync::broadcast`, `crossbeam_channel`
//!
//! ## Why this pattern exists in Rust
//! In classic OOP, the Observer pattern relies on objects holding mutable references to each other
//! (`Subject` holds a list of `Observer` references). This inherently violates Rust's borrowing rules:
//! you cannot have multiple mutable references, and a `Subject` cannot easily mutate an `Observer` while
//! the `Observer` potentially mutates the `Subject` (re-entrancy).
//!
//! To solve this naturally in Rust, we use two primary approaches:
//! 1. **Synchronous Closures (`Vec<Box<dyn FnMut>>`):** Best for single-threaded UI or event loops.
//! 2. **Channel-Based Asynchronous Broadcasting (`mpsc` / `broadcast`):** Best for decoupled, multi-threaded systems.
//!
//! ## Architecture
//!
//! **Approach 1: Synchronous Closures**
//! ```text
//! struct Button {
//!     on_click: Vec<Box<dyn FnMut()>>, // Trait objects for callbacks
//! }
//! ```
//!
//! **Approach 2: Channels (Idiomatic Rust)**
//! ```text
//!          [ Sender ] ---> (Channel) ---> [ Receiver 1 ]
//!                                   ---> [ Receiver 2 ]
//! ```
//!
//! **Invariants Enforced:**
//! - **No Data Races:** Channels enforce `Send` bounds, guaranteeing thread-safe event passing.
//! - **Memory Safety:** Closures (`FnMut`) capture state explicitly according to borrowing rules, preventing dangling references.
//!
//! ## When to use
//! - **Synchronous:** Simple UI components, synchronous state machines, local event emitters.
//! - **Channels:** Distributed systems, cross-thread communication, decoupled actor systems.
//!
//! ## Anti-patterns
//! - Trying to use `Rc<RefCell<dyn Observer>>` to exactly mirror Java. It leads to runtime panics (`BorrowMutError`) and memory leaks via reference cycles.

use std::sync::mpsc;

// ============================================================================
// Approach 1: Synchronous Closures (Event Emitter)
// ============================================================================

type ObserverCallback<T> = Box<dyn FnMut(&T)>;

/// A generic Subject that notifies registered observers via closures.
///
/// **TRADEOFF:** Because we use `Box<dyn FnMut>`, each observer adds a heap allocation and dynamic dispatch overhead.
///
/// **OWNERSHIP INSIGHT:** The `Subject` takes ownership of the closures. Observers cannot hold references to the `Subject` itself if they mutate it, preventing re-entrancy bugs.
#[derive(Default)]
pub struct Subject<T> {
    observers: Vec<ObserverCallback<T>>,
}

impl<T> Subject<T> {
    #[must_use]
    pub fn new() -> Self {
        Self {
            observers: Vec::new(),
        }
    }

    /// Registers a new observer closure.
    ///
    /// **GOTCHA:** The closure must be `'static` if `Subject` lives arbitrarily long.
    /// You must `move` captured variables (like `Rc` or `Arc`) into the closure.
    pub fn attach<F>(&mut self, observer: F)
    where
        F: FnMut(&T) + 'static,
    {
        self.observers.push(Box::new(observer));
    }

    /// Notifies all observers with the given event data.
    pub fn notify(&mut self, event: &T) {
        for observer in &mut self.observers {
            observer(event);
        }
    }
}

// ============================================================================
// Approach 2: Channel-Based Asynchronous Broadcasting
// ============================================================================

/// Represents an event that can be broadcasted.
#[derive(Debug, Clone)]
pub enum SystemEvent {
    UserLoggedIn(String),
    ServerStarted,
    Shutdown,
}

/// A decoupled publisher using channels.
///
/// **PRODUCTION NOTE:** In production, use `tokio::sync::broadcast` for multi-producer,
/// multi-consumer (MPMC) patterns. Here, we use multiple `mpsc` senders to simulate it.
///
/// **COMPILE-TIME WIN:** The `SystemEvent` enum makes the state space closed and exhaustive.
/// You cannot send an undocumented event type.
pub struct EventBus {
    subscribers: Vec<mpsc::Sender<SystemEvent>>,
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

impl EventBus {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            subscribers: Vec::new(),
        }
    }

    /// Subscribes to the event bus, returning a receiver channel.
    ///
    /// **ANTI-PATTERN:** Returning an `ID` and forcing the subscriber to implement a trait
    /// forces tight coupling. Returning a `Receiver` gives the subscriber control over *how*
    /// and *when* to process events.
    pub fn subscribe(&mut self) -> mpsc::Receiver<SystemEvent> {
        let (tx, rx) = mpsc::channel();
        self.subscribers.push(tx);
        rx
    }

    /// Broadcasts an event to all active subscribers.
    pub fn broadcast(&mut self, event: &SystemEvent) {
        // Retain only the senders where the receiver hasn't disconnected.
        self.subscribers.retain(|tx| tx.send(event.clone()).is_ok());
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    #[test]
    fn test_synchronous_closures() {
        let mut button = Subject::<String>::new();

        // Use Rc<RefCell> to mutate external state from within the closure
        let click_count = Rc::new(RefCell::new(0));

        let count_clone = Rc::clone(&click_count);
        button.attach(move |event| {
            if event == "click" {
                *count_clone.borrow_mut() += 1;
            }
        });

        let last_event = Rc::new(RefCell::new(String::new()));
        let last_event_clone = Rc::clone(&last_event);
        button.attach(move |event| {
            *last_event_clone.borrow_mut() = event.clone();
        });

        button.notify(&"hover".to_string());
        assert_eq!(*click_count.borrow(), 0);
        assert_eq!(*last_event.borrow(), "hover");

        button.notify(&"click".to_string());
        assert_eq!(*click_count.borrow(), 1);
        assert_eq!(*last_event.borrow(), "click");
    }

    #[test]
    fn test_channel_broadcaster() {
        let mut bus = EventBus::new();

        let rx1 = bus.subscribe();
        let rx2 = bus.subscribe();

        bus.broadcast(&SystemEvent::ServerStarted);
        bus.broadcast(&SystemEvent::UserLoggedIn("Alice".to_string()));

        // Drop rx2 to test the disconnect logic
        drop(rx2);
        bus.broadcast(&SystemEvent::Shutdown);

        // Assert rx1 receives everything
        assert!(matches!(rx1.recv().unwrap(), SystemEvent::ServerStarted));
        assert!(matches!(
            rx1.recv().unwrap(),
            SystemEvent::UserLoggedIn(user) if user == "Alice"
        ));
        assert!(matches!(rx1.recv().unwrap(), SystemEvent::Shutdown));

        // Assert bus cleaned up the dropped receiver
        assert_eq!(bus.subscribers.len(), 1);
    }
}

// ============================================================================
// The "Anti-Pattern" (What code looks like *without* this Rust-native approach)
// ============================================================================
//
// If you try to write Java-style Observer in Rust, it looks like this:
//
// ```rust
// trait Observer { fn update(&mut self, event: &str); }
// struct Subject { observers: Vec<Rc<RefCell<dyn Observer>>> }
//
// impl Subject {
//     fn notify(&self, event: &str) {
//         for obs in &self.observers {
//             // PANIC RISK: If `obs.update` mutates the Subject, it triggers a
//             // BorrowMutError at runtime. We've lost compile-time safety.
//             obs.borrow_mut().update(event);
//         }
//     }
// }
// ```
//
// **META-PATTERN:** "Make illegal states unrepresentable." The OOP approach above makes
// re-entrancy bugs and memory leaks representable at runtime. The Rust-native channel approach
// shifts these invariants to compile time: if you have a `Sender`, you can send; if you have a
// `Receiver`, you process events asynchronously, making re-entrancy impossible by design.

// ============================================================================
// Footer
// ============================================================================
// **Standard Library & Crates:** `std::sync::mpsc`, `crossbeam_channel`, `tokio::sync::broadcast`
//
// **GoF Equivalent:** Observer. In GoF, observers register themselves with the subject. In Rust,
// we invert the relationship using channels: the subject hands out a receiver, and the observer
// processes it independently, solving the lifetime and mutability conflicts.
//
// **Alternatives:** If you only have one observer, a simple callback function or `FnOnce` (if
// only firing once) is vastly simpler than building an event bus or a closure registry.
//
// **Suggested Combinations:**
// - **Command Pattern:** Observers often trigger Commands.
// - **Actor Pattern:** Channels are the primitive building blocks of true Actor systems.
