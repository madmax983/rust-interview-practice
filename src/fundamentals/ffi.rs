//! # FFI: ABI, `repr(C)`, Ownership Transfer, Callbacks, and Unwinding
//!
//! The foreign function interface is where Rust's guarantees stop and a written contract
//! takes over. This module plays **both sides** of the boundary: it consumes a real C
//! library (libc's `strlen` and `qsort`), and it *exports* a small C API (`rip_*` symbols,
//! `#[unsafe(no_mangle)]`). Each exported API also has the safe Rust wrapper a consumer
//! would write around it.
//!
//! Drills in this module:
//!
//! 1. **ABI basics** — `unsafe extern "C"` blocks, `safe fn` items, `link_name`, nullable
//!    function pointers (`Option<extern "C" fn>`), and the platform-sized `c_*` aliases.
//! 2. **Layout** — `repr(C)` padding with `offset_of!` checked at compile time, `repr(C,
//!    packed)`, `repr(transparent)`, `repr(i32)` status codes, and a C tagged union.
//! 3. **Strings and buffers** — `CString`/`CStr`, `c"..."` literals, the snprintf-style
//!    "query length, then fill" pattern, and slices from `(ptr, len)` pairs.
//! 4. **Ownership transfer** — opaque handles via `Box::into_raw`/`Box::from_raw`, owned
//!    strings and buffers, and the rule that memory goes back to the allocator it came from.
//! 5. **Callbacks** — `void *user_data` trampolines, generic monomorphised comparators for
//!    `qsort`, and registered callbacks with a destroy notifier.
//! 6. **Unwinding** — `extern "C"` aborts on panic, `extern "C-unwind"` lets it unwind,
//!    `catch_unwind` turns a panic into a status code, and callback panics are caught in the
//!    trampoline and resumed on the Rust side.
//!
//! ## Example
//!
//! ```
//! use rust_interview_practice::fundamentals::ffi::{Bus, CounterHandle, c_sort, shout};
//! use std::cell::Cell;
//! use std::rc::Rc;
//!
//! // Ownership transfer: a Rust object behind an opaque C handle, freed on drop.
//! let mut counter = CounterHandle::new("hits").unwrap();
//! counter.increment(2);
//! assert_eq!(counter.describe(), "hits=2");
//!
//! // Owned strings come back from "C" and are freed by the allocator that made them.
//! assert_eq!(shout("ffi").unwrap(), "FFI");
//!
//! // A generic `extern "C"` comparator lets libc's qsort sort any `Ord` type.
//! let mut words = ["pear", "apple", "fig"];
//! c_sort(&mut words);
//! assert_eq!(words, ["apple", "fig", "pear"]);
//!
//! // A Rust closure registered with a C event bus through a `user_data` trampoline.
//! let total = Rc::new(Cell::new(0));
//! let mut bus = Bus::new();
//! let sink = Rc::clone(&total);
//! bus.subscribe(move |event| sink.set(sink.get() + event));
//! bus.emit(40);
//! bus.emit(2);
//! assert_eq!(total.get(), 42);
//! ```
//!
//! ## The FFI rules this module drills
//!
//! 1. Every type that crosses the boundary has a defined layout: a primitive, a `repr(C)`
//!    or `repr(transparent)` type, a raw pointer, or an `Option` of a function pointer or
//!    `NonNull`.
//! 2. Never accept a Rust `enum` or `bool` *from* C. Take the raw integer and validate it:
//!    an out-of-range discriminant is undefined behaviour the moment it exists.
//! 3. Memory is freed by the allocator that allocated it. A pointer from `Box::into_raw` or
//!    `CString::into_raw` goes back through the matching `rip_*_free`, never `free()`.
//! 4. Every pointer argument is checked for null. `slice::from_raw_parts` needs a non-null,
//!    aligned pointer even when `len == 0`.
//! 5. A panic must not unwind out of an `extern "C"` function. Since Rust 1.81 it aborts the
//!    process. Use `catch_unwind` at the boundary, or `extern "C-unwind"` when the caller
//!    can handle unwinding. Unwinding *into* Rust from C (`longjmp`, C++ exceptions through
//!    `"C"`) is undefined behaviour.
//! 6. Every `unsafe` block states the contract it relies on in a `SAFETY:` comment.
//!
//! ## Compile-time guard rails
//!
//! In edition 2024 an `extern` block must be written `unsafe extern`: declaring a foreign
//! signature is itself a promise the compiler cannot check.
//!
//! ```compile_fail
//! extern "C" {
//!     fn strlen(s: *const std::ffi::c_char) -> usize;
//! }
//! ```
//!
//! `no_mangle` is an unsafe attribute (two crates exporting the same symbol is UB):
//!
//! ```compile_fail
//! #[no_mangle]
//! pub extern "C" fn exported() {}
//! ```
//!
//! A type with no defined C layout in an `extern "C"` signature is flagged by the
//! `improper_ctypes_definitions` lint (a warning by default; denied here):
//!
//! ```compile_fail
//! #![deny(improper_ctypes_definitions)]
//! pub extern "C" fn takes_string(s: String) -> usize {
//!     s.len()
//! }
//! ```

use std::alloc::{Layout, handle_alloc_error};
use std::any::Any;
use std::cell::RefCell;
use std::cmp::Ordering;
use std::ffi::{CStr, CString, c_char, c_int, c_void};
use std::fmt;
use std::mem::{align_of, offset_of, size_of};
use std::ops::{ControlFlow, Deref};
use std::panic::{self, AssertUnwindSafe};
use std::ptr::{self, NonNull};
use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};

// ============================================================================
// 1. ABI basics: extern blocks, safe items, link_name, function pointers
// ============================================================================

unsafe extern "C" {
    /// `size_t strlen(const char *s);` — `unsafe` to call: `s` must be a valid C string.
    fn strlen(s: *const c_char) -> usize;

    /// `void qsort(void *base, size_t nmemb, size_t size, int (*compar)(const void *, const void *));`
    fn qsort(base: *mut c_void, nmemb: usize, size: usize, compar: Comparator);

    /// Our own exported [`rip_add_wrapping`], imported back through the C ABI under a
    /// different Rust name. It takes only integers and is total, so it can be declared
    /// `safe` — callers need no `unsafe` block. (libc's `abs` could *not* be `safe`:
    /// `abs(INT_MIN)` is undefined behaviour in C.)
    #[link_name = "rip_add_wrapping"]
    safe fn imported_add_wrapping(a: c_int, b: c_int) -> c_int;
}

/// A C comparator: `int (*)(const void *, const void *)`.
pub type Comparator = unsafe extern "C" fn(*const c_void, *const c_void) -> c_int;

/// A nullable C function pointer. `Option<fn>` uses the null niche, so this is exactly one
/// pointer wide and `None` is passed to C as `NULL`.
pub type NullableComparator = Option<Comparator>;

/// Exported: `int rip_add_wrapping(int a, int b);` with two's-complement wrap-around.
///
/// Signed overflow is undefined behaviour in C, so the wrapping is part of the contract.
#[unsafe(no_mangle)]
#[must_use]
pub const extern "C" fn rip_add_wrapping(a: c_int, b: c_int) -> c_int {
    a.wrapping_add(b)
}

/// Calls [`rip_add_wrapping`] through the `extern` block: a real C-ABI call to our own
/// symbol, resolved by the linker via `link_name`.
#[must_use]
pub fn add_via_c_abi(a: i32, b: i32) -> i32 {
    imported_add_wrapping(a, b)
}

/// The length of a C string, computed by libc's `strlen`.
#[must_use]
pub fn c_strlen(s: &CStr) -> usize {
    // SAFETY: `&CStr` guarantees a valid, NUL-terminated buffer that outlives the call.
    unsafe { strlen(s.as_ptr()) }
}

/// Calls a comparator that came from C, which may be `NULL`.
///
/// # Safety
///
/// `a` and `b` must satisfy whatever the comparator dereferences them as.
#[must_use]
pub unsafe fn call_nullable(
    cmp: NullableComparator,
    a: *const c_void,
    b: *const c_void,
) -> Option<c_int> {
    let f = cmp?;
    // SAFETY: forwarded to the caller.
    Some(unsafe { f(a, b) })
}

// ============================================================================
// 2. Layout: repr(C), offset_of!, packed, transparent, status codes, tagged unions
// ============================================================================

/// A C struct: `struct header { uint8_t tag; uint32_t len; uint16_t flags; };`
///
/// `repr(C)` keeps declaration order, so padding is inserted: 3 bytes after `tag`, 2 bytes
/// after `flags`. Default `repr(Rust)` may reorder the fields to remove most of it.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Header {
    /// Message kind.
    pub tag: u8,
    /// Payload length in bytes.
    pub len: u32,
    /// Bit flags.
    pub flags: u16,
}

/// The same fields in default `repr(Rust)`: layout unspecified, usually reordered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct HeaderRust {
    /// Message kind.
    pub tag: u8,
    /// Payload length in bytes.
    pub len: u32,
    /// Bit flags.
    pub flags: u16,
}

// The layout is part of the ABI, so pin it at compile time: a change fails the build.
const _: () = {
    assert!(size_of::<Header>() == 12);
    assert!(align_of::<Header>() == 4);
    assert!(offset_of!(Header, tag) == 0);
    assert!(offset_of!(Header, len) == 4);
    assert!(offset_of!(Header, flags) == 8);
};

impl Header {
    /// Size of a `Header` on the wire (including padding).
    pub const SIZE: usize = size_of::<Self>();

    /// Reads a header from raw bytes, as if C had `memcpy`'d it into a buffer.
    ///
    /// Sound because every field is an integer (any bit pattern is valid) and
    /// `read_unaligned` makes no alignment demand on the byte buffer.
    #[must_use]
    pub const fn read_from(bytes: &[u8; Self::SIZE]) -> Self {
        // SAFETY: `bytes` is `SIZE` readable bytes; every bit pattern is a valid `Header`.
        unsafe { ptr::read_unaligned(bytes.as_ptr().cast::<Self>()) }
    }

    /// Writes the header into bytes field by field, with zeroed padding.
    ///
    /// Do **not** transmute `&Header` to `&[u8; 12]`: the padding bytes are uninitialised,
    /// and reading uninitialised bytes is undefined behaviour (and a data leak).
    #[must_use]
    pub fn to_bytes(&self) -> [u8; Self::SIZE] {
        let mut out = [0_u8; Self::SIZE];
        out[offset_of!(Self, tag)] = self.tag;
        let len_at = offset_of!(Self, len);
        out[len_at..len_at + 4].copy_from_slice(&self.len.to_ne_bytes());
        let flags_at = offset_of!(Self, flags);
        out[flags_at..flags_at + 2].copy_from_slice(&self.flags.to_ne_bytes());
        out
    }
}

