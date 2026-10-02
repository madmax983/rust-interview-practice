//! # Stacked Borrows (toy model)
//!
//! Stacked Borrows (Jung et al., POPL 2020) is the aliasing model Miri uses by
//! default. Every memory location carries a **borrow stack** of *items*; each
//! item pairs a pointer tag with a permission:
//!
//! | Permission        | Created by                 | Grants        |
//! |-------------------|----------------------------|---------------|
//! | `Unique`          | `&mut T` (and the owner)   | read + write  |
//! | `SharedReadWrite` | raw pointer from `&mut T`  | read + write  |
//! | `SharedReadOnly`  | `&T`, raw from `&T`        | read          |
//!
//! An access through tag `t` finds the topmost item for `t` that grants it (no
//! such item ⇒ **UB**) and then *pops* everything above it that is incompatible:
//!
//! - **read**: removes the `Unique` items above the granting item;
//! - **write**: removes every item above, except a contiguous run of
//!   `SharedReadWrite` siblings directly on top of a `SharedReadWrite` granter.
//!
//! Retagging pushes a new item: `&mut` performs a *write* access through its
//! parent first (so merely creating a `&mut` invalidates other borrows!), `&T`
//! performs a read, and a raw pointer from `&mut` is inserted right above its
//! parent without any access. Popping a *protected* item (a `&mut` function
//! argument whose call has not returned) is UB.
//!
//! ## Simplifications
//!
//! No `UnsafeCell`, no exposed/wildcard provenance, no `Disabled` items (popped
//! items are removed instead), and retags apply only to the locations they
//! cover — which is precisely what makes out-of-range raw pointers UB here.
//!
//! ## Example
//!
//! ```
//! use rust_interview_practice::unsafe_semantics::stacked_borrows::{Permission, StackedBorrows};
//! use rust_interview_practice::unsafe_semantics::trace::{mut_ref, raw, read};
//!
//! let mut sb = StackedBorrows::new(1);
//! sb.apply(0, &raw("p", "x")).unwrap();
//! sb.apply(1, &mut_ref("r", "p")).unwrap();
//! assert_eq!(sb.perms(0), vec![("x", Permission::Unique), ("p", Permission::SharedReadWrite), ("r", Permission::Unique)]);
//!
//! // Reading through the parent pops the `&mut` above it.
//! sb.apply(2, &read("p", 0)).unwrap();
//! assert_eq!(sb.perms(0).len(), 2);
//! ```

use std::collections::HashSet;
use std::ops::Range;

use super::trace::{Op, ROOT, TraceError, UbKind, Verdict, to_verdict};

/// What an item in a borrow stack allows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Permission {
    /// Exclusive: `&mut T` or the owner.
    Unique,
    /// Shared mutable: raw pointers derived from `&mut T`.
    SharedReadWrite,
    /// Shared immutable: `&T` and pointers derived from it.
    SharedReadOnly,
}

/// The kind of memory access being performed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Access {
    Read,
    Write,
}

impl Permission {
    const fn grants(self, access: Access) -> bool {
        match access {
            Access::Read => true,
            Access::Write => !matches!(self, Self::SharedReadOnly),
        }
    }
}

/// One entry of a borrow stack.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Item {
    /// The pointer this item belongs to.
    pub tag: &'static str,
    /// What the pointer may do.
    pub perm: Permission,
    /// `true` while the pointer is a live `&mut` function argument.
    pub protected: bool,
}

/// Borrow-stack state for one allocation of `len` locations.
#[derive(Debug, Clone)]
pub struct StackedBorrows {
    stacks: Vec<Vec<Item>>,
    known: HashSet<&'static str>,
}

impl StackedBorrows {
    /// A fresh allocation owned by [`ROOT`]: every stack is `[Unique(x)]`.
    #[must_use]
    pub fn new(len: usize) -> Self {
        let root = Item {
            tag: ROOT,
            perm: Permission::Unique,
            protected: false,
        };
        Self {
            stacks: vec![vec![root]; len],
            known: HashSet::from([ROOT]),
        }
    }

    /// The borrow stack of `loc`, bottom first.
    #[must_use]
    pub fn stack(&self, loc: usize) -> &[Item] {
        &self.stacks[loc]
    }

