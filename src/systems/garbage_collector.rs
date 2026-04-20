//! # Garbage Collector (Mark-and-Sweep) Implementation
//!
//! Implements a basic Mark-and-Sweep Garbage Collector in Rust.
//!
//! **Replaces Crates:** `gc`, `rust-gc`, `boehm-gc`
//!
//! **Real-world Usage:**
//! - Core memory management in runtimes for languages like Java, Go, Python, and JavaScript.
//! - Managing cyclic object graphs where simple reference counting (`Rc`/`Arc`) would leak memory.
//! - Game engines and scripting language integrations (e.g., Lua).
//!
//! **Why build it yourself?**
//! Building a garbage collector teaches you the fundamentals of dynamic memory management,
//! how object reachability is determined through tracing, and the complexities of managing
//! raw memory in a systems language. It highlights the exact problem Rust's ownership system
//! was designed to avoid, and demonstrates how to safely implement self-referential or
//! cyclical data structures using unsafe code responsibly.

use std::any::Any;
use std::cell::RefCell;
use std::collections::HashSet;
use std::ptr::NonNull;

// =========================================================================================
// Architecture
// =========================================================================================
//
// Data Structure:
//
//      Roots (Stack/Registers)
//         │
//         ▼
//      ┌──────┐    ┌──────┐
//      │ Gc<T>├───►│ Gc<U>│ (Heap Objects)
//      └──────┘    └──────┘
//         ▲           │
//         └───────────┘ (Cyclic Reference)
//
// Allocation: Objects are allocated on the heap, and a raw pointer is stored in an Arena.
// Mark Phase: Starting from known "roots", traverse all reachable objects and set a `marked` flag.
// Sweep Phase: Iterate over all allocated objects. Free the ones that are not marked, and clear the flag for the rest.
//
// Invariants:
// 1. All live objects are reachable from a root.
// 2. The collector accurately traces all references within an object via the `Trace` trait.
// 3. Unreachable objects are safely dropped and deallocated exactly once.
//
// Complexity:
// ┌───────────┬──────────────┬─────────────┐
// │ Operation │ Time         │ Space       │
// ├───────────┼──────────────┼─────────────┤
// │ Allocate  │ O(1)*        │ O(1)        │
// │ Mark      │ O(L)         │ O(D)        │
// │ Sweep     │ O(N)         │ O(1)        │
// └───────────┴──────────────┴─────────────┘
// L = Live objects, D = Max depth of object graph, N = Total allocated objects
// * Allocation might trigger O(L + N) collection.
//
// Design Decisions:
// - **Tracing Trait**: Uses a `Trace` trait to traverse object fields.
//   - *Tradeoff*: Requires manual `Trace` implementation for all GC-managed types.
//   - *Alternative*: Conservative GC (scanning stack/heap for pointer-like values), which is error-prone and can leak.
// - **Single-threaded**: Thread-local collector.
//   - *Tradeoff*: Simpler, avoids locking overhead. Cannot share `Gc` references across threads.
//   - *Alternative*: Concurrent/Parallel GC, significantly more complex.

/// A trait for objects managed by the Garbage Collector to trace their references.
pub trait Trace {
    /// Traces all `Gc` references held by this object.
    fn trace(&self);
}

// Basic types don't contain Gc references.
impl Trace for i32 {
    fn trace(&self) {}
}
impl Trace for f64 {
    fn trace(&self) {}
}
impl Trace for String {
    fn trace(&self) {}
}
impl Trace for bool {
    fn trace(&self) {}
}

impl<T: Trace> Trace for Option<T> {
    fn trace(&self) {
        if let Some(val) = self {
            val.trace();
        }
    }
}

impl<T: Trace> Trace for Vec<T> {
    fn trace(&self) {
        for item in self {
            item.trace();
        }
    }
}

impl<T: Trace> Trace for RefCell<T> {
    fn trace(&self) {
        self.borrow().trace();
    }
}

/// Metadata header for all GC-managed objects.
struct GcBox<T: ?Sized> {
    marked: RefCell<bool>,
    next: Option<NonNull<GcBox<dyn Trace>>>,
    value: T,
}

