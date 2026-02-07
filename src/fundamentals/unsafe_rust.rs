//! # Unsafe Rust Patterns
//!
//! Understanding and safely using Rust's unsafe superpowers.
//! Master raw pointers, FFI, and when unsafe is necessary.
//!
//! **WARNING:** Unsafe code requires extra care. Always document safety invariants.

// ============================================================================
// The Five Unsafe Superpowers
// ============================================================================

#[allow(dead_code)]
fn demonstrate_unsafe_superpowers() {
    // 1. Dereference raw pointers
    let x = 5;
    let ptr = &x as *const i32;
    unsafe {
        println!("Value: {}", *ptr);
    }

    // 2. Call unsafe functions
    unsafe {
        dangerous_function();
    }

    // 3. Access or modify mutable static variables (no direct references in Rust 2024)
    unsafe {
        COUNTER += 1;
        let count = COUNTER; // Read into local to avoid creating reference
        println!("Counter: {count}");
    }

    // 4. Implement unsafe traits
    // (See UnsafeCell example below)

    // 5. Access fields of unions
    let u = MyUnion { i: 42 };
    unsafe {
        println!("Union value: {}", u.i);
    }
}

unsafe fn dangerous_function() {
    println!("This is unsafe!");
}

static mut COUNTER: i32 = 0;

union MyUnion {
    i: i32,
    f: f32,
}

// ============================================================================
// Raw Pointers
// ============================================================================

#[allow(dead_code)]
fn demonstrate_raw_pointers() {
    let mut x = 42;

    // Creating raw pointers is safe
    let ptr_const: *const i32 = &x;
    let ptr_mut: *mut i32 = &mut x;

    // Dereferencing is unsafe
    unsafe {
        println!("const ptr: {}", *ptr_const);
        *ptr_mut = 100;
        println!("after mutation: {}", *ptr_mut);
    }

    // Null pointers
    let null_ptr: *const i32 = std::ptr::null();
    println!("Is null: {}", null_ptr.is_null());

    // Pointer arithmetic (unsafe!)
    let arr = [1, 2, 3, 4, 5];
    let ptr = arr.as_ptr();

    unsafe {
        println!("First: {}", *ptr);
        println!("Second: {}", *ptr.add(1)); // Pointer arithmetic
        println!("Third: {}", *ptr.offset(2)); // Same as add
    }

    // Casting between pointer types
    let ptr_i32: *const i32 = &42;
    let ptr_u8: *const u8 = ptr_i32.cast();
    let _ = ptr_u8;
}

/// Creating raw pointers from references.
#[allow(dead_code)]
fn raw_pointers_from_references() {
    let x = 5;
    let y = 10;

    // Both mutable and immutable raw pointers (allowed!)
    let ptr1 = &x as *const i32;
    let ptr2 = &x as *const i32;

    // This is fine - raw pointers ignore aliasing rules
    unsafe {
        println!("{}, {}", *ptr1, *ptr2);
    }

    let _ = y;
}

// ============================================================================
// Unsafe Functions
// ============================================================================

/// Unsafe function - caller must uphold safety invariants.
///
/// # Safety
///
/// - `ptr` must be valid and properly aligned
/// - `ptr` must point to an initialized `i32`
/// - The memory `ptr` points to must not be accessed through any other pointer
///   for the duration of the lifetime returned
unsafe fn read_value(ptr: *const i32) -> i32 {
    // SAFETY: Caller guarantees ptr is valid, aligned, and initialized
    unsafe { *ptr }
}

/// Safe wrapper around unsafe function.
#[allow(dead_code)]
fn safe_read_value(value: &i32) -> i32 {
    // Safe because we have a valid reference
    unsafe { read_value(value as *const i32) }
}

/// slice::from_raw_parts - classic unsafe function.
#[allow(dead_code)]
fn demonstrate_from_raw_parts() {
    let values = vec![1, 2, 3, 4, 5];
    let ptr = values.as_ptr();
    let len = values.len();

    // SAFETY: ptr is valid, points to len contiguous initialized values,
    // and values lives long enough
    let slice = unsafe { std::slice::from_raw_parts(ptr, len) };

    println!("Slice: {slice:?}");
}

// ============================================================================
// FFI (Foreign Function Interface)
// ============================================================================

