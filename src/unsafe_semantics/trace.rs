//! # Pointer Traces
//!
//! A tiny, model-independent vocabulary for describing what a piece of unsafe code
//! does to a single allocation: which references and raw pointers it creates, from
//! which parent, and which locations it reads or writes through them.
//!
//! Both [`stacked_borrows`](super::stacked_borrows) and
//! [`tree_borrows`](super::tree_borrows) consume the same [`Op`] sequence, so one
//! trace can be judged by both aliasing models side by side.
//!
//! ## Example
//!
//! ```
//! use rust_interview_practice::unsafe_semantics::trace::{raw, mut_ref, write, Verdict};
//! use rust_interview_practice::unsafe_semantics::{stacked_borrows, tree_borrows};
//!
//! // let p = &raw mut x; let r = &mut *p; *p = 1; *r = 2;
//! let ops = [raw("p", "x"), mut_ref("r", "p"), write("p", 0), write("r", 0)];
//! assert_eq!(stacked_borrows::verdict(1, &ops), Verdict::Ub);
//! assert_eq!(tree_borrows::verdict(1, &ops), Verdict::Ub);
//! ```

use std::error::Error;
use std::fmt;
use std::ops::Range;

/// Name of the pointer that owns the allocation (think: the local variable `x`).
pub const ROOT: &str = "x";

/// One step of a pointer trace.
///
/// Pointer names are plain strings so traces read like the Rust they describe.
/// Every name other than [`ROOT`] must be introduced by a retag (`RetagMut`,
/// `RetagShared`, `Raw`) before it is used.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Op {
    /// `let new = &mut *from;` covering the locations in `range`.
    ///
    /// `protect: true` models a `&mut` function argument: the reference is
    /// *protected* until the matching [`Op::EndProtect`] (the function returns).
    RetagMut {
        new: &'static str,
        from: &'static str,
        range: Range<usize>,
        protect: bool,
    },
    /// `let new = &*from;` covering the locations in `range`.
    RetagShared {
        new: &'static str,
        from: &'static str,
        range: Range<usize>,
    },
    /// `let new = from as *mut T;` (or `ptr::from_mut`, `&raw mut *from`).
    Raw {
        new: &'static str,
        from: &'static str,
        range: Range<usize>,
    },
    /// `*via.add(loc)` used as a value.
    Read { via: &'static str, loc: usize },
    /// `*via.add(loc) = ...`.
    Write { via: &'static str, loc: usize },
    /// The function that received the protected reference `tag` returns.
    EndProtect { tag: &'static str },
}

/// `let new = &mut *from;` over location 0.
#[must_use]
pub const fn mut_ref(new: &'static str, from: &'static str) -> Op {
    Op::RetagMut {
        new,
        from,
        range: 0..1,
        protect: false,
    }
}

/// A `&mut` function argument: like [`mut_ref`], but protected until
/// [`end_protect`].
#[must_use]
pub const fn mut_arg(new: &'static str, from: &'static str) -> Op {
    Op::RetagMut {
        new,
        from,
        range: 0..1,
        protect: true,
    }
}

/// `let new = &*from;` over location 0.
#[must_use]
pub const fn shared_ref(new: &'static str, from: &'static str) -> Op {
    Op::RetagShared {
        new,
        from,
        range: 0..1,
    }
}

/// `let new = from as *mut T;` over location 0.
#[must_use]
pub const fn raw(new: &'static str, from: &'static str) -> Op {
    Op::Raw {
        new,
        from,
        range: 0..1,
    }
}

/// Read location `loc` through `via`.
#[must_use]
pub const fn read(via: &'static str, loc: usize) -> Op {
    Op::Read { via, loc }
}

/// Write location `loc` through `via`.
#[must_use]
pub const fn write(via: &'static str, loc: usize) -> Op {
    Op::Write { via, loc }
}

/// The protected reference `tag` goes out of scope (its function returned).
#[must_use]
pub const fn end_protect(tag: &'static str) -> Op {
    Op::EndProtect { tag }
}

/// Did a model (or Miri) accept the program?
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Verdict {
    /// Every access was permitted.
    Ok,
    /// The aliasing model flagged undefined behaviour.
    Ub,
}

impl fmt::Display for Verdict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Ok => "ok",
            Self::Ub => "ub",
        })
    }
}

/// Why an aliasing model rejected an access.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UbKind {
    /// Accessed a location outside the allocation.
    OutOfBounds,
    /// Stacked Borrows: no item in the location's stack grants this access
    /// (the pointer was popped, or never covered this location).
    NoGrantingItem,
    /// Stacked Borrows: the access would pop a protected item.
    ProtectorPopped,
    /// Tree Borrows: wrote through a `Frozen` (shared-reference) permission.
    WriteThroughFrozen,
    /// Tree Borrows: accessed through a `Disabled` permission.
    AccessThroughDisabled,
    /// Tree Borrows: a foreign access would invalidate a protected reference.
    ProtectorInvalidated,
}

