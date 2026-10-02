//! # Borrowck Case Studies
//!
//! Each case is told three ways:
//!
//! 1. **Real Rust** in a doctest. Rejected programs are `compile_fail,E0xxx`
//!    doctests, so if a future rustc starts accepting one, `cargo test` fails
//!    and the note gets revisited.
//! 2. **The fix**, as a public function with unit tests (the part to type out).
//! 3. **The same program in toy MIR** (`*_mir()` builders), checked by
//!    [`crate::compiler_literacy::nll::borrowck`]. [`CASES`] records rustc's
//!    verdict next to each builder and a test asserts the mini checker agrees.
//!
//! | # | Case | rustc | Lesson |
//! |---|------|-------|--------|
//! | 1 | [NLL problem case #1](#1-nll-problem-case-1-borrow-ends-at-last-use) | ok | a borrow ends at its last use, not at `}` |
//! | 2 | [Read, then push](#2-shared-borrow-then-push-e0502) | E0502 | `&v[0]` keeps `v` frozen while used |
//! | 3 | [Two-phase borrows](#3-two-phase-borrows) | ok / E0502 | autoref `&mut` is reserved, then activated |
//! | 4 | [NLL problem case #2](#4-nll-problem-case-2-conditional-control-flow) | ok | liveness is per-branch |
//! | 5 | [NLL problem case #3](#5-nll-problem-case-3-conditional-return-e0499) | E0499 ×2 | a returned borrow is live *everywhere* |
//! | 6 | [NLL problem case #4](#6-nll-problem-case-4-mutating-mut-references) | ok | overwriting `p` kills borrows of `*p` |
//! | 7 | [Drop keeps borrows alive](#7-a-drop-impl-keeps-borrows-alive-e0506--e0503) | E0506 / E0503 | `Drop` makes a value live at scope end |
//! | 8 | [Out of scope](#8-borrowed-value-does-not-live-long-enough-e0597) | E0597 | `StorageDead` while borrowed |
//! | 9 | [Return a local](#9-returning-a-reference-to-a-local-e0515) | E0515 | `return` kills every local |
//! | 10 | [Wrong lifetime](#10-lifetime-may-not-live-long-enough) | no code | universal regions need declared bounds |
//! | 11 | [Move while borrowed](#11-move-out-while-borrowed-e0505) | E0505 | a move is a deep write |
//! | 12 | [Use while mut-borrowed](#12-use-while-mutably-borrowed-e0503) | E0503 | even reads conflict with `&mut` |
//! | 13 | [Methods vs fields](#13-disjoint-fields-vs-methods-e0499) | E0499 / ok | functions borrow the whole receiver |
//!
//! ## 1. NLL problem case #1: borrow ends at last use
//!
//! Rejected under the old lexical borrow checker, accepted under NLL:
//!
//! ```
//! let mut data = vec![1, 2, 3];
//! let slice = &mut data[..];
//! slice[0] = 9; // last use of `slice`
//! data.push(4); // fine: the borrow's region ended above
//! assert_eq!(data, [9, 2, 3, 4]);
//! ```
//!
//! ## 2. Shared borrow, then push (E0502)
//!
//! ```compile_fail,E0502
//! let mut v = vec![1, 2, 3];
//! let first = &v[0];
//! v.push(4); // may reallocate: `first` would dangle
//! println!("{first}");
//! ```
//!
//! Fix: [`first_then_push`] copies the value out before mutating.
//!
//! ## 3. Two-phase borrows
//!
//! `v.push(v.len())` desugars to `Vec::push(&mut *v, Vec::len(&*v))` — the
//! `&mut` is created *first* (see [`crate::compiler_literacy::mir_reading::TWO_PHASE_MIR`]).
//! It compiles because autoref borrows are two-phase:
//!
//! ```
//! let mut v: Vec<usize> = vec![];
//! v.push(v.len());
//! assert_eq!(v, [0]);
//! ```
//!
//! Spell the `&mut` out yourself and it is an ordinary mutable borrow:
//!
//! ```compile_fail,E0502
//! fn explicit(v: &mut Vec<usize>) {
//!     Vec::push(&mut *v, v.len());
//! }
//! ```
//!
//! ## 4. NLL problem case #2: conditional control flow
//!
//! ```
//! use std::collections::HashMap;
//!
//! fn process_or_default(map: &mut HashMap<i32, Vec<i32>>, key: i32) {
//!     match map.get_mut(&key) {
//!         Some(v) => v.push(1),        // borrow live on this branch only
//!         None => {
//!             map.insert(key, vec![1]); // so this borrow is fine
//!         }
//!     }
//! }
//! # let mut m = HashMap::new();
//! # process_or_default(&mut m, 7);
//! # process_or_default(&mut m, 7);
//! # assert_eq!(m[&7], [1, 1]);
//! ```
//!
//! ## 5. NLL problem case #3: conditional return (E0499)
//!
//! Still rejected by NLL (accepted by Polonius). Returning `value` forces the
//! `get_mut` borrow to outlive `'r` — and NLL's location-insensitive regions
//! then contain *every* point of the body, including the `None` arm.
//!
//! ```compile_fail,E0499
//! use std::collections::HashMap;
//!
//! fn get_default<'r>(map: &'r mut HashMap<i32, String>, key: i32) -> &'r mut String {
//!     match map.get_mut(&key) {
//!         Some(value) => value,
//!         None => {
//!             map.insert(key, String::new());
//!             map.get_mut(&key).unwrap()
//!         }
//!     }
//! }
//! ```
//!
//! Fix: [`get_default`] uses the entry API — one borrow, no branch.
//!
//! ## 6. NLL problem case #4: mutating `&mut` references
//!
//! ```
//! let (mut a, mut b) = (1, 2);
//! let mut p = &mut a;
//! let r = &mut *p; // reborrow of *p
//! p = &mut b;      // overwrite p: the borrow of the OLD *p is no longer reachable via p
//! *p += 1;         // so this does not conflict with `r`
//! *r += 1;
//! assert_eq!((a, b), (2, 3));
//! ```
//!
//! ## 7. A `Drop` impl keeps borrows alive (E0506 / E0503)
//!
//! ```compile_fail,E0506
//! struct Guard<'a>(&'a mut i32);
//! impl Drop for Guard<'_> {
//!     fn drop(&mut self) {
//!         *self.0 += 1;
//!     }
//! }
//!
//! fn f() -> i32 {
//!     let mut x = 0;
//!     let _g = Guard(&mut x);
//!     x = 5; // `_g` is dropped at `}` and its destructor uses the borrow
//!     x
//! }
//! ```
//!
//! ```compile_fail,E0503
//! struct Guard<'a>(&'a mut i32);
//! impl Drop for Guard<'_> {
//!     fn drop(&mut self) {
//!         *self.0 += 1;
//!     }
//! }
//!
//! fn f() -> i32 {
//!     let mut x = 0;
//!     let _g = Guard(&mut x);
//!     x // read while `_g` still holds `&mut x`
//! }
//! ```
//!
//! Without the `Drop` impl the wrapper is dead after its last use and both
//! compile. Fix with a `Drop` type: end its scope early — see [`bump_with_guard`].
//!
//! ## 8. Borrowed value does not live long enough (E0597)
//!
//! ```compile_fail,E0597
//! fn f() -> i32 {
//!     let r;
//!     {
//!         let x = 5;
//!         r = &x;
//!     } // StorageDead(x) while `r` is still going to be used
//!     *r
//! }
//! ```
//!
//! ## 9. Returning a reference to a local (E0515)
//!
//! ```compile_fail,E0515
//! fn dangle() -> &'static i32 {
//!     let x = 5;
//!     &x
//! }
//! ```
//!
//! ## 10. Lifetime may not live long enough
//!
//! This error has no code: it comes from the universal-region check, not from
//! an access conflict. `y: &'b i32` flows into the `&'a i32` return slot,
//! which requires `'b: 'a` — never promised by the signature.
//!
//! ```compile_fail
//! fn pick<'a, 'b>(x: &'a i32, y: &'b i32) -> &'a i32 {
//!     let _ = x;
//!     y
//! }
//! ```
//!
//! Fix: declare the bound, as in [`pick_second`].
//!
//! ## 11. Move out while borrowed (E0505)
//!
//! ```compile_fail,E0505
//! fn f() -> usize {
//!     let s = String::from("hi");
//!     let r = &s;
//!     let t = s;
//!     r.len() + t.len()
//! }
//! ```
//!
//! ## 12. Use while mutably borrowed (E0503)
//!
//! ```compile_fail,E0503
//! fn f() -> i32 {
//!     let mut x = 1;
//!     let r = &mut x;
//!     let y = x; // even a copy-read conflicts with a live `&mut`
//!     *r += 1;
//!     y
//! }
//! ```
//!
//! ## 13. Disjoint fields vs methods (E0499)
//!
//! Borrowck sees through field paths but not through function signatures:
//! `fn x_mut(&mut self) -> &mut i32` borrows *all* of `*self` for as long as
//! the result lives.
//!
//! ```compile_fail,E0499
//! struct P { x: i32, y: i32 }
//! impl P {
//!     fn x_mut(&mut self) -> &mut i32 { &mut self.x }
//!     fn y_mut(&mut self) -> &mut i32 { &mut self.y }
//! }
//! fn f(p: &mut P) {
//!     let a = p.x_mut();
//!     let b = p.y_mut();
//!     *a += 1;
//!     *b += 1;
//! }
//! ```
//!
//! Fix: borrow the fields directly ([`bump_both`]), or return both at once
//! ([`Point::split_mut`]).