// Calling C functions from Rust (extern blocks must be unsafe in Rust 2024).
unsafe extern "C" {
    // These would be defined in a C library
    // fn abs(input: i32) -> i32;
    // fn strlen(s: *const u8) -> usize;
}

/// Exposing Rust functions to C (no_mangle is unsafe in Rust 2024).
#[unsafe(no_mangle)]
pub extern "C" fn rust_function(x: i32) -> i32 {
    x * 2
}

/// C-compatible struct.
#[repr(C)]
struct CPoint {
    x: i32,
    y: i32,
}

/// String handling across FFI.
#[allow(dead_code)]
fn demonstrate_ffi_strings() {
    use std::ffi::{CStr, CString};

    // Rust string to C string
    let rust_str = "Hello, C!";
    let c_string = CString::new(rust_str).expect("CString::new failed");
    let c_ptr = c_string.as_ptr();

    // C string to Rust string (unsafe!)
    unsafe {
        let back_to_rust = CStr::from_ptr(c_ptr);
        let rust_string = back_to_rust.to_str().expect("Invalid UTF-8");
        println!("From C: {rust_string}");
    }
}

/// Opaque C types.
#[repr(C)]
struct OpaqueType {
    _private: [u8; 0],
}

// ============================================================================
// Implementing Unsafe Traits
// ============================================================================

/// Send - type can be transferred across thread boundaries.
/// Sync - type can be shared between threads.
///
/// These are unsafe traits because implementing them incorrectly can cause data races.

struct MyType {
    data: *mut i32, // Raw pointer - not Send/Sync by default
}

// SAFETY: We ensure exclusive access through other means
unsafe impl Send for MyType {}
unsafe impl Sync for MyType {}

/// UnsafeCell - interior mutability primitive.
use std::cell::UnsafeCell;

struct MyCell<T> {
    value: UnsafeCell<T>,
}

impl<T> MyCell<T> {
    fn new(value: T) -> Self {
        MyCell {
            value: UnsafeCell::new(value),
        }
    }

    fn get(&self) -> &T {
        // SAFETY: We're returning an immutable reference, which is safe
        // as long as caller doesn't mutate through another reference
        unsafe { &*self.value.get() }
    }

    fn set(&self, value: T) {
        // SAFETY: We have exclusive access
        unsafe {
            *self.value.get() = value;
        }
    }
}

// ============================================================================
// Common Unsafe Patterns
// ============================================================================

/// Transmute - reinterpret bits (very dangerous!).
#[allow(dead_code)]
fn demonstrate_transmute() {
    // Transmute changes type without changing bits
    let x: u32 = 42;

    // SAFETY: u32 and i32 have same size and layout
    let y: i32 = unsafe { std::mem::transmute(x) };
    println!("Transmuted: {y}");

    // Common use: transmute lifetime (dangerous!)
    // fn extend_lifetime<'a, 'b>(r: &'a str) -> &'b str {
    //     unsafe { std::mem::transmute(r) }
    // }
    // DON'T DO THIS - almost always wrong!

    // Safer alternative: use as casts when possible
    let safe_cast = x as i32;
    let _ = safe_cast;
}

/// Uninitialized memory with MaybeUninit.
#[allow(dead_code)]
fn demonstrate_maybe_uninit() {
    use std::mem::MaybeUninit;

    // Create uninitialized buffer
    let mut buffer: [MaybeUninit<i32>; 10] = unsafe { MaybeUninit::uninit().assume_init() };

    // Initialize each element
    for i in 0..10 {
        buffer[i] = MaybeUninit::new(i as i32);
    }

    // SAFETY: All elements are now initialized
    let initialized: [i32; 10] = unsafe { std::mem::transmute(buffer) };

    println!("Initialized: {initialized:?}");

    // Better pattern: transmute_copy
    let mut data = MaybeUninit::<i32>::uninit();
    data.write(42);

    // SAFETY: We just initialized it
    let value = unsafe { data.assume_init() };
    println!("Value: {value}");
}

/// Bypassing borrow checker (carefully!).
#[allow(dead_code)]
fn demonstrate_mutable_aliasing() {
    let mut data = vec![1, 2, 3, 4, 5];

    // Get two mutable references to different parts (safe!)
    let ptr = data.as_mut_ptr();

    unsafe {
        let first_half = std::slice::from_raw_parts_mut(ptr, 2);
        let second_half = std::slice::from_raw_parts_mut(ptr.add(2), 3);

        first_half[0] = 10;
        second_half[0] = 20;
    }

    println!("Data: {data:?}"); // [10, 2, 20, 4, 5]
}

