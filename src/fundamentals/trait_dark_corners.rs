//! # Trait Dark Corners
//!
//! Drills for the parts of the trait system that interviewers love to probe and
//! that autocomplete can't rescue you from:
//!
//! 1. [`gats`] — generic associated types (`type Item<'a> where Self: 'a`)
//! 2. [`hrtb`] — higher-ranked trait bounds (`for<'a> Fn(&'a str) -> &'a str`)
//! 3. [`coherence`] — the orphan rule, overlapping impls, `#[fundamental]` types
//! 4. [`dyn_compat`] — what makes a trait usable as `dyn Trait`
//!
//! Every "this does not compile" claim lives in a `compile_fail` doctest, so the
//! compiler keeps these notes honest: if a future toolchain starts accepting one
//! of them, `cargo test` fails and the note gets updated.
//!
//! ## Cheat sheet
//!
//! | Symptom | Corner | Fix |
//! |---------|--------|-----|
//! | "missing required bound on `Item`" | GAT | add `where Self: 'a` |
//! | Iterator can't hand out `&mut` into itself | GAT | lending iterator |
//! | GAT + `F: FnMut(Self::Item<'_>)` demands `'static` | GAT×HRTB | drive with `while let` |
//! | "`local` does not live long enough" in a callback | HRTB | `F: for<'a> Fn(&'a T)` |
//! | Closure `\|s: &str\| s` fails to infer | HRTB | pass through an `fn` with an HRTB bound |
//! | "implementation is not general enough" | HRTB | need `for<'de> Decode<'de>` (owned) |
//! | E0117 only traits defined in the current crate... | orphan rule | newtype wrapper |
//! | E0210 type parameter must be covered | orphan rule | put a local type first |
//! | E0119 upstream crates may add a new impl | overlap | opt-in marker trait |
//! | E0038 the trait is not dyn compatible | dyn | `where Self: Sized` / erase generics |
//! | `Box<dyn Trait>` won't hold a borrow | dyn | `Box<dyn Trait + 'a>` |

// ============================================================================
// 1. GENERIC ASSOCIATED TYPES
// ============================================================================

/// Generic associated types: associated types that take their own generic
/// parameters (lifetimes or types).
///
/// The classic motivation is the **lending iterator**: `std::iter::Iterator`
/// fixes `Item` once per iterator, so it can never yield a reference that
/// borrows from the iterator itself. That makes overlapping mutable windows
/// impossible:
///
/// ```compile_fail
/// struct WindowsMut<'s> {
///     slice: &'s mut [i32],
///     start: usize,
/// }
///
/// impl<'s> Iterator for WindowsMut<'s> {
///     type Item = &'s mut [i32];
///
///     fn next(&mut self) -> Option<Self::Item> {
///         let start = self.start;
///         self.start += 1;
///         // error: lifetime may not live long enough — the window can only
///         // borrow for as long as `&mut self`, not for all of `'s`.
///         self.slice.get_mut(start..start + 2)
///     }
/// }
/// ```
///
/// A GAT declared without `where Self: 'a` is rejected — the compiler insists
/// on the bound because every use of `Item<'a>` in the trait implies it:
///
/// ```compile_fail
/// trait LendingIterator {
///     type Item<'a>; // error: missing required bound on `Item`
///     fn next(&mut self) -> Option<Self::Item<'_>>;
/// }
/// ```
///
/// GATs + HRTBs collide: a closure bound over *every* `Item<'a>` (e.g. a
/// provided `for_each` adapter) can't see the `Self: 'a` clause, so the
/// compiler conservatively demands `Self: 'static` — and any iterator that
/// borrows a non-`'static` slice is rejected at the call site. This is why
/// [`gats::LendingIterator`] has no `for_each`; drive it with `while let`.
///
/// ```compile_fail,E0521
/// trait LendingIterator {
///     type Item<'a>
///     where
///         Self: 'a;
///     fn next(&mut self) -> Option<Self::Item<'_>>;
///     fn for_each<F>(mut self, mut f: F)
///     where
///         Self: Sized,
///         F: FnMut(Self::Item<'_>), // implies `Self: 'static`
///     {
///         while let Some(item) = self.next() {
///             f(item);
///         }
///     }
/// }
///
/// struct Once<'s>(Option<&'s mut i32>);
/// impl<'s> LendingIterator for Once<'s> {
///     type Item<'a>
///         = &'a mut i32
///     where
///         Self: 'a;
///     fn next(&mut self) -> Option<Self::Item<'_>> {
///         self.0.as_deref_mut()
///     }
/// }
///
/// fn bump(x: &mut i32) {
///     // error[E0521]: borrowed data escapes outside of function
///     Once(Some(x)).for_each(|v| *v += 1);
/// }
/// ```
pub mod gats {
    use std::ops::Deref;
    use std::rc::Rc;
    use std::sync::Arc;

    // ------------------------------------------------------------------------
    // Lifetime GAT: the lending iterator
    // ------------------------------------------------------------------------

    /// An iterator whose items may borrow from the iterator itself.
    ///
    /// `Item<'a>` is tied to the `&'a mut self` borrow in `next`, so each item
    /// must be dropped before `next` can be called again.
    pub trait LendingIterator {
        /// The item type, parameterized by the borrow of `self`.
        type Item<'a>
        where
            Self: 'a;

        /// Advance and lend out the next item.
        fn next(&mut self) -> Option<Self::Item<'_>>;
    }

