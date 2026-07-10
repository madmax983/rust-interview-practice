//! # Garbage Collector (Mark-and-Sweep) Implementation
//!
//! Implements a basic Mark-and-Sweep Garbage Collector in Rust behind a
//! **sound, rooting-based safe API**.
//!
//! **Replaces Crates:** `gc`, `rust-gc`, `boehm-gc`
//!
//! **Real-world Usage:**
//! - Core memory management in runtimes for languages like Java, Go, Python, and JavaScript.
//! - Managing cyclic object graphs where simple reference counting (`Rc`/`Arc`) would leak memory.
//! - Game engines and scripting language integrations (e.g., Lua).
//!
//! # Why the API is shaped the way it is (soundness)
//!
//! An earlier version of this module handed out references through
//! `impl Deref for Gc<T>` and asked callers to list the live roots by hand when
//! calling `collect`. That was **unsound**: `Gc<T>: Deref` promised a valid
//! `&T`, but nothing stopped the collector from sweeping the object out from
//! under a live handle. The safety of `deref` rested on an invariant ("only
//! dereference reachable objects") that the *safe* API did not enforce, so
//! ordinary safe code could trigger use-after-free:
//!
//! ```text
//! let handle = gc.alloc(42);        // old Gc<i32>: Copy, derefs to &i32
//! gc.collect(std::iter::empty());   // caller "forgot" to list `handle`
//! let n = *handle;                  // UAF, in SAFE code
//! ```
//!
//! The redesign closes the hole:
//!
//! 1. **`Gc<T>` is no longer dereferenceable.** It is an opaque, `Copy` heap
//!    edge you can store inside other GC objects to build the object graph, but
//!    you cannot read through it directly (see the `compile_fail` demo below).
//! 2. **Reading a value requires a [`Root`] guard.** [`Collector::alloc`]
//!    returns `Root<'gc, T>`, and [`Collector::root`] turns a live [`Gc`] handle
//!    back into one. A `Root` registers its object as a GC root for as long as
//!    the guard lives and unregisters it on `Drop`. `Root` is the *only* thing
//!    that derefs to `&T`, and a rooted object is never swept, so the reference
//!    can never dangle. Dropping the root is the very same act as giving up the
//!    ability to read the value, so the old footgun (drop the root, then
//!    dereference) cannot even be written.
//! 3. **`collect` takes no manual root list.** Roots register themselves, so you
//!    can no longer forget to root something you are still using — the only way
//!    to *use* it is through a `Root`.
//! 4. **Resurrecting a handle is checked.** [`Collector::root`] returns
//!    `Option<Root>`: every allocation gets a unique, never-reused id, and
//!    `root` only succeeds if a live object with that id is still in the arena.
//!    This makes reviving a stale handle safe (you get `None`) and defeats ABA
//!    address reuse and type confusion by construction.
//!
//! ```
//! use rust_interview_practice::systems::garbage_collector::Collector;
//!
//! let gc = Collector::new();
//! let a = gc.alloc(10);      // Root<'_, i32>, rooted + alive
//! {
//!     let _b = gc.alloc(20); // rooted only inside this block
//! } // `_b` dropped here -> its object is now unrooted
//! gc.collect();              // sweeps the `20`, keeps the `10`
//! assert_eq!(*a, 10);        // `a` is still rooted, still valid
//! ```
//!
//! The bare-handle footgun is rejected at compile time — `Gc<T>` has no `Deref`:
//!
//! ```compile_fail
//! use rust_interview_practice::systems::garbage_collector::{Collector, Gc};
//!
//! let gc = Collector::new();
//! let root = gc.alloc(42_i32);
//! let handle: Gc<i32> = root.to_gc(); // opaque edge, no Deref
//! drop(root);                         // object may now be swept
//! let _ = *handle;                    // ERROR: `Gc<T>` cannot be dereferenced
//! ```
//!
//! **Why build it yourself?**
//! Building a garbage collector teaches you the fundamentals of dynamic memory
//! management, how object reachability is determined through tracing, and the
//! complexities of managing raw memory in a systems language. It highlights the
//! exact problem Rust's ownership system was designed to avoid, and shows how to
//! wrap inherently `unsafe` machinery in an API that is sound to use from safe
//! code.

