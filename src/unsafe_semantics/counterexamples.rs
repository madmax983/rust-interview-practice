//! # Aliasing Counterexamples
//!
//! Each counterexample is a pair of real Rust functions:
//!
//! - `*_unsound` — an `unsafe fn` that **has undefined behaviour** under at least
//!   one aliasing model. It compiles, and natively it will probably "work", which
//!   is exactly why it is dangerous. **Never call these outside Miri.**
//! - `*_sound` — the minimal fix, safe to call anywhere.
//!
//! The [`CATALOG`] records, for every pair, the verdict **Miri** gives under
//! Stacked Borrows and under Tree Borrows, plus a [`trace`](super::trace) of each
//! version so the toy models in this module can be checked against Miri.
//!
//! | Counterexample                       | Stacked Borrows | Tree Borrows |
//! |--------------------------------------|-----------------|--------------|
//! | `write_through_shared`               | UB              | UB           |
//! | `stale_mut_after_parent_write`       | UB              | UB           |
//! | `raw_invalidated_by_new_mut`         | UB              | UB           |
//! | `shared_ref_outlives_raw_write`      | UB              | UB           |
//! | `protector_violation`                | UB              | UB           |
//! | `out_of_range_raw`                   | UB              | ok           |
//! | `read_parent_then_write_child`       | UB              | ok           |
//! | `unused_mut_reborrow`                | UB              | ok           |
//!
//! The last three are the interesting ones: Tree Borrows deliberately accepts
//! them, but code that relies on that is still rejected by Miri's default model.
//!
//! ## Running the unsound half
//!
//! ```bash
//! cargo +nightly miri run --bin miri_counterexample -- write_through_shared
//! MIRIFLAGS=-Zmiri-tree-borrows cargo +nightly miri run --bin miri_counterexample -- out_of_range_raw
//! scripts/miri-counterexamples.sh   # checks every verdict in the catalog
//! ```

// The examples are called through `fn` pointers in `CATALOG`; `const fn` would
// add noise without changing anything that is being taught.
#![allow(clippy::missing_const_for_fn)]

use std::ptr;

use super::trace::{
    Op, ROOT, Verdict, end_protect, mut_arg, mut_ref, raw, read, shared_ref, write,
};

/// A sound/unsound pair plus the verdicts Miri gives for the unsound half.
#[derive(Debug, Clone, Copy)]
pub struct Counterexample {
    /// Stable identifier, used by the `miri_counterexample` binary.
    pub name: &'static str,
    /// One-line explanation of the rule being broken.
    pub lesson: &'static str,
    /// Miri's verdict for `unsound` under Stacked Borrows (the default).
    pub stacked: Verdict,
    /// Miri's verdict for `unsound` under `-Zmiri-tree-borrows`.
    pub tree: Verdict,
    /// The program with undefined behaviour. Only run it under Miri.
    pub unsound: unsafe fn() -> i32,
    /// The fixed program; returns [`Counterexample::expected`].
    pub sound: fn() -> i32,
    /// What `sound` returns.
    pub expected: i32,
    /// Number of `i32` locations the traces touch.
    pub len: usize,
    /// `unsound` as a pointer trace.
    pub unsound_trace: &'static [Op],
    /// `sound` as a pointer trace (accepted by both models).
    pub sound_trace: &'static [Op],
}

// ============================================================================
// 1. Writing through a pointer derived from `&T`
// ============================================================================

/// Writes through a raw pointer that was derived from a shared reference.
///
/// A `&T` (and every pointer derived from it) is read-only: SB gives it a
/// `SharedReadOnly` item, TB a `Frozen` node. Casting away `const` does not
/// restore write permission. Rust's `invalid_reference_casting` lint catches the
/// simplest spelling of this bug; it does not catch it across function calls.
///
/// # Safety
///
/// Never call this: it has undefined behaviour. Run it under Miri via the
/// `miri_counterexample` binary.
#[must_use]
#[allow(invalid_reference_casting)]
pub unsafe fn write_through_shared_unsound() -> i32 {
    let x = 0_i32;
    let shared = &x;
    let p = ptr::from_ref(shared).cast_mut();
    // SAFETY: none — `p` inherits `shared`'s read-only permission. UB.
    unsafe { p.write(1) };
    x
}

