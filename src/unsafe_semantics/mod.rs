//! # Unsafe Semantics: Stacked Borrows vs Tree Borrows
//!
//! `unsafe` code is only sound if it obeys Rust's **aliasing model** — rules that
//! decide when a reference or raw pointer may still be used. The borrow checker
//! enforces them for safe code; for raw pointers *you* must, and the compiler
//! optimizes as if you did. Miri is the tool that checks them at runtime.
//!
//! Rust has two candidate models, both implemented in Miri:
//!
//! - **Stacked Borrows** (Miri's default) — a borrow *stack* per location.
//! - **Tree Borrows** (`MIRIFLAGS=-Zmiri-tree-borrows`) — a borrow *tree* per
//!   allocation; more permissive in several places real code relies on.
//!
//! This module has three layers:
//!
//! | Module                                   | What it is                                  |
//! |------------------------------------------|---------------------------------------------|
//! | [`trace`]                                | model-independent pointer-operation traces  |
//! | [`stacked_borrows`] / [`tree_borrows`]   | executable toy models of both rules         |
//! | [`counterexamples`]                      | real `unsafe` Rust: UB programs + their fixes, with recorded Miri verdicts |
//!
//! ```mermaid
//! flowchart LR
//!     C[counterexamples.rs<br/>real unsafe Rust] -->|cargo miri run<br/>SB and TB| M[Miri verdicts]
//!     C -->|hand-written traces| T[trace::Op]
//!     T --> SB[stacked_borrows]
//!     T --> TB[tree_borrows]
//!     M -. must agree .- SB
//!     M -. must agree .- TB
//! ```
//!
//! ## The rules in one paragraph each
//!
//! **Stacked Borrows.** Creating `&mut` is a *write* through its parent and
//! pushes a `Unique` item; `&T` is a read and pushes `SharedReadOnly`; a raw
//! pointer from `&mut` slots a `SharedReadWrite` item right above its parent.
//! Using a pointer pops everything above it that its access conflicts with; using
//! a pointer whose item is gone is UB. Permissions exist only for the bytes a
//! reference covers.
//!
//! **Tree Borrows.** Every reference is a node under its parent; raw pointers
//! share their parent's node. A fresh `&mut` is `Reserved` (survives foreign
//! reads) until its first write makes it `Active`; `&T` is `Frozen`. Foreign
//! writes disable a node, foreign reads freeze an `Active` one. Permissions cover
//! the whole allocation.
//!
//! Both models add **protectors**: a `&mut` function argument must stay valid
//! until the call returns, even if the function never touches it again.
//!
//! ## Running it
//!
//! ```bash
//! cargo test --lib unsafe_semantics                     # native: models + sound fixes
//! cargo +nightly miri test --lib unsafe_semantics       # sound fixes under Stacked Borrows
//! MIRIFLAGS=-Zmiri-tree-borrows \
//!     cargo +nightly miri test --lib unsafe_semantics   # ... and under Tree Borrows
//! scripts/miri-counterexamples.sh                       # every UB verdict, both models
//! ```
//!
//! ## Interview takeaways
//!
//! 1. Derive raw pointers **once**, from the widest reference you need
//!    (`as_mut_ptr()` on the whole buffer), and don't interleave them with new
//!    references to the same memory.
//! 2. A `&T` freezes its target for as long as it is used; a `&mut` argument
//!    freezes *everyone else* for the whole call.
//! 3. Code that passes Tree Borrows but fails Stacked Borrows is not "fine" —
//!    neither model is normative yet. Write code that passes **both**.

pub mod counterexamples;
pub mod stacked_borrows;
pub mod trace;
pub mod tree_borrows;

#[cfg(test)]
mod tests {
    //! Small-scope exhaustive checks: enumerate every *well-nested* trace up to a
    //! bounded length and verify properties of both models on all of them.

    use super::trace::{Op, ROOT, mut_ref, raw, read, shared_ref, write};
    use super::{stacked_borrows, tree_borrows};

    const NAMES: [&str; 8] = ["p0", "p1", "p2", "p3", "p4", "p5", "p6", "p7"];
    /// Exhaustive depth: 6 actions natively; Miri is ~1000x slower.
    const DEPTH: usize = if cfg!(miri) { 3 } else { 6 };

    #[derive(Clone, Copy)]
    enum Action {
        PushMut,
        PushShared,
        PushRaw,
        Read,
        Write,
        Pop,
    }

    const ACTIONS: [Action; 6] = [
        Action::PushMut,
        Action::PushShared,
        Action::PushRaw,
        Action::Read,
        Action::Write,
        Action::Pop,
    ];