/// `#pragma pack(1)` / `__attribute__((packed))`: no padding, fields may be misaligned.
///
/// Taking a reference to a misaligned field is an error (E0793):
///
/// ```compile_fail,E0793
/// use rust_interview_practice::fundamentals::ffi::PackedHeader;
///
/// let h = PackedHeader::new(1, 2, 3);
/// let len: &u32 = &h.len; // error[E0793]: reference to packed field is unaligned
/// ```
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct PackedHeader {
    /// Message kind.
    pub tag: u8,
    /// Payload length, at offset 1 (misaligned).
    pub len: u32,
    /// Bit flags, at offset 5 (misaligned).
    pub flags: u16,
}

const _: () = {
    assert!(size_of::<PackedHeader>() == 7);
    assert!(align_of::<PackedHeader>() == 1);
    assert!(offset_of!(PackedHeader, len) == 1);
};

impl PackedHeader {
    /// Creates a packed header.
    #[must_use]
    pub const fn new(tag: u8, len: u32, flags: u16) -> Self {
        Self { tag, len, flags }
    }

    /// Reads `len` by value: copying out of a packed field is fine.
    #[must_use]
    pub const fn len_by_copy(&self) -> u32 {
        self.len
    }

    /// Reads `len` through a raw pointer: `&raw const` never creates a reference, and
    /// `read_unaligned` tolerates the misalignment.
    #[must_use]
    pub const fn len_by_raw_pointer(&self) -> u32 {
        let p = &raw const self.len;
        // SAFETY: `p` points at an initialised `u32` inside `self`; alignment is handled.
        unsafe { p.read_unaligned() }
    }
}

/// A newtype with the exact ABI of its field, so it can appear in `extern "C"` signatures
/// where C sees a plain `uint64_t`.
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Millis(pub u64);

/// Exported: `uint64_t rip_millis_add(uint64_t a, uint64_t b);` (saturating).
#[unsafe(no_mangle)]
#[must_use]
pub const extern "C" fn rip_millis_add(a: Millis, b: Millis) -> Millis {
    Millis(a.0.saturating_add(b.0))
}

/// Status codes returned by every fallible exported function.
///
/// Returning a fieldless `repr(i32)` enum *to* C is fine. Receiving one *from* C is not:
/// take a `c_int` and use [`FfiStatus::from_raw`].
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FfiStatus {
    /// Success.
    Ok = 0,
    /// A required pointer argument was null.
    NullPointer = 1,
    /// A string argument was not valid UTF-8.
    InvalidUtf8 = 2,
    /// An argument was out of range or malformed.
    InvalidArgument = 3,
    /// A panic was caught at the boundary.
    Panic = 4,
}

impl FfiStatus {
    /// Validates a raw status code from C. Never transmute it.
    #[must_use]
    pub const fn from_raw(raw: c_int) -> Option<Self> {
        match raw {
            0 => Some(Self::Ok),
            1 => Some(Self::NullPointer),
            2 => Some(Self::InvalidUtf8),
            3 => Some(Self::InvalidArgument),
            4 => Some(Self::Panic),
            _ => None,
        }
    }
}

/// Raw tags for [`CValue`]. A plain `u32`, not an enum: C may store anything here.
pub mod value_tag {
    /// `payload.int` is active.
    pub const INT: u32 = 0;
    /// `payload.float` is active.
    pub const FLOAT: u32 = 1;
    /// `payload.boolean` is active (0 or 1).
    pub const BOOL: u32 = 2;
}

/// The untagged half of a C tagged union.
#[repr(C)]
#[derive(Clone, Copy)]
pub union ValuePayload {
    /// Active when `tag == INT`.
    pub int: i64,
    /// Active when `tag == FLOAT`.
    pub float: f64,
    /// Active when `tag == BOOL`. A `u8`, not `bool`: C may write a 2.
    pub boolean: u8,
}

/// `struct value { uint32_t tag; union { int64_t i; double f; uint8_t b; } payload; };`
#[repr(C)]
#[derive(Clone, Copy)]
pub struct CValue {
    /// Which union field is active; see [`value_tag`].
    pub tag: u32,
    /// The payload, interpreted according to `tag`.
    pub payload: ValuePayload,
}

impl fmt::Debug for CValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match Value::try_from(*self) {
            Ok(v) => f.debug_tuple("CValue").field(&v).finish(),
            Err(e) => f.debug_tuple("CValue").field(&e).finish(),
        }
    }
}

/// The safe Rust view of a [`CValue`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Value {
    /// An integer.
    Int(i64),
    /// A float.
    Float(f64),
    /// A boolean.
    Bool(bool),
}

impl From<Value> for CValue {
    fn from(value: Value) -> Self {
        match value {
            Value::Int(int) => Self {
                tag: value_tag::INT,
                payload: ValuePayload { int },
            },
            Value::Float(float) => Self {
                tag: value_tag::FLOAT,
                payload: ValuePayload { float },
            },
            Value::Bool(b) => Self {
                tag: value_tag::BOOL,
                payload: ValuePayload {
                    boolean: u8::from(b),
                },
            },
        }
    }
}

impl TryFrom<CValue> for Value {
    type Error = FfiError;

    /// Validates the tag (and the bool byte) before reading the union.
    fn try_from(raw: CValue) -> Result<Self, FfiError> {
        // SAFETY (all arms): the tag says which field C wrote; every field is plain data
        // with no invalid bit patterns (`boolean` is a `u8`, validated below).
        match raw.tag {
            value_tag::INT => Ok(Self::Int(unsafe { raw.payload.int })),
            value_tag::FLOAT => Ok(Self::Float(unsafe { raw.payload.float })),
            value_tag::BOOL => match unsafe { raw.payload.boolean } {
                0 => Ok(Self::Bool(false)),
                1 => Ok(Self::Bool(true)),
                other => Err(FfiError::InvalidArgument(format!("bool byte {other}"))),
            },
            other => Err(FfiError::InvalidArgument(format!("value tag {other}"))),
        }
    }
}

// ============================================================================
// Errors and the boundary guard (used by every exported function below)
// ============================================================================

/// The Rust-side error for the exported API; each maps to an [`FfiStatus`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FfiError {
    /// The named pointer argument was null.
    NullPointer(&'static str),
    /// A string argument was not UTF-8.
    InvalidUtf8,
    /// An argument was malformed.
    InvalidArgument(String),
}

impl FfiError {
    /// The status code reported to C.
    #[must_use]
    pub const fn status(&self) -> FfiStatus {
        match self {
            Self::NullPointer(_) => FfiStatus::NullPointer,
            Self::InvalidUtf8 => FfiStatus::InvalidUtf8,
            Self::InvalidArgument(_) => FfiStatus::InvalidArgument,
        }
    }
}

impl fmt::Display for FfiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NullPointer(arg) => write!(f, "null pointer: {arg}"),
            Self::InvalidUtf8 => f.write_str("invalid UTF-8"),
            Self::InvalidArgument(why) => write!(f, "invalid argument: {why}"),
        }
    }
}

impl std::error::Error for FfiError {}

thread_local! {
    /// errno-style last error for the exported API. Set on failure, read by
    /// `rip_last_error_message`, and never cleared by a later success.
    static LAST_ERROR: RefCell<Option<CString>> = const { RefCell::new(None) };

    /// A panic caught inside a callback trampoline, waiting to be resumed on the Rust side
    /// once control is back out of C.
    static PENDING_PANIC: RefCell<Option<Box<dyn Any + Send>>> = const { RefCell::new(None) };
}

fn set_last_error(message: &str) {
    // An interior NUL would make `CString::new` fail; replace it rather than lose the error.
    let c = CString::new(message.replace('\0', "\u{FFFD}")).unwrap_or_default();
    LAST_ERROR.with_borrow_mut(|slot| *slot = Some(c));
}

/// Best-effort text of a panic payload (`panic!` produces `&str` or `String`).
#[must_use]
pub fn panic_message(payload: &(dyn Any + Send)) -> &str {
    payload
        .downcast_ref::<&str>()
        .copied()
        .or_else(|| payload.downcast_ref::<String>().map(String::as_str))
        .unwrap_or("<non-string panic payload>")
}

/// The boundary guard: runs `f`, and turns both errors and panics into a status code plus
/// a last-error message. Every exported fallible function body goes through this.
pub fn ffi_guard(f: impl FnOnce() -> Result<(), FfiError>) -> FfiStatus {
    // `AssertUnwindSafe`: after a panic we only report it; no state `f` touched is reused
    // in a way that could observe a broken invariant.
    match panic::catch_unwind(AssertUnwindSafe(f)) {
        Ok(Ok(())) => FfiStatus::Ok,
        Ok(Err(e)) => {
            set_last_error(&e.to_string());
            e.status()
        }
        Err(payload) => {
            set_last_error(&format!("panic: {}", panic_message(&*payload)));
            FfiStatus::Panic
        }
    }
}

/// Like [`ffi_guard`] for functions that return a pointer: null on any failure.
fn ffi_guard_ptr<T>(f: impl FnOnce() -> Result<*mut T, FfiError>) -> *mut T {
    let mut out = ptr::null_mut();
    let status = ffi_guard(|| {
        out = f()?;
        Ok(())
    });
    if status == FfiStatus::Ok {
        out
    } else {
        ptr::null_mut()
    }
}

/// Guard for code running *inside* a callback that C invoked: a panic must not unwind
/// through the C frames, so it is caught, stashed, and `on_panic` is returned to C.
fn guard_callback<R>(on_panic: R, f: impl FnOnce() -> R) -> R {
    match panic::catch_unwind(AssertUnwindSafe(f)) {
        Ok(r) => r,
        Err(payload) => {
            PENDING_PANIC.with_borrow_mut(|slot| {
                // Keep the first panic; a second one from the same C call is dropped.
                if slot.is_none() {
                    *slot = Some(payload);
                }
            });
            on_panic
        }
    }
}

/// Re-raises a panic stashed by a callback trampoline. Call after every C call that may
/// have invoked Rust callbacks, once the C frames are gone.
fn resume_pending_panic() {
    if let Some(payload) = PENDING_PANIC.with_borrow_mut(Option::take) {
        panic::resume_unwind(payload);
    }
}

// ============================================================================
// 3. Strings and buffers
// ============================================================================

/// A `'static` C string literal: NUL-terminated at compile time, no allocation.
pub const GREETING: &CStr = c"hello from rust";