/// Fix: derive the writable pointer from the owner (or use `Cell`).
#[must_use]
pub fn write_through_shared_sound() -> i32 {
    let mut x = 0_i32;
    let p = &raw mut x;
    // SAFETY: `p` comes straight from the owner and nothing else is live.
    unsafe { p.write(1) };
    x
}

const WRITE_THROUGH_SHARED_UNSOUND: &[Op] = &[
    shared_ref("shared", ROOT),
    raw("p", "shared"),
    write("p", 0),
];
const WRITE_THROUGH_SHARED_SOUND: &[Op] = &[raw("p", ROOT), write("p", 0), read(ROOT, 0)];

// ============================================================================
// 2. Using a `&mut` after its parent wrote
// ============================================================================

/// Uses a `&mut` after writing through the raw pointer it was derived from.
///
/// A write through the parent invalidates the child: SB pops the child's
/// `Unique` item, TB moves it to `Disabled`.
///
/// # Safety
///
/// Never call this: it has undefined behaviour. Run it under Miri.
#[must_use]
pub unsafe fn stale_mut_after_parent_write_unsound() -> i32 {
    let mut x = 0_i32;
    let p = &raw mut x;
    // SAFETY: `p` is valid and unaliased at this point.
    let r = unsafe { &mut *p };
    // SAFETY: none — writing via the parent invalidates `r`...
    unsafe { p.write(1) };
    // ...so this use of `r` is UB.
    *r = 2;
    x
}

/// Fix: finish with the child before going back to the parent.
#[must_use]
pub fn stale_mut_after_parent_write_sound() -> i32 {
    let mut x = 0_i32;
    let p = &raw mut x;
    // SAFETY: `p` is valid and unaliased.
    let r = unsafe { &mut *p };
    *r = 2; // last use of `r`
    // SAFETY: `r` is dead; the parent may be used again.
    unsafe { p.write(p.read() + 1) };
    x
}

const STALE_MUT_UNSOUND: &[Op] = &[
    raw("p", ROOT),
    mut_ref("r", "p"),
    write("p", 0),
    write("r", 0),
];
const STALE_MUT_SOUND: &[Op] = &[
    raw("p", ROOT),
    mut_ref("r", "p"),
    write("r", 0),
    read("p", 0),
    write("p", 0),
    read(ROOT, 0),
];

// ============================================================================
// 3. A raw pointer outliving a fresh `&mut` of the owner
// ============================================================================

/// Keeps using a raw pointer after a new `&mut` to the owner was created and
/// written through.
///
/// This is the shape of the classic "cache `as_mut_ptr()`, then touch the
/// collection again" bug.
///
/// # Safety
///
/// Never call this: it has undefined behaviour. Run it under Miri.
#[must_use]
pub unsafe fn raw_invalidated_by_new_mut_unsound() -> i32 {
    let mut x = 0_i32;
    let p = ptr::from_mut(&mut x);
    let r = &mut x; // SB: retag is a write; pops `p`
    *r = 1; // TB: foreign write disables `p`
    // SAFETY: none — `p`'s borrow was invalidated by `r`. UB.
    unsafe { p.write(2) };
    x
}

/// Fix: do all raw accesses while the raw pointer is the active borrow, then
/// return to the owner.
#[must_use]
pub fn raw_invalidated_by_new_mut_sound() -> i32 {
    let mut x = 0_i32;
    let p = ptr::from_mut(&mut x);
    // SAFETY: `p` is the most recent borrow of `x`.
    unsafe { p.write(2) };
    let r = &mut x;
    *r += 1;
    x
}

const RAW_INVALIDATED_UNSOUND: &[Op] = &[
    mut_ref("tmp", ROOT),
    raw("p", "tmp"),
    mut_ref("r", ROOT),
    write("r", 0),
    write("p", 0),
];
const RAW_INVALIDATED_SOUND: &[Op] = &[
    mut_ref("tmp", ROOT),
    raw("p", "tmp"),
    write("p", 0),
    mut_ref("r", ROOT),
    read("r", 0),
    write("r", 0),
    read(ROOT, 0),
];