use std::collections::HashMap;

use super::mir_reading::{BasicBlock, Local};
use super::nll::{
    Body, BodyBuilder, BorrowKind, ErrorCode, Mutability, Operand, Place, Rvalue, Statement,
    Terminator, Ty,
};

// =========================================================================================
// The fixes (real Rust)
// =========================================================================================

/// Case 2 fix: copy the element out, *then* mutate. Returns the old first
/// element, or `None` (without pushing) for an empty vector.
pub fn first_then_push(v: &mut Vec<i32>) -> Option<i32> {
    let first = *v.first()?; // the shared borrow ends here
    v.push(first);
    Some(first)
}

/// Case 3: legal thanks to two-phase borrows.
pub fn push_len(v: &mut Vec<usize>) {
    v.push(v.len());
}

/// Case 4: NLL problem case #2, accepted as written.
#[allow(clippy::implicit_hasher)] // keep the drill signature as in the case study
pub fn process_or_default(map: &mut HashMap<i32, Vec<i32>>, key: i32) {
    match map.get_mut(&key) {
        Some(v) => v.push(1),
        None => {
            map.insert(key, vec![1]);
        }
    }
}

/// Case 5 fix: the entry API does lookup-or-insert with a single borrow.
#[allow(clippy::implicit_hasher)] // keep the drill signature as in the case study
pub fn get_default(map: &mut HashMap<i32, String>, key: i32) -> &mut String {
    map.entry(key).or_default()
}

/// Case 6: NLL problem case #4, accepted as written. Returns `(a, b)`.
#[must_use]
pub const fn reassign_reborrowed() -> (i32, i32) {
    let (mut a, mut b) = (1, 2);
    let mut p = &mut a;
    let r = &mut *p;
    p = &mut b;
    *p += 1;
    *r += 1;
    (a, b)
}

/// Increments its target when dropped (case 7).
#[derive(Debug)]
pub struct Guard<'a>(pub &'a mut i32);