    /// `(tag, permission)` pairs of the stack at `loc`, bottom first.
    #[must_use]
    pub fn perms(&self, loc: usize) -> Vec<(&'static str, Permission)> {
        self.stacks[loc]
            .iter()
            .map(|it| (it.tag, it.perm))
            .collect()
    }

    /// Apply one operation; `step` is only used for error reporting.
    ///
    /// # Errors
    ///
    /// [`TraceError::Ub`] if the model forbids the operation, or a malformed-trace
    /// variant if `op` uses an unknown pointer or redefines one.
    ///
    /// On error the state may be partially updated; stop using it.
    pub fn apply(&mut self, step: usize, op: &Op) -> Result<(), TraceError> {
        match op {
            Op::RetagMut {
                new,
                from,
                range,
                protect,
            } => {
                self.introduce(step, new, from)?;
                let item = Item {
                    tag: new,
                    perm: Permission::Unique,
                    protected: *protect,
                };
                self.for_range(step, from, range, |stack| {
                    // Creating a `&mut` is a write access through the parent.
                    access(stack, from, Access::Write)?;
                    stack.push(item);
                    Ok(())
                })
            }
            Op::RetagShared { new, from, range } => {
                self.introduce(step, new, from)?;
                let item = Item {
                    tag: new,
                    perm: Permission::SharedReadOnly,
                    protected: false,
                };
                self.for_range(step, from, range, |stack| {
                    access(stack, from, Access::Read)?;
                    stack.push(item);
                    Ok(())
                })
            }
            Op::Raw { new, from, range } => {
                self.introduce(step, new, from)?;
                self.for_range(step, from, range, |stack| {
                    if let Some(granting) = find_granting(stack, from, Access::Write) {
                        // Raw from a mutable pointer: a SharedReadWrite sibling,
                        // inserted right above its parent, no access performed.
                        let item = Item {
                            tag: new,
                            perm: Permission::SharedReadWrite,
                            protected: false,
                        };
                        stack.insert(granting + 1, item);
                    } else {
                        // Raw from a shared reference stays read-only.
                        access(stack, from, Access::Read)?;
                        stack.push(Item {
                            tag: new,
                            perm: Permission::SharedReadOnly,
                            protected: false,
                        });
                    }
                    Ok(())
                })
            }
            Op::Read { via, loc } => self.access_at(step, via, *loc, Access::Read),
            Op::Write { via, loc } => self.access_at(step, via, *loc, Access::Write),
            Op::EndProtect { tag } => {
                self.check_known(step, tag)?;
                for item in self.stacks.iter_mut().flatten() {
                    if item.tag == *tag {
                        item.protected = false;
                    }
                }
                Ok(())
            }
        }
    }

    fn check_known(&self, step: usize, ptr: &'static str) -> Result<(), TraceError> {
        if self.known.contains(ptr) {
            Ok(())
        } else {
            Err(TraceError::UnknownPointer { step, ptr })
        }
    }

    fn introduce(
        &mut self,
        step: usize,
        new: &'static str,
        from: &'static str,
    ) -> Result<(), TraceError> {
        self.check_known(step, from)?;
        if self.known.insert(new) {
            Ok(())
        } else {
            Err(TraceError::DuplicatePointer { step, ptr: new })
        }
    }

    fn for_range(
        &mut self,
        step: usize,
        via: &'static str,
        range: &Range<usize>,
        mut f: impl FnMut(&mut Vec<Item>) -> Result<(), UbKind>,
    ) -> Result<(), TraceError> {
        let ub = |kind| TraceError::Ub {
            step,
            ptr: via,
            kind,
        };
        for loc in range.clone() {
            let stack = self
                .stacks
                .get_mut(loc)
                .ok_or_else(|| ub(UbKind::OutOfBounds))?;
            f(stack).map_err(ub)?;
        }
        Ok(())
    }

    fn access_at(
        &mut self,
        step: usize,
        via: &'static str,
        loc: usize,
        kind: Access,
    ) -> Result<(), TraceError> {
        self.check_known(step, via)?;
        self.for_range(step, via, &(loc..loc + 1), |stack| access(stack, via, kind))
    }
}