/// Exported: `const char *rip_greeting(void);` — **borrowed**, static, never freed.
#[unsafe(no_mangle)]
#[must_use]
pub const extern "C" fn rip_greeting() -> *const c_char {
    GREETING.as_ptr()
}

/// Copies `src` into a C buffer snprintf-style: writes at most `cap - 1` bytes plus a NUL
/// and returns the full length of `src` (without the NUL). If the return value is `>= cap`,
/// the output was truncated. With `buf == NULL` or `cap == 0`, it only reports the length.
///
/// # Safety
///
/// If `buf` is non-null, it must be valid for writes of `cap` bytes.
unsafe fn copy_to_c_buffer(src: &CStr, buf: *mut c_char, cap: usize) -> usize {
    let bytes = src.to_bytes();
    if !buf.is_null() && cap > 0 {
        let n = bytes.len().min(cap - 1);
        // SAFETY: `buf` has room for `cap >= n + 1` bytes (caller contract); `src` and `buf`
        // cannot overlap because `buf` is writable and `src` is a shared borrow.
        unsafe {
            ptr::copy_nonoverlapping(bytes.as_ptr().cast::<c_char>(), buf, n);
            buf.add(n).write(0);
        }
    }
    bytes.len()
}

/// Exported: `size_t rip_copy_greeting(char *buf, size_t cap);` (snprintf-style).
///
/// # Safety
///
/// If `buf` is non-null, it must be valid for writes of `cap` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rip_copy_greeting(buf: *mut c_char, cap: usize) -> usize {
    // SAFETY: forwarded to the caller.
    unsafe { copy_to_c_buffer(GREETING, buf, cap) }
}

/// The consumer side of the snprintf pattern: query the length, allocate, fill, and retry
/// if the source grew in between.
pub fn read_c_string(mut fill: impl FnMut(*mut c_char, usize) -> usize) -> CString {
    let mut needed = fill(ptr::null_mut(), 0);
    loop {
        let mut buf = vec![0_u8; needed + 1];
        let written = fill(buf.as_mut_ptr().cast::<c_char>(), buf.len());
        if written < buf.len() {
            buf.truncate(written + 1);
            // The callee wrote exactly `written` bytes plus a NUL; an interior NUL would
            // mean it broke the contract, so fall back to the prefix before it.
            return CString::from_vec_with_nul(buf).unwrap_or_else(|e| {
                let nul = e.as_bytes().iter().position(|&b| b == 0).unwrap_or(0);
                CString::new(&e.as_bytes()[..nul]).unwrap_or_default()
            });
        }
        needed = written;
    }
}

/// Borrows a C string argument as `&str`, rejecting null and non-UTF-8.
///
/// # Safety
///
/// If non-null, `s` must point at a NUL-terminated string valid for `'a`.
unsafe fn str_arg<'a>(s: *const c_char, name: &'static str) -> Result<&'a str, FfiError> {
    if s.is_null() {
        return Err(FfiError::NullPointer(name));
    }
    // SAFETY: non-null, and the caller guarantees NUL termination and lifetime.
    unsafe { CStr::from_ptr(s) }
        .to_str()
        .map_err(|_| FfiError::InvalidUtf8)
}

/// Builds a slice from a C `(ptr, len)` pair.
///
/// `slice::from_raw_parts` requires a non-null, aligned pointer *even for `len == 0`*, and C
/// callers routinely pass `(NULL, 0)`. So an empty input becomes `&[]` without touching
/// `ptr`, and a null or misaligned `ptr` with `len > 0` is rejected.
///
/// # Safety
///
/// If `len > 0`, `ptr` must point at `len` initialised `T`s, valid and unmutated for `'a`.
pub unsafe fn slice_from_c<'a, T>(
    ptr: *const T,
    len: usize,
    name: &'static str,
) -> Result<&'a [T], FfiError> {
    if len == 0 {
        return Ok(&[]);
    }
    if ptr.is_null() {
        return Err(FfiError::NullPointer(name));
    }
    if !ptr.is_aligned() {
        return Err(FfiError::InvalidArgument(format!("{name} is misaligned")));
    }
    // SAFETY: non-null, aligned, and the caller guarantees `len` live elements for `'a`.
    Ok(unsafe { std::slice::from_raw_parts(ptr, len) })
}

/// A plain C point, passed by value and through out-parameters.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Point {
    /// X coordinate.
    pub x: f64,
    /// Y coordinate.
    pub y: f64,
}

/// Exported: `int rip_point_parse(const char *s, struct point *out);` parses `"x,y"`.
///
/// Out-parameter rule: `*out` is written **only** on success, so C can keep a default.
///
/// # Safety
///
/// `s` must be null or a valid C string; `out` must be null or valid for one `Point` write.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rip_point_parse(s: *const c_char, out: *mut Point) -> FfiStatus {
    ffi_guard(|| {
        // SAFETY: forwarded to the caller.
        let text = unsafe { str_arg(s, "s") }?;
        if out.is_null() {
            return Err(FfiError::NullPointer("out"));
        }
        let (x, y) = text
            .split_once(',')
            .ok_or_else(|| FfiError::InvalidArgument(format!("expected \"x,y\", got {text:?}")))?;
        let parse = |part: &str| {
            part.trim()
                .parse::<f64>()
                .map_err(|e| FfiError::InvalidArgument(format!("{part:?}: {e}")))
        };
        let point = Point {
            x: parse(x)?,
            y: parse(y)?,
        };
        // SAFETY: `out` is non-null and valid for writes (caller contract). `write` does not
        // read or drop the old (possibly uninitialised) value.
        unsafe { out.write(point) };
        Ok(())
    })
}

/// Exported: `int rip_points_centroid(const struct point *pts, size_t len, struct point *out);`
///
/// # Safety
///
/// `(pts, len)` must describe `len` readable points (or `len == 0`); `out` must be null or
/// valid for one `Point` write.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rip_points_centroid(
    pts: *const Point,
    len: usize,
    out: *mut Point,
) -> FfiStatus {
    ffi_guard(|| {
        // SAFETY: forwarded to the caller.
        let points = unsafe { slice_from_c(pts, len, "pts") }?;
        if out.is_null() {
            return Err(FfiError::NullPointer("out"));
        }
        if points.is_empty() {
            return Err(FfiError::InvalidArgument("centroid of zero points".into()));
        }
        #[allow(clippy::cast_precision_loss)] // point counts are far below 2^52
        let n = points.len() as f64;
        let (sx, sy) = points
            .iter()
            .fold((0.0, 0.0), |(sx, sy), p| (sx + p.x, sy + p.y));
        // SAFETY: `out` is non-null and valid for writes (caller contract).
        unsafe {
            out.write(Point {
                x: sx / n,
                y: sy / n,
            });
        };
        Ok(())
    })
}

// ============================================================================
// 4. Ownership transfer: opaque handles, owned strings, owned buffers
// ============================================================================

static LIVE_COUNTERS: AtomicUsize = AtomicUsize::new(0);

/// The number of [`Counter`]s created by `rip_counter_new` and not yet freed (leak check).
#[must_use]
pub fn live_counters() -> usize {
    LIVE_COUNTERS.load(AtomicOrdering::SeqCst)
}

/// A Rust object exposed to C only as an opaque pointer (`typedef struct Counter Counter;`).
///
/// It is deliberately *not* `repr(C)`: C never sees inside, so Rust keeps full control
/// of the layout and can hold non-FFI types like `CString`.
#[derive(Debug)]
pub struct Counter {
    name: CString,
    count: u64,
}

impl Drop for Counter {
    fn drop(&mut self) {
        LIVE_COUNTERS.fetch_sub(1, AtomicOrdering::SeqCst);
    }
}

/// Exported: `Counter *rip_counter_new(const char *name);` — **owned**; free with
/// [`rip_counter_free`]. Returns null on error (see `rip_last_error_message`).
///
/// # Safety
///
/// `name` must be null or a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rip_counter_new(name: *const c_char) -> *mut Counter {
    ffi_guard_ptr(|| {
        // SAFETY: forwarded to the caller.
        let name = unsafe { str_arg(name, "name") }?;
        if name.is_empty() {
            return Err(FfiError::InvalidArgument("empty counter name".into()));
        }
        // A valid C string has no interior NUL, so this cannot fail.
        let name = CString::new(name).map_err(|e| FfiError::InvalidArgument(e.to_string()))?;
        LIVE_COUNTERS.fetch_add(1, AtomicOrdering::SeqCst);
        // Ownership moves to C: the box is leaked until `rip_counter_free` rebuilds it.
        Ok(Box::into_raw(Box::new(Counter { name, count: 0 })))
    })
}

/// Exported: `int rip_counter_increment(Counter *c, uint64_t by);` (saturating).
///
/// # Safety
///
/// `c` must be null or a live pointer from [`rip_counter_new`], not aliased during the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rip_counter_increment(c: *mut Counter, by: u64) -> FfiStatus {
    ffi_guard(|| {
        // SAFETY: caller contract: null or live and unaliased; `as_mut` checks null.
        let counter = unsafe { c.as_mut() }.ok_or(FfiError::NullPointer("c"))?;
        counter.count = counter.count.saturating_add(by);
        Ok(())
    })
}

/// Exported: `int rip_counter_get(const Counter *c, uint64_t *out);`
///
/// # Safety
///
/// `c` must be null or a live pointer from [`rip_counter_new`]; `out` null or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rip_counter_get(c: *const Counter, out: *mut u64) -> FfiStatus {
    ffi_guard(|| {
        // SAFETY: caller contract; `as_ref` checks null.
        let counter = unsafe { c.as_ref() }.ok_or(FfiError::NullPointer("c"))?;
        if out.is_null() {
            return Err(FfiError::NullPointer("out"));
        }
        // SAFETY: non-null and writable (caller contract).
        unsafe { out.write(counter.count) };
        Ok(())
    })
}

/// Exported: `const char *rip_counter_name(const Counter *c);` — **borrowed**: valid until
/// the counter is freed. Null if `c` is null.
///
/// # Safety
///
/// `c` must be null or a live pointer from [`rip_counter_new`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rip_counter_name(c: *const Counter) -> *const c_char {
    // SAFETY: caller contract; `as_ref` checks null.
    unsafe { c.as_ref() }.map_or(ptr::null(), |counter| counter.name.as_ptr())
}

