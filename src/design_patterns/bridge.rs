//! # Bridge Pattern
//!
//! Replaces: **Bridge Pattern** (OOP / GoF)
//!
//! Real Rust usage: `wgpu` (Abstracts Vulkan/Metal/DX), `sqlx` (Database drivers), Driver architectures
//!
//! ## Why this pattern exists in Rust
//! In classical OOP, the Bridge pattern prevents a "Cartesian product" explosion of subclasses
//! (e.g., `WindowsButton`, `MacButton`, `WindowsCheckbox`, `MacCheckbox`) by separating the
//! *Abstraction* (what it does) from the *Implementation* (how it does it) using composition.
//!
//! In Rust, because we don't have inheritance, we never face the subclass explosion problem in
//! the first place. The Bridge pattern simply becomes standard Rust composition:
//! A `struct` (Abstraction) containing a generic `T` or `Box<dyn Trait>` (Implementation).
//!
//! ## Architecture
//!
//! ```text
//! [ Abstraction (Struct) ] -----> [ Implementation (Trait) ]
//!   (e.g., RemoteControl)           (e.g., trait Device)
//!                                       |---> TV
//!                                       |---> DVD Player
//! ```
//!
//! **Approach 1: Static Dispatch (Generics)**
//! Fastest performance, monomorphized at compile time.
//!
//! **Approach 2: Dynamic Dispatch (Trait Objects)**
//! Allows swapping implementations at runtime or storing heterogeneous types.
//!
//! ## When to use
//! - When you want to decouple an abstraction from its implementation so the two can vary independently.
//! - Writing cross-platform libraries (like UI or graphics).
//!
//! ## Anti-patterns
//! - Deeply nested trait hierarchies trying to mimic C++ inheritance before applying Bridge.
//!
//! ```rust
//! // ANTI-PATTERN:
//! // Translating the OOP Bridge literally:
//! // trait IDevice { ... }
//! // trait IRemoteControl {
//! //     fn get_device(&self) -> &dyn IDevice; // Lifetime nightmares!
//! // }
//! ```
//! Rust prefers composition directly rather than defining traits for both
//! sides of the bridge when only one side needs to vary.

// ============================================================================
// The Implementation (How it does it)
// ============================================================================

/// The Implementation trait. This defines the low-level operations.
pub trait Device {
    fn is_enabled(&self) -> bool;
    fn enable(&mut self);
    fn disable(&mut self);
    fn get_volume(&self) -> u8;
    fn set_volume(&mut self, percent: u8);
}

// ----------------------------------------------------------------------------
// Concrete Implementations
// ----------------------------------------------------------------------------

#[derive(Default)]
pub struct Tv {
    on: bool,
    volume: u8,
}

impl Device for Tv {
    fn is_enabled(&self) -> bool {
        self.on
    }
    fn enable(&mut self) {
        self.on = true;
    }
    fn disable(&mut self) {
        self.on = false;
    }
    fn get_volume(&self) -> u8 {
        self.volume
    }
    fn set_volume(&mut self, percent: u8) {
        self.volume = percent.min(100);
    }
}

#[derive(Default)]
pub struct Radio {
    on: bool,
    volume: u8,
}

impl Device for Radio {
    fn is_enabled(&self) -> bool {
        self.on
    }
    fn enable(&mut self) {
        self.on = true;
    }
    fn disable(&mut self) {
        self.on = false;
    }
    fn get_volume(&self) -> u8 {
        self.volume
    }
    fn set_volume(&mut self, percent: u8) {
        self.volume = percent.min(100);
    }
}

// ============================================================================
// Approach 1: Static Dispatch Bridge (Idiomatic, Zero-Cost)
// ============================================================================

/// The Abstraction. It uses the Implementation via a generic type `D`.
/// COMPILE-TIME WIN: The compiler generates specific code for `RemoteControl<Tv>`
/// and `RemoteControl<Radio>`, resulting in zero-cost abstraction (no vtable lookups).
pub struct RemoteControl<D: Device> {
    // OWNERSHIP INSIGHT: The abstraction takes ownership of the implementation.
    device: D,
}

impl<D: Device> RemoteControl<D> {
    pub const fn new(device: D) -> Self {
        Self { device }
    }

    pub fn toggle_power(&mut self) {
        if self.device.is_enabled() {
            self.device.disable();
        } else {
            self.device.enable();
        }
    }

    pub fn volume_up(&mut self) {
        let old = self.device.get_volume();
        self.device.set_volume(old.saturating_add(10));
    }