impl Drop for Guard<'_> {
    fn drop(&mut self) {
        *self.0 += 1;
    }
}

/// Case 7 fix: give the guard its own scope so it is dropped before `x` is
/// touched again. Returns `5 + 1`.
#[must_use]
pub fn bump_with_guard() -> i32 {
    let mut x = 0;
    {
        let _g = Guard(&mut x);
    } // drop runs here: x == 1
    x += 5;
    x
}

/// Case 10 fix: `'b: 'a` lets a `&'b` flow into the `&'a` return slot.
#[must_use]
pub const fn pick_second<'a, 'b: 'a>(_x: &'a i32, y: &'b i32) -> &'a i32 {
    y
}

/// A 2-D point for case 13.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Point {
    /// Horizontal coordinate.
    pub x: i32,
    /// Vertical coordinate.
    pub y: i32,
}

impl Point {
    /// Mutable access to both fields at once — one borrow of `self`, split
    /// inside the function where borrowck can see the fields.
    pub const fn split_mut(&mut self) -> (&mut i32, &mut i32) {
        (&mut self.x, &mut self.y)
    }
}

/// Case 13 fix: field paths are disjoint places, so two `&mut` coexist.
pub const fn bump_both(p: &mut Point) {
    let a = &mut p.x;
    let b = &mut p.y;
    *a += 1;
    *b += 1;
}

// =========================================================================================
// The same programs in toy MIR
// =========================================================================================

/// rustc's verdict on a case.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RustcVerdict {
    /// Compiles.
    Accepted,
    /// Rejected with these errors, in order.
    Rejected(&'static [ErrorCode]),
}

/// One case study: rustc's verdict and the toy-MIR encoding.
#[derive(Debug, Clone, Copy)]
pub struct CaseStudy {
    /// Short identifier.
    pub name: &'static str,
    /// What rustc says (kept honest by the doctests above).
    pub rustc: RustcVerdict,
    /// Builds the toy MIR.
    pub build: fn() -> Body,
}

/// Every case, with rustc's verdict.
pub const CASES: &[CaseStudy] = &[
    CaseStudy {
        name: "nll_problem_case_1",
        rustc: RustcVerdict::Accepted,
        build: nll_problem_case_1_mir,
    },
    CaseStudy {
        name: "shared_then_push",
        rustc: RustcVerdict::Rejected(&[ErrorCode::E0502]),
        build: shared_then_push_mir,
    },
    CaseStudy {
        name: "two_phase_push",
        rustc: RustcVerdict::Accepted,
        build: two_phase_push_mir,
    },
    CaseStudy {
        name: "explicit_reborrow_push",
        rustc: RustcVerdict::Rejected(&[ErrorCode::E0502]),
        build: explicit_reborrow_push_mir,
    },
    CaseStudy {
        name: "nll_problem_case_2",
        rustc: RustcVerdict::Accepted,
        build: nll_problem_case_2_mir,
    },
    CaseStudy {
        name: "nll_problem_case_3",
        rustc: RustcVerdict::Rejected(&[ErrorCode::E0499, ErrorCode::E0499]),
        build: nll_problem_case_3_mir,
    },
    CaseStudy {
        name: "get_default_entry",
        rustc: RustcVerdict::Accepted,
        build: get_default_entry_mir,
    },
    CaseStudy {
        name: "nll_problem_case_4",
        rustc: RustcVerdict::Accepted,
        build: nll_problem_case_4_mir,
    },
    CaseStudy {
        name: "drop_guard_assign",
        rustc: RustcVerdict::Rejected(&[ErrorCode::E0506]),
        build: drop_guard_assign_mir,
    },
    CaseStudy {
        name: "drop_guard_read",
        rustc: RustcVerdict::Rejected(&[ErrorCode::E0503]),
        build: drop_guard_read_mir,
    },
    CaseStudy {
        name: "plain_wrapper_assign",
        rustc: RustcVerdict::Accepted,
        build: plain_wrapper_assign_mir,
    },
    CaseStudy {
        name: "out_of_scope",
        rustc: RustcVerdict::Rejected(&[ErrorCode::E0597]),
        build: out_of_scope_mir,
    },
    CaseStudy {
        name: "return_local_ref",
        rustc: RustcVerdict::Rejected(&[ErrorCode::E0515]),
        build: return_local_ref_mir,
    },
    CaseStudy {
        name: "pick_wrong_lifetime",
        rustc: RustcVerdict::Rejected(&[ErrorCode::LifetimeMayNotLiveLongEnough]),
        build: pick_wrong_lifetime_mir,
    },
    CaseStudy {
        name: "pick_with_bound",
        rustc: RustcVerdict::Accepted,
        build: pick_with_bound_mir,
    },
    CaseStudy {
        name: "move_while_borrowed",
        rustc: RustcVerdict::Rejected(&[ErrorCode::E0505]),
        build: move_while_borrowed_mir,
    },
    CaseStudy {
        name: "use_while_mut_borrowed",
        rustc: RustcVerdict::Rejected(&[ErrorCode::E0503]),
        build: use_while_mut_borrowed_mir,
    },
    CaseStudy {
        name: "methods_borrow_whole_receiver",
        rustc: RustcVerdict::Rejected(&[ErrorCode::E0499]),
        build: methods_borrow_whole_receiver_mir,
    },
    CaseStudy {
        name: "disjoint_fields",
        rustc: RustcVerdict::Accepted,
        build: disjoint_fields_mir,
    },
];

fn call(
    func: &str,
    args: Vec<Operand>,
    dest: impl Into<Place>,
    returns_borrow_from: Vec<usize>,
    target: BasicBlock,
) -> Terminator {
    Terminator::Call {
        func: func.to_string(),
        args,
        dest: dest.into(),
        returns_borrow_from,
        target,
    }
}