    /// Pointers currently "in scope", innermost last, with their mutability.
    type Scope = Vec<(&'static str, bool)>;

    /// What a legal action does to the trace.
    enum Step {
        /// Append this op.
        Emit(Op),
        /// The innermost borrow went out of scope; nothing to append.
        Pop,
    }

    /// Turn an action into a [`Step`] if it is legal in the current scope, the
    /// way the borrow checker would only ever allow using the innermost borrow.
    fn step(scope: &mut Scope, fresh: &mut usize, action: Action) -> Option<Step> {
        let &(top, mutable) = scope.last()?;
        let mut push = |op: fn(&'static str, &'static str) -> Op, is_mut: bool| {
            let name = NAMES[*fresh];
            *fresh += 1;
            scope.push((name, is_mut));
            Some(Step::Emit(op(name, top)))
        };
        match action {
            Action::PushMut if mutable => push(mut_ref, true),
            Action::PushShared => push(shared_ref, false),
            Action::PushRaw => push(raw, mutable),
            Action::Read => Some(Step::Emit(read(top, 0))),
            Action::Write if mutable => Some(Step::Emit(write(top, 0))),
            Action::Pop if scope.len() > 1 => {
                scope.pop();
                Some(Step::Pop)
            }
            _ => None,
        }
    }

    /// Every well-nested trace of exactly `DEPTH` legal actions (and all prefixes).
    fn well_nested_traces() -> Vec<Vec<Op>> {
        fn go(scope: &Scope, fresh: usize, ops: &[Op], depth: usize, out: &mut Vec<Vec<Op>>) {
            out.push(ops.to_vec());
            if depth == 0 {
                return;
            }
            for action in ACTIONS {
                let mut scope = scope.clone();
                let mut fresh = fresh;
                if let Some(next) = step(&mut scope, &mut fresh, action) {
                    let mut ops = ops.to_vec();
                    if let Step::Emit(op) = next {
                        ops.push(op);
                    }
                    go(&scope, fresh, &ops, depth - 1, out);
                }
            }
        }
        let mut out = Vec::new();
        go(&vec![(ROOT, true)], 0, &[], DEPTH, &mut out);
        out
    }

    #[test]
    fn test_enumeration_is_nontrivial() {
        let traces = well_nested_traces();
        let longest = traces.iter().map(Vec::len).max().unwrap_or(0);
        assert_eq!(longest, DEPTH);
        assert!(traces.len() > DEPTH * ACTIONS.len());
    }

    /// Borrow-checker-shaped (LIFO) pointer usage is accepted by both models.
    #[test]
    fn test_well_nested_traces_are_accepted_by_both_models() {
        for ops in well_nested_traces() {
            assert_eq!(stacked_borrows::run(1, &ops), Ok(()), "SB: {ops:?}");
            assert_eq!(tree_borrows::run(1, &ops), Ok(()), "TB: {ops:?}");
        }
    }

    /// Without protectors, the owner can always take its memory back: after any
    /// well-nested prefix, reading and writing through the root is never UB.
    #[test]
    fn test_owner_can_always_reclaim() {
        for prefix in well_nested_traces() {
            for reclaim in [read(ROOT, 0), write(ROOT, 0)] {
                let mut ops = prefix.clone();
                ops.push(reclaim);
                assert_eq!(stacked_borrows::run(1, &ops), Ok(()), "SB: {ops:?}");
                assert_eq!(tree_borrows::run(1, &ops), Ok(()), "TB: {ops:?}");
            }
        }
    }

    /// After the owner writes, every *derived* pointer is dead in both models:
    /// reading through any of them is UB.
    #[test]
    fn test_owner_write_kills_all_derived_references() {
        for prefix in well_nested_traces() {
            let derived: Vec<&'static str> = NAMES
                .iter()
                .copied()
                .filter(|name| {
                    prefix.iter().any(|op| {
                        matches!(op, Op::RetagMut { new, .. } | Op::RetagShared { new, .. } if new == name)
                    })
                })
                .collect();
            for name in derived {
                let mut ops = prefix.clone();
                ops.push(write(ROOT, 0));
                ops.push(read(name, 0));
                assert_eq!(
                    stacked_borrows::verdict(1, &ops),
                    super::trace::Verdict::Ub,
                    "SB: {ops:?}"
                );
                assert_eq!(
                    tree_borrows::verdict(1, &ops),
                    super::trace::Verdict::Ub,
                    "TB: {ops:?}"
                );
            }
        }
    }
}