use std::cell::{Cell, RefCell};
use std::marker::PhantomData;
use std::ops::Deref;
use std::ptr::NonNull;

// =========================================================================================
// Architecture
// =========================================================================================
//
// Allocation: Objects are boxed on the heap and threaded onto an intrusive
// singly linked list (the arena) owned by the `Collector`.
// Mark Phase:  Starting from every directly-rooted object (root count > 0),
//              trace all reachable objects and set their `marked` flag.
// Sweep Phase: Walk the arena; drop+free objects that are not marked, and clear
//              the flag on the survivors.
//
// Invariants:
// 1. A `Root<'gc, T>` keeps its object's root count >= 1, so the object is
//    always marked and never swept while the guard is alive. This is what makes
//    `Root: Deref -> &T` sound.
// 2. `Gc<T>` is an opaque edge with no safe way to read through it; the only
//    way to obtain `&T` is a `Root`, which guarantees liveness.
// 3. Every object carries a unique, monotonically-increasing id, so a stale
//    handle can be detected (`Collector::root` -> `None`) instead of being
//    dereferenced.
//
// Complexity:
// ┌───────────┬──────────────┬─────────────┐
// │ Operation │ Time         │ Space       │
// ├───────────┼──────────────┼─────────────┤
// │ Allocate  │ O(1)         │ O(1)        │
// │ Mark      │ O(N + L)     │ O(D)        │
// │ Sweep     │ O(N)         │ O(1)        │
// │ root()    │ O(N)         │ O(1)        │
// └───────────┴──────────────┴─────────────┘
// N = total allocated objects, L = live objects, D = max object-graph depth.
// (`root()` scans the arena for the id; a production GC would use a slot table
// for O(1) — see the footer.)

/// A trait for objects managed by the Garbage Collector to trace their references.
pub trait Trace {
    /// Traces all `Gc` references held by this object, marking them reachable.
    fn trace(&self);
}

// Leaf types hold no `Gc` references.
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
///
/// The header fields come before `value` so their offsets are identical for
/// every `T`, which is what lets the collector read `marked`/`roots`/`id`/`next`
/// through a `&GcBox<dyn Trace>` fat pointer.
struct GcBox<T: ?Sized> {
    marked: Cell<bool>,
    /// Number of live `Root`s pointing at this object. Non-zero => a GC root.
    roots: Cell<usize>,
    /// Unique, never-reused allocation id (used for safe handle resurrection).
    id: u64,
    next: Option<NonNull<GcBox<dyn Trace>>>,
    value: T,
}

impl<T: ?Sized> GcBox<T> {
    const fn is_marked(&self) -> bool {
        self.marked.get()
    }
    fn mark(&self) {
        self.marked.set(true);
    }
    fn unmark(&self) {
        self.marked.set(false);
    }
    const fn is_rooted(&self) -> bool {
        self.roots.get() > 0
    }
    fn add_root(&self) {
        self.roots.set(self.roots.get() + 1);
    }
    fn remove_root(&self) {
        self.roots.set(self.roots.get() - 1);
    }
}

/// An opaque, `Copy` handle to a garbage-collected object — a heap *edge*.
///
/// Store a `Gc<T>` inside other GC-managed objects to build the object graph.
/// It deliberately does **not** implement `Deref`: a bare handle can outlive the
/// object it points at, so reading through it directly would be unsound. To read
/// the value, revive the handle into a [`Root`] with [`Collector::root`].
pub struct Gc<T: Trace + 'static> {
    ptr: NonNull<GcBox<T>>,
    id: u64,
}

// Only the pointer + id are copied, never the underlying object (tracing GC, not
// reference counting), so `Gc<T>` is freely `Copy`.
impl<T: Trace + 'static> Clone for Gc<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T: Trace + 'static> Copy for Gc<T> {}

impl<T: Trace + 'static> Trace for Gc<T> {
    fn trace(&self) {
        // SAFETY: a `Gc` only exists while the arena that owns its `GcBox` is
        // alive; tracing takes a shared reference and recurses at most once per
        // object thanks to the `marked` guard.
        let gc_box = unsafe { self.ptr.as_ref() };
        if !gc_box.is_marked() {
            gc_box.mark();
            gc_box.value.trace();
        }
    }
}