fn copy(p: impl Into<Place>) -> Operand {
    Operand::Copy(p.into())
}

fn mov(p: impl Into<Place>) -> Operand {
    Operand::Move(p.into())
}

const fn konst(c: i64) -> Rvalue {
    Rvalue::Use(Operand::Const(c))
}

fn vec_ty() -> Ty {
    Ty::opaque("Vec<i32>")
}

fn map_ty() -> Ty {
    Ty::opaque("HashMap<i32, V>")
}

/// Case 1: `let slice = &mut data[..]; slice[0] = 9; data.push(4);`
#[must_use]
pub fn nll_problem_case_1_mir() -> Body {
    let mut b = BodyBuilder::new("nll_problem_case_1");
    let data = b.var("data", vec_ty());
    let slice_ty = b.ref_ty(Mutability::Mut, Ty::opaque("[i32]"));
    let slice = b.var("slice", slice_ty);
    let tmp_ty = b.ref_ty(Mutability::Mut, vec_ty());
    let tmp = b.temp(tmp_ty);
    let unit = b.temp(Ty::Unit);
    let recv_ty = b.ref_ty(Mutability::Mut, vec_ty());
    let recv = b.temp(recv_ty);
    let (bb0, bb1, bb2) = (b.new_block(), b.new_block(), b.new_block());
    let br = b.borrow(BorrowKind::Mut, Place::from(data));
    b.assign(bb0, tmp, br);
    b.terminate(bb0, call("index_mut", vec![mov(tmp)], slice, vec![0], bb1));
    b.assign(bb1, Place::from(slice).deref(), konst(9));
    let br = b.borrow(BorrowKind::TwoPhaseMut, Place::from(data));
    b.assign(bb1, recv, br);
    b.terminate(
        bb1,
        call(
            "push",
            vec![mov(recv), Operand::Const(4)],
            unit,
            vec![],
            bb2,
        ),
    );
    b.build()
}

fn first_and_push(receiver: BorrowKind) -> Body {
    let mut b = BodyBuilder::new("shared_then_push");
    let v = b.var("v", vec_ty());
    let first_ty = b.ref_ty(Mutability::Not, Ty::Int);
    let first = b.var("first", first_ty);
    let tmp_ty = b.ref_ty(Mutability::Not, vec_ty());
    let tmp = b.temp(tmp_ty);
    let recv_ty = b.ref_ty(Mutability::Mut, vec_ty());
    let recv = b.temp(recv_ty);
    let unit = b.temp(Ty::Unit);
    let out = b.temp(Ty::Int);
    let (bb0, bb1, bb2) = (b.new_block(), b.new_block(), b.new_block());
    let br = b.borrow(BorrowKind::Shared, Place::from(v));
    b.assign(bb0, tmp, br);
    b.terminate(bb0, call("index", vec![mov(tmp)], first, vec![0], bb1));
    let br = b.borrow(receiver, Place::from(v));
    b.assign(bb1, recv, br);
    b.terminate(
        bb1,
        call(
            "push",
            vec![mov(recv), Operand::Const(4)],
            unit,
            vec![],
            bb2,
        ),
    );
    b.assign(bb2, out, Rvalue::Use(copy(Place::from(first).deref())));
    b.build()
}

/// Case 2: `let first = &v[0]; v.push(4); println!("{first}");`
#[must_use]
pub fn shared_then_push_mir() -> Body {
    first_and_push(BorrowKind::TwoPhaseMut)
}

fn len_then_push(receiver: BorrowKind) -> Body {
    let mut b = BodyBuilder::new("push_len");
    let vt = b.ref_ty(Mutability::Mut, Ty::opaque("Vec<usize>"));
    let v = b.arg("v", vt);
    let unit = b.temp(Ty::Unit);
    let recv_ty = b.ref_ty(Mutability::Mut, Ty::opaque("Vec<usize>"));
    let recv = b.temp(recv_ty);
    let len = b.temp(Ty::Int);
    let len_arg_ty = b.ref_ty(Mutability::Not, Ty::opaque("Vec<usize>"));
    let len_arg = b.temp(len_arg_ty);
    let (bb0, bb1, bb2) = (b.new_block(), b.new_block(), b.new_block());
    let br = b.borrow(receiver, Place::from(v).deref());
    b.assign(bb0, recv, br);
    let br = b.borrow(BorrowKind::Shared, Place::from(v).deref());
    b.assign(bb0, len_arg, br);
    b.terminate(bb0, call("len", vec![mov(len_arg)], len, vec![], bb1));
    b.terminate(
        bb1,
        call("push", vec![mov(recv), mov(len)], unit, vec![], bb2),
    );
    b.build()
}

/// Case 3: `v.push(v.len())` — the receiver borrow is two-phase.
#[must_use]
pub fn two_phase_push_mir() -> Body {
    len_then_push(BorrowKind::TwoPhaseMut)
}

/// Case 3 (rejected): `Vec::push(&mut *v, v.len())` — an ordinary `&mut`.
#[must_use]
pub fn explicit_reborrow_push_mir() -> Body {
    len_then_push(BorrowKind::Mut)
}

