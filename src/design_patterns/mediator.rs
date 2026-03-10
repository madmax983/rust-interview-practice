// Suppress pedantic and nursery lints for design pattern examples.
#![allow(clippy::pedantic, clippy::nursery, unused)]
//! # Mediator Pattern
//!
//! Replaces: **Mediator Pattern** (OOP)
//!
//! Real Rust usage: `std::sync::mpsc` (channels), Actix (actor mailboxes), central ECS systems.
//!
//! ## Why this pattern exists in Rust
//! The Mediator pattern is used to reduce chaotic dependencies between objects. Instead of objects
//! communicating directly with each other (a web of dependencies), they communicate through a central Mediator.
//!
//! In traditional OOP, this often results in a web of bi-directional references: the Mediator holds
//! references to all Colleagues, and all Colleagues hold a reference to the Mediator.
//! In Rust, a web of `Rc<RefCell<T>>` is an **anti-pattern**. It leads to runtime panics and poor performance.
//!
//! The idiomatic Rust solution is **Message Passing**. Colleagues own a `Sender` to send events to the Mediator.
//! The Mediator receives these events, updates central state, and then either calls methods on the Colleagues
//! or sends messages back to them.
//!
//! ## Architecture
//!
//! **Approach: Channel-based Mediator**
//! ```text
//! [ Colleague A ] --(mpsc::Sender)--> [ Event Queue ]
//!                                           |
//!                                     (mpsc::Receiver)
//!                                           v
//!                                      [ Mediator ] --(mutates state)
//!                                           |
//!                                    (calls methods)
//!                                           v
//!                                 [ Colleague B, C, D ]
//! ```
//!
//! **Invariants:**
//! - Colleagues do not know about each other.
//! - Colleagues do not hold a reference to the Mediator, only a mechanism to send messages.
//! - The Mediator has exclusive mutable access to the Colleagues when applying updates.
//!
//! ## When to use
//! - UI frameworks where clicking a button updates multiple unrelated views.
//! - Chat systems or game lobbies where clients broadcast to a server.
//!
//! ## Anti-patterns
//! - Using `Rc<RefCell<Mediator>>` inside every Colleague. It creates circular dependencies and makes
//!   memory leaks possible.
//!
//! ## Footer
//! The GoF Mediator is fundamentally about managing state transitions between decoupled entities. Rust's
//! ownership model naturally pushes this towards an Event Loop / Actor architecture.

use std::sync::mpsc::{self, Receiver, Sender};

// ============================================================================
// Approach: Channel-based Mediator
// ============================================================================

/// Events that Colleagues can send to the Mediator.
// COMPILE-TIME WIN: Enums make the set of possible events exhaustive and type-safe.
#[derive(Debug, PartialEq)]
pub enum UiEvent {
    CheckboxToggled(bool),
    SubmitClicked,
}

/// A Colleague: A simple UI Checkbox.
pub struct Checkbox {
    pub is_checked: bool,
    sender: Sender<UiEvent>,
}

impl Checkbox {
    #[must_use]
    pub fn new(sender: Sender<UiEvent>) -> Self {
        Self {
            is_checked: false,
            sender,
        }
    }

    pub fn toggle(&mut self) {
        self.is_checked = !self.is_checked;
        // Colleague notifies the Mediator, but doesn't know what happens next.
        let _ = self.sender.send(UiEvent::CheckboxToggled(self.is_checked));
    }
}

/// A Colleague: A simple UI Submit Button.
pub struct SubmitButton {
    pub is_enabled: bool,
    sender: Sender<UiEvent>,
}

impl SubmitButton {
    #[must_use]
    pub fn new(sender: Sender<UiEvent>) -> Self {
        Self {
            is_enabled: false, // Disabled by default
            sender,
        }
    }

    pub fn click(&self) {
        if self.is_enabled {
            let _ = self.sender.send(UiEvent::SubmitClicked);
        }
    }
}

/// The Mediator (or "Controller"). It owns the Colleagues and processes their events.
pub struct DialogMediator {
    // OWNERSHIP INSIGHT: The Mediator owns the Colleagues, avoiding lifetimes and `Rc/RefCell`.
    pub checkbox: Checkbox,
    pub submit_button: SubmitButton,
    receiver: Receiver<UiEvent>,

    // Internal state tracking
    pub submitted: bool,
}

impl DialogMediator {
    #[must_use]
    pub fn new() -> (Self, Sender<UiEvent>) {
        let (tx, rx) = mpsc::channel();

        let mediator = Self {
            checkbox: Checkbox::new(tx.clone()),
            submit_button: SubmitButton::new(tx.clone()),
            receiver: rx,
            submitted: false,
        };

        (mediator, tx)
    }

    /// The Event Loop: Process all pending events.
    pub fn process_events(&mut self) {
        // We use `try_recv` to process events currently in the queue without blocking.
        while let Ok(event) = self.receiver.try_recv() {
            match event {
                UiEvent::CheckboxToggled(checked) => {
                    // Mediator logic: The submit button is only enabled if the checkbox is checked.
                    self.submit_button.is_enabled = checked;
                }
                UiEvent::SubmitClicked => {
                    self.submitted = true;
                }
            }
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mediator_event_flow() {
        // Note: tx is the main sender, but our components hold their own clones.
        let (mut dialog, _tx) = DialogMediator::new();

        assert!(!dialog.submit_button.is_enabled);
        assert!(!dialog.submitted);

        // User clicks the submit button while it's disabled.
        dialog.submit_button.click();
        dialog.process_events();
        assert!(!dialog.submitted, "Should not submit if disabled");

        // User toggles the checkbox.
        dialog.checkbox.toggle();

        // At this point, the button is still disabled because the Mediator hasn't processed the event.
        assert!(!dialog.submit_button.is_enabled);

        // Mediator processes events.
        dialog.process_events();

        // Now the Mediator has updated the button state based on the checkbox event.
        assert!(dialog.submit_button.is_enabled);

        // User clicks the submit button again.
        dialog.submit_button.click();
        dialog.process_events();

        // The Mediator processed the submit event.
        assert!(dialog.submitted);
    }
}