/// Index of the topmost item for `tag` that grants `access`.
fn find_granting(stack: &[Item], tag: &str, access: Access) -> Option<usize> {
    stack
        .iter()
        .rposition(|it| it.tag == tag && it.perm.grants(access))
}

/// Perform `access` through `tag`, popping incompatible items.
fn access(stack: &mut Vec<Item>, tag: &str, access: Access) -> Result<(), UbKind> {
    let granting = find_granting(stack, tag, access).ok_or(UbKind::NoGrantingItem)?;
    let above = granting + 1;
    match access {
        Access::Read => {
            // Reads only invalidate the `Unique` items above the granter.
            let popped = |it: &Item| it.perm == Permission::Unique;
            if stack[above..].iter().any(|it| popped(it) && it.protected) {
                return Err(UbKind::ProtectorPopped);
            }
            let mut idx = 0;
            stack.retain(|it| {
                idx += 1;
                idx <= above || !popped(it)
            });
        }
        Access::Write => {
            // Writes keep only a run of SharedReadWrite siblings of the granter.
            let first_incompatible = if stack[granting].perm == Permission::SharedReadWrite {
                stack[above..]
                    .iter()
                    .position(|it| it.perm != Permission::SharedReadWrite)
                    .map_or(stack.len(), |offset| above + offset)
            } else {
                above
            };
            if stack[first_incompatible..].iter().any(|it| it.protected) {
                return Err(UbKind::ProtectorPopped);
            }
            stack.truncate(first_incompatible);
        }
    }
    Ok(())
}