    /// Overlapping mutable windows over a slice — impossible with `Iterator`.
    #[derive(Debug)]
    pub struct WindowsMut<'s, T> {
        slice: &'s mut [T],
        start: usize,
        size: usize,
    }

    impl<'s, T> WindowsMut<'s, T> {
        /// Windows of `size` elements. A `size` of zero yields nothing.
        pub const fn new(slice: &'s mut [T], size: usize) -> Self {
            Self {
                slice,
                start: 0,
                size,
            }
        }
    }

    impl<T> LendingIterator for WindowsMut<'_, T> {
        type Item<'a>
            = &'a mut [T]
        where
            Self: 'a;

        fn next(&mut self) -> Option<Self::Item<'_>> {
            if self.size == 0 {
                return None;
            }
            let start = self.start;
            let end = start.checked_add(self.size)?;
            self.start += 1;
            self.slice.get_mut(start..end)
        }
    }

    /// Running prefix sums computed in place by walking overlapping `[a, b]`
    /// windows and doing `b += a`.
    ///
    /// Time: O(n) — one pass over the windows. Space: O(1).
    pub fn prefix_sums_in_place(values: &mut [i64]) {
        // No `for` sugar for lending iterators: drive them with `while let`.
        let mut windows = WindowsMut::new(values, 2);
        while let Some(w) = windows.next() {
            w[1] += w[0];
        }
    }

    // ------------------------------------------------------------------------
    // Type GAT: abstracting over a *family* of pointer types
    // ------------------------------------------------------------------------

    /// A family of smart pointers (`Rc`, `Arc`, ...), chosen once and then
    /// applied to any `T`. Without GATs you'd need a separate trait per `T`.
    pub trait PointerFamily {
        /// The pointer type constructor, e.g. `Rc<T>`.
        type Pointer<T>: Deref<Target = T> + Clone;

        /// Allocate `value` behind this family's pointer.
        fn new<T>(value: T) -> Self::Pointer<T>;
    }

    /// Single-threaded family: `Rc<T>`.
    #[derive(Debug, Clone, Copy)]
    pub struct RcFamily;

    /// Thread-safe family: `Arc<T>`.
    #[derive(Debug, Clone, Copy)]
    pub struct ArcFamily;

    impl PointerFamily for RcFamily {
        type Pointer<T> = Rc<T>;

        fn new<T>(value: T) -> Rc<T> {
            Rc::new(value)
        }
    }

    impl PointerFamily for ArcFamily {
        type Pointer<T> = Arc<T>;

        fn new<T>(value: T) -> Arc<T> {
            Arc::new(value)
        }
    }

    struct Node<T, P: PointerFamily> {
        value: T,
        next: Link<T, P>,
    }

    type Link<T, P> = Option<<P as PointerFamily>::Pointer<Node<T, P>>>;

    /// Persistent (immutable, structurally shared) stack, generic over the
    /// pointer family.
    ///
    /// `PersistentStack<T, ArcFamily>` is `Send + Sync` when
    /// `T` is, and the very same code with `RcFamily` is cheaper but local.
    pub struct PersistentStack<T, P: PointerFamily> {
        head: Link<T, P>,
        len: usize,
    }

    // Manual impl: `#[derive(Clone)]` would demand `T: Clone` and `P: Clone`,
    // but cloning only bumps a refcount.
    impl<T, P: PointerFamily> Clone for PersistentStack<T, P> {
        fn clone(&self) -> Self {
            Self {
                head: self.head.clone(),
                len: self.len,
            }
        }
    }

    impl<T, P: PointerFamily> Default for PersistentStack<T, P> {
        fn default() -> Self {
            Self::new()
        }
    }

    impl<T, P: PointerFamily> PersistentStack<T, P> {
        /// An empty stack.
        #[must_use]
        pub const fn new() -> Self {
            Self { head: None, len: 0 }
        }

        /// A new stack with `value` on top; `self` is untouched and shares
        /// its whole spine with the result.
        #[must_use]
        pub fn push(&self, value: T) -> Self {
            Self {
                head: Some(P::new(Node {
                    value,
                    next: self.head.clone(),
                })),
                len: self.len + 1,
            }
        }

        /// The stack without its top element (or `None` if empty).
        #[must_use]
        pub fn pop(&self) -> Option<Self> {
            self.head.as_ref().map(|node| Self {
                head: node.next.clone(),
                len: self.len - 1,
            })
        }

        /// The top element, if any.
        #[must_use]
        pub fn peek(&self) -> Option<&T> {
            self.head.as_deref().map(|node| &node.value)
        }

        /// Number of elements.
        #[must_use]
        pub const fn len(&self) -> usize {
            self.len
        }

        /// Whether the stack has no elements.
        #[must_use]
        pub const fn is_empty(&self) -> bool {
            self.len == 0
        }

        /// Iterate from top to bottom.
        pub fn iter(&self) -> impl Iterator<Item = &T> {
            let mut cursor = self.head.as_deref();
            std::iter::from_fn(move || {
                let node = cursor?;
                cursor = node.next.as_deref();
                Some(&node.value)
            })
        }
    }

    // ------------------------------------------------------------------------
    // Lifetime GAT on a collection trait: borrowing iterators
    // ------------------------------------------------------------------------

    /// A container that can hand out a borrowing iterator whose type depends on
    /// the borrow — the shape of `IntoIterator for &'a C`, but as one trait.
    pub trait Container {
        /// Element type.
        type Elem;

        /// Borrowing iterator type for a borrow of length `'a`.
        type Iter<'a>: Iterator<Item = &'a Self::Elem>
        where
            Self: 'a;

        /// Iterate by reference.
        fn elems(&self) -> Self::Iter<'_>;
    }

    impl<T> Container for Vec<T> {
        type Elem = T;
        type Iter<'a>
            = std::slice::Iter<'a, T>
        where
            Self: 'a;

        fn elems(&self) -> Self::Iter<'_> {
            self.iter()
        }
    }

    impl<K: Ord, V> Container for std::collections::BTreeMap<K, V> {
        type Elem = V;
        type Iter<'a>
            = std::collections::btree_map::Values<'a, K, V>
        where
            Self: 'a;

        fn elems(&self) -> Self::Iter<'_> {
            self.values()
        }
    }

    /// Largest element of any [`Container`], generic over the GAT.
    #[must_use]
    pub fn max_elem<C>(container: &C) -> Option<&C::Elem>
    where
        C: Container,
        C::Elem: Ord,
    {
        container.elems().max()
    }
}

