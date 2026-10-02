//! # Tree Borrows (toy model)
//!
//! Tree Borrows (Villani, Hostert, Dreyer & Jung, PLDI 2025) replaces the
//! per-location *stack* with a per-allocation **tree** of references: every
//! retag adds a child node under the pointer it was derived from. Raw pointers
//! do **not** get their own node — they share the tag of the reference they
//! came from.
//!
//! Each node holds a permission per location. An access through node `n` is a
//! **child** (local) access for `n` and its ancestors and a **foreign** access
//! for every other node:
//!
//! ```text
//!                  child read     child write    foreign read   foreign write
//! Reserved   ->    Reserved       Active         Reserved       Disabled
//! Active     ->    Active         Active         Frozen         Disabled
//! Frozen     ->    Frozen         UB             Frozen         Disabled
//! Disabled   ->    UB             UB             Disabled       Disabled
//! ```
//!
//! ```mermaid
//! stateDiagram-v2
//!     [*] --> Reserved: &mut retag
//!     [*] --> Frozen: & retag
//!     Reserved --> Active: child write
//!     Reserved --> Disabled: foreign write
//!     Active --> Frozen: foreign read
//!     Active --> Disabled: foreign write
//!     Frozen --> Disabled: foreign write
//! ```
//!
//! A fresh `&mut` starts **Reserved**, not Active: it tolerates foreign *reads*
//! until its first write. That single change (plus raw pointers not being
//! retagged and permissions existing for the whole allocation) is why Tree
//! Borrows accepts several programs Stacked Borrows rejects.
//!
//! **Protectors:** while a `&mut` argument is protected, a foreign write (or a
//! foreign read of an `Active` node) that would disable it is UB, and a foreign
//! read marks a `Reserved` node *conflicted* so its first child write is UB.
//!
//! ## Simplifications
//!
//! No `UnsafeCell` (interior-mutable `Reserved`), no wildcard provenance, and
//! every location of a node is treated as initialized; protectors apply only to
//! the retagged range.
//!
//! ## Example
//!
//! ```
//! use rust_interview_practice::unsafe_semantics::tree_borrows::{Permission, TreeBorrows};
//! use rust_interview_practice::unsafe_semantics::trace::{mut_ref, raw, read, write};
//!
//! let mut tb = TreeBorrows::new(1);
//! tb.apply(0, &raw("p", "x")).unwrap();
//! tb.apply(1, &mut_ref("r", "p")).unwrap();
//! tb.apply(2, &read("p", 0)).unwrap(); // foreign read: `r` stays Reserved
//! tb.apply(3, &write("r", 0)).unwrap(); // first child write: Reserved -> Active
//! assert_eq!(tb.perm("r", 0), Some(Permission::Active));
//! ```

use std::collections::HashMap;
use std::ops::Range;

use super::trace::{Op, ROOT, TraceError, UbKind, Verdict, to_verdict};

/// Per-location permission of a node in the borrow tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Permission {
    /// A `&mut` that has not been written through yet.
    ///
    /// `conflicted` is set by a foreign read while the node is protected; the
    /// next child write is then UB.
    Reserved { conflicted: bool },
    /// A `&mut` that has been written through.
    Active,
    /// Read-only: a `&T`, or an `Active` node after a foreign read.
    Frozen,
    /// Dead: any child access is UB.
    Disabled,
}

impl Permission {
    /// The initial permission of a fresh `&mut`.
    pub const RESERVED: Self = Self::Reserved { conflicted: false };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Access {
    Read,
    Write,
}

#[derive(Debug, Clone)]
struct Node {
    /// The pointer whose retag created this node.
    label: &'static str,
    parent: Option<usize>,
    perms: Vec<Permission>,
    protected: bool,
    protected_range: Range<usize>,
}

/// Borrow-tree state for one allocation of `len` locations.
#[derive(Debug, Clone)]
pub struct TreeBorrows {
    len: usize,
    nodes: Vec<Node>,
    /// Pointer name -> node. Raw pointers alias their parent's node.
    names: HashMap<&'static str, usize>,
}

impl TreeBorrows {
    /// A fresh allocation owned by [`ROOT`], which is `Active` everywhere.
    #[must_use]
    pub fn new(len: usize) -> Self {
        let root = Node {
            label: ROOT,
            parent: None,
            perms: vec![Permission::Active; len],
            protected: false,
            protected_range: 0..0,
        };
        Self {
            len,
            nodes: vec![root],
            names: HashMap::from([(ROOT, 0)]),
        }
    }