// ============================================================================
// 4. A `&T` that is still used after a mutation
// ============================================================================

/// Reads through a shared reference after the value was mutated behind it.
///
/// `&T` promises the value does not change while the reference is live.
///
/// # Safety
///
/// Never call this: it has undefined behaviour. Run it under Miri.
#[must_use]
pub unsafe fn shared_ref_outlives_raw_write_unsound() -> i32 {
    let mut x = 0_i32;
    let p = &raw mut x;
    // SAFETY: `p` is valid; no writes are expected while `shared` lives...
    let shared = unsafe { &*p };
    // SAFETY: none — ...but this write breaks that promise.
    unsafe { p.write(1) };
    *shared // UB: `shared` was invalidated
}

/// Fix: end the shared borrow (its last use) before mutating.
#[must_use]
pub fn shared_ref_outlives_raw_write_sound() -> i32 {
    let mut x = 0_i32;
    let p = &raw mut x;
    // SAFETY: `p` is valid and nothing mutates while `shared` is used.
    let shared = unsafe { &*p };
    let before = *shared; // last use of `shared`
    // SAFETY: `shared` is dead; `p` is the active borrow.
    unsafe {
        p.write(before + 1);
        p.read()
    }
}

const SHARED_OUTLIVES_UNSOUND: &[Op] = &[
    raw("p", ROOT),
    shared_ref("shared", "p"),
    write("p", 0),
    read("shared", 0),
];
const SHARED_OUTLIVES_SOUND: &[Op] = &[
    raw("p", ROOT),
    shared_ref("shared", "p"),
    read("shared", 0),
    write("p", 0),
    read("p", 0),
];

// ============================================================================
// 5. Protectors: `&mut` arguments are exclusive for the whole call
// ============================================================================

/// Writes through `alias` while `unique` (which points to the same place) is a
/// live, *protected* `&mut` argument.
///
/// # Safety
///
/// `alias` must not alias `unique` — the caller below violates this on purpose.
unsafe fn write_through_alias(_unique: &mut i32, alias: *mut i32) {
    // SAFETY: none when `alias` points into `_unique`. UB even though
    // `_unique` is never used: the protector covers the entire call.
    unsafe { alias.write(1) };
}

/// Passes a `&mut` and an aliasing raw pointer to the same function.
///
/// # Safety
///
/// Never call this: it has undefined behaviour. Run it under Miri.
#[must_use]
pub unsafe fn protector_violation_unsound() -> i32 {
    let mut x = 0_i32;
    let p = &raw mut x;
    // SAFETY: none — `p` and `&mut *p` alias for the whole call. UB.
    unsafe { write_through_alias(&mut *p, p) };
    x
}

fn write_through_derived(unique: &mut i32) {
    let alias = ptr::from_mut(unique);
    // SAFETY: `alias` is derived from `unique`, so it is a child, not a rival.
    unsafe { alias.write(1) };
}

/// Fix: derive the raw pointer *from* the `&mut` inside the callee.
#[must_use]
pub fn protector_violation_sound() -> i32 {
    let mut x = 0_i32;
    write_through_derived(&mut x);
    x
}

const PROTECTOR_UNSOUND: &[Op] = &[
    raw("p", ROOT),
    mut_arg("unique", "p"),
    write("p", 0),
    end_protect("unique"),
];
const PROTECTOR_SOUND: &[Op] = &[
    mut_arg("unique", ROOT),
    raw("alias", "unique"),
    write("alias", 0),
    end_protect("unique"),
    read(ROOT, 0),
];

// ============================================================================
// 6. (SB only) Raw pointer used outside the reference it came from
// ============================================================================

