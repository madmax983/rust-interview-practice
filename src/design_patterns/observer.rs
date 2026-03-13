// Suppress pedantic and nursery lints for design pattern examples.
#![allow(clippy::pedantic, clippy::nursery, unused)]

//! # Observer Pattern
//!
//! Replaces: **Observer Pattern** (OOP), **Callbacks / Event Listeners**
//!
//! Real Rust usage: `tokio::sync::mpsc`, `std::sync::mpsc`, `actix`
//!
//! ## Why this pattern exists in Rust
//! In traditional OOP, the Observer pattern relies on objects holding mutable references to their listeners,
//! often leading to memory leaks or complex lifetime management. In Rust, sharing mutable state is
//! strictly controlled by the compiler ("Shared XOR Mutable").
//!
//! Instead of lists of mutable object references, Rust implements observation through:
//! 1. **Channels (Asynchronous/Concurrent):** Using `mpsc` (Multi-Producer, Single-Consumer) or broadcast channels,
//!    where the subject sends messages to subscribers. This is the idiomatic, safe approach for concurrent systems.
//! 2. **Trait Objects (Synchronous):** A list of boxed closures `Vec<Box<dyn FnMut(...)>>`.
//!
//! By strictly isolating state via message passing, Rust implements the meta-pattern:
//! **"make illegal states unrepresentable"**, completely eliminating dangling listener references.
//!
//! ## Architecture
//!
//! **Approach 1: Channel-Based (Idiomatic)**
//! ```text
//! [ Subject ] --(Event)--> [ Sender ] =====> [ Receiver ] --> [ Observer Task ]
//! ```
//!
//! **Approach 2: Synchronous Callbacks**
//! ```text
//! [ Subject ]
//!    |--- vec![ Box<dyn FnMut(&Event)> ]
//!    |---> listener_1(event)
//!    |---> listener_2(event)
//! ```
//!
//! **Invariants:**
//! - With channels, the subject does not need to know when or how the event is processed.
//! - Observers can be dropped without notifying the subject (the channel simply closes or errors on send).
//! - Compile-time guarantees prevent data races when events are sent across threads.
//!
//! ## When to use
//! - **Channels:** For decoupled, concurrent systems, GUI event loops, or async programming.
//! - **Callbacks:** For simple, single-threaded synchronous updates where low latency is critical.
//!
//! ## Anti-patterns
//! - A `Vec<&mut dyn Observer>` that requires immense lifetime acrobatics or `Rc<RefCell<T>>` hell to manage.

use std::sync::mpsc;

// ============================================================================
// Approach 1: Channel-Based Observer (Idiomatic Rust)
// ============================================================================

#[derive(Clone, Debug, PartialEq)]
pub struct Event {
    pub name: String,
    pub payload: i32,
}

/// A thread-safe, decoupled subject using channels.
///
/// **COMPILE-TIME WIN:** The compiler guarantees `Event` is safe to send across threads (`Send`),
/// preventing data races automatically.
pub struct ConcurrentSubject {
    // The subject holds a list of senders. It doesn't know who is listening.
    subscribers: Vec<mpsc::Sender<Event>>,
}

impl Default for ConcurrentSubject {
    fn default() -> Self {
        Self::new()
    }
}

impl ConcurrentSubject {
    pub fn new() -> Self {
        Self {
            subscribers: Vec::new(),
        }
    }

    /// Subscribes to the subject, returning a Receiver that the observer can listen to.
    ///
    /// **OWNERSHIP INSIGHT:** We return the `Receiver` to the caller, transferring ownership.
    /// The subject retains the `Sender`. When the receiver is dropped, `send()` will return an error,
    /// allowing us to clean up dead subscribers.
    pub fn subscribe(&mut self) -> mpsc::Receiver<Event> {
        // TRADEOFF: Channels introduce minor allocation overhead compared to raw pointers,
        // but completely eliminate the risk of memory leaks and race conditions.
        let (tx, rx) = mpsc::channel();
        self.subscribers.push(tx);
        rx
    }

    pub fn notify(&mut self, event: Event) {
        // We use `retain` to automatically remove disconnected subscribers.
        // If `send` returns an Err, it means the Receiver was dropped.
        // GOTCHA: Forgetting to handle dropped Receivers will cause a memory leak of Senders.
        self.subscribers.retain(|tx| tx.send(event.clone()).is_ok());
    }
}

// ============================================================================
// Approach 2: Synchronous Callbacks (Single-threaded)
// ============================================================================