    /// Permission of pointer `name` at `loc`, if both exist.
    #[must_use]
    pub fn perm(&self, name: &str, loc: usize) -> Option<Permission> {
        let node = *self.names.get(name)?;
        self.nodes[node].perms.get(loc).copied()
    }

    /// Name of the reference whose node `name` is a child of (`None` for the
    /// root). Raw pointers share their reference's node, so a raw pointer's
    /// parent is that reference's parent.
    #[must_use]
    pub fn parent_of(&self, name: &str) -> Option<&'static str> {
        let node = *self.names.get(name)?;
        let parent = self.nodes[node].parent?;
        Some(self.nodes[parent].label)
    }

    /// Apply one operation; `step` is only used for error reporting.
    ///
    /// # Errors
    ///
    /// [`TraceError::Ub`] if the model forbids the operation, or a malformed-trace
    /// variant if `op` uses an unknown pointer or redefines one.
    ///
    /// Accesses are atomic: if one is UB the state is left unchanged.
    pub fn apply(&mut self, step: usize, op: &Op) -> Result<(), TraceError> {
        match op {
            Op::RetagMut {
                new,
                from,
                range,
                protect,
            } => self.retag(step, new, from, range, Permission::RESERVED, *protect),
            Op::RetagShared { new, from, range } => {
                self.retag(step, new, from, range, Permission::Frozen, false)
            }
            Op::Raw { new, from, .. } => {
                // Raw pointers are not retagged: they reuse the parent's node.
                let node = self.lookup(step, from)?;
                self.bind(step, new, node)
            }
            Op::Read { via, loc } => {
                let node = self.lookup(step, via)?;
                self.access(step, via, node, *loc, Access::Read)
            }
            Op::Write { via, loc } => {
                let node = self.lookup(step, via)?;
                self.access(step, via, node, *loc, Access::Write)
            }
            Op::EndProtect { tag } => {
                let node = self.lookup(step, tag)?;
                self.nodes[node].protected = false;
                Ok(())
            }
        }
    }

    fn lookup(&self, step: usize, ptr: &'static str) -> Result<usize, TraceError> {
        self.names
            .get(ptr)
            .copied()
            .ok_or(TraceError::UnknownPointer { step, ptr })
    }

    fn bind(&mut self, step: usize, ptr: &'static str, node: usize) -> Result<(), TraceError> {
        if self.names.contains_key(ptr) {
            return Err(TraceError::DuplicatePointer { step, ptr });
        }
        self.names.insert(ptr, node);
        Ok(())
    }

    fn retag(
        &mut self,
        step: usize,
        new: &'static str,
        from: &'static str,
        range: &Range<usize>,
        initial: Permission,
        protect: bool,
    ) -> Result<(), TraceError> {
        let parent = self.lookup(step, from)?;
        if self.names.contains_key(new) {
            return Err(TraceError::DuplicatePointer { step, ptr: new });
        }
        if range.end > self.len {
            return Err(TraceError::Ub {
                step,
                ptr: new,
                kind: UbKind::OutOfBounds,
            });
        }
        let node = self.nodes.len();
        self.nodes.push(Node {
            label: new,
            parent: Some(parent),
            perms: vec![initial; self.len],
            protected: false,
            protected_range: range.clone(),
        });
        self.names.insert(new, node);
        // A retag is a read access through the new reference over its range.
        for loc in range.clone() {
            self.access(step, new, node, loc, Access::Read)?;
        }
        // The protector starts after the retag's own read.
        self.nodes[node].protected = protect;
        Ok(())
    }

    fn is_ancestor_or_self(&self, ancestor: usize, mut node: usize) -> bool {
        loop {
            if node == ancestor {
                return true;
            }
            match self.nodes[node].parent {
                Some(parent) => node = parent,
                None => return false,
            }
        }
    }

    fn access(
        &mut self,
        step: usize,
        via: &'static str,
        node: usize,
        loc: usize,
        access: Access,
    ) -> Result<(), TraceError> {
        let ub = |kind| TraceError::Ub {
            step,
            ptr: via,
            kind,
        };
        if loc >= self.len {
            return Err(ub(UbKind::OutOfBounds));
        }
        let mut updated = Vec::with_capacity(self.nodes.len());
        for (idx, n) in self.nodes.iter().enumerate() {
            let child = self.is_ancestor_or_self(idx, node);
            let protected = n.protected && n.protected_range.contains(&loc);
            let next = transition(n.perms[loc], child, access, protected).map_err(ub)?;
            updated.push(next);
        }
        for (n, perm) in self.nodes.iter_mut().zip(updated) {
            n.perms[loc] = perm;
        }
        Ok(())
    }
}