/// Steps from `&mut arr[0]` to `arr[1]` with pointer arithmetic.
///
/// Under Stacked Borrows a reference only carries permission for the bytes it
/// covers, so its raw pointer cannot reach `arr[1]`. Tree Borrows tracks
/// permissions for the whole allocation and accepts this.
///
/// # Safety
///
/// Never call this: it has undefined behaviour under Stacked Borrows. Run it
/// under Miri.
#[must_use]
pub unsafe fn out_of_range_raw_unsound() -> i32 {
    let mut arr = [0_i32; 2];
    let first = ptr::from_mut(&mut arr[0]);
    // SAFETY: in bounds, but (SB) outside `first`'s provenance. UB under SB.
    unsafe { first.add(1).write(1) };
    arr[1]
}

/// Fix: take the pointer from the whole array (`as_mut_ptr`).
#[must_use]
pub fn out_of_range_raw_sound() -> i32 {
    let mut arr = [0_i32; 2];
    let base = arr.as_mut_ptr();
    // SAFETY: `base` covers the whole array and index 1 is in bounds.
    unsafe { base.add(1).write(1) };
    arr[1]
}

const OUT_OF_RANGE_UNSOUND: &[Op] = &[
    mut_ref("elem", ROOT),
    raw("first", "elem"),
    write("first", 1),
    read(ROOT, 1),
];
const OUT_OF_RANGE_SOUND: &[Op] = &[
    Op::RetagMut {
        new: "slice",
        from: ROOT,
        range: 0..2,
        protect: false,
    },
    Op::Raw {
        new: "base",
        from: "slice",
        range: 0..2,
    },
    write("base", 1),
    read(ROOT, 1),
];

// ============================================================================
// 7. (SB only) Reading through the parent before the child's first write
// ============================================================================

/// Reads through the parent after creating a `&mut`, then writes through it.
///
/// SB: the read pops the child's `Unique` item. TB: a fresh `&mut` is
/// `Reserved` and tolerates foreign reads until its first write — the same idea
/// that makes two-phase borrows like `v.push(v.len())` work.
///
/// # Safety
///
/// Never call this: it has undefined behaviour under Stacked Borrows. Run it
/// under Miri.
#[must_use]
pub unsafe fn read_parent_then_write_child_unsound() -> i32 {
    let mut x = 1_i32;
    let p = &raw mut x;
    // SAFETY: `p` is valid and unaliased.
    let r = unsafe { &mut *p };
    // SAFETY: none under SB — this read invalidates `r`.
    let old = unsafe { p.read() };
    *r = old + 1; // SB: UB; TB: fine
    x
}

/// Fix: read through the child you are about to write with.
#[must_use]
pub fn read_parent_then_write_child_sound() -> i32 {
    let mut x = 1_i32;
    let p = &raw mut x;
    // SAFETY: `p` is valid and unaliased.
    let r = unsafe { &mut *p };
    let old = *r;
    *r = old + 1;
    x
}

const READ_PARENT_UNSOUND: &[Op] = &[
    raw("p", ROOT),
    mut_ref("r", "p"),
    read("p", 0),
    write("r", 0),
    read(ROOT, 0),
];
const READ_PARENT_SOUND: &[Op] = &[
    raw("p", ROOT),
    mut_ref("r", "p"),
    read("r", 0),
    write("r", 0),
    read(ROOT, 0),
];

// ============================================================================
// 8. (SB only) Merely creating a `&mut` counts as a write
// ============================================================================

/// Creates (but never uses) a `&mut` while a `&T` is live, then reads the `&T`.
///
/// SB: a `&mut` retag is a write access, which pops the shared item even if the
/// `&mut` is never used. TB: the unused `&mut` stays `Reserved`, so the shared
/// reference survives.
///
/// # Safety
///
/// Never call this: it has undefined behaviour under Stacked Borrows. Run it
/// under Miri.
#[must_use]
pub unsafe fn unused_mut_reborrow_unsound() -> i32 {
    let mut x = 7_i32;
    let p = &raw mut x;
    // SAFETY: `p` is valid.
    let shared = unsafe { &*p };
    // SAFETY: none under SB — creating this `&mut` invalidates `shared`.
    let _unused = unsafe { &mut *p };
    *shared // SB: UB; TB: fine
}