/// Shared shape of cases 4 and 5: `match map.get_mut(&key) { Some(v) => .., None => .. }`.
fn get_mut_match(name: &str, return_value: bool) -> Body {
    let mut b = BodyBuilder::new(name);
    let (map_ty_ref, value_ty) = if return_value {
        let r = b.universal("'r");
        b.return_ty(Ty::Ref {
            region: r,
            mutbl: Mutability::Mut,
            pointee: Box::new(Ty::opaque("String")),
        });
        (
            Ty::Ref {
                region: r,
                mutbl: Mutability::Mut,
                pointee: Box::new(map_ty()),
            },
            Ty::opaque("String"),
        )
    } else {
        (b.ref_ty(Mutability::Mut, map_ty()), Ty::opaque("Vec<i32>"))
    };
    let map = b.arg("map", map_ty_ref);
    let key = b.arg("key", Ty::Int);
    let recv_ty = b.ref_ty(Mutability::Mut, map_ty());
    let recv = b.temp(recv_ty);
    let found_ty = b.ref_ty(Mutability::Mut, value_ty);
    let found = b.var("value", found_ty);
    let is_some = b.temp(Ty::Int);
    let ins_ty = b.ref_ty(Mutability::Mut, map_ty());
    let ins = b.temp(ins_ty);
    let unit = b.temp(Ty::Unit);
    let again_ty = b.ref_ty(Mutability::Mut, map_ty());
    let again = b.temp(again_ty);
    let bbs: Vec<BasicBlock> = (0..7).map(|_| b.new_block()).collect();

    // bb0: _recv = &mut *map; value = get_mut(move _recv)
    let br = b.borrow(BorrowKind::TwoPhaseMut, Place::from(map).deref());
    b.assign(bbs[0], recv, br);
    b.terminate(
        bbs[0],
        call("get_mut", vec![mov(recv)], found, vec![0], bbs[1]),
    );
    // bb1: switch on is_some(value)
    b.terminate(
        bbs[1],
        call("is_some", vec![copy(found)], is_some, vec![], bbs[2]),
    );
    b.terminate(
        bbs[2],
        Terminator::SwitchInt {
            discr: copy(is_some),
            targets: vec![bbs[4], bbs[3]],
        },
    );
    // bb3: Some(value) => ...
    if return_value {
        b.assign(bbs[3], Local::RETURN_PLACE, Rvalue::Use(mov(found)));
        b.terminate(bbs[3], Terminator::Return);
    } else {
        b.terminate(bbs[3], call("push", vec![mov(found)], unit, vec![], bbs[6]));
    }
    // bb4: None => map.insert(key, ..)
    let br = b.borrow(BorrowKind::TwoPhaseMut, Place::from(map).deref());
    b.assign(bbs[4], ins, br);
    b.terminate(
        bbs[4],
        call("insert", vec![mov(ins), copy(key)], unit, vec![], bbs[5]),
    );
    // bb5: (case 3 only) map.get_mut(&key).unwrap()
    if return_value {
        let br = b.borrow(BorrowKind::TwoPhaseMut, Place::from(map).deref());
        b.assign(bbs[5], again, br);
        b.terminate(
            bbs[5],
            call(
                "get_mut_unwrap",
                vec![mov(again)],
                Local::RETURN_PLACE,
                vec![0],
                bbs[6],
            ),
        );
    } else {
        b.terminate(bbs[5], Terminator::Goto(bbs[6]));
    }
    b.build()
}

/// Case 4: NLL problem case #2.
#[must_use]
pub fn nll_problem_case_2_mir() -> Body {
    get_mut_match("process_or_default", false)
}

/// Case 5: NLL problem case #3 (`get_default` with a conditional return).
#[must_use]
pub fn nll_problem_case_3_mir() -> Body {
    get_mut_match("get_default", true)
}

/// Case 5 fix: `map.entry(key).or_default()` — one borrow flows to `'r`.
#[must_use]
pub fn get_default_entry_mir() -> Body {
    let mut b = BodyBuilder::new("get_default");
    let r = b.universal("'r");
    b.return_ty(Ty::Ref {
        region: r,
        mutbl: Mutability::Mut,
        pointee: Box::new(Ty::opaque("String")),
    });
    let map = b.arg(
        "map",
        Ty::Ref {
            region: r,
            mutbl: Mutability::Mut,
            pointee: Box::new(map_ty()),
        },
    );
    let key = b.arg("key", Ty::Int);
    let recv_ty = b.ref_ty(Mutability::Mut, map_ty());
    let recv = b.temp(recv_ty);
    let (bb0, bb1) = (b.new_block(), b.new_block());
    let br = b.borrow(BorrowKind::TwoPhaseMut, Place::from(map).deref());
    b.assign(bb0, recv, br);
    b.terminate(
        bb0,
        call(
            "entry_or_default",
            vec![mov(recv), copy(key)],
            Local::RETURN_PLACE,
            vec![0],
            bb1,
        ),
    );
    b.build()
}

/// Case 6: `p = &mut b` kills the reborrow of the old `*p`.
#[must_use]
pub fn nll_problem_case_4_mir() -> Body {
    let mut b = BodyBuilder::new("reassign_reborrowed");
    let a = b.var("a", Ty::Int);
    let bv = b.var("b", Ty::Int);
    let p_ty = b.ref_ty(Mutability::Mut, Ty::Int);
    let p = b.var("p", p_ty);
    let r_ty = b.ref_ty(Mutability::Mut, Ty::Int);
    let r = b.var("r", r_ty);
    let out_a = b.temp(Ty::Int);
    let out_b = b.temp(Ty::Int);
    let bb0 = b.new_block();
    b.assign(bb0, a, konst(1));
    b.assign(bb0, bv, konst(2));
    let br = b.borrow(BorrowKind::Mut, Place::from(a));
    b.assign(bb0, p, br);
    let br = b.borrow(BorrowKind::Mut, Place::from(p).deref());
    b.assign(bb0, r, br);
    let br = b.borrow(BorrowKind::Mut, Place::from(bv));
    b.assign(bb0, p, br);
    b.assign(bb0, Place::from(p).deref(), konst(3));
    b.assign(bb0, Place::from(r).deref(), konst(2));
    b.assign(bb0, out_a, Rvalue::Use(copy(a)));
    b.assign(bb0, out_b, Rvalue::Use(copy(bv)));
    b.build()
}