/// The Tree Borrows state machine for one node at one location.
// Arms mirror the state table row by row, so some bodies repeat on purpose.
#[allow(clippy::match_same_arms)]
const fn transition(
    perm: Permission,
    child: bool,
    access: Access,
    protected: bool,
) -> Result<Permission, UbKind> {
    use Permission::{Active, Disabled, Frozen, Reserved};
    match (child, access, perm) {
        // Child (local) accesses: may this pointer do it?
        (true, _, Disabled) => Err(UbKind::AccessThroughDisabled),
        (true, Access::Read, p) => Ok(p),
        (true, Access::Write, Frozen) => Err(UbKind::WriteThroughFrozen),
        (true, Access::Write, Reserved { conflicted: true }) if protected => {
            Err(UbKind::ProtectorInvalidated)
        }
        (true, Access::Write, Reserved { .. } | Active) => Ok(Active),
        // Foreign accesses: how does someone else's access affect this pointer?
        (false, Access::Read, Reserved { conflicted }) => Ok(Reserved {
            conflicted: conflicted || protected,
        }),
        (false, Access::Read, Active) if protected => Err(UbKind::ProtectorInvalidated),
        (false, Access::Read, Active) => Ok(Frozen),
        (false, Access::Read, p @ (Frozen | Disabled)) => Ok(p),
        (false, Access::Write, Disabled) => Ok(Disabled),
        (false, Access::Write, _) if protected => Err(UbKind::ProtectorInvalidated),
        (false, Access::Write, _) => Ok(Disabled),
    }
}

/// Run `ops` against a fresh allocation of `len` locations.
///
/// # Errors
///
/// The first [`TraceError`] encountered; later operations are not run.
pub fn run(len: usize, ops: &[Op]) -> Result<(), TraceError> {
    let mut tb = TreeBorrows::new(len);
    ops.iter()
        .enumerate()
        .try_for_each(|(step, op)| tb.apply(step, op))
}

/// [`run`] collapsed to a [`Verdict`].
///
/// # Panics
///
/// Panics if the trace is malformed.
#[must_use]
pub fn verdict(len: usize, ops: &[Op]) -> Verdict {
    to_verdict(&run(len, ops))
}

#[cfg(test)]
mod tests {
    use super::Permission::{Active, Disabled, Frozen};
    use super::*;
    use crate::unsafe_semantics::trace::{
        end_protect, mut_arg, mut_ref, raw, read, shared_ref, write,
    };

    fn ub_kind(len: usize, ops: &[Op]) -> Option<UbKind> {
        run(len, ops).err().and_then(|e| e.ub_kind())
    }

    #[test]
    fn test_fresh_allocation_root_is_active() {
        let tb = TreeBorrows::new(2);
        assert_eq!(tb.perm(ROOT, 0), Some(Active));
        assert_eq!(tb.perm(ROOT, 1), Some(Active));
        assert_eq!(tb.perm(ROOT, 2), None);
        assert_eq!(tb.perm("ghost", 0), None);
        assert_eq!(tb.parent_of(ROOT), None);
    }

    #[test]
    fn test_retags_create_children_raw_does_not() {
        let mut tb = TreeBorrows::new(1);
        tb.apply(0, &mut_ref("a", ROOT)).unwrap();
        tb.apply(1, &raw("p", "a")).unwrap();
        tb.apply(2, &shared_ref("s", "p")).unwrap();
        assert_eq!(tb.perm("a", 0), Some(Permission::RESERVED));
        assert_eq!(tb.perm("p", 0), Some(Permission::RESERVED));
        assert_eq!(tb.perm("s", 0), Some(Frozen));
        assert_eq!(tb.parent_of("a"), Some(ROOT));
        // `s` was derived from raw `p`, which *is* node `a`.
        assert_eq!(tb.parent_of("s"), Some("a"));
    }

    #[test]
    fn test_state_machine_table() {
        use Access::{Read, Write};
        let r = Permission::RESERVED;
        // (perm, child, access) -> expected, unprotected
        let table = [
            (r, true, Read, Ok(r)),
            (r, true, Write, Ok(Active)),
            (r, false, Read, Ok(r)),
            (r, false, Write, Ok(Disabled)),
            (Active, true, Read, Ok(Active)),
            (Active, true, Write, Ok(Active)),
            (Active, false, Read, Ok(Frozen)),
            (Active, false, Write, Ok(Disabled)),
            (Frozen, true, Read, Ok(Frozen)),
            (Frozen, true, Write, Err(UbKind::WriteThroughFrozen)),
            (Frozen, false, Read, Ok(Frozen)),
            (Frozen, false, Write, Ok(Disabled)),
            (Disabled, true, Read, Err(UbKind::AccessThroughDisabled)),
            (Disabled, true, Write, Err(UbKind::AccessThroughDisabled)),
            (Disabled, false, Read, Ok(Disabled)),
            (Disabled, false, Write, Ok(Disabled)),
        ];
        for (perm, child, access, expected) in table {
            assert_eq!(
                transition(perm, child, access, false),
                expected,
                "{perm:?} child={child} {access:?}"
            );
        }
    }