    pub fn volume_down(&mut self) {
        let old = self.device.get_volume();
        self.device.set_volume(old.saturating_sub(10));
    }

    // For testing purposes
    pub const fn device(&self) -> &D {
        &self.device
    }
}

/// Refined Abstraction. We can extend the abstraction independently of the device.
pub struct AdvancedRemoteControl<D: Device> {
    remote: RemoteControl<D>,
}

impl<D: Device> AdvancedRemoteControl<D> {
    pub const fn new(device: D) -> Self {
        Self {
            remote: RemoteControl::new(device),
        }
    }

    pub fn toggle_power(&mut self) {
        self.remote.toggle_power();
    }

    pub fn mute(&mut self) {
        self.remote.device.set_volume(0);
    }

    pub const fn device(&self) -> &D {
        self.remote.device()
    }
}

// ============================================================================
// Approach 2: Dynamic Dispatch Bridge (Runtime Swapping)
// ============================================================================

/// Sometimes you need to change the implementation at runtime.
/// TRADEOFF: This requires heap allocation (`Box`) and dynamic dispatch (`dyn Device`),
/// incurring a slight performance overhead.
pub struct DynamicRemoteControl {
    device: Box<dyn Device>,
}

impl DynamicRemoteControl {
    #[must_use]
    pub fn new(device: Box<dyn Device>) -> Self {
        Self { device }
    }

    pub fn toggle_power(&mut self) {
        if self.device.is_enabled() {
            self.device.disable();
        } else {
            self.device.enable();
        }
    }

    // GOTCHA: We can't easily retrieve the concrete type out of the Box
    // without using `std::any::Any` downcasting. The bridge is intentionally opaque.
    #[must_use]
    pub fn is_on(&self) -> bool {
        self.device.is_enabled()
    }
}

// ============================================================================
// Footer
// ============================================================================
//
// How this pattern appears in std/crates:
// - The `embedded-hal` crate defines traits for hardware peripherals (I2C, SPI),
//   and device drivers (e.g., a screen driver) take generic `<I2C: embedded_hal::blocking::i2c::Write>`
//   to operate, completely bridging the driver logic from the specific microcontroller implementation.
//
// What the GoF/OOP equivalent is and why it doesn't translate directly:
// In OOP, you'd often have an `AbstractRemoteControl` that holds an `IDevice` reference.
// Because Rust heavily prefers composition over inheritance, we skip the `AbstractRemoteControl`
// base class entirely and just build composable structs.
//
// Suggested combinations with other patterns in this collection:
// - **Strategy Pattern**: The Bridge pattern is structurally identical to Strategy.
//   The difference is intent: Bridge is meant to vary an implementation (e.g. Vulkan vs Metal),
//   while Strategy is meant to vary an algorithm (e.g. QuickSort vs MergeSort).
// - **Adapter Pattern**: An adapter makes things work after they're designed;
//   a bridge makes them work before they are.
//
// META-PATTERN: "Make illegal states unrepresentable"
// By using strict trait bounds, it is physically impossible to construct a `RemoteControl`
// with something that doesn't fully implement the `Device` contract, moving integration
// errors to compile-time.

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_static_bridge_tv() {
        let tv = Tv::default();
        let mut remote = RemoteControl::new(tv);

        assert!(!remote.device().is_enabled());
        remote.toggle_power();
        assert!(remote.device().is_enabled());

        remote.volume_up();
        assert_eq!(remote.device().get_volume(), 10);
    }

    #[test]
    fn test_static_bridge_radio_advanced() {
        let radio = Radio::default();
        let mut remote = AdvancedRemoteControl::new(radio);

        remote.toggle_power();

        // Increase volume first so mute has an effect
        remote.remote.volume_up();
        assert_eq!(remote.device().get_volume(), 10);

        // Mute uses the refined abstraction feature
        remote.mute();
        assert_eq!(remote.device().get_volume(), 0);
    }

    #[test]
    fn test_dynamic_bridge() {
        // Can hold a Tv
        let mut remote = DynamicRemoteControl::new(Box::<Tv>::default());
        remote.toggle_power();
        assert!(remote.is_on());

        // Can be reassigned to a Radio at runtime!
        // PRODUCTION NOTE: This is the main reason to choose Approach 2 over Approach 1.
        remote = DynamicRemoteControl::new(Box::<Radio>::default());
        assert!(!remote.is_on()); // Radio starts off
    }
}