/// Exported: `char *rip_counter_describe(const Counter *c);` — **owned**: free it with
/// [`rip_string_free`], never `free()`.
///
/// # Safety
///
/// `c` must be null or a live pointer from [`rip_counter_new`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rip_counter_describe(c: *const Counter) -> *mut c_char {
    ffi_guard_ptr(|| {
        // SAFETY: caller contract; `as_ref` checks null.
        let counter = unsafe { c.as_ref() }.ok_or(FfiError::NullPointer("c"))?;
        let text = format!("{}={}", counter.name.to_string_lossy(), counter.count);
        let owned = CString::new(text).map_err(|e| FfiError::InvalidArgument(e.to_string()))?;
        Ok(owned.into_raw())
    })
}

/// Exported: `void rip_counter_free(Counter *c);` — null is a no-op, like `free(NULL)`.
///
/// # Safety
///
/// `c` must be null or a pointer from [`rip_counter_new`] that has not been freed yet.
/// It must not be used afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rip_counter_free(c: *mut Counter) {
    if !c.is_null() {
        // SAFETY: `c` came from `Box::into_raw` in `rip_counter_new` and is freed only once.
        drop(unsafe { Box::from_raw(c) });
    }
}

/// Exported: `char *rip_shout(const char *s);` — **owned** upper-cased copy, or null on
/// error. Free with [`rip_string_free`].
///
/// # Safety
///
/// `s` must be null or a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rip_shout(s: *const c_char) -> *mut c_char {
    ffi_guard_ptr(|| {
        // SAFETY: forwarded to the caller.
        let text = unsafe { str_arg(s, "s") }?;
        let upper = CString::new(text.to_uppercase())
            .map_err(|e| FfiError::InvalidArgument(e.to_string()))?;
        Ok(upper.into_raw())
    })
}

/// Exported: `void rip_string_free(char *s);` — frees strings this library returned.
///
/// # Safety
///
/// `s` must be null or a pointer returned as owned by this library (`CString::into_raw`)
/// and not yet freed. The C side must not have changed its length (no interior NUL).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rip_string_free(s: *mut c_char) {
    if !s.is_null() {
        // SAFETY: `s` came from `CString::into_raw` and is freed only once.
        drop(unsafe { CString::from_raw(s) });
    }
}

/// Exported: `uint64_t *rip_fibonacci(size_t n, size_t *out_len);`
///
/// Returns an **owned** buffer of the first `n` Fibonacci numbers (saturating). Free with [`rip_u64_buffer_free`] and the
/// same length. Returns null when `n == 0` or `out_len` is null.
///
/// # Safety
///
/// `out_len` must be null or valid for one `usize` write.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rip_fibonacci(n: usize, out_len: *mut usize) -> *mut u64 {
    ffi_guard_ptr(|| {
        if out_len.is_null() {
            return Err(FfiError::NullPointer("out_len"));
        }
        if n == 0 {
            return Err(FfiError::InvalidArgument("n must be positive".into()));
        }
        let mut values = Vec::with_capacity(n);
        let (mut a, mut b) = (0_u64, 1_u64);
        for _ in 0..n {
            values.push(a);
            (a, b) = (b, a.saturating_add(b));
        }
        // `Box<[T]>` has capacity == length, so `(ptr, len)` is enough to free it later.
        // A raw `Vec` would also need its capacity passed back.
        let boxed: Box<[u64]> = values.into_boxed_slice();
        // SAFETY: `out_len` is non-null and writable (caller contract).
        unsafe { out_len.write(boxed.len()) };
        Ok(Box::into_raw(boxed).cast::<u64>())
    })
}

/// Exported: `void rip_u64_buffer_free(uint64_t *ptr, size_t len);`
///
/// # Safety
///
/// `ptr` must be null or a pointer from [`rip_fibonacci`] with the exact `len` it reported,
/// not yet freed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rip_u64_buffer_free(ptr: *mut u64, len: usize) {
    if !ptr.is_null() {
        // SAFETY: rebuild the exact `Box<[u64]>` (same pointer and length) that was leaked.
        drop(unsafe { Box::from_raw(ptr::slice_from_raw_parts_mut(ptr, len)) });
    }
}

/// Reads the last error message through the two-call snprintf pattern.
///
/// Exported as `size_t rip_last_error_message(char *buf, size_t cap);`; returns 0 and
/// writes an empty string when there is no error.
///
/// # Safety
///
/// If `buf` is non-null, it must be valid for writes of `cap` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rip_last_error_message(buf: *mut c_char, cap: usize) -> usize {
    LAST_ERROR.with_borrow(|slot| {
        let msg = slot.as_deref().unwrap_or(c"");
        // SAFETY: forwarded to the caller.
        unsafe { copy_to_c_buffer(msg, buf, cap) }
    })
}

/// The last error on this thread, read back through the exported C API.
#[must_use]
pub fn last_error() -> Option<String> {
    // SAFETY: `read_c_string` passes either null or a buffer of exactly `cap` bytes.
    let c = read_c_string(|buf, cap| unsafe { rip_last_error_message(buf, cap) });
    let text = c.into_string().ok()?;
    (!text.is_empty()).then_some(text)
}

/// An owned string returned by this library, freed with [`rip_string_free`] on drop.
#[derive(Debug)]
pub struct RipString(NonNull<c_char>);

impl RipString {
    /// Takes ownership of a string the library returned, or `None` for null.
    ///
    /// # Safety
    ///
    /// `raw` must be null or an owned string from this library, not freed elsewhere.
    #[must_use]
    pub unsafe fn from_raw(raw: *mut c_char) -> Option<Self> {
        NonNull::new(raw).map(Self)
    }
}

impl Deref for RipString {
    type Target = CStr;

    fn deref(&self) -> &CStr {
        // SAFETY: the pointer is a live, NUL-terminated string owned by `self`.
        unsafe { CStr::from_ptr(self.0.as_ptr()) }
    }
}

impl Drop for RipString {
    fn drop(&mut self) {
        // SAFETY: we own the string and free it exactly once, with the matching function.
        unsafe { rip_string_free(self.0.as_ptr()) };
    }
}

/// Safe wrapper over [`rip_shout`].
///
/// # Errors
///
/// Returns [`FfiError::InvalidArgument`] if `s` contains a NUL byte, or the library's
/// last-error message if the call fails.
pub fn shout(s: &str) -> Result<String, FfiError> {
    let arg = CString::new(s).map_err(|e| FfiError::InvalidArgument(e.to_string()))?;
    // SAFETY: `arg` is a valid C string for the duration of the call.
    let raw = unsafe { rip_shout(arg.as_ptr()) };
    // SAFETY: `rip_shout` returns null or an owned string from this library.
    let owned = unsafe { RipString::from_raw(raw) }
        .ok_or_else(|| FfiError::InvalidArgument(last_error().unwrap_or_default()))?;
    Ok(owned.to_string_lossy().into_owned())
}

/// Safe RAII wrapper over the opaque `Counter *` handle: the consumer's view of a C API.
///
/// `NonNull` makes it `!Send`/`!Sync` by default, which is the conservative choice for a
/// foreign handle until the library documents its thread safety.
#[derive(Debug)]
pub struct CounterHandle(NonNull<Counter>);

impl CounterHandle {
    /// Creates a counter, or `None` if the name is empty or contains NUL.
    #[must_use]
    pub fn new(name: &str) -> Option<Self> {
        let name = CString::new(name).ok()?;
        // SAFETY: `name` is a valid C string for the duration of the call.
        NonNull::new(unsafe { rip_counter_new(name.as_ptr()) }).map(Self)
    }

    /// Adds `by` to the counter (saturating).
    pub fn increment(&mut self, by: u64) {
        // SAFETY: we own a live handle, and `&mut self` makes this the only access.
        let status = unsafe { rip_counter_increment(self.0.as_ptr(), by) };
        debug_assert_eq!(status, FfiStatus::Ok);
    }

    /// The current count.
    #[must_use]
    pub fn get(&self) -> u64 {
        let mut out = 0;
        // SAFETY: live handle; `out` is a valid `u64` slot.
        let status = unsafe { rip_counter_get(self.0.as_ptr(), &raw mut out) };
        debug_assert_eq!(status, FfiStatus::Ok);
        out
    }

    /// The counter's name, borrowed from the handle: the `&self` lifetime is exactly the
    /// "valid until freed" contract of `rip_counter_name`.
    #[must_use]
    pub fn name(&self) -> &CStr {
        // SAFETY: live handle, so the result is non-null and lives as long as `self`.
        unsafe { CStr::from_ptr(rip_counter_name(self.0.as_ptr())) }
    }

    /// `"name=count"`, via an owned string that must be freed by the library.
    #[must_use]
    pub fn describe(&self) -> String {
        // SAFETY: live handle; the result is an owned library string or null.
        unsafe { RipString::from_raw(rip_counter_describe(self.0.as_ptr())) }
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default()
    }

    /// Releases ownership to the caller (e.g. to hand the handle to C).
    #[must_use]
    pub const fn into_raw(self) -> *mut Counter {
        let raw = self.0.as_ptr();
        std::mem::forget(self);
        raw
    }

    /// Re-takes ownership of a raw handle.
    ///
    /// # Safety
    ///
    /// `raw` must be a live pointer from [`rip_counter_new`] not owned by anything else.
    #[must_use]
    pub unsafe fn from_raw(raw: *mut Counter) -> Option<Self> {
        NonNull::new(raw).map(Self)
    }
}

impl Drop for CounterHandle {
    fn drop(&mut self) {
        // SAFETY: we own the handle and free it exactly once.
        unsafe { rip_counter_free(self.0.as_ptr()) };
    }
}

/// Safe wrapper over [`rip_fibonacci`]: copies the owned buffer into a `Vec` and frees it.
#[must_use]
pub fn fibonacci(n: usize) -> Vec<u64> {
    let mut len = 0;
    // SAFETY: `len` is a valid out-parameter.
    let raw = unsafe { rip_fibonacci(n, &raw mut len) };
    if raw.is_null() {
        return Vec::new();
    }
    // SAFETY: `rip_fibonacci` returned `len` initialised values at `raw`.
    let values = unsafe { std::slice::from_raw_parts(raw, len) }.to_vec();
    // SAFETY: freed once, with the length it reported.
    unsafe { rip_u64_buffer_free(raw, len) };
    values
}

// ============================================================================
// 5. Callbacks: user_data trampolines, qsort comparators, registered callbacks
// ============================================================================

/// `int (*visit)(void *user_data, int64_t value);` — return non-zero to stop.
pub type VisitFn = Option<unsafe extern "C" fn(user_data: *mut c_void, value: i64) -> c_int>;