// ============================================================================
// 2. HIGHER-RANKED TRAIT BOUNDS
// ============================================================================

/// Higher-ranked trait bounds: `for<'a> Bound<'a>` means "for **every**
/// lifetime `'a`", rather than "for one lifetime the caller picks".
///
/// That distinction matters whenever the callee borrows something the caller
/// can't name — typically a local:
///
/// ```compile_fail,E0597
/// fn apply_to_local<'a, F>(f: F) -> String
/// where
///     F: Fn(&'a str) -> &'a str, // `'a` is chosen by the *caller*...
/// {
///     let local = String::from("  hi  ");
///     f(&local).to_owned() // ...so a local can't satisfy it: E0597
/// }
/// ```
///
/// A closure annotated `|s: &str|` gets a higher-ranked *argument* lifetime,
/// but its return lifetime is inferred as one fixed region — so returning a
/// borrow of the argument fails unless an HRTB bound drives inference:
///
/// ```compile_fail
/// let first_word = |s: &str| s.split(' ').next().unwrap_or("");
/// // error: lifetime may not live long enough
/// let _ = first_word("hello world");
/// ```
///
/// Owned-only decoding (the `serde::de::DeserializeOwned` shape) rejects
/// borrowing types, because `&'de str` only implements `Decode<'de>` for one
/// specific `'de`:
///
/// ```compile_fail
/// use rust_interview_practice::fundamentals::trait_dark_corners::hrtb::decode_owned;
/// // error: implementation of `Decode` is not general enough
/// let _ = decode_owned::<&str>(b"borrowed");
/// ```
pub mod hrtb {
    // ------------------------------------------------------------------------
    // HRTB on closures
    // ------------------------------------------------------------------------

    /// Run `f` on a string the callee owns. Needs `for<'a>` because the borrow
    /// is of a local the caller cannot name.
    ///
    /// Note: the elided form `F: Fn(&str) -> &str` desugars to exactly this
    /// bound — elision in `Fn` sugar *is* an HRTB.
    pub fn apply_to_local<F>(f: F) -> String
    where
        F: for<'a> Fn(&'a str) -> &'a str,
    {
        let local = String::from("  hello, world  ");
        f(&local).to_owned()
    }

    /// Identity function that forces a closure to be inferred with a
    /// higher-ranked signature.
    ///
    /// The stable workaround for closures that
    /// return a borrow of their argument (`for<'a> |s: &'a str|` closure
    /// binders are still unstable).
    pub const fn hrtb_str_closure<F>(f: F) -> F
    where
        F: for<'a> Fn(&'a str) -> &'a str,
    {
        f
    }

    /// The first space-separated word, via a closure made higher-ranked by
    /// [`hrtb_str_closure`].
    #[must_use]
    pub fn first_word(text: &str) -> &str {
        let first = hrtb_str_closure(|s| s.split(' ').next().unwrap_or(""));
        first(text)
    }

    // ------------------------------------------------------------------------
    // HRTB on function pointers
    // ------------------------------------------------------------------------

    /// A higher-ranked function pointer type. `str::trim` and friends coerce
    /// to it directly.
    pub type StrTransform = for<'a> fn(&'a str) -> &'a str;

    /// Zero-allocation trimming pipeline built from method paths.
    pub const TRIM_PIPELINE: [StrTransform; 3] = [str::trim, strip_quotes, strip_trailing_dots];

    fn strip_quotes(s: &str) -> &str {
        s.strip_prefix('"')
            .and_then(|s| s.strip_suffix('"'))
            .unwrap_or(s)
    }

    fn strip_trailing_dots(s: &str) -> &str {
        s.trim_end_matches('.')
    }

    /// Apply each transform in turn. All results borrow from `input`.
    #[must_use]
    pub fn run_pipeline<'a>(input: &'a str, steps: &[StrTransform]) -> &'a str {
        steps.iter().fold(input, |acc, step| step(acc))
    }

    // ------------------------------------------------------------------------
    // HRTB on non-Fn traits: "every borrow of C is iterable"
    // ------------------------------------------------------------------------

    /// `(count, sum)` over any collection whose *references* iterate `&u64`:
    /// `Vec`, arrays, `VecDeque`, `HashSet`, `BTreeSet`, ...
    ///
    /// The collection is taken by value and borrowed twice inside the
    /// function, so no caller-chosen lifetime could describe those borrows.
    #[must_use]
    pub fn count_and_sum<C>(collection: C) -> (usize, u64)
    where
        for<'a> &'a C: IntoIterator<Item = &'a u64>,
    {
        let count = (&collection).into_iter().count();
        let sum = (&collection).into_iter().sum();
        (count, sum)
    }

    // ------------------------------------------------------------------------
    // HRTB as a supertrait: the `DeserializeOwned` pattern
    // ------------------------------------------------------------------------

    /// Decode a value that may borrow from the input for `'de`.
    pub trait Decode<'de>: Sized {
        /// Parse `input`, returning `None` on malformed data.
        fn decode(input: &'de str) -> Option<Self>;
    }

    /// Zero-copy: borrows straight out of the input.
    impl<'de> Decode<'de> for &'de str {
        fn decode(input: &'de str) -> Option<Self> {
            Some(input.trim())
        }
    }

    impl Decode<'_> for String {
        fn decode(input: &str) -> Option<Self> {
            Some(input.trim().to_owned())
        }
    }

    impl Decode<'_> for u32 {
        fn decode(input: &str) -> Option<Self> {
            input.trim().parse().ok()
        }
    }

    /// Types decodable from input of **any** lifetime — i.e. that never borrow.
    pub trait DecodeOwned: for<'de> Decode<'de> {}

    impl<T> DecodeOwned for T where T: for<'de> Decode<'de> {}

    /// Zero-copy decode: `T` may borrow from `input`.
    #[must_use]
    pub fn decode_borrowed<'de, T: Decode<'de>>(input: &'de str) -> Option<T> {
        T::decode(input)
    }

    /// Decode from raw bytes via a temporary buffer that dies at the end of
    /// this function — only possible for [`DecodeOwned`] types.
    #[must_use]
    pub fn decode_owned<T: DecodeOwned>(raw: &[u8]) -> Option<T> {
        let buffer = String::from_utf8_lossy(raw).into_owned();
        T::decode(&buffer)
    }
}