// OWNERSHIP INSIGHT: We use `Box<dyn FnMut>` to store closures.
// Because the Subject owns these closures, they cannot easily mutate variables
// in the external environment unless those variables are wrapped in `Rc<RefCell<T>>`.
pub struct Subject {
    #[allow(clippy::type_complexity)]
    listeners: Vec<Box<dyn FnMut(&Event)>>,
}

impl Default for Subject {
    fn default() -> Self {
        Self::new()
    }
}

impl Subject {
    pub fn new() -> Self {
        Self {
            listeners: Vec::new(),
        }
    }

    pub fn subscribe<F>(&mut self, callback: F)
    where
        F: FnMut(&Event) + 'static,
    {
        // PRODUCTION NOTE: Storing closures requires heap allocation (`Box`) and dynamic dispatch (`dyn`).
        // If high-performance is needed, a channel or a trait-based approach may be superior.
        self.listeners.push(Box::new(callback));
    }

    pub fn notify(&mut self, event: &Event) {
        for listener in &mut self.listeners {
            listener(event);
        }
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
    use std::thread;

    #[test]
    fn test_concurrent_subject() {
        let mut subject = ConcurrentSubject::new();

        // Subscriber 1
        let rx1 = subject.subscribe();
        // Subscriber 2
        let rx2 = subject.subscribe();

        // Notify both
        subject.notify(Event {
            name: "Start".to_string(),
            payload: 1,
        });

        // Verify both received
        assert_eq!(rx1.recv().unwrap().name, "Start");
        assert_eq!(rx2.recv().unwrap().payload, 1);

        // Disconnect subscriber 1
        drop(rx1);

        // Notify again. Subject should automatically clean up rx1's sender.
        subject.notify(Event {
            name: "Update".to_string(),
            payload: 2,
        });

        assert_eq!(subject.subscribers.len(), 1); // rx1 removed
        assert_eq!(rx2.recv().unwrap().name, "Update");
    }

    #[test]
    fn test_concurrent_threads() {
        let mut subject = ConcurrentSubject::new();
        let rx = subject.subscribe();

        let handle = thread::spawn(move || {
            let mut sum = 0;
            while let Ok(event) = rx.recv() {
                if event.name == "Stop" {
                    break;
                }
                sum += event.payload;
            }
            sum
        });

        subject.notify(Event {
            name: "Data".to_string(),
            payload: 10,
        });
        subject.notify(Event {
            name: "Data".to_string(),
            payload: 20,
        });
        subject.notify(Event {
            name: "Stop".to_string(),
            payload: 0,
        });

        let result = handle.join().unwrap();
        assert_eq!(result, 30);
    }

    #[test]
    fn test_synchronous_callbacks() {
        let mut subject = Subject::new();

        // We use Rc<RefCell> to mutate external state from within the closure
        let received_events = Rc::new(RefCell::new(Vec::new()));

        let state_clone = Rc::clone(&received_events);
        subject.subscribe(move |event| {
            state_clone.borrow_mut().push(event.clone());
        });

        subject.notify(&Event {
            name: "Click".to_string(),
            payload: 42,
        });

        let events = received_events.borrow();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].name, "Click");
        assert_eq!(events[0].payload, 42);
    }
}

// ============================================================================
// Footer
// ============================================================================
//
// How this pattern appears in std/crates:
// - `tokio::sync::broadcast`: A true multi-producer, multi-consumer broadcast channel often used for pub/sub.
// - `std::sync::mpsc`: Standard multi-producer, single-consumer channels.
//
// What the GoF/OOP equivalent is and why it doesn't translate directly:
// In OOP, observers register themselves (`this`) with the subject. In Rust, you cannot easily
// pass `&mut self` to a subject and keep it alive indefinitely without violating borrowing rules.
// Attempting to do so requires a dense web of `Rc<RefCell<Observer>>`, which is an anti-pattern.
//
// When to reach for this vs. simpler alternatives:
// Reach for channels when you need decoupled, concurrent event processing.
// If you just need to call a function on state change in a single thread, a simple callback
// (`Option<Box<dyn FnMut>>`) is often simpler than a full observer list.
//
// Suggested combinations with other patterns in this collection:
// - **Actor Pattern**: The channel-based observer is essentially a lightweight Actor pattern.
// - **Command Pattern**: Events sent through the channel are often Commands that dictate state changes.
//
// META-PATTERN: "Make illegal states unrepresentable"
// By transferring ownership of the `Receiver` to the observer via Channels, it becomes impossible for the observer
// to be notified after it has been destroyed (the channel simply closes), preventing the classic OOP
// "dangling listener / memory leak" problem at compile time.