    #[test]
    fn test_protected_transitions() {
        use Access::{Read, Write};
        let r = Permission::RESERVED;
        let conflicted = Permission::Reserved { conflicted: true };
        assert_eq!(transition(r, false, Read, true), Ok(conflicted));
        assert_eq!(
            transition(conflicted, true, Write, true),
            Err(UbKind::ProtectorInvalidated)
        );
        // Once the protector is gone, a conflicted node may be written again.
        assert_eq!(transition(conflicted, true, Write, false), Ok(Active));
        assert_eq!(
            transition(Active, false, Read, true),
            Err(UbKind::ProtectorInvalidated)
        );
        assert_eq!(
            transition(r, false, Write, true),
            Err(UbKind::ProtectorInvalidated)
        );
        assert_eq!(transition(Disabled, false, Write, true), Ok(Disabled));
    }

    #[test]
    fn test_reserved_tolerates_foreign_reads() {
        let ops = [
            raw("p", ROOT),
            mut_ref("r", "p"),
            read("p", 0),
            write("r", 0),
        ];
        assert_eq!(run(1, &ops), Ok(()));
    }

    #[test]
    fn test_active_is_frozen_by_foreign_read() {
        let mut tb = TreeBorrows::new(1);
        for (step, op) in [mut_ref("r", ROOT), write("r", 0), read(ROOT, 0)]
            .iter()
            .enumerate()
        {
            tb.apply(step, op).unwrap();
        }
        assert_eq!(tb.perm("r", 0), Some(Frozen));
        assert_eq!(
            tb.apply(3, &write("r", 0)).unwrap_err().ub_kind(),
            Some(UbKind::WriteThroughFrozen)
        );
    }

    #[test]
    fn test_ub_access_leaves_state_unchanged() {
        let mut tb = TreeBorrows::new(1);
        tb.apply(0, &shared_ref("s", ROOT)).unwrap();
        tb.apply(1, &mut_ref("m", ROOT)).unwrap();
        assert!(tb.apply(2, &write("s", 0)).is_err());
        assert_eq!(tb.perm("m", 0), Some(Permission::RESERVED));
        assert_eq!(tb.perm(ROOT, 0), Some(Active));
    }

    #[test]
    fn test_protector_violation() {
        let during = [raw("p", ROOT), mut_arg("r", "p"), write("p", 0)];
        assert_eq!(ub_kind(1, &during), Some(UbKind::ProtectorInvalidated));
        let after = [
            raw("p", ROOT),
            mut_arg("r", "p"),
            end_protect("r"),
            write("p", 0),
        ];
        assert_eq!(run(1, &after), Ok(()));
    }

    #[test]
    fn test_out_of_range_raw_is_allowed() {
        let ops = [mut_ref("r", ROOT), raw("p", "r"), write("p", 1)];
        assert_eq!(run(2, &ops), Ok(()));
    }

    #[test]
    fn test_out_of_bounds() {
        assert_eq!(ub_kind(1, &[write(ROOT, 1)]), Some(UbKind::OutOfBounds));
        let wide = [Op::RetagShared {
            new: "s",
            from: ROOT,
            range: 0..2,
        }];
        assert_eq!(ub_kind(1, &wide), Some(UbKind::OutOfBounds));
    }

    #[test]
    fn test_malformed_traces() {
        assert_eq!(
            run(1, &[write("ghost", 0)]),
            Err(TraceError::UnknownPointer {
                step: 0,
                ptr: "ghost"
            })
        );
        assert_eq!(
            run(1, &[raw("p", ROOT), raw("p", ROOT)]),
            Err(TraceError::DuplicatePointer { step: 1, ptr: "p" })
        );
        assert_eq!(
            run(1, &[mut_ref(ROOT, ROOT)]),
            Err(TraceError::DuplicatePointer { step: 0, ptr: ROOT })
        );
    }

    #[test]
    fn test_verdict_wrapper() {
        assert_eq!(verdict(1, &[write(ROOT, 0)]), Verdict::Ok);
        assert_eq!(verdict(1, &[read(ROOT, 9)]), Verdict::Ub);
    }
}