/// A stack root: a guard that keeps its object alive and grants `&T` access.
///
/// While a `Root<'gc, T>` exists the object it points to has a non-zero root
/// count, so [`Collector::collect`] always marks it and never sweeps it. That is
/// what makes `Root`'s [`Deref`] sound: the `&T` it yields is bounded by `&self`,
/// so it cannot outlive the guard, and the object cannot be freed while the
/// guard lives. The `'gc` lifetime ties the root to its [`Collector`], so a root
/// can never outlive the arena that backs it.
pub struct Root<'gc, T: Trace + 'static> {
    gc: Gc<T>,
    _marker: PhantomData<&'gc Collector>,
}

impl<T: Trace + 'static> Root<'_, T> {
    fn new(gc: Gc<T>) -> Self {
        // SAFETY: `gc` was produced from a live object owned by the collector
        // this root borrows for `'gc`; registering a root only reads/writes the
        // `Cell<usize>` root counter through a shared reference.
        unsafe { gc.ptr.as_ref().add_root() };
        Self {
            gc,
            _marker: PhantomData,
        }
    }

    /// Returns the opaque, `Copy` heap edge for this object.
    ///
    /// Store the returned [`Gc`] inside other GC-managed objects to build the
    /// graph. It does **not** keep the object alive and cannot be dereferenced;
    /// revive it with [`Collector::root`] to read it again.
    #[must_use]
    pub const fn to_gc(&self) -> Gc<T> {
        self.gc
    }
}

impl<T: Trace + 'static> Deref for Root<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        // SAFETY: this `Root` holds a root-count on the object, so the collector
        // cannot have swept it, and this GC never moves objects (no compaction),
        // so the pointer stays valid. The returned `&T` is bounded by `&self`, so
        // it cannot outlive the root that guarantees the object's liveness.
        unsafe { &self.gc.ptr.as_ref().value }
    }
}

// Cloning a root adds another registration, so both guards keep the object alive.
impl<T: Trace + 'static> Clone for Root<'_, T> {
    fn clone(&self) -> Self {
        Self::new(self.gc)
    }
}

impl<T: Trace + 'static> Drop for Root<'_, T> {
    fn drop(&mut self) {
        // SAFETY: the object outlives this root (`'gc`), so the pointer is valid;
        // we drop this root's contribution to the object's root count.
        unsafe { self.gc.ptr.as_ref().remove_root() };
    }
}

/// The Garbage Collector: owns the arena and performs mark-and-sweep.
pub struct Collector {
    head: Cell<Option<NonNull<GcBox<dyn Trace>>>>,
    next_id: Cell<u64>,
    allocated_bytes: Cell<usize>,
}

impl Default for Collector {
    fn default() -> Self {
        Self::new()
    }
}

impl Collector {
    /// Creates a new, empty `Collector`.
    #[must_use]
    pub fn new() -> Self {
        Self {
            head: Cell::new(None),
            next_id: Cell::new(0),
            allocated_bytes: Cell::new(0),
        }
    }

    /// Total bytes of `GcBox` headers+values ever allocated (never decremented;
    /// illustrative only — a real GC would track live bytes to trigger collection).
    #[must_use]
    pub const fn allocated_bytes(&self) -> usize {
        self.allocated_bytes.get()
    }

    /// Allocates `value` on the GC heap and returns a [`Root`] keeping it alive.
    pub fn alloc<T: Trace + 'static>(&self, value: T) -> Root<'_, T> {
        let size = std::mem::size_of::<GcBox<T>>();
        let id = self.next_id.get();
        self.next_id.set(id + 1);

        let gc_box = Box::new(GcBox {
            marked: Cell::new(false),
            roots: Cell::new(0),
            id,
            next: self.head.get(),
            value,
        });

        // SAFETY: `Box::into_raw` never returns null; the collector now owns the
        // allocation and only frees it during `sweep`.
        let ptr = unsafe { NonNull::new_unchecked(Box::into_raw(gc_box)) };
        let dyn_ptr: NonNull<GcBox<dyn Trace>> = ptr;
        self.head.set(Some(dyn_ptr));
        self.allocated_bytes.set(self.allocated_bytes.get() + size);

        Root::new(Gc { ptr, id })
    }