/// Fix: don't create the `&mut` until the shared borrow is finished.
#[must_use]
pub fn unused_mut_reborrow_sound() -> i32 {
    let mut x = 7_i32;
    let p = &raw mut x;
    // SAFETY: `p` is valid.
    let shared = unsafe { &*p };
    let value = *shared; // last use of `shared`
    // SAFETY: `shared` is dead.
    let _later = unsafe { &mut *p };
    value
}

const UNUSED_MUT_UNSOUND: &[Op] = &[
    raw("p", ROOT),
    shared_ref("shared", "p"),
    mut_ref("unused", "p"),
    read("shared", 0),
];
const UNUSED_MUT_SOUND: &[Op] = &[
    raw("p", ROOT),
    shared_ref("shared", "p"),
    read("shared", 0),
    mut_ref("later", "p"),
];

// ============================================================================
// Catalog
// ============================================================================

/// Every counterexample, with the verdicts Miri produces for its unsound half.
///
/// `scripts/miri-counterexamples.sh` re-checks the `stacked` and `tree` columns
/// against a real Miri run; the unit tests check them against the toy models.
pub static CATALOG: &[Counterexample] = &[
    Counterexample {
        name: "write_through_shared",
        lesson: "pointers derived from &T are read-only, even after a cast to *mut",
        stacked: Verdict::Ub,
        tree: Verdict::Ub,
        unsound: write_through_shared_unsound,
        sound: write_through_shared_sound,
        expected: 1,
        len: 1,
        unsound_trace: WRITE_THROUGH_SHARED_UNSOUND,
        sound_trace: WRITE_THROUGH_SHARED_SOUND,
    },
    Counterexample {
        name: "stale_mut_after_parent_write",
        lesson: "a write through the parent invalidates every child &mut",
        stacked: Verdict::Ub,
        tree: Verdict::Ub,
        unsound: stale_mut_after_parent_write_unsound,
        sound: stale_mut_after_parent_write_sound,
        expected: 3,
        len: 1,
        unsound_trace: STALE_MUT_UNSOUND,
        sound_trace: STALE_MUT_SOUND,
    },
    Counterexample {
        name: "raw_invalidated_by_new_mut",
        lesson: "a fresh &mut of the owner invalidates older raw pointers",
        stacked: Verdict::Ub,
        tree: Verdict::Ub,
        unsound: raw_invalidated_by_new_mut_unsound,
        sound: raw_invalidated_by_new_mut_sound,
        expected: 3,
        len: 1,
        unsound_trace: RAW_INVALIDATED_UNSOUND,
        sound_trace: RAW_INVALIDATED_SOUND,
    },
    Counterexample {
        name: "shared_ref_outlives_raw_write",
        lesson: "&T guarantees immutability for as long as it is used",
        stacked: Verdict::Ub,
        tree: Verdict::Ub,
        unsound: shared_ref_outlives_raw_write_unsound,
        sound: shared_ref_outlives_raw_write_sound,
        expected: 1,
        len: 1,
        unsound_trace: SHARED_OUTLIVES_UNSOUND,
        sound_trace: SHARED_OUTLIVES_SOUND,
    },
    Counterexample {
        name: "protector_violation",
        lesson: "a &mut argument is exclusive for the whole call, used or not",
        stacked: Verdict::Ub,
        tree: Verdict::Ub,
        unsound: protector_violation_unsound,
        sound: protector_violation_sound,
        expected: 1,
        len: 1,
        unsound_trace: PROTECTOR_UNSOUND,
        sound_trace: PROTECTOR_SOUND,
    },
    Counterexample {
        name: "out_of_range_raw",
        lesson: "SB: a pointer from &mut arr[0] may not reach arr[1]; TB allows it",
        stacked: Verdict::Ub,
        tree: Verdict::Ok,
        unsound: out_of_range_raw_unsound,
        sound: out_of_range_raw_sound,
        expected: 1,
        len: 2,
        unsound_trace: OUT_OF_RANGE_UNSOUND,
        sound_trace: OUT_OF_RANGE_SOUND,
    },
    Counterexample {
        name: "read_parent_then_write_child",
        lesson: "SB: a parent read kills a fresh &mut; TB keeps it Reserved",
        stacked: Verdict::Ub,
        tree: Verdict::Ok,
        unsound: read_parent_then_write_child_unsound,
        sound: read_parent_then_write_child_sound,
        expected: 2,
        len: 1,
        unsound_trace: READ_PARENT_UNSOUND,
        sound_trace: READ_PARENT_SOUND,
    },
    Counterexample {
        name: "unused_mut_reborrow",
        lesson: "SB: creating a &mut is a write, even if it is never used",
        stacked: Verdict::Ub,
        tree: Verdict::Ok,
        unsound: unused_mut_reborrow_unsound,
        sound: unused_mut_reborrow_sound,
        expected: 7,
        len: 1,
        unsound_trace: UNUSED_MUT_UNSOUND,
        sound_trace: UNUSED_MUT_SOUND,
    },
];