/// Split at mut - safe abstraction over unsafe code.
#[allow(dead_code)]
fn safe_split_at_mut<T>(slice: &mut [T], mid: usize) -> (&mut [T], &mut [T]) {
    assert!(mid <= slice.len());

    let ptr = slice.as_mut_ptr();
    let len = slice.len();

    unsafe {
        (
            std::slice::from_raw_parts_mut(ptr, mid),
            std::slice::from_raw_parts_mut(ptr.add(mid), len - mid),
        )
    }
}

// ============================================================================
// Safety Invariants and Documentation
// ============================================================================

/// Vector with unsafe internal implementation.
struct MyVec<T> {
    ptr: *mut T,
    len: usize,
    capacity: usize,
}

impl<T> MyVec<T> {
    /// Creates a new empty vector.
    fn new() -> Self {
        MyVec {
            ptr: std::ptr::NonNull::dangling().as_ptr(),
            len: 0,
            capacity: 0,
        }
    }

    /// Pushes a value to the end.
    ///
    /// # Safety Invariants
    ///
    /// - ptr must be valid for len elements
    /// - capacity >= len
    /// - ptr must be properly aligned
    fn push(&mut self, value: T) {
        if self.len == self.capacity {
            self.grow();
        }

        unsafe {
            // SAFETY: We just ensured capacity > len
            std::ptr::write(self.ptr.add(self.len), value);
        }

        self.len += 1;
    }

    fn grow(&mut self) {
        // Simplified growth logic
        let new_capacity = if self.capacity == 0 {
            1
        } else {
            self.capacity * 2
        };

        let new_layout = std::alloc::Layout::array::<T>(new_capacity).unwrap();

        let new_ptr = unsafe { std::alloc::alloc(new_layout) as *mut T };

        if self.capacity > 0 {
            unsafe {
                std::ptr::copy_nonoverlapping(self.ptr, new_ptr, self.len);
                std::alloc::dealloc(
                    self.ptr as *mut u8,
                    std::alloc::Layout::array::<T>(self.capacity).unwrap(),
                );
            }
        }

        self.ptr = new_ptr;
        self.capacity = new_capacity;
    }
}

impl<T> Drop for MyVec<T> {
    fn drop(&mut self) {
        if self.capacity > 0 {
            unsafe {
                // Drop all elements
                for i in 0..self.len {
                    std::ptr::drop_in_place(self.ptr.add(i));
                }

                // Deallocate memory
                std::alloc::dealloc(
                    self.ptr as *mut u8,
                    std::alloc::Layout::array::<T>(self.capacity).unwrap(),
                );
            }
        }
    }
}

// ============================================================================
// Testing Unsafe Code
// ============================================================================

#[cfg(test)]
mod unsafe_tests {
    use super::*;

    #[test]
    fn test_safe_read_value() {
        let x = 42;
        assert_eq!(safe_read_value(&x), 42);
    }

    #[test]
    fn test_split_at_mut() {
        let mut arr = [1, 2, 3, 4, 5];
        let (left, right) = safe_split_at_mut(&mut arr, 2);

        assert_eq!(left, &[1, 2]);
        assert_eq!(right, &[3, 4, 5]);

        left[0] = 10;
        right[0] = 30;

        assert_eq!(arr, [10, 2, 30, 4, 5]);
    }

    #[test]
    fn test_my_vec() {
        let mut vec = MyVec::new();
        vec.push(1);
        vec.push(2);
        vec.push(3);

        assert_eq!(vec.len, 3);
    }
}

// ============================================================================
// Miri - Undefined Behavior Detection
// ============================================================================

/// Miri is a tool to detect undefined behavior in unsafe code.
///
/// Run with:
/// ```bash
/// cargo +nightly miri test
/// ```
///
/// Miri catches:
/// - Use after free
/// - Double free
/// - Invalid pointer arithmetic
/// - Uninitialized memory reads
/// - Data races
/// - Violating aliasing rules
///
/// Example of UB that Miri would catch:
/// ```rust,ignore
/// fn ub_example() {
///     let mut x = 5;
///     let ptr = &mut x as *mut i32;
///
///     unsafe {
///         *ptr = 10;
///         drop(x); // Use after moving - UB!
///     }
/// }
/// ```
#[allow(dead_code)]
const MIRI_GUIDE: &str = "Run: cargo +nightly miri test";