/// Run `ops` against a fresh allocation of `len` locations.
///
/// # Errors
///
/// The first [`TraceError`] encountered; later operations are not run.
pub fn run(len: usize, ops: &[Op]) -> Result<(), TraceError> {
    let mut sb = StackedBorrows::new(len);
    ops.iter()
        .enumerate()
        .try_for_each(|(step, op)| sb.apply(step, op))
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
    use super::Permission::{SharedReadOnly, SharedReadWrite, Unique};
    use super::*;
    use crate::unsafe_semantics::trace::{
        end_protect, mut_arg, mut_ref, raw, read, shared_ref, write,
    };

    fn ub_kind(len: usize, ops: &[Op]) -> Option<UbKind> {
        run(len, ops).err().and_then(|e| e.ub_kind())
    }

    #[test]
    fn test_fresh_allocation_is_owned_by_root() {
        let sb = StackedBorrows::new(2);
        assert_eq!(sb.perms(0), vec![(ROOT, Unique)]);
        assert_eq!(sb.perms(1), vec![(ROOT, Unique)]);
        assert!(!sb.stack(0)[0].protected);
    }

    #[test]
    fn test_retags_push_expected_items() {
        let mut sb = StackedBorrows::new(1);
        sb.apply(0, &mut_ref("a", ROOT)).unwrap();
        sb.apply(1, &raw("p", "a")).unwrap();
        sb.apply(2, &shared_ref("s", "p")).unwrap();
        sb.apply(3, &raw("q", "s")).unwrap();
        assert_eq!(
            sb.perms(0),
            vec![
                (ROOT, Unique),
                ("a", Unique),
                ("p", SharedReadWrite),
                ("s", SharedReadOnly),
                ("q", SharedReadOnly),
            ]
        );
    }

    #[test]
    fn test_raw_is_inserted_right_above_parent() {
        let mut sb = StackedBorrows::new(1);
        sb.apply(0, &mut_ref("a", ROOT)).unwrap();
        sb.apply(1, &shared_ref("s", "a")).unwrap();
        // `p` is derived from `a`, so it goes directly above `a`, below `s`.
        sb.apply(2, &raw("p", "a")).unwrap();
        assert_eq!(
            sb.perms(0),
            vec![
                (ROOT, Unique),
                ("a", Unique),
                ("p", SharedReadWrite),
                ("s", SharedReadOnly)
            ]
        );
    }

    #[test]
    fn test_read_pops_only_unique_items_above() {
        let mut sb = StackedBorrows::new(1);
        sb.apply(0, &raw("p", ROOT)).unwrap();
        sb.apply(1, &mut_ref("r", "p")).unwrap();
        sb.apply(2, &raw("q", "r")).unwrap();
        sb.apply(3, &read("p", 0)).unwrap();
        // `r` (Unique) is gone; its SharedReadWrite child `q` survives a read.
        assert_eq!(
            sb.perms(0),
            vec![
                (ROOT, Unique),
                ("p", SharedReadWrite),
                ("q", SharedReadWrite)
            ]
        );
    }

    #[test]
    fn test_write_keeps_shared_read_write_siblings() {
        let mut sb = StackedBorrows::new(1);
        sb.apply(0, &raw("p", ROOT)).unwrap();
        sb.apply(1, &raw("q", ROOT)).unwrap();
        sb.apply(2, &shared_ref("s", "q")).unwrap();
        // Stack: [x, q, p, s]; a write via `q` keeps sibling `p`, pops `s`.
        sb.apply(3, &write("q", 0)).unwrap();
        assert_eq!(
            sb.perms(0),
            vec![
                (ROOT, Unique),
                ("q", SharedReadWrite),
                ("p", SharedReadWrite)
            ]
        );
    }

    #[test]
    fn test_write_through_unique_pops_everything_above() {
        let mut sb = StackedBorrows::new(1);
        sb.apply(0, &mut_ref("a", ROOT)).unwrap();
        sb.apply(1, &raw("p", "a")).unwrap();
        sb.apply(2, &write(ROOT, 0)).unwrap();
        assert_eq!(sb.perms(0), vec![(ROOT, Unique)]);
    }

    #[test]
    fn test_write_through_shared_is_ub() {
        let ops = [shared_ref("s", ROOT), raw("p", "s"), write("p", 0)];
        assert_eq!(ub_kind(1, &ops), Some(UbKind::NoGrantingItem));
    }

    #[test]
    fn test_creating_mut_is_a_write() {
        let ops = [
            raw("p", ROOT),
            shared_ref("s", "p"),
            mut_ref("m", "p"),
            read("s", 0),
        ];
        assert_eq!(ub_kind(1, &ops), Some(UbKind::NoGrantingItem));
    }

    #[test]
    fn test_protector_pop_is_ub_until_end_protect() {
        let during = [raw("p", ROOT), mut_arg("r", "p"), write("p", 0)];
        assert_eq!(ub_kind(1, &during), Some(UbKind::ProtectorPopped));

        let read_during = [raw("p", ROOT), mut_arg("r", "p"), read("p", 0)];
        assert_eq!(ub_kind(1, &read_during), Some(UbKind::ProtectorPopped));

        let after = [
            raw("p", ROOT),
            mut_arg("r", "p"),
            end_protect("r"),
            write("p", 0),
        ];
        assert_eq!(run(1, &after), Ok(()));
    }

    #[test]
    fn test_out_of_bounds_access_is_ub() {
        assert_eq!(ub_kind(1, &[read(ROOT, 1)]), Some(UbKind::OutOfBounds));
        let wide = [Op::RetagMut {
            new: "r",
            from: ROOT,
            range: 0..3,
            protect: false,
        }];
        assert_eq!(ub_kind(2, &wide), Some(UbKind::OutOfBounds));
    }

    #[test]
    fn test_retag_only_covers_its_range() {
        let ops = [mut_ref("r", ROOT), raw("p", "r"), write("p", 1)];
        assert_eq!(ub_kind(2, &ops), Some(UbKind::NoGrantingItem));
    }

    #[test]
    fn test_malformed_traces() {
        assert_eq!(
            run(1, &[read("ghost", 0)]),
            Err(TraceError::UnknownPointer {
                step: 0,
                ptr: "ghost"
            })
        );
        assert_eq!(
            run(1, &[mut_ref("r", ROOT), mut_ref("r", ROOT)]),
            Err(TraceError::DuplicatePointer { step: 1, ptr: "r" })
        );
        assert_eq!(
            run(1, &[end_protect("ghost")]),
            Err(TraceError::UnknownPointer {
                step: 0,
                ptr: "ghost"
            })
        );
    }

    #[test]
    fn test_verdict_wrapper() {
        assert_eq!(verdict(1, &[write(ROOT, 0)]), Verdict::Ok);
        assert_eq!(verdict(1, &[read(ROOT, 5)]), Verdict::Ub);
    }
}
