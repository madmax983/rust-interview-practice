//! # Compiler Literacy
//!
//! Drills for reading what rustc's borrow checker actually sees, so a borrowck
//! error becomes a statement about a control-flow graph rather than a riddle.
//! Work through them in order; each builds on the previous:
//!
//! 1. [`mir_reading`] — the `-Zunpretty=mir` text format: locals, places,
//!    operands, points, terminators. Real rustc dumps plus a parser that turns
//!    them into a CFG you can query.
//! 2. [`region_constraints`] — regions as sets of points, liveness and outlives
//!    constraints, the least-fixpoint solver, universal regions and blame
//!    paths. The solver reproduces rustc's own *Inferred Region Values* from a
//!    captured `-Zdump-mir=nll` file.
//! 3. [`nll`] — a miniature `rustc_borrowck`: liveness → constraint generation →
//!    region solve → borrows in scope → access checks, over a toy MIR with
//!    reborrows, kills, two-phase borrows and drop-liveness.
//! 4. [`borrowck_case_studies`] — the classic NLL problem cases and the common
//!    error codes (E0499, E0502, E0503, E0505, E0506, E0515, E0597, "lifetime
//!    may not live long enough"), each as a rustc `compile_fail` doctest, a
//!    fix, and a toy-MIR encoding the mini checker must judge the same way.
//!
//! ```text
//!   source ──► MIR ───────────► liveness ──► constraints ──► solve ──► borrows in scope ──► errors
//!              (mir_reading)    (nll)        (nll)           (region_  (nll)                 (nll, checked
//!                                                             constraints)                   against rustc in
//!                                                                                            borrowck_case_studies)
//! ```
//!
//! ## Keeping the notes honest
//!
//! - Captured MIR / NLL dumps are real rustc 1.97 output; the tests parse them
//!   and the region solver must reproduce rustc's answer exactly.
//! - Rejected programs are `compile_fail,E0xxx` doctests. rustdoc only checks
//!   the error *code* on nightly, so CI runs these doctests on nightly too:
//!
//! ```bash
//! cargo test compiler_literacy                       # unit tests + doctests (stable)
//! cargo +nightly test --doc compiler_literacy        # also verifies E0xxx codes
//! ```

pub mod borrowck_case_studies;
pub mod mir_reading;
pub mod nll;
pub mod region_constraints;