// ============================================================================
// 3. COHERENCE AND THE ORPHAN RULE
// ============================================================================

/// Coherence guarantees there is **at most one** impl of a trait for a type
/// across the whole crate graph. Two rules enforce it.
///
/// ## Orphan rule
///
/// `impl ForeignTrait<T1..Tn> for T0` is allowed only if some `Ti` is a
/// local type and no uncovered type parameter appears before it.
///
/// Foreign trait for a foreign type:
///
/// ```compile_fail,E0117
/// impl std::fmt::Display for Vec<i32> {
///     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
///         write!(f, "{}", self.len())
///     }
/// }
/// ```
///
/// An uncovered type parameter *before* the first local type:
///
/// ```compile_fail,E0210
/// struct Local;
/// impl<T> From<Local> for T {
///     fn from(_: Local) -> T {
///         unimplemented!()
///     }
/// }
/// ```
///
/// ## Overlap check
///
/// A blanket impl over a foreign trait bound conflicts with a concrete impl
/// for a foreign type, because the upstream crate might add the bound later
/// (here: std could one day `impl Display for Vec<u8>`):
///
/// ```compile_fail,E0119
/// use std::fmt::Display;
/// trait Describe {
///     fn describe(&self) -> String;
/// }
/// impl<T: Display> Describe for T {
///     fn describe(&self) -> String {
///         self.to_string()
///     }
/// }
/// impl Describe for Vec<u8> {
///     fn describe(&self) -> String {
///         format!("{} bytes", self.len())
///     }
/// }
/// ```
///
/// For a **local** type the compiler may use negative reasoning — it knows
/// nobody else can implement `Display` for it — so the same pair of impls
/// compiles (see [`coherence::Opaque`]). That makes adding `Display` to the
/// local type later a breaking change for yourself.
pub mod coherence {
    use std::fmt::{self, Display};
    use std::ops::Add;

    // ------------------------------------------------------------------------
    // Newtype: the universal orphan-rule escape hatch
    // ------------------------------------------------------------------------

    /// Local wrapper so we may implement the foreign `Display` for what is
    /// really a foreign `Vec<String>`.
    #[derive(Debug, Clone, PartialEq, Eq, Default)]
    pub struct CommaList(pub Vec<String>);

    impl Display for CommaList {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            for (i, item) in self.0.iter().enumerate() {
                if i > 0 {
                    f.write_str(", ")?;
                }
                f.write_str(item)?;
            }
            Ok(())
        }
    }

    // ------------------------------------------------------------------------
    // Allowed orphan shapes that look like they shouldn't be
    // ------------------------------------------------------------------------

    /// A local unit type. Deliberately not `Copy`, so `&a + &b` (borrowing)
    /// and `a + b` (consuming) are genuinely different operations.
    #[derive(Debug, Clone, PartialEq, PartialOrd, Default)]
    pub struct Meters(pub f64);

    /// `impl From<Local> for Foreign` is fine: `T0 = f64` is foreign but
    /// `T1 = Meters` is local and nothing uncovered precedes it.
    impl From<Meters> for f64 {
        fn from(m: Meters) -> Self {
            m.0
        }
    }

    /// `&Meters` counts as local because `&` is `#[fundamental]` (as are
    /// `&mut` and `Box`), so we may implement the foreign `Add` on it.
    impl Add for &Meters {
        type Output = Meters;

        fn add(self, rhs: Self) -> Meters {
            Meters(self.0 + rhs.0)
        }
    }

    impl Add for Meters {
        type Output = Self;

        fn add(self, rhs: Self) -> Self {
            Self(self.0 + rhs.0)
        }
    }

    /// A local generic container.
    #[derive(Debug, Clone, PartialEq, Eq, Default)]
    pub struct Bag<T>(pub Vec<T>);

    /// `impl<T> From<Local<T>> for Vec<T>`: `T` is *covered* by `Vec`, and the
    /// local `Bag<T>` follows — allowed, even though it targets a std type.
    impl<T> From<Bag<T>> for Vec<T> {
        fn from(bag: Bag<T>) -> Self {
            bag.0
        }
    }

    // ------------------------------------------------------------------------
    // Overlap: blanket impls and how to keep specific impls alongside them
    // ------------------------------------------------------------------------

    /// Local trait with a blanket impl for every `Display` type.
    pub trait Describe {
        /// Human-readable description.
        fn describe(&self) -> String;
    }

    impl<T: Display> Describe for T {
        fn describe(&self) -> String {
            format!("<{self}>")
        }
    }

    /// Local type without `Display`. Negative reasoning lets this specific
    /// impl coexist with the blanket one above.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Opaque;

    impl Describe for Opaque {
        fn describe(&self) -> String {
            String::from("<opaque>")
        }
    }

    /// Workaround for the E0119 case: blanket-impl over a **local** opt-in
    /// marker instead of the foreign `Display`. Only this crate can implement
    /// `LabelViaDisplay` for `Vec<u8>`, so overlap is decidable.
    pub trait Label {
        /// Short label for logs.
        fn label(&self) -> String;
    }

    /// Opt-in marker: "label me using my `Display` impl".
    pub trait LabelViaDisplay: Display {}

    impl<T: LabelViaDisplay> Label for T {
        fn label(&self) -> String {
            format!("[{self}]")
        }
    }

    impl LabelViaDisplay for i32 {}
    impl LabelViaDisplay for String {}

    impl Label for Vec<u8> {
        fn label(&self) -> String {
            format!("[{} bytes]", self.len())
        }
    }

    // ------------------------------------------------------------------------
    // Generic trait parameters: many impls per type are not overlap
    // ------------------------------------------------------------------------

    /// Unit conversion keyed by the *target* unit; a type parameter (unlike an
    /// associated type) allows one impl per target.
    pub trait ConvertTo<Unit> {
        /// Convert `self` into `Unit`.
        fn convert(&self) -> Unit;
    }

    /// Feet, for conversions.
    #[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Default)]
    pub struct Feet(pub f64);

    /// Centimeters, for conversions.
    #[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Default)]
    pub struct Centimeters(pub f64);

    impl ConvertTo<Feet> for Meters {
        fn convert(&self) -> Feet {
            Feet(self.0 / 0.3048)
        }
    }

    impl ConvertTo<Centimeters> for Meters {
        fn convert(&self) -> Centimeters {
            Centimeters(self.0 * 100.0)
        }
    }
}