/// `let mut x = 0; let _g = Wrapper(&mut x); [x = 5;] _0 = x; drop(_g)`.
fn wrapper(name: &str, has_drop: bool, assign: bool) -> Body {
    let mut b = BodyBuilder::new(name);
    b.return_ty(Ty::Int);
    let x = b.var("x", Ty::Int);
    let field = b.ref_ty(Mutability::Mut, Ty::Int);
    let g = b.var(
        "_g",
        Ty::Struct {
            name: if has_drop { "Guard" } else { "Plain" }.to_string(),
            fields: vec![field],
            has_drop,
        },
    );
    let tmp_ty = b.ref_ty(Mutability::Mut, Ty::Int);
    let tmp = b.temp(tmp_ty);
    let (bb0, bb1) = (b.new_block(), b.new_block());
    b.assign(bb0, x, konst(0));
    let br = b.borrow(BorrowKind::Mut, Place::from(x));
    b.assign(bb0, tmp, br);
    b.assign(
        bb0,
        g,
        Rvalue::Aggregate {
            name: if has_drop { "Guard" } else { "Plain" }.to_string(),
            operands: vec![mov(tmp)],
        },
    );
    if assign {
        b.assign(bb0, x, konst(5));
    }
    b.assign(bb0, Local::RETURN_PLACE, Rvalue::Use(copy(x)));
    b.terminate(
        bb0,
        Terminator::Drop {
            place: Place::from(g),
            target: bb1,
        },
    );
    b.build()
}

/// Case 7: assigning to `x` while a `Drop` guard holds `&mut x`.
#[must_use]
pub fn drop_guard_assign_mir() -> Body {
    wrapper("drop_guard_assign", true, true)
}

/// Case 7: reading `x` while a `Drop` guard holds `&mut x`.
#[must_use]
pub fn drop_guard_read_mir() -> Body {
    wrapper("drop_guard_read", true, false)
}

/// Case 7 control: the same program without a `Drop` impl compiles.
#[must_use]
pub fn plain_wrapper_assign_mir() -> Body {
    wrapper("plain_wrapper_assign", false, true)
}

/// Case 8: `let r; { let x = 5; r = &x; } *r`.
#[must_use]
pub fn out_of_scope_mir() -> Body {
    let mut b = BodyBuilder::new("out_of_scope");
    b.return_ty(Ty::Int);
    let r_ty = b.ref_ty(Mutability::Not, Ty::Int);
    let r = b.var("r", r_ty);
    let x = b.var("x", Ty::Int);
    let bb0 = b.new_block();
    b.push(bb0, Statement::StorageLive(x));
    b.assign(bb0, x, konst(5));
    let br = b.borrow(BorrowKind::Shared, Place::from(x));
    b.assign(bb0, r, br);
    b.push(bb0, Statement::StorageDead(x));
    b.assign(
        bb0,
        Local::RETURN_PLACE,
        Rvalue::Use(copy(Place::from(r).deref())),
    );
    b.build()
}

/// Case 9: `fn dangle() -> &'static i32 { let x = 5; &x }`.
#[must_use]
pub fn return_local_ref_mir() -> Body {
    let mut b = BodyBuilder::new("dangle");
    let st = b.universal("'static");
    b.return_ty(Ty::Ref {
        region: st,
        mutbl: Mutability::Not,
        pointee: Box::new(Ty::Int),
    });
    let x = b.var("x", Ty::Int);
    let bb0 = b.new_block();
    b.assign(bb0, x, konst(5));
    let br = b.borrow(BorrowKind::Shared, Place::from(x));
    b.assign(bb0, Local::RETURN_PLACE, br);
    b.build()
}

fn pick(bound: bool) -> Body {
    let mut b = BodyBuilder::new("pick");
    let a = b.universal("'a");
    let bl = b.universal("'b");
    if bound {
        b.assume_outlives(bl, a);
    }
    let shared = |region| Ty::Ref {
        region,
        mutbl: Mutability::Not,
        pointee: Box::new(Ty::Int),
    };
    b.return_ty(shared(a));
    b.arg("x", shared(a));
    let y = b.arg("y", shared(bl));
    let bb0 = b.new_block();
    b.assign(bb0, Local::RETURN_PLACE, Rvalue::Use(copy(y)));
    b.build()
}

/// Case 10: `fn pick<'a, 'b>(x: &'a i32, y: &'b i32) -> &'a i32 { y }`.
#[must_use]
pub fn pick_wrong_lifetime_mir() -> Body {
    pick(false)
}

/// Case 10 fix: `'b: 'a`.
#[must_use]
pub fn pick_with_bound_mir() -> Body {
    pick(true)
}

/// Case 11: `let r = &s; let t = s; r.len()`.
#[must_use]
#[allow(clippy::many_single_char_names)] // names mirror the Rust being encoded
pub fn move_while_borrowed_mir() -> Body {
    let mut b = BodyBuilder::new("move_while_borrowed");
    let s = b.var("s", Ty::opaque("String"));
    let r_ty = b.ref_ty(Mutability::Not, Ty::opaque("String"));
    let r = b.var("r", r_ty);
    let t = b.var("t", Ty::opaque("String"));
    let n = b.temp(Ty::Int);
    let (bb0, bb1) = (b.new_block(), b.new_block());
    b.assign(bb0, s, konst(0));
    let br = b.borrow(BorrowKind::Shared, Place::from(s));
    b.assign(bb0, r, br);
    b.assign(bb0, t, Rvalue::Use(mov(s)));
    b.terminate(bb0, call("len", vec![copy(r)], n, vec![], bb1));
    b.build()
}

