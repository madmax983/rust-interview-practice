// Suppress pedantic and nursery lints for design pattern examples.
#![allow(clippy::pedantic, clippy::nursery, unused)]
#![allow(clippy::empty_line_after_doc_comments)]
//! # Macro & Metaprogramming Patterns
//!
//! Replaces: **C Preprocessor Macros** (C/C++), **Reflection & Annotations** (Java), **Decorators** (Python)
//!
//! Real Rust usage: `vec![]`, `println!`, `serde::Deserialize` (Derive), `tokio::main` (Attribute)
//!
//! ## Why these patterns exist in Rust
//! Rust uses macros for compile-time metaprogramming. This allows generating boilerplate
//! while preserving type safety and zero runtime cost.
//! - **Declarative Macros (`macro_rules!`):** Match on syntax trees (ASTs) and expand them.
//!   They are hygienic (they don't leak variables into the caller's scope).
//! - **Procedural Macros:** Rust functions that take a TokenStream and return a TokenStream.
//!   Requires a separate crate (`proc-macro = true`).
//! - **Conditional Compilation (`cfg`):** Include/exclude code based on OS, architecture, or
//!   Cargo features.
//!
//! ## Architecture
//!
//! ```text
//! [ Source Code ] --> [ macro_rules! expansion ] --> [ AST ] --> [ Compiler ]
//! ```

// ============================================================================
// Pattern 1: Declarative Macros (The Push-Down Accumulator & TT Muncher)
// ============================================================================

/// A macro to create a `HashMap` easily, similar to `vec![]`.
///
/// **COMPILE-TIME WIN:** This expands at compile time into standard `.insert()` calls,
/// allowing for cleaner syntax without any runtime parsing overhead.
#[macro_export]
macro_rules! hash_map {
    // Base case: empty map
    () => {
        std::collections::HashMap::new()
    };

    // Match a list of key-value pairs separated by commas
    // The `$( ... ),*` syntax repeats the pattern inside, separated by commas.
    // The `$( ... ),+` requires at least one match.
    ( $( $key:expr => $val:expr ),* $(,)? ) => {
        {
            let mut map = std::collections::HashMap::new();
            $(
                map.insert($key, $val);
            )*
            map
        }
    };
}

// ============================================================================
// Pattern 2: The `cfg` and Feature Flag Pattern
// ============================================================================

/// Replaces: `#ifdef` blocks in C/C++.
///
/// **OWNERSHIP INSIGHT:** Because `cfg` operates before type-checking, you can have
/// entirely different struct definitions or trait implementations based on features,
/// without polluting the final binary.
pub struct PlatformConfig {
    #[cfg(target_os = "windows")]
    pub windows_registry_key: String,

    #[cfg(not(target_os = "windows"))]
    pub posix_socket_path: String,
}

impl PlatformConfig {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        #[cfg(target_os = "windows")]
        {
            Self {
                windows_registry_key: r"HKLM\Software\MyApp".to_string(),
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            Self {
                posix_socket_path: "/tmp/myapp.sock".to_string(),
            }
        }
    }

    pub fn get_path(&self) -> &str {
        #[cfg(target_os = "windows")]
        return &self.windows_registry_key;

        #[cfg(not(target_os = "windows"))]
        return &self.posix_socket_path;
    }
}

// ============================================================================
// Pattern 3: Derive Macro Pattern (Conceptual)
// ============================================================================

/// In a real project, to build a derive macro like `#[derive(Describe)]`, you must
/// create a separate crate with `proc-macro = true` in its `Cargo.toml`.
///
/// The code would look like this (using `syn` and `quote` crates):
///
/// ```rust,ignore
/// #[proc_macro_derive(Describe)]
/// pub fn describe_derive(input: TokenStream) -> TokenStream {
///     // 1. Parse the input tokens into a syntax tree
///     let ast: DeriveInput = syn::parse(input).unwrap();
///     let name = &ast.ident;
///
///     // 2. Build the output using `quote!`
///     let expanded = quote! {
///         impl Describe for #name {
///             fn describe(&self) -> String {
///                 stringify!(#name).to_string()
///             }
///         }
///     };
///
///     // 3. Return the generated tokens
///     TokenStream::from(expanded)
/// }
/// ```

pub trait Describe {
    fn describe(&self) -> String;
}

// Simulated expansion of `#[derive(Describe)]` for a struct:
pub struct MyStruct {
    pub value: i32,
}

impl Describe for MyStruct {
    fn describe(&self) -> String {
        "MyStruct".to_string()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_map_macro() {
        // Empty map
        let empty: std::collections::HashMap<i32, i32> = hash_map!();
        assert!(empty.is_empty());

        // Populated map
        let map = hash_map! {
            "alice" => 10,
            "bob" => 20, // Trailing comma is allowed by our macro definition
        };

        assert_eq!(map.len(), 2);
        assert_eq!(map.get("alice"), Some(&10));
        assert_eq!(map.get("bob"), Some(&20));
    }

    #[test]
    fn test_cfg_pattern() {
        let config = PlatformConfig::new();
        // We just verify it compiles and runs without panicking on the current OS
        let path = config.get_path();
        assert!(!path.is_empty());
    }

    #[test]
    fn test_describe_trait() {
        let s = MyStruct { value: 42 };
        assert_eq!(s.describe(), "MyStruct");
    }
}