// ============================================================================
// When to Use Unsafe
// ============================================================================

/// Guidelines for using unsafe:
///
/// **Valid reasons:**
/// - FFI with C libraries
/// - Performance-critical code after profiling
/// - Implementing core abstractions (Vec, Box, Rc, etc.)
/// - Zero-copy deserialization
/// - Memory-mapped I/O
///
/// **Invalid reasons:**
/// - Avoiding the borrow checker (learn lifetimes instead!)
/// - "It's faster" (profile first!)
/// - Convenience without safety justification
///
/// **Safety checklist:**
/// 1. Document safety invariants
/// 2. Minimize unsafe surface area
/// 3. Provide safe wrappers
/// 4. Test thoroughly (including with Miri)
/// 5. Peer review unsafe code
/// 6. Consider alternatives first
///
/// **Minimizing unsafe:**
/// ```rust
/// // Bad: large unsafe block
/// unsafe {
///     // 100 lines of code
/// }
///
/// // Good: minimal unsafe blocks
/// let value = unsafe { *ptr };
/// process(value);
/// ```
#[allow(dead_code)]
const UNSAFE_GUIDELINES: &str = "See module docs";

// ============================================================================
// Practical Patterns
// ============================================================================

/// Pattern 1: Safe wrapper around unsafe operation.
struct Buffer {
    data: Vec<u8>,
}

impl Buffer {
    fn new(size: usize) -> Self {
        Buffer {
            data: vec![0; size],
        }
    }

    /// Safe interface - validates index.
    fn write_u32(&mut self, index: usize, value: u32) -> Result<(), String> {
        if index + 4 > self.data.len() {
            return Err("Index out of bounds".to_string());
        }

        // SAFETY: We just checked bounds
        unsafe {
            let ptr = self.data.as_mut_ptr().add(index) as *mut u32;
            *ptr = value;
        }

        Ok(())
    }
}

/// Pattern 2: NonNull for non-null raw pointers.
use std::ptr::NonNull;

struct LinkedNode {
    value: i32,
    next: Option<NonNull<LinkedNode>>,
}

/// Pattern 3: PhantomData for unused lifetime parameters.
use std::marker::PhantomData;

struct Iter<'a, T> {
    ptr: *const T,
    end: *const T,
    _marker: PhantomData<&'a T>,
}

/// Pattern 4: Pin for self-referential structs.
#[allow(dead_code)]
fn demonstrate_pin() {
    use std::pin::Pin;

    struct SelfReferential {
        data: String,
        ptr: *const String,
    }

    let value = SelfReferential {
        data: "hello".to_string(),
        ptr: std::ptr::null(),
    };

    let _pinned = Box::pin(value);
    // Pinned value cannot be moved
}

// ============================================================================
// Common Unsafe Mistakes
// ============================================================================

/// Mistake 1: Dangling pointers.
#[allow(dead_code)]
fn dangling_pointer_mistake() {
    let ptr = {
        let x = 42;
        &x as *const i32
    }; // x dropped here

    // DON'T DO THIS - ptr is now dangling!
    // unsafe { *ptr }
    let _ = ptr;
}

/// Mistake 2: Violating aliasing rules.
#[allow(dead_code)]
fn aliasing_mistake() {
    let mut data = vec![1, 2, 3];
    let ptr = data.as_mut_ptr();

    // DON'T DO THIS - creating aliasing mutable references!
    // unsafe {
    //     let ref1 = &mut *ptr;
    //     let ref2 = &mut *ptr;
    //     *ref1 = 10;
    //     *ref2 = 20; // UB!
    // }
}

/// Mistake 3: Uninitialized memory.
#[allow(dead_code)]
fn uninitialized_mistake() {
    use std::mem::MaybeUninit;

    let mut x: MaybeUninit<i32> = MaybeUninit::uninit();

    // DON'T DO THIS - reading uninitialized memory!
    // let value = unsafe { x.assume_init() }; // UB!

    // Correct: initialize first
    x.write(42);
    let value = unsafe { x.assume_init() };
    let _ = value;
}