// ============================================================================
// 4. DYN COMPATIBILITY (formerly "object safety")
// ============================================================================

/// A trait is **dyn compatible** when every method can go in a vtable.
/// Each of these breaks that (E0038) unless the item is opted out with
/// `where Self: Sized`:
///
/// - generic methods (a vtable can't hold infinitely many monomorphizations)
/// - `Self` by value in arguments or return (size unknown behind `dyn`)
/// - associated consts and GATs
/// - `-> impl Trait` / `async fn` in the trait (opaque return type per impl)
/// - a `Sized` supertrait
///
/// ```compile_fail,E0038
/// trait Visitor {
///     fn visit<T: std::fmt::Debug>(&self, value: T);
/// }
/// fn take(_: &dyn Visitor) {}
/// ```
///
/// ```compile_fail,E0038
/// trait Duplicate {
///     fn duplicate(&self) -> Self; // like `Clone::clone`
/// }
/// fn take(_: Box<dyn Duplicate>) {}
/// ```
///
/// ```compile_fail,E0038
/// trait Numbers {
///     fn numbers(&self) -> impl Iterator<Item = u32>;
/// }
/// fn take(_: &dyn Numbers) {}
/// ```
///
/// ```compile_fail,E0038
/// trait Versioned {
///     const VERSION: u32;
/// }
/// fn take(_: &dyn Versioned) {}
/// ```
///
/// `Box<dyn Trait>` silently means `Box<dyn Trait + 'static>`, so it can't
/// hold a borrow:
///
/// ```compile_fail
/// fn boxed(s: &str) -> Box<dyn std::fmt::Display> {
///     Box::new(s) // error: lifetime may not live long enough
/// }
/// ```
///
/// With `impl PartialEq for dyn Shape`, comparing two boxes through
/// `assert_eq!` tries to move out of them (rust-lang/rust#31740):
///
/// ```compile_fail,E0507
/// use rust_interview_practice::fundamentals::trait_dark_corners::dyn_compat::{Shape, Square};
/// let a: Box<dyn Shape> = Box::new(Square { side: 1.0 });
/// let b: Box<dyn Shape> = Box::new(Square { side: 1.0 });
/// assert_eq!(a, b); // E0507: cannot move out of a shared reference
/// ```
///
/// Compare the unsized places instead:
///
/// ```
/// use rust_interview_practice::fundamentals::trait_dark_corners::dyn_compat::{Shape, Square};
/// let a: Box<dyn Shape> = Box::new(Square { side: 1.0 });
/// let b: Box<dyn Shape> = Box::new(Square { side: 1.0 });
/// assert_eq!(*a, *b);
/// assert!(a == b); // plain `==` on the boxes is fine
/// ```
pub mod dyn_compat {
    use std::any::Any;
    use std::f64::consts::PI;
    use std::fmt::{self, Debug, Display};

    // ------------------------------------------------------------------------
    // Clone / PartialEq for trait objects via helper supertraits
    // ------------------------------------------------------------------------

    /// `Clone` returns `Self`, so it can't be a supertrait of a dyn trait.
    /// Instead, add a dyn-compatible helper and blanket-implement it.
    pub trait ShapeClone {
        /// Clone into a fresh box.
        fn clone_box(&self) -> Box<dyn Shape>;
    }

    impl<T: Shape + Clone> ShapeClone for T {
        fn clone_box(&self) -> Box<dyn Shape> {
            Box::new(self.clone())
        }
    }

    /// `PartialEq<Self>` mentions `Self` in argument position, so compare
    /// against `&dyn Shape` and downcast.
    pub trait ShapeEq {
        /// Equal iff same concrete type and equal values.
        fn dyn_eq(&self, other: &dyn Shape) -> bool;
    }

    impl<T: Shape + PartialEq> ShapeEq for T {
        fn dyn_eq(&self, other: &dyn Shape) -> bool {
            // Trait upcasting (`&dyn Shape` -> `&dyn Any`) — no `as_any` needed.
            let other: &dyn Any = other;
            other.downcast_ref::<Self>().is_some_and(|o| self == o)
        }
    }

    /// A dyn-compatible trait that still offers generic and `Self`-returning
    /// conveniences, opted out of the vtable with `where Self: Sized`.
    pub trait Shape: ShapeClone + ShapeEq + Any + Debug {
        /// Area of the shape.
        fn area(&self) -> f64;