impl<T: ?Sized> GcBox<T> {
    fn is_marked(&self) -> bool {
        *self.marked.borrow()
    }

    fn mark(&self) {
        *self.marked.borrow_mut() = true;
    }

    fn unmark(&self) {
        *self.marked.borrow_mut() = false;
    }
}

/// A Garbage-Collected smart pointer.
pub struct Gc<T: ?Sized + 'static> {
    ptr: NonNull<GcBox<T>>,
}

// RUST INSIGHT: `Gc<T>` implements `Clone` to create multiple references to the same object.
// Unlike `Rc<T>`, it doesn't increment a reference count, avoiding cyclic leaks.
impl<T: ?Sized> Clone for Gc<T> {
    fn clone(&self) -> Self {
        // GOTCHA: We must only copy the pointer, never deep-copy the underlying object
        // or increment a ref-count (since we rely on tracing, not ref-counting).
        Self { ptr: self.ptr }
    }
}

// UNSAFE JUSTIFICATION:
// Gc pointers are safe to dereference as long as the Garbage Collector ensures
// they are not swept while still reachable. We restrict `Gc` creation to the thread-local
// Collector to prevent data races.
impl<T: ?Sized> std::ops::Deref for Gc<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &self.ptr.as_ref().value }
    }
}

impl<T: Trace + 'static> Trace for Gc<T> {
    fn trace(&self) {
        let gc_box = unsafe { self.ptr.as_ref() };
        if !gc_box.is_marked() {
            gc_box.mark();
            gc_box.value.trace();
        }
    }
}

/// The Garbage Collector that tracks allocations and performs mark-and-sweep.
pub struct Collector {
    head: Option<NonNull<GcBox<dyn Trace>>>,
    _allocated_bytes: usize,
    _threshold: usize,
}

impl Collector {
    /// Creates a new Collector instance.
    pub fn new() -> Self {
        Self {
            head: None,
            _allocated_bytes: 0,
            _threshold: 1024 * 1024, // 1MB initial threshold
        }
    }

    /// Allocates an object on the GC heap.
    /// In a real system, this would also check `allocated_bytes > threshold`
    /// and automatically trigger a collection, passing in roots.
    // PRODUCTION NOTE: A real GC uses a `Finalize` trait to safely drop cyclic objects.
    // If a cycle is swept and A drops before B, B's Drop method could access A (Use-After-Free).
    // Our implementation assumes T does not implement a custom Drop that accesses Gc pointers.
    pub fn alloc<T: Trace + 'static>(&mut self, value: T) -> Gc<T> {
        let size = std::mem::size_of::<GcBox<T>>();

        let gc_box = Box::new(GcBox {
            marked: RefCell::new(false),
            next: self.head,
            value,
        });

        // UNSAFE JUSTIFICATION:
        // We leak the Box to raw pointer and take ownership in the Collector.
        // It will only be freed during the `sweep` phase.
        let ptr = unsafe { NonNull::new_unchecked(Box::into_raw(gc_box)) };

        // Coerce to a trait object pointer for the list
        let dyn_ptr: NonNull<GcBox<dyn Trace>> = ptr;
        self.head = Some(dyn_ptr);
        self._allocated_bytes += size;

        Gc { ptr }
    }

    /// Performs a garbage collection cycle.
    /// `roots` is an iterator of starting points (e.g., variables on the stack).
    pub fn collect<'a, I>(&mut self, roots: I)
    where
        I: IntoIterator<Item = &'a dyn Trace>,
    {
        // 1. Mark Phase
        for root in roots {
            root.trace();
        }

        // 2. Sweep Phase
        let mut current = self.head;
        let mut prev_ptr: Option<NonNull<GcBox<dyn Trace>>> = None;

        while let Some(mut node_ptr) = current {
            // UNSAFE JUSTIFICATION: We own the nodes, and no other thread can access them.
            let node = unsafe { node_ptr.as_mut() };
            let next_ptr = node.next;

            if node.is_marked() {
                // Keep the object, just unmark it for the next cycle
                node.unmark();
                prev_ptr = Some(node_ptr);
            } else {
                // Object is unreachable, sweep it!
                if let Some(mut prev) = prev_ptr {
                    unsafe { prev.as_mut().next = next_ptr };
                } else {
                    self.head = next_ptr;
                }

                // UNSAFE JUSTIFICATION: We reconstruct the Box to safely drop the inner value
                // and deallocate the memory. Since it wasn't marked, there are no live `Gc<T>` pointers to it.
                unsafe {
                    let _dropped_box = Box::from_raw(node_ptr.as_ptr());
                    // `_dropped_box` goes out of scope here, calling drop logic
                }
            }

            current = next_ptr;
        }
    }
}

