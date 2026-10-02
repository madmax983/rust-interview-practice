//! # Proc-macro shims for `fundamentals::proc_macros`
//!
//! A `proc-macro = true` crate can export only macros, and `proc_macro::TokenStream` only
//! works inside the compiler. So every macro here is a one-liner: convert
//! `proc_macro::TokenStream` into `proc_macro2::TokenStream` with `.into()`, call the real
//! implementation in `rust_interview_practice::fundamentals::proc_macros` (which is unit
//! tested there), and convert back.
//!
//! The doctests below show each macro in use, and each `compile_fail` doctest pins down an
//! error path: if a macro ever starts accepting that input, `cargo test` fails.
//!
//! ```
//! use rust_interview_practice::fundamentals::proc_macros::Describe;
//! use rust_interview_practice_macros::{Builder, Describe};
//!
//! #[derive(Builder, Describe)]
//! struct Config {
//!     name: String,
//!     #[builder(each = "tag")]
//!     tags: Vec<String>,
//! }
//!
//! let config = Config::builder().name("demo").tag("a").tag("b").build().unwrap();
//! assert_eq!(config.tags, ["a", "b"]);
//! assert_eq!(Config::MEMBERS, ["name", "tags"]);
//! ```

use proc_macro::TokenStream;
use rust_interview_practice::fundamentals::proc_macros as imp;

/// Implements `Describe` (type name + field/variant names).
///
/// Helper attributes: `#[describe(skip)]` and `#[describe(rename = "...")]`.
///
/// ```compile_fail
/// #[derive(rust_interview_practice_macros::Describe)]
/// union Bits { int: u32, float: f32 } // unions are rejected
/// ```
///
/// ```compile_fail
/// #[derive(rust_interview_practice_macros::Describe)]
/// struct S { #[describe(hide)] a: u8 } // unknown helper key
/// ```
#[proc_macro_derive(Describe, attributes(describe))]
pub fn derive_describe(input: TokenStream) -> TokenStream {
    imp::derive_describe(input.into()).into()
}

/// Generates `FooBuilder` with chained setters and `build() -> Result<Foo, BuilderError>`.
///
/// `Option<T>` fields are optional, `#[builder(default)]` falls back to `Default`, and
/// `#[builder(each = "item")]` on a `Vec<T>` adds a one-element-at-a-time setter.
///
/// ```compile_fail
/// #[derive(rust_interview_practice_macros::Builder)]
/// enum Shape { Circle } // only structs with named fields
/// ```
///
/// ```compile_fail
/// #[derive(rust_interview_practice_macros::Builder)]
/// struct S { #[builder(each = "x")] xs: Option<u8> } // `each` needs a Vec
/// ```
#[proc_macro_derive(Builder, attributes(builder))]
pub fn derive_builder(input: TokenStream) -> TokenStream {
    imp::derive_builder(input.into()).into()
}

/// `#[retry(times = N)]`: re-runs a `Result`-returning fn until `Ok` or `N` attempts.
///
/// ```compile_fail
/// #[rust_interview_practice_macros::retry(times = 3)]
/// fn not_a_result() -> u8 { 1 }
/// ```
///
/// ```compile_fail
/// #[rust_interview_practice_macros::retry] // missing `times = N`
/// fn f() -> Result<(), ()> { Ok(()) }
/// ```
#[proc_macro_attribute]
pub fn retry(args: TokenStream, item: TokenStream) -> TokenStream {
    imp::retry(args.into(), item.into()).into()
}

/// `#[memoize]`: caches a one-argument fn in a per-thread `HashMap`.
///
/// ```compile_fail
/// #[rust_interview_practice_macros::memoize]
/// fn generic<T: Clone>(t: T) -> T { t } // one cache per fn, so no generics
/// ```
///
/// ```compile_fail
/// // `f32` is not `Hash + Eq`; the error points at `f32`, thanks to `quote_spanned!`.
/// #[rust_interview_practice_macros::memoize]
/// fn half(x: f32) -> f32 { x / 2.0 }
/// ```
///
/// ```compile_fail
/// // The cached value is cloned out of the cache; this error points at `Mutex<u8>`.
/// #[rust_interview_practice_macros::memoize]
/// fn lock(x: u8) -> std::sync::Mutex<u8> { std::sync::Mutex::new(x) }
/// ```
#[proc_macro_attribute]
pub fn memoize(args: TokenStream, item: TokenStream) -> TokenStream {
    imp::memoize(args.into(), item.into()).into()
}

/// `#[checked]`: rewrites `+ - *` into checked arithmetic that returns `None` on overflow.
///
/// ```
/// #[rust_interview_practice_macros::checked]
/// fn area(w: u8, h: u8) -> Option<u8> {
///     Some(w * h)
/// }
/// assert_eq!(area(10, 20), Some(200));
/// assert_eq!(area(16, 16), None);
/// ```
///
/// ```compile_fail
/// #[rust_interview_practice_macros::checked]
/// fn f(a: u8) -> u8 { a + 1 } // must return Option
/// ```
///
/// ```compile_fail
/// #[rust_interview_practice_macros::checked]
/// fn f(a: u8) -> Option<u8> { Some(10 - a) } // `10.checked_sub` would be ambiguous
/// ```
#[proc_macro_attribute]
pub fn checked(args: TokenStream, item: TokenStream) -> TokenStream {
    imp::checked(args.into(), item.into()).into()
}

/// `seq!(N in 0..4 { ... })`: repeats the body, replacing `N` and pasting `name~N`.
///
/// ```
/// rust_interview_practice_macros::seq!(N in 1..=3 {
///     const C~N: u32 = N * 100;
/// });
/// assert_eq!((C1, C2, C3), (100, 200, 300));
/// ```
///
/// ```compile_fail
/// rust_interview_practice_macros::seq!(N in 3..1 {}); // start after end
/// ```
#[proc_macro]
pub fn seq(input: TokenStream) -> TokenStream {
    imp::seq(input.into()).into()
}