/// Case 12: `let r = &mut x; let y = x; *r += 1;`.
#[must_use]
pub fn use_while_mut_borrowed_mir() -> Body {
    let mut b = BodyBuilder::new("use_while_mut_borrowed");
    b.return_ty(Ty::Int);
    let x = b.var("x", Ty::Int);
    let r_ty = b.ref_ty(Mutability::Mut, Ty::Int);
    let r = b.var("r", r_ty);
    let y = b.var("y", Ty::Int);
    let bb0 = b.new_block();
    b.assign(bb0, x, konst(1));
    let br = b.borrow(BorrowKind::Mut, Place::from(x));
    b.assign(bb0, r, br);
    b.assign(bb0, y, Rvalue::Use(copy(x)));
    b.assign(bb0, Place::from(r).deref(), konst(2));
    b.assign(bb0, Local::RETURN_PLACE, Rvalue::Use(copy(y)));
    b.build()
}

fn point_ty() -> Ty {
    Ty::Struct {
        name: "P".to_string(),
        fields: vec![Ty::Int, Ty::Int],
        has_drop: false,
    }
}

/// Case 13: `let a = p.x_mut(); let b = p.y_mut(); *a += 1;`.
#[must_use]
pub fn methods_borrow_whole_receiver_mir() -> Body {
    let mut b = BodyBuilder::new("methods_borrow_whole_receiver");
    let p_ty = b.ref_ty(Mutability::Mut, point_ty());
    let p = b.arg("p", p_ty);
    let a_ty = b.ref_ty(Mutability::Mut, Ty::Int);
    let a = b.var("a", a_ty);
    let bv_ty = b.ref_ty(Mutability::Mut, Ty::Int);
    let bv = b.var("b", bv_ty);
    let r1_ty = b.ref_ty(Mutability::Mut, point_ty());
    let r1 = b.temp(r1_ty);
    let r2_ty = b.ref_ty(Mutability::Mut, point_ty());
    let r2 = b.temp(r2_ty);
    let (bb0, bb1, bb2) = (b.new_block(), b.new_block(), b.new_block());
    let br = b.borrow(BorrowKind::TwoPhaseMut, Place::from(p).deref());
    b.assign(bb0, r1, br);
    b.terminate(bb0, call("x_mut", vec![mov(r1)], a, vec![0], bb1));
    let br = b.borrow(BorrowKind::TwoPhaseMut, Place::from(p).deref());
    b.assign(bb1, r2, br);
    b.terminate(bb1, call("y_mut", vec![mov(r2)], bv, vec![0], bb2));
    b.assign(bb2, Place::from(a).deref(), konst(1));
    b.assign(bb2, Place::from(bv).deref(), konst(1));
    b.build()
}