impl fmt::Display for UbKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::OutOfBounds => "out-of-bounds access",
            Self::NoGrantingItem => "no item in the borrow stack grants this access",
            Self::ProtectorPopped => "access would pop a protected item",
            Self::WriteThroughFrozen => "write through a Frozen (shared) permission",
            Self::AccessThroughDisabled => "access through a Disabled permission",
            Self::ProtectorInvalidated => "foreign access invalidates a protected reference",
        })
    }
}

/// Error produced while running a trace through a model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TraceError {
    /// The model detected undefined behaviour at `step`, accessing through `ptr`.
    Ub {
        step: usize,
        ptr: &'static str,
        kind: UbKind,
    },
    /// The trace itself is ill-formed: it used `ptr` before introducing it.
    UnknownPointer { step: usize, ptr: &'static str },
    /// The trace itself is ill-formed: it introduced `ptr` twice.
    DuplicatePointer { step: usize, ptr: &'static str },
}

impl TraceError {
    /// `Some(kind)` if this is undefined behaviour rather than a malformed trace.
    #[must_use]
    pub const fn ub_kind(&self) -> Option<UbKind> {
        match self {
            Self::Ub { kind, .. } => Some(*kind),
            Self::UnknownPointer { .. } | Self::DuplicatePointer { .. } => None,
        }
    }
}

impl fmt::Display for TraceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ub { step, ptr, kind } => {
                write!(f, "undefined behaviour at step {step} via `{ptr}`: {kind}")
            }
            Self::UnknownPointer { step, ptr } => {
                write!(
                    f,
                    "malformed trace: step {step} uses unknown pointer `{ptr}`"
                )
            }
            Self::DuplicatePointer { step, ptr } => {
                write!(f, "malformed trace: step {step} redefines pointer `{ptr}`")
            }
        }
    }
}

impl Error for TraceError {}

/// Collapse a model run into a [`Verdict`].
///
/// # Panics
///
/// Panics if the trace is malformed — that is a bug in the trace, not a verdict.
#[must_use]
pub fn to_verdict(result: &Result<(), TraceError>) -> Verdict {
    match result {
        Ok(()) => Verdict::Ok,
        Err(TraceError::Ub { .. }) => Verdict::Ub,
        Err(malformed) => panic!("{malformed}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_helpers_cover_location_zero() {
        assert_eq!(
            mut_ref("r", ROOT),
            Op::RetagMut {
                new: "r",
                from: "x",
                range: 0..1,
                protect: false
            }
        );
        assert!(matches!(
            mut_arg("r", ROOT),
            Op::RetagMut { protect: true, .. }
        ));
        assert!(matches!(shared_ref("s", ROOT), Op::RetagShared { range, .. } if range == (0..1)));
        assert!(matches!(raw("p", ROOT), Op::Raw { range, .. } if range == (0..1)));
    }

    #[test]
    fn test_verdict_display() {
        assert_eq!(Verdict::Ok.to_string(), "ok");
        assert_eq!(Verdict::Ub.to_string(), "ub");
    }

    #[test]
    fn test_trace_error_display_and_kind() {
        let ub = TraceError::Ub {
            step: 3,
            ptr: "r",
            kind: UbKind::NoGrantingItem,
        };
        assert_eq!(ub.ub_kind(), Some(UbKind::NoGrantingItem));
        assert!(ub.to_string().contains("step 3 via `r`"));

        let unknown = TraceError::UnknownPointer { step: 0, ptr: "q" };
        assert_eq!(unknown.ub_kind(), None);
        assert!(unknown.to_string().contains("unknown pointer `q`"));

        let dup = TraceError::DuplicatePointer { step: 1, ptr: "x" };
        assert!(dup.to_string().contains("redefines pointer `x`"));
    }

    #[test]
    fn test_every_ub_kind_has_a_distinct_message() {
        let kinds = [
            UbKind::OutOfBounds,
            UbKind::NoGrantingItem,
            UbKind::ProtectorPopped,
            UbKind::WriteThroughFrozen,
            UbKind::AccessThroughDisabled,
            UbKind::ProtectorInvalidated,
        ];
        let messages: std::collections::HashSet<String> =
            kinds.iter().map(ToString::to_string).collect();
        assert_eq!(messages.len(), kinds.len());
        assert!(messages.iter().all(|m| !m.is_empty()));
    }

    #[test]
    fn test_to_verdict() {
        assert_eq!(to_verdict(&Ok(())), Verdict::Ok);
        let ub = Err(TraceError::Ub {
            step: 0,
            ptr: "x",
            kind: UbKind::OutOfBounds,
        });
        assert_eq!(to_verdict(&ub), Verdict::Ub);
    }

    #[test]
    #[should_panic(expected = "malformed trace")]
    fn test_to_verdict_panics_on_malformed_trace() {
        let _ = to_verdict(&Err(TraceError::UnknownPointer { step: 0, ptr: "q" }));
    }
}