/// Look up a counterexample by name.
#[must_use]
pub fn find(name: &str) -> Option<&'static Counterexample> {
    CATALOG.iter().find(|c| c.name == name)
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;
    use crate::unsafe_semantics::{stacked_borrows, tree_borrows};

    #[test]
    fn test_catalog_names_are_unique_and_findable() {
        let names: HashSet<_> = CATALOG.iter().map(|c| c.name).collect();
        assert_eq!(names.len(), CATALOG.len());
        for c in CATALOG {
            assert_eq!(find(c.name).map(|f| f.name), Some(c.name));
        }
        assert!(find("no_such_counterexample").is_none());
    }

    #[test]
    fn test_every_unsound_example_is_ub_somewhere() {
        for c in CATALOG {
            assert!(
                c.stacked == Verdict::Ub || c.tree == Verdict::Ub,
                "{} is accepted by both models",
                c.name
            );
        }
    }

    #[test]
    fn test_catalog_covers_sb_only_ub() {
        let sb_only = CATALOG
            .iter()
            .filter(|c| c.stacked == Verdict::Ub && c.tree == Verdict::Ok)
            .count();
        assert_eq!(sb_only, 3);
    }

    /// The sound halves run natively *and* under Miri (both models).
    #[test]
    fn test_sound_versions_return_expected_values() {
        for c in CATALOG {
            assert_eq!((c.sound)(), c.expected, "{}", c.name);
        }
    }

    #[test]
    fn test_individual_sound_versions() {
        assert_eq!(write_through_shared_sound(), 1);
        assert_eq!(stale_mut_after_parent_write_sound(), 3);
        assert_eq!(raw_invalidated_by_new_mut_sound(), 3);
        assert_eq!(shared_ref_outlives_raw_write_sound(), 1);
        assert_eq!(protector_violation_sound(), 1);
        assert_eq!(out_of_range_raw_sound(), 1);
        assert_eq!(read_parent_then_write_child_sound(), 2);
        assert_eq!(unused_mut_reborrow_sound(), 7);
    }

    /// The toy Stacked Borrows model agrees with Miri on every unsound trace.
    #[test]
    fn test_stacked_model_matches_miri() {
        for c in CATALOG {
            assert_eq!(
                stacked_borrows::verdict(c.len, c.unsound_trace),
                c.stacked,
                "{}: {:?}",
                c.name,
                stacked_borrows::run(c.len, c.unsound_trace)
            );
        }
    }

    /// The toy Tree Borrows model agrees with Miri on every unsound trace.
    #[test]
    fn test_tree_model_matches_miri() {
        for c in CATALOG {
            assert_eq!(
                tree_borrows::verdict(c.len, c.unsound_trace),
                c.tree,
                "{}: {:?}",
                c.name,
                tree_borrows::run(c.len, c.unsound_trace)
            );
        }
    }

    /// Both toy models accept every fix.
    #[test]
    fn test_models_accept_sound_traces() {
        for c in CATALOG {
            assert_eq!(
                stacked_borrows::run(c.len, c.sound_trace),
                Ok(()),
                "SB rejects fix for {}",
                c.name
            );
            assert_eq!(
                tree_borrows::run(c.len, c.sound_trace),
                Ok(()),
                "TB rejects fix for {}",
                c.name
            );
        }
    }
}