    /// Revives a handle into a [`Root`], if the object is still live.
    ///
    /// Returns `None` if the object was already swept. This is the *safe*
    /// resurrection path: we scan the arena for a box whose unique id matches the
    /// handle's. Because ids are never reused, a match proves this exact
    /// allocation is still alive (and is genuinely a `GcBox<T>`), so the pointer
    /// is valid to root. It also defeats ABA: if the address was recycled by a
    /// later allocation, that new box carries a different id and will not match.
    #[must_use]
    pub fn root<T: Trace + 'static>(&self, handle: Gc<T>) -> Option<Root<'_, T>> {
        let mut current = self.head.get();
        while let Some(node_ptr) = current {
            // SAFETY: every node on the arena list is a live, collector-owned
            // `GcBox`; we only read its `id`/`next` through a shared reference.
            let node = unsafe { node_ptr.as_ref() };
            if node.id == handle.id {
                return Some(Root::new(handle));
            }
            current = node.next;
        }
        None
    }

    /// Performs a mark-and-sweep collection using the registered roots.
    pub fn collect(&self) {
        // 1. Mark: from every directly-rooted object, mark all reachable objects.
        let mut current = self.head.get();
        while let Some(node_ptr) = current {
            // SAFETY: shared read of a live, collector-owned node.
            let node = unsafe { node_ptr.as_ref() };
            if node.is_rooted() && !node.is_marked() {
                node.mark();
                node.value.trace();
            }
            current = node.next;
        }

        // 2. Sweep: free unmarked objects, unmark survivors.
        let mut current = self.head.get();
        let mut prev_ptr: Option<NonNull<GcBox<dyn Trace>>> = None;

        while let Some(node_ptr) = current {
            // SAFETY: `node_ptr` was produced by `Box::into_raw` in `alloc` and is
            // still owned by this single-threaded collector, so it is valid. We
            // take only a SHARED reference here. This is crucial: a `&mut GcBox`
            // would cover the whole box (including `value`) and could alias a `&T`
            // handed out by `Root::deref`, which is Stacked-Borrows/Miri UB. Reads
            // of `next`/`marked` and the `unmark` write (via `Cell`) all go through
            // a shared reference, so no such aliasing `&mut` is ever formed.
            let node = unsafe { node_ptr.as_ref() };
            let next_ptr = node.next;

            if node.is_marked() {
                node.unmark();
                prev_ptr = Some(node_ptr);
            } else {
                // Unreachable: unlink and free it. No `Root` points at an unmarked
                // object (a root forces a mark), so there is no outstanding `&T`.
                if let Some(prev) = prev_ptr {
                    // SAFETY: `prev` is a valid, live node we still own. We write
                    // ONLY the `next` field via a raw field pointer instead of
                    // forming a `&mut GcBox` over the whole box, so we never alias
                    // any outstanding `&T`.
                    unsafe {
                        std::ptr::addr_of_mut!((*prev.as_ptr()).next).write(next_ptr);
                    }
                } else {
                    self.head.set(next_ptr);
                }

                // SAFETY: the object is unmarked (hence unrooted and unreachable),
                // and the shared `node` borrow above has ended (its last use was
                // reading `next`/`marked`), so reconstructing the `Box` drops the
                // value and frees the allocation exactly once.
                unsafe {
                    let _dropped = Box::from_raw(node_ptr.as_ptr());
                }
            }

            current = next_ptr;
        }
    }
}