        /// Static name of the concrete shape.
        fn name(&self) -> &'static str;

        /// Constructor-style method: fine in a dyn trait once `Self: Sized`.
        fn unit() -> Self
        where
            Self: Sized;

        /// Generic method: excluded from the vtable, callable on concrete types.
        fn write_to<W: fmt::Write>(&self, out: &mut W) -> fmt::Result
        where
            Self: Sized,
        {
            self.write_dyn(out)
        }

        /// The dyn-callable twin of [`Shape::write_to`]: erase the generic
        /// behind `&mut dyn Write`.
        fn write_dyn(&self, out: &mut dyn fmt::Write) -> fmt::Result {
            write!(out, "{}({:.2})", self.name(), self.area())
        }
    }

    /// A circle.
    #[derive(Debug, Clone, Copy, PartialEq)]
    pub struct Circle {
        /// Radius.
        pub radius: f64,
    }

    /// A square.
    #[derive(Debug, Clone, Copy, PartialEq)]
    pub struct Square {
        /// Side length.
        pub side: f64,
    }

    impl Shape for Circle {
        fn area(&self) -> f64 {
            PI * self.radius * self.radius
        }

        fn name(&self) -> &'static str {
            "circle"
        }

        fn unit() -> Self {
            Self { radius: 1.0 }
        }
    }

    impl Shape for Square {
        fn area(&self) -> f64 {
            self.side * self.side
        }

        fn name(&self) -> &'static str {
            "square"
        }

        fn unit() -> Self {
            Self { side: 1.0 }
        }
    }

    // Recursion trap: `self.clone_box()` here would autoref to
    // `&Box<dyn Shape>`; `(**self)` makes the dyn dispatch explicit.
    impl Clone for Box<dyn Shape> {
        fn clone(&self) -> Self {
            (**self).clone_box()
        }
    }

    // Implemented on the unsized `dyn Shape`; std then provides
    // `PartialEq for Box<dyn Shape>` and `Vec<Box<dyn Shape>>`.
    // Gotcha: `assert_eq!(box_a, box_b)` fails with E0507 (rust-lang/rust#31740);
    // write `assert_eq!(*box_a, *box_b)` to compare the `dyn Shape` places.
    impl PartialEq for dyn Shape {
        fn eq(&self, other: &Self) -> bool {
            self.dyn_eq(other)
        }
    }

    // ------------------------------------------------------------------------
    // Inherent methods on a trait object type
    // ------------------------------------------------------------------------

    /// `impl dyn Trait { .. }` adds inherent methods to the trait object
    /// itself — exactly how `dyn Any::is` and `downcast_ref` are defined.
    impl dyn Shape {
        /// Is the erased value a `T`?
        #[must_use]
        pub fn is<T: Shape>(&self) -> bool {
            let any: &dyn Any = self;
            any.is::<T>()
        }

        /// Recover the concrete type.
        #[must_use]
        pub fn downcast_ref<T: Shape>(&self) -> Option<&T> {
            let any: &dyn Any = self;
            any.downcast_ref::<T>()
        }
    }

    /// Sum of areas through dynamic dispatch.
    #[must_use]
    pub fn total_area(shapes: &[Box<dyn Shape>]) -> f64 {
        shapes.iter().map(|s| s.area()).sum()
    }

    /// Render every shape through the dyn-callable `write_dyn`.
    ///
    /// # Errors
    ///
    /// Propagates any `fmt::Error` from the writer (never for `String`).
    pub fn render_all(shapes: &[Box<dyn Shape>]) -> Result<String, fmt::Error> {
        let mut out = String::new();
        for (i, shape) in shapes.iter().enumerate() {
            if i > 0 {
                out.push_str(", ");
            }
            shape.write_dyn(&mut out)?;
        }
        Ok(out)
    }

    /// Count shapes of concrete type `T` using the inherent `dyn Shape::is`.
    #[must_use]
    pub fn count_of<T: Shape>(shapes: &[Box<dyn Shape>]) -> usize {
        shapes.iter().filter(|s| s.is::<T>()).count()
    }

    // ------------------------------------------------------------------------
    // Upcasting, auto traits, and default object lifetimes
    // ------------------------------------------------------------------------

    /// Trait upcasting coercion: `&dyn Shape` to its supertrait `&dyn Debug`.
    #[must_use]
    pub fn as_debug(shape: &dyn Shape) -> &dyn Debug {
        shape
    }

    /// Auto traits (`Send`, `Sync`) may be dropped from a trait object, never
    /// added.
    #[must_use]
    pub fn forget_send(shape: Box<dyn Shape + Send>) -> Box<dyn Shape> {
        shape
    }

    /// The explicit `+ 'a` lets the box hold a borrow; without it the default
    /// object lifetime for `Box<dyn ..>` is `'static`.
    #[must_use]
    pub fn boxed_display<'a>(s: &'a str) -> Box<dyn Display + 'a> {
        Box::new(s)
    }

    /// `&'a dyn Trait` defaults to `&'a (dyn Trait + 'a)`, so this needs no
    /// annotation at all. (`T: Sized` is implicit: `&str` can't become
    /// `&dyn Display` because `str` has no size to put in a vtable.)
    #[must_use]
    pub fn borrowed_display<T: Display>(value: &T) -> &dyn Display {
        value
    }

    /// The `type_id` footgun: on a `Box<dyn Any>`, method resolution finds
    /// `Box<dyn Any>: Any` first and reports the **box's** type. Deref first.
    /// Returns `(id via the box, id via the contents)`.
    #[must_use]
    #[allow(clippy::type_id_on_box)] // the footgun is the point of this drill
    pub fn type_ids_of_boxed_any(boxed: &Box<dyn Any>) -> (std::any::TypeId, std::any::TypeId) {
        (boxed.type_id(), (**boxed).type_id())
    }
}