// Ensure the Collector cleans up everything when it gets dropped
impl Drop for Collector {
    fn drop(&mut self) {
        let mut current = self.head;
        while let Some(node_ptr) = current {
            unsafe {
                let next = node_ptr.as_ref().next;
                let _ = Box::from_raw(node_ptr.as_ptr());
                current = next;
            }
        }
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `gc` crate: Provides thread-local garbage collection with derive macros for `Trace`.
// - `rust-gc`: A more robust implementation using compiler plugins/macros.
//
// Missing vs. Production:
// - **Automatic Root Tracking**: Real GCs track roots automatically (via stack scanning or smart pointers that register themselves). Here we must pass roots manually to `collect`.
// - **Generational GC**: Real GCs segregate objects by age (nursery vs. tenured) because most objects die young.
// - **Compaction**: This GC doesn't move objects to prevent memory fragmentation.
// - **Derive Macros**: Real crates provide `#[derive(Trace)]` to avoid boilerplate.
//
//
// Next Steps:
// 1. Implement a `#[derive(Trace)]` macro using `syn` and `quote`.
// 2. Implement generational collection.
// 3. Implement a Tri-color concurrent marker.
// 4. Benchmark using `criterion` by creating a large cyclic graph and measuring `collect()` time.

#[cfg(test)]
mod tests {
    use super::*;

    struct Node {
        val: i32,
        next: RefCell<Option<Gc<Node>>>,
    }

    impl Trace for Node {
        fn trace(&self) {
            self.val.trace();
            self.next.trace();
        }
    }

    #[test]
    fn test_gc_alloc_and_deref() {
        let mut gc = Collector::new();
        let val = gc.alloc(42);
        assert_eq!(*val, 42);
    }

    #[test]
    fn test_gc_collection_sweeps_unreachable() {
        let mut gc = Collector::new();

        // Create an object, but don't hold a root to it.
        // We drop the Gc pointer, but the allocation remains in the Collector's arena.
        {
            let _val = gc.alloc(10);
        }

        // Pass empty roots array. The object should be swept.
        let roots: [&dyn Trace; 0] = [];
        gc.collect(roots);

        // Internally head should be None, but we can't observe that directly without adding accessors.
        // The Drop implementation of Collector will not panic, meaning it's clean.
        assert!(gc.head.is_none());
    }

    #[test]
    fn test_gc_retains_reachable() {
        let mut gc = Collector::new();

        let val = gc.alloc(10);
        let roots: [&dyn Trace; 1] = [&val];

        gc.collect(roots);

        // If it was kept, we can still dereference it safely.
        assert_eq!(*val, 10);
        assert!(gc.head.is_some());
    }

    #[test]
    fn test_gc_cyclic_references_are_collected() {
        let mut gc = Collector::new();

        {
            let node1 = gc.alloc(Node {
                val: 1,
                next: RefCell::new(None),
            });
            let node2 = gc.alloc(Node {
                val: 2,
                next: RefCell::new(Some(node1.clone())),
            });

            // Create cycle
            *node1.next.borrow_mut() = Some(node2.clone());

            // Both node1 and node2 go out of scope here.
            // The cyclic reference exists entirely within the GC heap.
        }

        // Run collection without roots
        let roots: [&dyn Trace; 0] = [];
        gc.collect(roots);

        // Cycle should be broken and swept.
        assert!(gc.head.is_none());
    }
}