// Free every remaining object when the collector itself is dropped.
impl Drop for Collector {
    fn drop(&mut self) {
        let mut current = self.head.get();
        while let Some(node_ptr) = current {
            // SAFETY: sole owner tearing down; each node is freed exactly once.
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
// Comparison to canonical crates:
// - `gc` / `rust-gc`: thread-local tracing GC with a `#[derive(Trace)]` macro and
//   a `Gc<T>` that roots itself while on the stack. Our `Root` guard makes the
//   same "on the stack => rooted" idea explicit and visible.
// - `gc-arena`: uses branded lifetimes and a `mutate` phase so collection can only
//   run when no GC references are borrowed — another sound answer to the same hole.
//
// Missing vs. production:
// - **Automatic root discovery** via stack scanning (we make rooting explicit).
// - **O(1) resurrection**: `root()` scans the arena for an id; a slot table keyed
//   by id would make it O(1).
// - **Generational / incremental / concurrent** collection and **compaction**.
// - **Finalization ordering** for cyclic `Drop` (we assume values' `Drop` does not
//   read through `Gc` handles).

#[cfg(test)]
mod tests {
    use super::*;

    struct Node {
        val: i32,
        next: RefCell<Option<Gc<Self>>>,
    }

    impl Trace for Node {
        fn trace(&self) {
            self.val.trace();
            self.next.trace();
        }
    }

    #[test]
    fn test_gc_alloc_and_deref() {
        let gc = Collector::new();
        let val = gc.alloc(42);
        assert_eq!(*val, 42);
    }

    #[test]
    fn test_gc_collection_sweeps_unreachable() {
        let gc = Collector::new();
        {
            let _val = gc.alloc(10);
        } // root dropped here -> object is unrooted
        gc.collect();
        assert!(gc.head.get().is_none());
    }

    #[test]
    fn test_gc_retains_reachable() {
        let gc = Collector::new();
        let val = gc.alloc(10);
        gc.collect();
        // Still rooted, so still valid to dereference.
        assert_eq!(*val, 10);
        assert!(gc.head.get().is_some());
    }

    #[test]
    fn test_gc_mixed_roots_survive_and_unreachable_freed() {
        let gc = Collector::new();
        let a = gc.alloc(10);
        let c = gc.alloc(30);
        {
            let _b = gc.alloc(20); // unrooted at end of block
            let _d = gc.alloc(40); // unrooted at end of block
        }

        gc.collect();
        // Rooted objects survive and stay dereferenceable.
        assert_eq!(*a, 10);
        assert_eq!(*c, 30);
        assert!(gc.head.get().is_some());

        // Drop the remaining roots; a second collection frees everything.
        drop(a);
        drop(c);
        gc.collect();
        assert!(gc.head.get().is_none());
    }

    #[test]
    fn test_gc_reachable_through_root_survives_collection() {
        let gc = Collector::new();
        let root = gc.alloc(Node {
            val: 1,
            next: RefCell::new(None),
        });
        let child = gc.alloc(Node {
            val: 2,
            next: RefCell::new(None),
        });
        // Link child into root's field, then drop the child's own root: the child
        // is now reachable only through `root`.
        *root.next.borrow_mut() = Some(child.to_gc());
        drop(child);

        gc.collect();

        // The child survived because it is reachable from a root. Revive its
        // handle to read it — the safe traversal pattern.
        let child_gc = (*root.next.borrow()).unwrap();
        let child_root = gc.root(child_gc).expect("child still reachable via root");
        assert_eq!(child_root.val, 2);
        assert_eq!(root.val, 1);
    }

    #[test]
    fn test_gc_root_on_stale_handle_returns_none() {
        let gc = Collector::new();
        let handle = {
            let tmp = gc.alloc(99);
            tmp.to_gc()
        }; // `tmp` root dropped -> object unrooted
        gc.collect(); // object swept
        // Reviving the stale handle is safe and simply fails.
        assert!(gc.root(handle).is_none());
    }

    #[test]
    fn test_gc_cyclic_references_are_collected() {
        let gc = Collector::new();
        {
            let node1 = gc.alloc(Node {
                val: 1,
                next: RefCell::new(None),
            });
            let node2 = gc.alloc(Node {
                val: 2,
                next: RefCell::new(Some(node1.to_gc())),
            });
            // Close the cycle.
            *node1.next.borrow_mut() = Some(node2.to_gc());
            // Both roots dropped at end of block; the cycle lives only in the heap.
        }

        gc.collect();
        // The cycle is unreachable from any root and is swept (no leak).
        assert!(gc.head.get().is_none());
    }
}