/// Exported: a C-style iterator that calls `visit` for each value until it returns
/// non-zero. `out_visited` is an optional out-parameter (pass null to ignore).
///
/// # Safety
///
/// `(values, len)` must be readable; `visit` must be safe to call with `user_data`;
/// `out_visited` must be null or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rip_for_each(
    values: *const i64,
    len: usize,
    visit: VisitFn,
    user_data: *mut c_void,
    out_visited: *mut usize,
) -> FfiStatus {
    ffi_guard(|| {
        // SAFETY: forwarded to the caller.
        let values = unsafe { slice_from_c(values, len, "values") }?;
        let visit = visit.ok_or(FfiError::NullPointer("visit"))?;
        let mut visited = 0;
        for &value in values {
            visited += 1;
            // SAFETY: the caller guarantees `visit` accepts `user_data`.
            if unsafe { visit(user_data, value) } != 0 {
                break;
            }
        }
        // SAFETY: optional out-parameter: written only when non-null (caller contract).
        if let Some(out) = unsafe { out_visited.as_mut() } {
            *out = visited;
        }
        Ok(())
    })
}

/// Runs a Rust closure as the `visit` callback of [`rip_for_each`].
///
/// The closure travels through `void *user_data` as `&mut F`, and a generic trampoline
/// `trampoline::<F>` (one instantiation per closure type) casts it back. If the closure
/// panics, the trampoline catches it, tells C to stop, and the panic is resumed here once
/// the C frames are gone. Returns how many values were visited.
pub fn for_each_via_c<F>(values: &[i64], mut f: F) -> usize
where
    F: FnMut(i64) -> ControlFlow<()>,
{
    unsafe extern "C" fn trampoline<F>(user_data: *mut c_void, value: i64) -> c_int
    where
        F: FnMut(i64) -> ControlFlow<()>,
    {
        guard_callback(1, || {
            // SAFETY: `user_data` is the `&mut F` passed below; `rip_for_each` only uses it
            // during the call, while `f` is alive and not otherwise borrowed.
            let f = unsafe { &mut *user_data.cast::<F>() };
            c_int::from(f(value).is_break())
        })
    }

    let mut visited = 0;
    // SAFETY: `values` is a live slice; `trampoline::<F>` matches `user_data`'s real type.
    let status = unsafe {
        rip_for_each(
            values.as_ptr(),
            values.len(),
            Some(trampoline::<F>),
            (&raw mut f).cast::<c_void>(),
            &raw mut visited,
        )
    };
    resume_pending_panic();
    debug_assert_eq!(status, FfiStatus::Ok);
    visited
}

/// A comparator for libc `qsort`, generic over `T: Ord`.
///
/// `qsort` passes no context pointer, so a closure cannot be threaded through. Instead the
/// type parameter *is* the context: each `T` gets its own monomorphised `extern "C"` fn.
/// A panic in `T::cmp` aborts the process (it cannot unwind through `"C"`), which is safe.
unsafe extern "C" fn compare_ord<T: Ord>(a: *const c_void, b: *const c_void) -> c_int {
    // SAFETY: `qsort` passes pointers to two elements of the `[T]` handed to `c_sort`.
    let (a, b) = unsafe { (&*a.cast::<T>(), &*b.cast::<T>()) };
    match a.cmp(b) {
        Ordering::Less => -1,
        Ordering::Equal => 0,
        Ordering::Greater => 1,
    }
}

/// Sorts with libc's `qsort` (not stable). Any Rust value may be moved with `memcpy`,
/// which is all `qsort` does to elements.
pub fn c_sort<T: Ord>(values: &mut [T]) {
    if values.len() < 2 || size_of::<T>() == 0 {
        return;
    }
    // SAFETY: `values` is a live, exclusive slice of `len` elements of `size_of::<T>()`
    // bytes, and `compare_ord::<T>` reads exactly those elements.
    unsafe {
        qsort(
            values.as_mut_ptr().cast::<c_void>(),
            values.len(),
            size_of::<T>(),
            compare_ord::<T>,
        );
    }
}

/// `void (*on_event)(void *user_data, int64_t event);`
pub type EventFn = Option<unsafe extern "C" fn(user_data: *mut c_void, event: i64)>;

/// `void (*destroy)(void *user_data);` — the `GDestroyNotify` pattern from `GLib`. Called exactly once
/// when the library is done with `user_data` (unsubscribe or bus free).
pub type DestroyFn = Option<unsafe extern "C" fn(user_data: *mut c_void)>;

/// The C library's view of one subscription. Its `Drop` runs the destroy notifier, so
/// every path that removes a subscriber releases `user_data` exactly once.
#[derive(Debug)]
struct Subscriber {
    id: u64,
    on_event: unsafe extern "C" fn(*mut c_void, i64),
    user_data: *mut c_void,
    destroy: DestroyFn,
}

impl Drop for Subscriber {
    fn drop(&mut self) {
        if let Some(destroy) = self.destroy {
            // SAFETY: the subscriber promised `destroy` accepts its own `user_data`, and
            // this runs once because `Subscriber` is dropped once.
            unsafe { destroy(self.user_data) };
        }
    }
}

/// An opaque C event bus: `typedef struct EventBus EventBus;`.
#[derive(Debug, Default)]
pub struct EventBus {
    next_id: u64,
    subscribers: Vec<Subscriber>,
}

/// Exported: `EventBus *rip_bus_new(void);` — **owned**; free with [`rip_bus_free`].
#[unsafe(no_mangle)]
#[must_use]
pub extern "C" fn rip_bus_new() -> *mut EventBus {
    Box::into_raw(Box::default())
}

/// Exported: `uint64_t rip_bus_subscribe(EventBus *, on_event, void *user_data, destroy);`
///
/// Returns a non-zero subscription id, or 0 on error. On error `destroy` is **not** called:
/// ownership of `user_data` stays with the caller.
///
/// # Safety
///
/// `bus` must be null or live; `on_event` and `destroy` must accept `user_data` until
/// `destroy` runs.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rip_bus_subscribe(
    bus: *mut EventBus,
    on_event: EventFn,
    user_data: *mut c_void,
    destroy: DestroyFn,
) -> u64 {
    let mut id = 0;
    ffi_guard(|| {
        // SAFETY: caller contract; `as_mut` checks null.
        let bus = unsafe { bus.as_mut() }.ok_or(FfiError::NullPointer("bus"))?;
        let on_event = on_event.ok_or(FfiError::NullPointer("on_event"))?;
        bus.next_id += 1;
        id = bus.next_id;
        bus.subscribers.push(Subscriber {
            id,
            on_event,
            user_data,
            destroy,
        });
        Ok(())
    });
    id
}

/// Exported: `int rip_bus_unsubscribe(EventBus *, uint64_t id);` — runs `destroy`.
///
/// # Safety
///
/// `bus` must be null or live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rip_bus_unsubscribe(bus: *mut EventBus, id: u64) -> FfiStatus {
    ffi_guard(|| {
        // SAFETY: caller contract; `as_mut` checks null.
        let bus = unsafe { bus.as_mut() }.ok_or(FfiError::NullPointer("bus"))?;
        let at = bus
            .subscribers
            .iter()
            .position(|s| s.id == id)
            .ok_or_else(|| FfiError::InvalidArgument(format!("unknown subscription {id}")))?;
        drop(bus.subscribers.remove(at)); // `Subscriber::drop` runs `destroy`.
        Ok(())
    })
}

/// Exported: `size_t rip_bus_emit(EventBus *, int64_t event);` — returns the number of
/// subscribers notified (0 if `bus` is null).
///
/// # Safety
///
/// `bus` must be null or live, and no callback may free or re-enter the bus.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rip_bus_emit(bus: *mut EventBus, event: i64) -> usize {
    // SAFETY: caller contract; `as_ref` checks null.
    let Some(bus) = (unsafe { bus.as_ref() }) else {
        return 0;
    };
    for s in &bus.subscribers {
        // SAFETY: the subscriber promised `on_event` accepts its `user_data`.
        unsafe { (s.on_event)(s.user_data, event) };
    }
    bus.subscribers.len()
}

/// Exported: `void rip_bus_free(EventBus *);` — runs every remaining `destroy`.
///
/// # Safety
///
/// `bus` must be null or a live pointer from [`rip_bus_new`], not used afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rip_bus_free(bus: *mut EventBus) {
    if !bus.is_null() {
        // SAFETY: `bus` came from `Box::into_raw` in `rip_bus_new` and is freed once.
        drop(unsafe { Box::from_raw(bus) });
    }
}

/// Identifies a subscription on a [`Bus`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SubscriptionId(u64);

/// Safe wrapper over the C event bus: subscribes owned Rust closures.
///
/// The closure is boxed and leaked into `user_data`; the bus owns it from then on and
/// releases it through `destroy::<F>`. A `Box<dyn FnMut>` is a *fat* pointer and cannot
/// fit in `void *`, but `subscribe(boxed_dyn)` still works: `Box<dyn FnMut>` is itself an
/// `FnMut`, so it gets boxed again (`Box<Box<dyn FnMut>>`), and the outer pointer is thin.
#[derive(Debug)]
pub struct Bus(NonNull<EventBus>);

impl Bus {
    /// Creates an empty bus.
    #[must_use]
    pub fn new() -> Self {
        // A null handle from a C constructor means allocation failed: abort like `Box` does.
        let raw = rip_bus_new();
        Self(NonNull::new(raw).unwrap_or_else(|| handle_alloc_error(Layout::new::<EventBus>())))
    }

    /// Subscribes `f`. `'static` because the bus may keep it past any borrow.
    ///
    /// # Panics
    ///
    /// Panics if the library rejects the subscription, which it only does for null
    /// arguments, so never for a live bus.
    pub fn subscribe<F: FnMut(i64) + 'static>(&mut self, f: F) -> SubscriptionId {
        unsafe extern "C" fn on_event<F: FnMut(i64)>(user_data: *mut c_void, event: i64) {
            guard_callback((), || {
                // SAFETY: `user_data` is the `Box<F>` leaked in `subscribe`, still owned by
                // the bus (not yet destroyed), and the bus never calls it re-entrantly.
                let f = unsafe { &mut *user_data.cast::<F>() };
                f(event);
            });
        }

        unsafe extern "C" fn destroy<F>(user_data: *mut c_void) {
            guard_callback((), || {
                // SAFETY: rebuilds the `Box<F>` leaked in `subscribe`; the bus calls
                // `destroy` exactly once.
                drop(unsafe { Box::from_raw(user_data.cast::<F>()) });
            });
        }