#[cfg(test)]
mod tests {
    use super::coherence::{
        Bag, Centimeters, CommaList, ConvertTo, Describe, Feet, Label, Meters, Opaque,
    };
    use super::dyn_compat::{
        Circle, Shape, Square, as_debug, borrowed_display, boxed_display, count_of, forget_send,
        render_all, total_area, type_ids_of_boxed_any,
    };
    use super::gats::{
        ArcFamily, LendingIterator, PersistentStack, RcFamily, WindowsMut, max_elem,
        prefix_sums_in_place,
    };
    use super::hrtb::{
        Decode, StrTransform, TRIM_PIPELINE, apply_to_local, count_and_sum, decode_borrowed,
        decode_owned, first_word, hrtb_str_closure, run_pipeline,
    };
    use std::any::{Any, TypeId};
    use std::collections::{BTreeMap, BTreeSet, HashSet, VecDeque};

    fn approx(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    // ---------------------------------------------------------------- GATs

    #[test]
    fn test_windows_mut_yields_overlapping_windows() {
        let mut data = [1, 2, 3, 4];
        let mut windows = WindowsMut::new(&mut data, 3);
        let mut seen = Vec::new();
        while let Some(w) = windows.next() {
            w[0] *= 10;
            seen.push(w.to_vec());
        }
        assert_eq!(seen, vec![vec![10, 2, 3], vec![20, 3, 4]]);
        assert_eq!(data, [10, 20, 3, 4]);
    }

    #[test]
    fn test_windows_mut_edge_cases() {
        let mut empty: [i32; 0] = [];
        assert!(WindowsMut::new(&mut empty, 1).next().is_none());

        let mut one = [7];
        assert!(WindowsMut::new(&mut one, 0).next().is_none());
        assert!(WindowsMut::new(&mut one, 2).next().is_none());

        let mut exact = [1, 2];
        let mut windows = WindowsMut::new(&mut exact, 2);
        assert_eq!(windows.next().map(|w| w.len()), Some(2));
        assert!(windows.next().is_none());
        assert!(windows.next().is_none(), "stays exhausted");
    }

    #[test]
    fn test_prefix_sums_in_place() {
        let mut v = [1, 2, 3, 4, 5];
        prefix_sums_in_place(&mut v);
        assert_eq!(v, [1, 3, 6, 10, 15]);

        let mut single = [42];
        prefix_sums_in_place(&mut single);
        assert_eq!(single, [42]);

        let mut empty: [i64; 0] = [];
        prefix_sums_in_place(&mut empty);
    }

    #[test]
    fn test_persistent_stack_shares_structure() {
        let empty: PersistentStack<i32, RcFamily> = PersistentStack::new();
        let a = empty.push(1);
        let b = a.push(2);
        let c = a.push(3);

        assert!(empty.is_empty());
        assert_eq!(b.iter().copied().collect::<Vec<_>>(), vec![2, 1]);
        assert_eq!(c.iter().copied().collect::<Vec<_>>(), vec![3, 1]);
        assert_eq!(a.len(), 1);
        assert_eq!(b.peek(), Some(&2));
        assert_eq!(b.pop().map(|s| s.len()), Some(1));
        assert!(empty.pop().is_none());
        assert!(empty.peek().is_none());
    }

    #[test]
    fn test_persistent_stack_arc_family_is_send() {
        let stack: PersistentStack<String, ArcFamily> = PersistentStack::default()
            .push("bottom".to_owned())
            .push("top".to_owned());
        let shared = stack.clone();
        let handle = std::thread::spawn(move || shared.iter().cloned().collect::<Vec<_>>());
        let from_thread = handle.join().expect("thread panicked");
        assert_eq!(from_thread, vec!["top", "bottom"]);
        assert_eq!(stack.len(), 2);
    }

    #[test]
    fn test_container_gat_over_vec_and_btreemap() {
        assert_eq!(max_elem(&vec![3, 9, 2]), Some(&9));
        assert_eq!(max_elem(&Vec::<i32>::new()), None);

        let map: BTreeMap<&str, u8> = [("a", 5), ("b", 1)].into_iter().collect();
        assert_eq!(max_elem(&map), Some(&5));
    }

    // ---------------------------------------------------------------- HRTBs

    #[test]
    fn test_apply_to_local_with_fn_item_and_closure() {
        assert_eq!(apply_to_local(str::trim), "hello, world");
        assert_eq!(
            apply_to_local(|s| s.trim().split(',').next().unwrap_or("")),
            "hello"
        );
    }

    #[test]
    fn test_hrtb_closure_helper() {
        assert_eq!(first_word("hello world"), "hello");
        assert_eq!(first_word(""), "");

        let last = hrtb_str_closure(|s| s.rsplit('/').next().unwrap_or(s));
        let owned = String::from("a/b/c");
        assert_eq!(last(&owned), "c");
    }

    #[test]
    fn test_fn_pointer_pipeline() {
        assert_eq!(run_pipeline("  \"done...\"  ", &TRIM_PIPELINE), "done");
        assert_eq!(run_pipeline("as-is", &[]), "as-is");

        let reversed: Vec<StrTransform> = TRIM_PIPELINE.iter().rev().copied().collect();
        // Order matters: dots are stripped before the quotes are visible.
        assert_eq!(run_pipeline("\"x.\"", &reversed), "x.");
    }

    #[test]
    fn test_count_and_sum_over_many_collections() {
        assert_eq!(count_and_sum(vec![1, 2, 3]), (3, 6));
        assert_eq!(count_and_sum([10_u64; 4]), (4, 40));
        assert_eq!(count_and_sum(VecDeque::from([5, 5])), (2, 10));
        assert_eq!(count_and_sum(HashSet::from([1, 1, 2])), (2, 3));
        assert_eq!(count_and_sum(BTreeSet::<u64>::new()), (0, 0));
    }

    #[test]
    fn test_decode_borrowed_and_owned() {
        let input = String::from("  42 ");
        let borrowed: Option<&str> = decode_borrowed(&input);
        assert_eq!(borrowed, Some("42"));
        assert_eq!(decode_borrowed::<u32>(&input), Some(42));

        assert_eq!(decode_owned::<u32>(b" 7 "), Some(7));
        assert_eq!(decode_owned::<String>(b" hi "), Some("hi".to_owned()));
        assert_eq!(decode_owned::<u32>(b"nope"), None);
        assert_eq!(<u32 as Decode<'_>>::decode("-1"), None);
    }

    // ------------------------------------------------------------ Coherence

    #[test]
    fn test_newtype_display() {
        let list = CommaList(vec!["a".into(), "b".into(), "c".into()]);
        assert_eq!(list.to_string(), "a, b, c");
        assert_eq!(CommaList::default().to_string(), "");
    }

    #[test]
    fn test_allowed_orphan_shapes() {
        let total: f64 = (Meters(1.5) + Meters(2.0)).into();
        assert!(approx(total, 3.5));
        let (a, b) = (Meters(1.0), Meters(0.5));
        let sum = &a + &b; // borrows: `a` and `b` are still usable
        assert!(approx(sum.0, 1.5));
        assert_eq!(a, Meters(1.0));
        assert_eq!(b, Meters(0.5));

        let v: Vec<char> = Bag(vec!['x', 'y']).into();
        assert_eq!(v, vec!['x', 'y']);
    }

    #[test]
    fn test_blanket_and_local_negative_reasoning() {
        assert_eq!(5.describe(), "<5>");
        assert_eq!("hi".describe(), "<hi>");
        assert_eq!(Opaque.describe(), "<opaque>");
    }

    #[test]
    fn test_marker_trait_avoids_overlap() {
        assert_eq!(7.label(), "[7]");
        assert_eq!(String::from("x").label(), "[x]");
        assert_eq!(vec![1_u8, 2, 3].label(), "[3 bytes]");
    }

    #[test]
    fn test_generic_trait_param_allows_multiple_impls() {
        let m = Meters(3.048);
        let feet: Feet = m.convert();
        let cm: Centimeters = m.convert();
        assert!(approx(feet.0, 10.0));
        assert!(approx(cm.0, 304.8));
    }

    // ---------------------------------------------------- Dyn compatibility

    fn sample() -> Vec<Box<dyn Shape>> {
        vec![
            Box::new(Circle { radius: 1.0 }),
            Box::new(Square { side: 2.0 }),
            Box::new(Square::unit()),
        ]
    }

    #[test]
    fn test_dyn_dispatch_and_where_self_sized_methods() {
        let shapes = sample();
        assert!(approx(total_area(&shapes), std::f64::consts::PI + 5.0));
        assert!(approx(total_area(&[]), 0.0));
        assert_eq!(
            render_all(&shapes).as_deref(),
            Ok("circle(3.14), square(4.00), square(1.00)")
        );

        // The generic, `Self: Sized` method still works on concrete types.
        let mut out = String::new();
        Circle::unit()
            .write_to(&mut out)
            .expect("String never fails");
        assert_eq!(out, "circle(3.14)");
    }

    #[test]
    fn test_box_dyn_clone_and_eq() {
        let shapes = sample();
        let cloned = shapes.clone();
        assert_eq!(shapes, cloned);

        let a: Box<dyn Shape> = Box::new(Square { side: 1.0 });
        let b: Box<dyn Shape> = Box::new(Circle { radius: 1.0 });
        let c: Box<dyn Shape> = Box::new(Square { side: 1.5 });
        let unit: Box<dyn Shape> = Box::new(Square::unit());
        // Compare the unsized `dyn Shape` places, not the boxes:
        // `assert_eq!(a, unit)` expands to `*l == *r` on `Box<dyn Shape>`
        // places, which tries to move out of the boxes (rust-lang/rust#31740).
        assert_eq!(*a, *unit);
        assert_ne!(*a, *b, "different concrete types are never equal");
        assert_ne!(*a, *c, "same type, different value");
    }

    #[test]
    fn test_inherent_methods_on_dyn() {
        let shapes = sample();
        assert_eq!(count_of::<Square>(&shapes), 2);
        assert_eq!(count_of::<Circle>(&shapes), 1);
        assert_eq!(
            shapes[0].downcast_ref::<Circle>().map(|c| c.radius),
            Some(1.0)
        );
        assert!(shapes[0].downcast_ref::<Square>().is_none());
    }

    #[test]
    fn test_upcasting_and_auto_traits() {
        let circle = Circle { radius: 2.0 };
        assert_eq!(format!("{:?}", as_debug(&circle)), "Circle { radius: 2.0 }");

        let sendable: Box<dyn Shape + Send> = Box::new(Square::unit());
        let plain = forget_send(sendable);
        assert_eq!(plain.name(), "square");
    }

    #[test]
    fn test_object_lifetimes() {
        let owned = String::from("borrowed!");
        assert_eq!(boxed_display(&owned).to_string(), "borrowed!");
        assert_eq!(borrowed_display(&owned).to_string(), "borrowed!");
        assert_eq!(borrowed_display(&42).to_string(), "42");
    }

    #[test]
    fn test_type_id_footgun() {
        let boxed: Box<dyn Any> = Box::new(5_i32);
        let (outer, inner) = type_ids_of_boxed_any(&boxed);
        assert_eq!(outer, TypeId::of::<Box<dyn Any>>());
        assert_eq!(inner, TypeId::of::<i32>());
    }
}
