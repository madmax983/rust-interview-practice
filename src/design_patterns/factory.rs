// Suppress pedantic and nursery lints for design pattern examples.
#![allow(clippy::pedantic, clippy::nursery, unused)]
//! # Factory Patterns
//!
//! Replaces: **Factory Method** (OOP), **Abstract Factory** (OOP)
//!
//! Real Rust usage: `String::new()`, `std::fs::OpenOptions` (Builder/Factory), `hyper::service::Service`
//!
//! ## Why these patterns exist in Rust
//! In traditional OOP, Factories (methods or objects) abstract object creation, often returning an interface or abstract base class.
//! Rust does not have constructors (`new` is just a convention), and returning abstract classes directly is not possible because size is not known at compile time.
//!
//! Instead, Rust uses:
//! 1. **Associated Functions (`new`)**: The native, idiomatic way to create types, acting as a Factory Method.
//! 2. **Traits returning `Self`**: To enforce a consistent initialization interface across different types.
//! 3. **Abstract Factory via Traits and Associated Types**: When families of related objects must be created, Rust uses traits with associated types (or generics) to guarantee compile-time type safety without dynamic dispatch overhead (unless `Box<dyn Trait>` is explicitly requested).
//!
//! ## Architecture
//!
//! **Approach 1: Idiomatic Factory Method (`new`)**
//! ```text
//! [ struct Button ] --(Button::new())--> [ Instance ]
//! ```
//!
//! **Approach 2: Abstract Factory (Traits + Associated Types)**
//! ```text
//! [ trait GUIFactory ] --(create_button)--> [ Self::Button ]
//!         |
//!         +--- [ WindowsFactory ] ---> [ WindowsButton ]
//!         |
//!         +--- [ MacFactory ] -------> [ MacButton ]
//! ```
//!
//! **Invariants:**
//! - Factory methods return `Self` or a concrete type when possible (zero-cost).
//! - Abstract factories tie related types together via associated types, preventing mixing a Mac Button with a Windows Checkbox at compile time.
//!
//! ## When to use
//! - **`new` functions:** 99% of the time. This is standard Rust.
//! - **Abstract Factory Traits:** When you have parallel hierarchies of types (e.g., Cross-Platform UI, Database Drivers) and need to ensure families of types are used consistently.
//!
//! ## Anti-patterns
//! - A `struct WidgetFactory` with no fields, just methods, trying to mimic Java. Use a module with functions or trait methods instead.
//! - Returning `Box<dyn Trait>` everywhere for creation when static dispatch (impl Trait or concrete types) would suffice.

// ============================================================================
// Approach 1: The Idiomatic Factory Method (`new`)
// ============================================================================

/// A simple user type.
#[derive(Debug, PartialEq)]
pub struct User {
    id: u32,
    username: String,
    role: Role,
}

#[derive(Debug, PartialEq)]
pub enum Role {
    Admin,
    Standard,
}

impl User {
    // OWNERSHIP INSIGHT: The `new` function consumes parameters and returns an owned instance.
    // This replaces `new User(...)` in Java/C++.
    #[must_use]
    pub fn new(id: u32, username: impl Into<String>) -> Self {
        Self {
            id,
            username: username.into(),
            role: Role::Standard,
        }
    }

    // Factory method for a specific variant.
    #[must_use]
    pub fn new_admin(id: u32, username: impl Into<String>) -> Self {
        Self {
            id,
            username: username.into(),
            role: Role::Admin,
        }
    }
}

// ============================================================================
// Approach 2: Abstract Factory (Traits and Associated Types)
// ============================================================================

// First, define the products.

pub trait Button {
    fn render(&self) -> String;
}

pub trait Checkbox {
    fn toggle(&mut self);
    fn is_checked(&self) -> bool;
}

// COMPILE-TIME WIN: The Abstract Factory trait uses associated types.
// This ensures that if you use a `GuiFactory`, the types of Button and Checkbox
// are strictly tied to that factory. You cannot mix them by accident.
pub trait GuiFactory {
    type Button: Button;
    type Checkbox: Checkbox;

    fn create_button(&self) -> Self::Button;
    fn create_checkbox(&self) -> Self::Checkbox;
}

// ----------------------------------------------------------------------------
// Windows Implementation
// ----------------------------------------------------------------------------

pub struct WindowsButton;
impl Button for WindowsButton {
    fn render(&self) -> String {
        "Windows Button".to_string()
    }
}

pub struct WindowsCheckbox {
    checked: bool,
}
impl Checkbox for WindowsCheckbox {
    fn toggle(&mut self) {
        self.checked = !self.checked;
    }
    fn is_checked(&self) -> bool {
        self.checked
    }
}

pub struct WindowsFactory;
impl GuiFactory for WindowsFactory {
    type Button = WindowsButton;
    type Checkbox = WindowsCheckbox;

    fn create_button(&self) -> Self::Button {
        WindowsButton
    }

    fn create_checkbox(&self) -> Self::Checkbox {
        WindowsCheckbox { checked: false }
    }
}

// ----------------------------------------------------------------------------
// macOS Implementation
// ----------------------------------------------------------------------------

pub struct MacButton;
impl Button for MacButton {
    fn render(&self) -> String {
        "Mac Button".to_string()
    }
}

pub struct MacCheckbox {
    checked: bool,
}
impl Checkbox for MacCheckbox {
    fn toggle(&mut self) {
        self.checked = !self.checked;
    }
    fn is_checked(&self) -> bool {
        self.checked
    }
}

pub struct MacFactory;
impl GuiFactory for MacFactory {
    type Button = MacButton;
    type Checkbox = MacCheckbox;

    fn create_button(&self) -> Self::Button {
        MacButton
    }

    fn create_checkbox(&self) -> Self::Checkbox {
        MacCheckbox { checked: false }
    }
}

// ============================================================================
// Client Code Usage
// ============================================================================

/// Renders a UI using any factory.
///
/// TRADEOFF: By using static dispatch (`impl GuiFactory`), a separate copy of this function
/// is compiled for every factory type used (Monomorphization). This increases binary size
/// but maximizes runtime performance.
#[must_use]
pub fn render_ui(factory: &impl GuiFactory) -> String {
    let button = factory.create_button();
    let checkbox = factory.create_checkbox();

    format!(
        "UI has {} and checkbox state: {}",
        button.render(),
        checkbox.is_checked()
    )
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_idiomatic_factory_methods() {
        let user1 = User::new(1, "alice");
        assert_eq!(user1.role, Role::Standard);

        let admin = User::new_admin(2, "bob");
        assert_eq!(admin.role, Role::Admin);
    }

    #[test]
    fn test_abstract_factory_windows() {
        let factory = WindowsFactory;
        let btn = factory.create_button();
        let mut chk = factory.create_checkbox();

        assert_eq!(btn.render(), "Windows Button");
        assert!(!chk.is_checked());
        chk.toggle();
        assert!(chk.is_checked());
    }

    #[test]
    fn test_abstract_factory_mac() {
        let factory = MacFactory;
        assert_eq!(factory.create_button().render(), "Mac Button");

        // COMPILE-TIME WIN: The following line would fail to compile because
        // the type expected by Mac checkbox logic is exclusively `MacCheckbox`.
        // let mut mismatched_chk: WindowsCheckbox = factory.create_checkbox();
    }

    #[test]
    fn test_render_ui() {
        assert_eq!(
            render_ui(&WindowsFactory),
            "UI has Windows Button and checkbox state: false"
        );
        assert_eq!(
            render_ui(&MacFactory),
            "UI has Mac Button and checkbox state: false"
        );
    }
}