        let user_data = Box::into_raw(Box::new(f)).cast::<c_void>();
        // SAFETY: live bus; `on_event::<F>`/`destroy::<F>` match `user_data`'s type.
        let id = unsafe {
            rip_bus_subscribe(
                self.0.as_ptr(),
                Some(on_event::<F>),
                user_data,
                Some(destroy::<F>),
            )
        };
        assert_ne!(id, 0, "rip_bus_subscribe failed on a live bus");
        SubscriptionId(id)
    }

    /// Unsubscribes; the closure is dropped. Returns `false` for an unknown id.
    pub fn unsubscribe(&mut self, id: SubscriptionId) -> bool {
        // SAFETY: live bus.
        let status = unsafe { rip_bus_unsubscribe(self.0.as_ptr(), id.0) };
        resume_pending_panic();
        status == FfiStatus::Ok
    }

    /// Delivers `event` to every subscriber; returns how many were notified.
    ///
    /// # Panics
    ///
    /// Re-raises the first panic from a subscriber, after every subscriber has run.
    pub fn emit(&mut self, event: i64) -> usize {
        // SAFETY: live bus; `&mut self` rules out re-entrant use from a callback.
        let delivered = unsafe { rip_bus_emit(self.0.as_ptr(), event) };
        resume_pending_panic();
        delivered
    }
}

impl Default for Bus {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for Bus {
    fn drop(&mut self) {
        // SAFETY: we own the bus and free it once; this runs every remaining `destroy`.
        unsafe { rip_bus_free(self.0.as_ptr()) };
        // A destroy that panicked while we are already unwinding must not double-panic.
        if !std::thread::panicking() {
            resume_pending_panic();
        }
    }
}

// ============================================================================
// 6. Unwinding: "C" aborts, "C-unwind" unwinds, catch_unwind converts
// ============================================================================

/// Exported with the `"C"` ABI: `int64_t rip_div_abort(int64_t a, int64_t b);`
///
/// Dividing by zero panics, and a panic cannot leave an `extern "C"` function: since Rust
/// 1.81 the process **aborts** with "panic in a function that cannot unwind". Abort is the
/// safe outcome: unwinding into C frames is undefined behaviour. (The unit tests check the
/// abort in a child process.)
#[unsafe(no_mangle)]
#[must_use]
pub const extern "C" fn rip_div_abort(a: i64, b: i64) -> i64 {
    a / b
}

/// Exported with the `"C-unwind"` ABI: a panic is allowed to unwind out of it.
///
/// Use `"C-unwind"` only when every caller up the stack is unwind-aware (Rust, or C++
/// built with exceptions). Rust callers can `catch_unwind` around it.
#[unsafe(no_mangle)]
#[must_use]
pub const extern "C-unwind" fn rip_div_unwind(a: i64, b: i64) -> i64 {
    a / b
}

/// Exported: `int rip_div_checked(int64_t a, int64_t b, int64_t *out);`
///
/// The correct design: the panic is caught at the boundary and reported as [`FfiStatus::Panic`] with a
/// last-error message. (Validating `b` up front would be better still; the panic path is
/// here to show the guard.)
///
/// # Safety
///
/// `out` must be null or valid for one `i64` write.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rip_div_checked(a: i64, b: i64, out: *mut i64) -> FfiStatus {
    ffi_guard(|| {
        if out.is_null() {
            return Err(FfiError::NullPointer("out"));
        }
        let q = rip_div_unwind(a, b);
        // SAFETY: non-null and writable (caller contract).
        unsafe { out.write(q) };
        Ok(())
    })
}

