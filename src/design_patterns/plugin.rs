//! # Plugin Architecture with Trait Objects
//!
//! Replaces: **Dynamic Class Loading** (Java), **Duck Typing Registries** (Python)
//!
//! Real Rust usage: `cargo`, `bevy` (ECS plugins), `log`
//!
//! ## Why this pattern exists in Rust
//! Rust is statically typed and compiled, making true dynamic plugins (`.so` or `.dll`
//! via `dlopen`/`libloading`) complex and brittle due to ABI instability (the Rust ABI
//! changes with every compiler version).
//! Instead, the standard approach is a static plugin registry using trait objects
//! (`Box<dyn Plugin>`) or crates like `inventory`/`linkme` for distributed registration.
//!
//! ## Architecture
//!
//! ```text
//! [ Plugin Trait ] <--- implements --- [ LoggerPlugin, MetricsPlugin ]
//!        |
//!        v
//! [ Plugin Registry ] --- runs hooks ---> [ Plugin trait methods ]
//! ```
//!
//! **Invariants:**
//! - Plugins must be object-safe (no generics in methods, no `Self` returns) to be boxed.
//! - The registry owns the plugins (`Box<dyn Plugin>`).
//! - If plugins need mutable state, they must use interior mutability (`Mutex` or `RefCell`)
//!   because registry iteration usually uses `&self`.
//!
//! ## When to use
//! - Extensible architectures where different teams/modules provide functionality.
//! - When you want to decouple the core system from optional features.

use std::any::Any;

// ============================================================================
// Pattern: The Static Plugin Registry
// ============================================================================

/// The fundamental trait all plugins must implement.
///
/// **GOTCHA:** This trait must be Object Safe.
/// Notice we don't use `fn setup<T>()` or `-> Self`.
pub trait Plugin: Send + Sync {
    /// A required name for the plugin
    fn name(&self) -> &'static str;

    /// Lifecycle hook: Called once when the plugin is registered
    fn on_register(&self, _app: &mut App) {}

    /// Lifecycle hook: Called every frame or tick
    fn on_tick(&self) {}

    /// Allow downcasting the trait object to the concrete type
    fn as_any(&self) -> &dyn Any;
}

/// The core application that hosts plugins.
pub struct App {
    plugins: Vec<Box<dyn Plugin>>,
    // In a real app, this might be a complex state struct
    pub tick_count: u32,
}

impl App {
    pub fn new() -> Self {
        Self {
            plugins: Vec::new(),
            tick_count: 0,
        }
    }

    /// Register a new plugin.
    ///
    /// **OWNERSHIP INSIGHT:** The `App` takes ownership of the plugin via `Box`.
    pub fn add_plugin<P: Plugin + 'static>(&mut self, plugin: P) {
        println!("Registering plugin: {}", plugin.name());

        // Let the plugin configure the app if it needs to
        plugin.on_register(self);

        // Store it for later hooks
        self.plugins.push(Box::new(plugin));
    }

    /// Simulate the main loop
    pub fn run_tick(&mut self) {
        self.tick_count += 1;

        // Iterate over all plugins and call their hook
        for plugin in &self.plugins {
            plugin.on_tick();
        }
    }

    /// Retrieve a plugin by type (demonstrating `Any` downcasting)
    pub fn get_plugin<P: Plugin + 'static>(&self) -> Option<&P> {
        self.plugins
            .iter()
            .find_map(|p| p.as_any().downcast_ref::<P>())
    }
}

// ============================================================================
// Logic Simulation (Concrete Plugins)
// ============================================================================

pub struct LoggerPlugin;

impl Plugin for LoggerPlugin {
    fn name(&self) -> &'static str {
        "Logger"
    }

    fn on_tick(&self) {
        // Minimal behavior
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

pub struct MetricsPlugin {
    pub id: String,
}

impl Plugin for MetricsPlugin {
    fn name(&self) -> &'static str {
        "Metrics"
    }

    fn on_register(&self, app: &mut App) {
        // Example: The plugin could modify the app state here
        app.tick_count = 0; // Reset just to prove we can access it
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_lifecycle() {
        let mut app = App::new();

        app.add_plugin(LoggerPlugin);
        app.add_plugin(MetricsPlugin { id: "prod-1".to_string() });

        // Verify on_register ran
        assert_eq!(app.tick_count, 0);

        app.run_tick();
        assert_eq!(app.tick_count, 1);
    }

    #[test]
    fn test_plugin_downcasting() {
        let mut app = App::new();
        app.add_plugin(MetricsPlugin { id: "test-id".to_string() });

        let metrics = app.get_plugin::<MetricsPlugin>().unwrap();
        assert_eq!(metrics.id, "test-id");

        let logger = app.get_plugin::<LoggerPlugin>();
        assert!(logger.is_none());
    }
}

// ============================================================================
// Footer
// ============================================================================
//
// Production Note:
// If you truly need dynamic loading (compiling a crate to a `.so` and loading it
// at runtime without recompiling the main app), use the `libloading` crate.
// However, because Rust has no stable ABI, both the host app and the plugin MUST
// be compiled with the exact same rustc version and compiler flags, and the interface
// must be strictly C-compatible (`#[repr(C)]` structs and `extern "C"` functions).