/// Case 13 fix: `let a = &mut p.x; let b = &mut p.y;`.
#[must_use]
pub fn disjoint_fields_mir() -> Body {
    let mut b = BodyBuilder::new("disjoint_fields");
    let p_ty = b.ref_ty(Mutability::Mut, point_ty());
    let p = b.arg("p", p_ty);
    let a_ty = b.ref_ty(Mutability::Mut, Ty::Int);
    let a = b.var("a", a_ty);
    let bv_ty = b.ref_ty(Mutability::Mut, Ty::Int);
    let bv = b.var("b", bv_ty);
    let bb0 = b.new_block();
    let br = b.borrow(BorrowKind::Mut, Place::from(p).deref().field(0));
    b.assign(bb0, a, br);
    let br = b.borrow(BorrowKind::Mut, Place::from(p).deref().field(1));
    b.assign(bb0, bv, br);
    b.assign(bb0, Place::from(a).deref(), konst(1));
    b.assign(bb0, Place::from(bv).deref(), konst(1));
    b.build()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler_literacy::mir_reading::Location;
    use crate::compiler_literacy::nll::borrowck;

    #[test]
    fn mini_borrowck_agrees_with_rustc_on_every_case() {
        for case in CASES {
            let body = (case.build)();
            let result = borrowck(&body);
            let expected: Vec<ErrorCode> = match case.rustc {
                RustcVerdict::Accepted => Vec::new(),
                RustcVerdict::Rejected(codes) => codes.to_vec(),
            };
            assert_eq!(
                result.codes(),
                expected,
                "case `{}`:\n{body}\n{:#?}",
                case.name,
                result.diagnostics
            );
        }
    }

    #[test]
    fn case_names_are_unique() {
        let mut names: Vec<&str> = CASES.iter().map(|c| c.name).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), CASES.len());
    }

    #[test]
    fn messages_match_rustc_wording() {
        let msg = |body: Body| borrowck(&body).diagnostics[0].message.clone();
        assert_eq!(
            msg(shared_then_push_mir()),
            "cannot borrow `v` as mutable because it is also borrowed as immutable"
        );
        assert_eq!(
            msg(explicit_reborrow_push_mir()),
            "cannot borrow `*v` as immutable because it is also borrowed as mutable"
        );
        assert_eq!(
            msg(nll_problem_case_3_mir()),
            "cannot borrow `*map` as mutable more than once at a time"
        );
        assert_eq!(
            msg(drop_guard_assign_mir()),
            "cannot assign to `x` because it is borrowed"
        );
        assert_eq!(
            msg(drop_guard_read_mir()),
            "cannot use `x` because it was mutably borrowed"
        );
        assert_eq!(msg(out_of_scope_mir()), "`x` does not live long enough");
        assert_eq!(
            msg(return_local_ref_mir()),
            "cannot return reference to local variable `x`"
        );
        assert_eq!(
            msg(pick_wrong_lifetime_mir()),
            "lifetime may not live long enough: `'b` must outlive `'a`"
        );
        assert_eq!(
            msg(move_while_borrowed_mir()),
            "cannot move out of `s` because it is borrowed"
        );
        assert_eq!(
            msg(methods_borrow_whole_receiver_mir()),
            "cannot borrow `*p` as mutable more than once at a time"
        );
    }

    #[test]
    fn two_phase_reservation_and_activation() {
        let result = borrowck(&two_phase_push_mir());
        let recv = result
            .borrow_at(Location::new(0, 0))
            .expect("receiver borrow");
        assert_eq!(recv.kind, BorrowKind::TwoPhaseMut);
        // Activated by the `push` call, not before.
        assert_eq!(
            recv.activations.iter().copied().collect::<Vec<_>>(),
            vec![Location::new(1, 0)]
        );
        assert!(recv.activated.is_empty(), "nothing after the call uses it");
        // The `len` borrow ends before activation.
        let len_borrow = result.borrow_at(Location::new(0, 1)).expect("len borrow");
        assert!(!len_borrow.in_scope.contains(&Location::new(1, 0)));
        // shared_then_push reports the conflict at the activation (the call).
        let result = borrowck(&shared_then_push_mir());
        assert_eq!(result.diagnostics[0].location, Some(Location::new(1, 1)));
    }

    #[test]
    fn problem_case_3_is_location_insensitivity() {
        let body = nll_problem_case_3_mir();
        let result = borrowck(&body);
        let first = result
            .borrow_at(Location::new(0, 0))
            .expect("get_mut borrow");
        let value = result.region_values.get(first.region);
        // Because `value` flows into the return slot ('r), the borrow's region
        // contains end('r) and every point — including the `None` arm (bb4).
        assert!(value.contains_point(Location::new(4, 0)));
        assert!(value.contains_universal(body.regions.universals()[0].vid));
        // Problem case #2 has the same shape minus the return: the borrow
        // never reaches the None arm.
        let case2 = borrowck(&nll_problem_case_2_mir());
        let first = case2
            .borrow_at(Location::new(0, 0))
            .expect("get_mut borrow");
        assert!(
            !case2
                .region_values
                .get(first.region)
                .contains_point(Location::new(4, 0))
        );
        assert!(
            first.in_scope.contains(&Location::new(3, 0)),
            "Some arm uses it"
        );
    }

    #[test]
    fn overwriting_a_reference_kills_reborrows() {
        let result = borrowck(&nll_problem_case_4_mir());
        let reborrow = result.borrow_at(Location::new(0, 3)).expect("r = &mut *p");
        // Still in scope at the overwrite (r is live), gone right after it.
        assert!(reborrow.in_scope.contains(&Location::new(0, 4)));
        assert!(!reborrow.in_scope.contains(&Location::new(0, 5)));
        assert!(result.is_ok());
    }

    #[test]
    fn drop_liveness_extends_the_borrow() {
        let with_drop = borrowck(&drop_guard_read_mir());
        let plain = borrowck(&wrapper("plain_read", false, false));
        let at_read = Location::new(0, 3);
        let guard = Local(2);
        assert!(with_drop.liveness.is_live(guard, at_read));
        assert!(!plain.liveness.is_live(guard, at_read));
        assert!(plain.is_ok());
    }

    #[test]
    fn region_error_blames_the_return() {
        let result = borrowck(&pick_wrong_lifetime_mir());
        assert_eq!(result.diagnostics[0].location, Some(Location::new(0, 0)));
        assert_eq!(result.diagnostics[0].borrow_location, None);
    }

    #[test]
    fn printed_mir_shows_regions_and_two_phase() {
        let text = two_phase_push_mir().to_string();
        assert!(
            text.contains("fn push_len(_1: &'?0 mut Vec<usize>) -> () {"),
            "{text}"
        );
        assert!(text.contains("_3 = &'?3 mut (*_1); // two-phase"), "{text}");
        assert!(text.contains("_2 = push(move _3, move _4) -> [return: bb2, unwind continue];"));
        let text = drop_guard_assign_mir().to_string();
        assert!(text.contains("let _2: Guard<'?0>;"), "{text}");
        assert!(text.contains("_2 = Guard(move _3);"));
        assert!(text.contains("drop(_2) -> [return: bb1, unwind continue];"));
    }

    // ---------------------------------------------------------------- real-Rust fixes

    #[test]
    fn first_then_push_copies_before_mutating() {
        let mut v = vec![3, 4];
        assert_eq!(first_then_push(&mut v), Some(3));
        assert_eq!(v, [3, 4, 3]);
        let mut empty = Vec::new();
        assert_eq!(first_then_push(&mut empty), None);
        assert!(empty.is_empty());
    }

    #[test]
    fn push_len_uses_two_phase_borrow() {
        let mut v = Vec::new();
        push_len(&mut v);
        push_len(&mut v);
        assert_eq!(v, [0, 1]);
    }

    #[test]
    fn process_or_default_inserts_then_appends() {
        let mut m = HashMap::new();
        process_or_default(&mut m, 1);
        process_or_default(&mut m, 1);
        process_or_default(&mut m, 2);
        assert_eq!(m[&1], [1, 1]);
        assert_eq!(m[&2], [1]);
    }

    #[test]
    fn get_default_inserts_once() {
        let mut m = HashMap::new();
        get_default(&mut m, 5).push_str("hi");
        get_default(&mut m, 5).push('!');
        assert_eq!(m[&5], "hi!");
        assert_eq!(m.len(), 1);
    }

    #[test]
    fn reassign_reborrowed_mutates_both() {
        assert_eq!(reassign_reborrowed(), (2, 3));
    }

    #[test]
    fn guard_runs_before_reuse() {
        assert_eq!(bump_with_guard(), 6);
        let mut x = 10;
        drop(Guard(&mut x));
        assert_eq!(x, 11);
    }

    #[test]
    fn pick_second_with_bound() {
        let a = 1;
        let b = 2;
        assert_eq!(*pick_second(&a, &b), 2);
    }

    #[test]
    fn disjoint_field_borrows() {
        let mut p = Point { x: 1, y: 2 };
        bump_both(&mut p);
        assert_eq!(p, Point { x: 2, y: 3 });
        let (x, y) = p.split_mut();
        *x *= 10;
        *y *= 10;
        assert_eq!(p, Point { x: 20, y: 30 });
    }
}