/// Exported: `void rip_describe_value(struct value v, ...)` — passes a tagged union **by
/// value**, validates it, and returns an **owned** description (null on a bad tag).
#[unsafe(no_mangle)]
#[must_use]
pub extern "C" fn rip_describe_value(v: CValue) -> *mut c_char {
    ffi_guard_ptr(|| {
        let text = match Value::try_from(v)? {
            Value::Int(i) => format!("int {i}"),
            Value::Float(f) => format!("float {f}"),
            Value::Bool(b) => format!("bool {b}"),
        };
        CString::new(text)
            .map(CString::into_raw)
            .map_err(|e| FfiError::InvalidArgument(e.to_string()))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::rc::Rc;
    use std::sync::{Mutex, MutexGuard};

    /// Serialises tests that read the global [`live_counters`] count.
    static COUNTER_LOCK: Mutex<()> = Mutex::new(());

    fn counter_lock() -> MutexGuard<'static, ()> {
        COUNTER_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Runs `f`, expecting a panic, and returns its message.
    fn panic_text(f: impl FnOnce()) -> String {
        let payload = panic::catch_unwind(AssertUnwindSafe(f)).expect_err("expected a panic");
        panic_message(&*payload).to_owned()
    }

    // ---- 1. ABI basics -------------------------------------------------------

    #[test]
    fn test_safe_extern_item_roundtrips_through_c_abi() {
        assert_eq!(add_via_c_abi(40, 2), 42);
        assert_eq!(add_via_c_abi(i32::MAX, 1), i32::MIN);
    }

    #[test]
    #[cfg_attr(miri, ignore)] // Miri cannot call libc's strlen through an extern block
    fn test_libc_strlen() {
        assert_eq!(c_strlen(c"hello"), 5);
        assert_eq!(c_strlen(c""), 0);
        assert_eq!(c_strlen(GREETING), GREETING.to_bytes().len());
    }

    #[test]
    fn test_nullable_function_pointer_is_one_pointer_wide() {
        assert_eq!(size_of::<NullableComparator>(), size_of::<usize>());
        assert_eq!(size_of::<VisitFn>(), size_of::<*const c_void>());
        assert_eq!(
            size_of::<Option<NonNull<Counter>>>(),
            size_of::<*mut Counter>()
        );
    }

    #[test]
    fn test_call_nullable_handles_null_and_real_comparators() {
        let (a, b) = (3_i32, 7_i32);
        let (pa, pb) = (
            (&raw const a).cast::<c_void>(),
            (&raw const b).cast::<c_void>(),
        );
        // SAFETY: both pointers are live `i32`s, matching `compare_ord::<i32>`.
        unsafe {
            assert_eq!(call_nullable(None, pa, pb), None);
            assert_eq!(call_nullable(Some(compare_ord::<i32>), pa, pb), Some(-1));
            assert_eq!(call_nullable(Some(compare_ord::<i32>), pb, pa), Some(1));
            assert_eq!(call_nullable(Some(compare_ord::<i32>), pa, pa), Some(0));
        }
    }

    // ---- 2. Layout -----------------------------------------------------------

    #[test]
    fn test_repr_c_keeps_order_and_pads() {
        assert_eq!(size_of::<Header>(), 12);
        assert!(size_of::<HeaderRust>() <= size_of::<Header>());
        assert_eq!(
            size_of::<HeaderRust>(),
            8,
            "rustc reorders to len, flags, tag"
        );
    }

    #[test]
    fn test_header_bytes_round_trip_with_zeroed_padding() {
        let h = Header {
            tag: 7,
            len: 0xDEAD_BEEF,
            flags: 0x0102,
        };
        let bytes = h.to_bytes();
        assert_eq!(&bytes[1..4], &[0, 0, 0], "padding after tag is zeroed");
        assert_eq!(&bytes[10..12], &[0, 0], "tail padding is zeroed");
        assert_eq!(Header::read_from(&bytes), h);
    }

    #[test]
    fn test_header_read_from_unaligned_buffer() {
        let h = Header {
            tag: 1,
            len: 99,
            flags: 3,
        };
        let mut storage = [0_u8; Header::SIZE + 1];
        storage[1..].copy_from_slice(&h.to_bytes());
        let unaligned: &[u8; Header::SIZE] = storage[1..].try_into().unwrap();
        assert_eq!(Header::read_from(unaligned), h);
    }

    #[test]
    fn test_packed_fields_read_without_references() {
        let p = PackedHeader::new(1, 0x0A0B_0C0D, 9);
        assert_eq!(size_of::<PackedHeader>(), 7);
        assert_eq!(p.len_by_copy(), 0x0A0B_0C0D);
        assert_eq!(p.len_by_raw_pointer(), 0x0A0B_0C0D);
        assert_eq!({ p.flags }, 9);
    }

    #[test]
    fn test_repr_transparent_matches_inner_abi() {
        assert_eq!(size_of::<Millis>(), size_of::<u64>());
        assert_eq!(align_of::<Millis>(), align_of::<u64>());
        assert_eq!(rip_millis_add(Millis(1), Millis(2)), Millis(3));
        assert_eq!(
            rip_millis_add(Millis(u64::MAX), Millis(1)),
            Millis(u64::MAX)
        );
    }

    #[test]
    fn test_status_from_raw_validates() {
        for status in [
            FfiStatus::Ok,
            FfiStatus::NullPointer,
            FfiStatus::InvalidUtf8,
            FfiStatus::InvalidArgument,
            FfiStatus::Panic,
        ] {
            assert_eq!(FfiStatus::from_raw(status as c_int), Some(status));
        }
        assert_eq!(FfiStatus::from_raw(-1), None);
        assert_eq!(FfiStatus::from_raw(5), None);
        assert_eq!(size_of::<FfiStatus>(), size_of::<c_int>());
    }

    #[test]
    fn test_tagged_union_round_trip() {
        for v in [
            Value::Int(-5),
            Value::Float(2.5),
            Value::Bool(true),
            Value::Bool(false),
        ] {
            assert_eq!(Value::try_from(CValue::from(v)), Ok(v));
        }
        assert_eq!(
            offset_of!(CValue, payload),
            8,
            "payload aligned to its i64/f64 field"
        );
        assert_eq!(size_of::<CValue>(), 16);
    }

    #[test]
    fn test_tagged_union_rejects_bad_tag_and_bad_bool() {
        let bad_tag = CValue {
            tag: 9,
            payload: ValuePayload { int: 0 },
        };
        assert!(matches!(
            Value::try_from(bad_tag),
            Err(FfiError::InvalidArgument(_))
        ));
        let bad_bool = CValue {
            tag: value_tag::BOOL,
            payload: ValuePayload { boolean: 2 },
        };
        assert!(matches!(
            Value::try_from(bad_bool),
            Err(FfiError::InvalidArgument(_))
        ));
        assert!(format!("{bad_tag:?}").contains("value tag 9"));
    }

    #[test]
    fn test_describe_value_by_value_returns_owned_string() {
        let raw = rip_describe_value(CValue::from(Value::Float(1.5)));
        // SAFETY: owned library string.
        let s = unsafe { RipString::from_raw(raw) }.expect("valid value");
        assert_eq!(s.to_str(), Ok("float 1.5"));

        let raw = rip_describe_value(CValue {
            tag: 77,
            payload: ValuePayload { int: 1 },
        });
        assert!(raw.is_null());
        assert_eq!(
            last_error().as_deref(),
            Some("invalid argument: value tag 77")
        );
    }

    // ---- 3. Strings and buffers ------------------------------------------------

    #[test]
    fn test_greeting_is_borrowed_static() {
        // SAFETY: `rip_greeting` returns a pointer to a `'static` C string.
        let s = unsafe { CStr::from_ptr(rip_greeting()) };
        assert_eq!(s, GREETING);
        assert_eq!(
            rip_greeting(),
            rip_greeting(),
            "same static, never reallocated"
        );
    }

    #[test]
    fn test_copy_greeting_query_then_fill() {
        // SAFETY: null/0 is the documented length query.
        let needed = unsafe { rip_copy_greeting(ptr::null_mut(), 0) };
        assert_eq!(needed, GREETING.to_bytes().len());
        let mut buf = vec![0x7F_u8; needed + 1];
        // SAFETY: `buf` has `buf.len()` writable bytes.
        let n = unsafe { rip_copy_greeting(buf.as_mut_ptr().cast(), buf.len()) };
        assert_eq!(n, needed);
        assert_eq!(CStr::from_bytes_until_nul(&buf).unwrap(), GREETING);
    }

    #[test]
    fn test_copy_greeting_truncates_and_always_terminates() {
        let mut buf = [0x7F_u8; 6];
        // SAFETY: `buf` has 6 writable bytes.
        let n = unsafe { rip_copy_greeting(buf.as_mut_ptr().cast(), buf.len()) };
        assert!(n >= buf.len(), "return value signals truncation");
        assert_eq!(&buf, b"hello\0");

        let mut one = [0x7F_u8; 1];
        // SAFETY: `one` has 1 writable byte.
        unsafe { rip_copy_greeting(one.as_mut_ptr().cast(), 1) };
        assert_eq!(one, [0]);
    }

    #[test]
    fn test_read_c_string_retries_when_source_grows() {
        let sources = [c"ab", c"abcdef", c"abcdef"];
        let mut call = 0;
        let s = read_c_string(|buf, cap| {
            let src = sources[call.min(sources.len() - 1)];
            call += 1;
            // SAFETY: `read_c_string` passes null or a `cap`-byte buffer.
            unsafe { copy_to_c_buffer(src, buf, cap) }
        });
        assert_eq!(s.as_c_str(), c"abcdef");
        assert_eq!(call, 3, "query, short fill, retry");
    }

    #[test]
    fn test_cstring_rejects_interior_nul() {
        let err = CString::new("a\0b").unwrap_err();
        assert_eq!(err.nul_position(), 1);
        assert!(shout("a\0b").is_err());
    }

    #[test]
    fn test_from_bytes_until_nul_bounds_untrusted_buffers() {
        let buf = *b"name\0garbage";
        assert_eq!(CStr::from_bytes_until_nul(&buf).unwrap(), c"name");
        assert!(CStr::from_bytes_until_nul(b"no terminator").is_err());
    }

    #[test]
    fn test_slice_from_c_accepts_null_when_empty() {
        // SAFETY: `len == 0`, so the pointer is never read.
        let empty: &[i64] = unsafe { slice_from_c(ptr::null(), 0, "p") }.unwrap();
        assert!(empty.is_empty());
        // SAFETY: null with `len > 0` is rejected before any read.
        let err = unsafe { slice_from_c::<i64>(ptr::null(), 3, "p") }.unwrap_err();
        assert_eq!(err, FfiError::NullPointer("p"));
    }

    #[test]
    fn test_slice_from_c_rejects_misaligned() {
        let words = [0_u64; 2];
        #[allow(clippy::cast_ptr_alignment)] // misaligned on purpose
        let misaligned = words.as_ptr().cast::<u8>().wrapping_add(1).cast::<u64>();
        // SAFETY: rejected by the alignment check before any read.
        let err = unsafe { slice_from_c(misaligned, 1, "p") }.unwrap_err();
        assert!(matches!(err, FfiError::InvalidArgument(_)));
    }

    #[test]
    fn test_point_parse_success_and_out_param_untouched_on_error() {
        let mut p = Point { x: -1.0, y: -1.0 };
        // SAFETY: valid C string and out-pointer.
        let ok = unsafe { rip_point_parse(c" 1.5 , -2 ".as_ptr(), &raw mut p) };
        assert_eq!(ok, FfiStatus::Ok);
        assert_eq!(p, Point { x: 1.5, y: -2.0 });

        let sentinel = Point { x: 9.0, y: 9.0 };
        let mut q = sentinel;
        // SAFETY: as above.
        let bad = unsafe { rip_point_parse(c"1.5;2".as_ptr(), &raw mut q) };
        assert_eq!(bad, FfiStatus::InvalidArgument);
        assert_eq!(q, sentinel, "out is written only on success");
        assert!(last_error().unwrap().contains("expected \"x,y\""));
    }

    #[test]
    fn test_point_parse_null_and_utf8_errors() {
        let mut p = Point::default();
        // SAFETY: null arguments are checked by the callee.
        unsafe {
            assert_eq!(
                rip_point_parse(ptr::null(), &raw mut p),
                FfiStatus::NullPointer
            );
            assert_eq!(last_error().as_deref(), Some("null pointer: s"));
            assert_eq!(
                rip_point_parse(c"1,2".as_ptr(), ptr::null_mut()),
                FfiStatus::NullPointer
            );
            assert_eq!(last_error().as_deref(), Some("null pointer: out"));
            let not_utf8 = [0xFF_u8, b',', b'1', 0];
            assert_eq!(
                rip_point_parse(not_utf8.as_ptr().cast(), &raw mut p),
                FfiStatus::InvalidUtf8
            );
        }
    }

    #[test]
    fn test_points_centroid() {
        let pts = [Point { x: 0.0, y: 0.0 }, Point { x: 4.0, y: 2.0 }];
        let mut out = Point::default();
        // SAFETY: `pts` is a live array; `out` is writable.
        unsafe {
            assert_eq!(
                rip_points_centroid(pts.as_ptr(), pts.len(), &raw mut out),
                FfiStatus::Ok
            );
            assert_eq!(out, Point { x: 2.0, y: 1.0 });
            assert_eq!(
                rip_points_centroid(ptr::null(), 0, &raw mut out),
                FfiStatus::InvalidArgument
            );
            assert_eq!(
                rip_points_centroid(ptr::null(), 2, &raw mut out),
                FfiStatus::NullPointer
            );
        }
    }

    // ---- 4. Ownership transfer -------------------------------------------------

    #[test]
    fn test_counter_raw_lifecycle() {
        let _guard = counter_lock();
        let before = live_counters();
        // SAFETY: the raw API is used exactly as documented; freed once at the end.
        unsafe {
            let c = rip_counter_new(c"reqs".as_ptr());
            assert!(!c.is_null());
            assert_eq!(live_counters(), before + 1);
            assert_eq!(rip_counter_increment(c, 5), FfiStatus::Ok);
            assert_eq!(rip_counter_increment(c, u64::MAX), FfiStatus::Ok);
            let mut n = 0;
            assert_eq!(rip_counter_get(c, &raw mut n), FfiStatus::Ok);
            assert_eq!(n, u64::MAX, "saturates");
            assert_eq!(CStr::from_ptr(rip_counter_name(c)), c"reqs");
            rip_counter_free(c);
        }
        assert_eq!(live_counters(), before);
    }

    #[test]
    fn test_counter_null_handles() {
        // SAFETY: null is accepted everywhere and reported, never dereferenced.
        unsafe {
            assert_eq!(
                rip_counter_increment(ptr::null_mut(), 1),
                FfiStatus::NullPointer
            );
            assert_eq!(
                rip_counter_get(ptr::null(), ptr::null_mut()),
                FfiStatus::NullPointer
            );
            assert!(rip_counter_name(ptr::null()).is_null());
            assert!(rip_counter_describe(ptr::null()).is_null());
            rip_counter_free(ptr::null_mut());
            rip_string_free(ptr::null_mut());
            rip_u64_buffer_free(ptr::null_mut(), 0);
            assert!(rip_counter_new(ptr::null()).is_null());
            assert!(rip_counter_new(c"".as_ptr()).is_null());
        }
        assert_eq!(
            last_error().as_deref(),
            Some("invalid argument: empty counter name")
        );
    }

    #[test]
    fn test_counter_handle_raii_frees_on_drop() {
        let _guard = counter_lock();
        let before = live_counters();
        {
            let mut c = CounterHandle::new("jobs").unwrap();
            c.increment(3);
            c.increment(4);
            assert_eq!(c.get(), 7);
            assert_eq!(c.name(), c"jobs");
            assert_eq!(c.describe(), "jobs=7");
            assert_eq!(live_counters(), before + 1);
        }
        assert_eq!(live_counters(), before);
        assert!(CounterHandle::new("").is_none());
        assert!(CounterHandle::new("a\0b").is_none());
        assert_eq!(live_counters(), before, "failed constructors leak nothing");
    }

    #[test]
    fn test_counter_handle_into_raw_and_back() {
        let _guard = counter_lock();
        let before = live_counters();
        let mut c = CounterHandle::new("x").unwrap();
        c.increment(1);
        let raw = c.into_raw();
        assert_eq!(live_counters(), before + 1, "into_raw does not free");
        // SAFETY: `raw` is live and owned by nothing else after `into_raw`.
        unsafe { assert_eq!(rip_counter_increment(raw, 1), FfiStatus::Ok) };
        // SAFETY: as above; ownership returns to Rust.
        let back = unsafe { CounterHandle::from_raw(raw) }.unwrap();
        assert_eq!(back.get(), 2);
        drop(back);
        assert_eq!(live_counters(), before);
    }

    #[test]
    fn test_owned_strings_from_library() {
        assert_eq!(shout("hello, ffi").unwrap(), "HELLO, FFI");
        assert_eq!(shout("").unwrap(), "");
        assert_eq!(shout("straße").unwrap(), "STRASSE", "length can change");
        // SAFETY: null is reported, not dereferenced.
        assert!(unsafe { rip_shout(ptr::null()) }.is_null());
    }

    #[test]
    fn test_fibonacci_buffer_transfer() {
        assert_eq!(fibonacci(8), vec![0, 1, 1, 2, 3, 5, 8, 13]);
        assert_eq!(fibonacci(1), vec![0]);
        assert!(fibonacci(0).is_empty());
        assert_eq!(*fibonacci(100).last().unwrap(), u64::MAX, "saturates");
        // SAFETY: null out-parameter is checked.
        assert!(unsafe { rip_fibonacci(3, ptr::null_mut()) }.is_null());
    }

    // ---- 5. Callbacks ------------------------------------------------------------

    #[test]
    fn test_for_each_closure_through_user_data() {
        let mut seen = Vec::new();
        let visited = for_each_via_c(&[1, 2, 3], |v| {
            seen.push(v);
            ControlFlow::Continue(())
        });
        assert_eq!(visited, 3);
        assert_eq!(seen, [1, 2, 3]);
    }

    #[test]
    fn test_for_each_early_stop() {
        let mut sum = 0;
        let visited = for_each_via_c(&[5, 6, 7, 8], |v| {
            sum += v;
            if sum > 10 {
                ControlFlow::Break(())
            } else {
                ControlFlow::Continue(())
            }
        });
        assert_eq!((visited, sum), (2, 11));
        assert_eq!(for_each_via_c(&[], |_| ControlFlow::Continue(())), 0);
    }

    #[test]
    fn test_for_each_rejects_null_callback() {
        let mut visited = 99;
        // SAFETY: the null callback is rejected before use.
        let status =
            unsafe { rip_for_each([1_i64].as_ptr(), 1, None, ptr::null_mut(), &raw mut visited) };
        assert_eq!(status, FfiStatus::NullPointer);
        assert_eq!(visited, 99, "out-parameter untouched on error");
    }

    #[test]
    fn test_callback_panic_is_caught_in_trampoline_and_resumed() {
        let mut calls = 0;
        let msg = panic_text(|| {
            for_each_via_c(&[1, 2, 3], |v| {
                calls += 1;
                assert!(v != 2, "boom at {v}");
                ControlFlow::Continue(())
            });
        });
        assert_eq!(msg, "boom at 2");
        assert_eq!(calls, 2, "trampoline told C to stop after the panic");
        assert!(
            PENDING_PANIC.with_borrow(Option::is_none),
            "panic was consumed"
        );
    }

    #[test]
    #[cfg_attr(miri, ignore)] // Miri cannot call libc's qsort
    fn test_c_sort_with_monomorphised_comparator() {
        let mut ints = [5, -1, 3, 3, 0, i32::MIN, i32::MAX];
        c_sort(&mut ints);
        assert_eq!(ints, [i32::MIN, -1, 0, 3, 3, 5, i32::MAX]);

        let mut strings = vec!["pear".to_owned(), "fig".to_owned(), "apple".to_owned()];
        c_sort(&mut strings);
        assert_eq!(
            strings,
            ["apple", "fig", "pear"],
            "heap-owning values move via memcpy"
        );

        let mut tuples = [(2, 'b'), (1, 'z'), (2, 'a')];
        c_sort(&mut tuples);
        assert_eq!(tuples, [(1, 'z'), (2, 'a'), (2, 'b')]);
    }

    #[test]
    fn test_c_sort_trivial_inputs_skip_qsort() {
        let mut empty: [i32; 0] = [];
        c_sort(&mut empty);
        let mut one = [1];
        c_sort(&mut one);
        assert_eq!(one, [1]);
        let mut zsts = [(), ()];
        c_sort(&mut zsts);
    }

    /// Records its drop so tests can check the bus releases `user_data`.
    struct DropFlag(Rc<Cell<u32>>);

    impl Drop for DropFlag {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }

    #[test]
    fn test_bus_delivers_to_all_subscribers() {
        let log = Rc::new(RefCell::new(Vec::new()));
        let mut bus = Bus::new();
        let (a, b) = (Rc::clone(&log), Rc::clone(&log));
        bus.subscribe(move |e| a.borrow_mut().push(('a', e)));
        bus.subscribe(move |e| b.borrow_mut().push(('b', e)));
        assert_eq!(bus.emit(1), 2);
        assert_eq!(*log.borrow(), [('a', 1), ('b', 1)]);
    }

    #[test]
    fn test_bus_unsubscribe_runs_destroy_once() {
        let drops = Rc::new(Cell::new(0));
        let hits = Rc::new(Cell::new(0));
        let mut bus = Bus::new();
        let (flag, h) = (DropFlag(Rc::clone(&drops)), Rc::clone(&hits));
        let id = bus.subscribe(move |_| {
            let _keep = &flag;
            h.set(h.get() + 1);
        });
        bus.emit(0);
        assert!(bus.unsubscribe(id));
        assert_eq!(drops.get(), 1);
        assert!(
            !bus.unsubscribe(id),
            "second unsubscribe is an error, not a double free"
        );
        assert_eq!(drops.get(), 1);
        assert_eq!(bus.emit(0), 0);
        assert_eq!(hits.get(), 1);
    }

    #[test]
    fn test_bus_drop_destroys_remaining_closures() {
        let drops = Rc::new(Cell::new(0));
        {
            let mut bus = Bus::new();
            for _ in 0..3 {
                let flag = DropFlag(Rc::clone(&drops));
                bus.subscribe(move |_| {
                    let _keep = &flag;
                });
            }
            assert_eq!(drops.get(), 0);
        }
        assert_eq!(drops.get(), 3);
    }

    #[test]
    fn test_bus_accepts_boxed_dyn_closure_via_double_box() {
        let total = Rc::new(Cell::new(0));
        let t = Rc::clone(&total);
        let boxed: Box<dyn FnMut(i64)> = Box::new(move |e| t.set(t.get() + e));
        let mut bus = Bus::default();
        bus.subscribe(boxed); // stored as Box<Box<dyn FnMut(i64)>>: a thin outer pointer
        bus.emit(20);
        bus.emit(22);
        assert_eq!(total.get(), 42);
    }

    #[test]
    fn test_bus_subscriber_panic_resumes_after_all_delivered() {
        let after = Rc::new(Cell::new(false));
        let mut bus = Bus::new();
        bus.subscribe(|e| panic!("subscriber failed on {e}"));
        let a = Rc::clone(&after);
        bus.subscribe(move |_| a.set(true));
        let msg = panic_text(|| {
            bus.emit(7);
        });
        assert_eq!(msg, "subscriber failed on 7");
        assert!(after.get(), "later subscribers still ran");
    }

    #[test]
    fn test_raw_bus_rejects_null_arguments() {
        // SAFETY: null arguments are checked by the callee.
        unsafe {
            assert_eq!(
                rip_bus_subscribe(ptr::null_mut(), None, ptr::null_mut(), None),
                0
            );
            let bus = rip_bus_new();
            assert_eq!(rip_bus_subscribe(bus, None, ptr::null_mut(), None), 0);
            assert_eq!(last_error().as_deref(), Some("null pointer: on_event"));
            assert_eq!(rip_bus_emit(ptr::null_mut(), 1), 0);
            assert_eq!(
                rip_bus_unsubscribe(ptr::null_mut(), 1),
                FfiStatus::NullPointer
            );
            rip_bus_free(bus);
            rip_bus_free(ptr::null_mut());
        }
    }

    // ---- 6. Unwinding --------------------------------------------------------------

    #[test]
    fn test_ffi_guard_maps_ok_err_and_panic() {
        assert_eq!(ffi_guard(|| Ok(())), FfiStatus::Ok);
        assert_eq!(
            ffi_guard(|| Err(FfiError::InvalidUtf8)),
            FfiStatus::InvalidUtf8
        );
        assert_eq!(last_error().as_deref(), Some("invalid UTF-8"));
        assert_eq!(ffi_guard(|| panic!("kaboom {}", 1)), FfiStatus::Panic);
        assert_eq!(last_error().as_deref(), Some("panic: kaboom 1"));
        assert_eq!(
            ffi_guard(|| std::panic::panic_any(42_u8)),
            FfiStatus::Panic,
            "non-string payloads are caught too"
        );
        assert_eq!(
            last_error().as_deref(),
            Some("panic: <non-string panic payload>")
        );
    }

    #[test]
    fn test_last_error_is_thread_local() {
        let _ = ffi_guard(|| Err(FfiError::NullPointer("here")));
        let other = std::thread::spawn(last_error).join().unwrap();
        assert_eq!(other, None);
        assert_eq!(last_error().as_deref(), Some("null pointer: here"));
    }

    #[test]
    fn test_set_last_error_survives_interior_nul() {
        set_last_error("a\0b");
        assert_eq!(last_error().as_deref(), Some("a\u{FFFD}b"));
    }

    #[test]
    fn test_c_unwind_panic_can_be_caught_by_rust() {
        assert_eq!(rip_div_unwind(7, 2), 3);
        let msg = panic_text(|| {
            let _ = rip_div_unwind(1, 0);
        });
        assert_eq!(msg, "attempt to divide by zero");
    }

    #[test]
    fn test_div_checked_converts_panic_to_status() {
        let mut q = 0;
        // SAFETY: `q` is a writable `i64`.
        unsafe {
            assert_eq!(rip_div_checked(9, 3, &raw mut q), FfiStatus::Ok);
            assert_eq!(q, 3);
            assert_eq!(rip_div_checked(1, 0, &raw mut q), FfiStatus::Panic);
            assert_eq!(q, 3, "out untouched");
            assert_eq!(
                last_error().as_deref(),
                Some("panic: attempt to divide by zero")
            );
            assert_eq!(rip_div_checked(i64::MIN, -1, &raw mut q), FfiStatus::Panic);
            assert_eq!(
                rip_div_checked(1, 1, ptr::null_mut()),
                FfiStatus::NullPointer
            );
        }
        assert_eq!(rip_div_abort(9, 3), 3);
    }

    const ABORT_CHILD_ENV: &str = "RIP_FFI_ABORT_CHILD";

    /// Child half of the abort test: does nothing unless spawned by the parent test.
    #[test]
    fn test_abort_child_process() {
        if std::env::var_os(ABORT_CHILD_ENV).is_some() {
            let zero = std::hint::black_box(0);
            let _ = rip_div_abort(1, zero);
            unreachable!("a panic in an extern \"C\" fn must abort");
        }
    }

    #[test]
    #[cfg_attr(miri, ignore)] // Miri cannot spawn processes
    fn test_panic_in_extern_c_aborts_the_process() {
        let exe = std::env::current_exe().unwrap();
        let output = std::process::Command::new(exe)
            .args([
                "--exact",
                "fundamentals::ffi::tests::test_abort_child_process",
                "--nocapture",
            ])
            .env(ABORT_CHILD_ENV, "1")
            .output()
            .unwrap();
        assert!(!output.status.success(), "child must not exit cleanly");
        #[cfg(unix)]
        {
            use std::os::unix::process::ExitStatusExt;
            assert_eq!(
                output.status.signal(),
                Some(6),
                "SIGABRT, not a test failure exit"
            );
        }
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("panic in a function that cannot unwind"),
            "unexpected child stderr: {stderr}"
        );
    }
}
