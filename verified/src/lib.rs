//! # A Verus-verified slice: `SortedSet`
//!
//! The same sorted-vector set as `testing_craft::property_testing::SortedSet`,
//! written in [Verus](https://github.com/verus-lang/verus) so that its
//! contract is **machine-checked for every input**, not just the ones a test
//! happened to generate.
//!
//! SPEC → PROOF → RED → GREEN, in one file:
//!
//! 1. **Spec.** The "math shadow" of the runtime state: a [`SortedSet`]
//!    *means* the mathematical set `self@: Set<u64>`. The representation
//!    invariant [`SortedSet::wf`] says the backing `Vec` is strictly
//!    increasing (sorted, no duplicates). Every operation's `ensures` clause
//!    is stated against `Set<u64>`, never against the `Vec`.
//! 2. **Proof.** Verus + Z3 prove each body meets its contract and preserves
//!    `wf`. The spine lemma [`lemma_strictly_sorted_no_duplicates`] shows why
//!    "strictly increasing" is the right invariant.
//! 3. **Tests.** The `tests` module checks the *same compiled code* against a
//!    `BTreeSet` model with proptest. Proofs cover what we modeled; property
//!    tests poke at what we might have forgotten to model.
//!
//! ## The proofs have teeth
//!
//! Each of these one-line bugs makes `cargo verus verify` fail, before any
//! test runs:
//!
//! | Mutation | Verus reports |
//! |----------|---------------|
//! | midpoint `(lo + hi) / 2` | possible arithmetic overflow |
//! | `lo = mid` instead of `mid + 1` | `decreases` not satisfied (may not terminate) |
//! | `insert` at the wrong index | assertion failed (sortedness) |
//! | `remove` that forgets to remove | assertion failed |
//! | `contains` returning the wrong answer | postcondition not satisfied |
//!
//! Plain `cargo build` / `cargo test` erase the ghost code (`spec`, `proof`,
//! `requires`, `ensures`, `invariant`), so this is ordinary, fast Rust at
//! runtime. `cargo verus verify` checks the proofs.
//!
//! ```text
//! cargo test                 # executable tests (stable Rust)
//! cargo verus verify         # proofs (needs Verus 0.2026.09.27.3cf1832)
//! ```

// Some bindings are used only inside proofs, which plain rustc erases.
#![cfg_attr(not(verus_keep_ghost), allow(unused_variables))]

use vstd::prelude::*;

verus! {

// ============================================================================
// SPEC
// ============================================================================

/// Strictly increasing: sorted, and therefore free of duplicates.
pub open spec fn strictly_sorted(s: Seq<u64>) -> bool {
    forall|i: int, j: int| 0 <= i < j < s.len() ==> s[i] < s[j]
}

/// Spine lemma: strictly increasing sequences have no duplicates. This is why
/// one ordering invariant gives us set semantics for free.
pub proof fn lemma_strictly_sorted_no_duplicates(s: Seq<u64>)
    requires
        strictly_sorted(s),
    ensures
        s.no_duplicates(),
{
    assert forall|i: int, j: int| 0 <= i < s.len() && 0 <= j < s.len() && i != j implies s[i]
        != s[j] by {
        if i < j {
            assert(s[i] < s[j]);
        } else {
            assert(s[j] < s[i]);
        }
    }
}

/// A set of `u64` stored as a strictly increasing `Vec`.
pub struct SortedSet {
    items: Vec<u64>,
}

impl View for SortedSet {
    type V = Set<u64>;

    /// The abstract value: the (finite) set of elements of the backing vector.
    closed spec fn view(&self) -> Set<u64> {
        self.items@.to_set()
    }
}

impl SortedSet {
    /// Representation invariant: the backing vector is strictly increasing.
    pub closed spec fn wf(&self) -> bool {
        strictly_sorted(self.items@)
    }

    /// The backing sequence, in order (for stating [`SortedSet::as_slice`]).
    pub closed spec fn elems(&self) -> Seq<u64> {
        self.items@
    }

    // ========================================================================
    // GREEN: the implementation, proven against the spec
    // ========================================================================

    /// An empty set.
    pub fn new() -> (s: Self)
        ensures
            s.wf(),
            s@ == Set::<u64>::empty(),
    {
        let s = SortedSet { items: Vec::new() };
        proof {
            s.items@.to_set_ensures();
        }
        assert(s@ =~= Set::<u64>::empty());
        s
    }

    /// Binary search with the same contract as `slice::binary_search`:
    /// `Ok(i)` if `items[i] == v`, otherwise `Err(i)` where `i` is the unique
    /// insertion point that keeps the vector sorted.
    ///
    /// Time: O(log n).
    pub fn binary_search(&self, v: u64) -> (r: Result<usize, usize>)
        requires
            self.wf(),
        ensures
            match r {
                Ok(i) => i < self.elems().len() && self.elems()[i as int] == v,
                Err(i) => {
                    &&& i <= self.elems().len()
                    &&& forall|k: int| 0 <= k < i ==> self.elems()[k] < v
                    &&& forall|k: int| i <= k < self.elems().len() ==> self.elems()[k] > v
                },
            },
    {
        let mut lo: usize = 0;
        let mut hi: usize = self.items.len();
        while lo < hi
            invariant
                self.wf(),
                lo <= hi <= self.items@.len(),
                forall|k: int| 0 <= k < lo ==> self.items@[k] < v,
                forall|k: int| hi <= k < self.items@.len() ==> self.items@[k] > v,
            decreases hi - lo,
        {
            // `lo + (hi - lo) / 2`, not `(lo + hi) / 2`: Verus rejects the
            // overflowing form, which is exactly the classic binary-search bug.
            let mid = lo + (hi - lo) / 2;
            let x = self.items[mid];
            if x == v {
                return Ok(mid);
            } else if x < v {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        Err(lo)
    }

    /// `true` iff `v` is in the set. Time: O(log n).
    pub fn contains(&self, v: u64) -> (b: bool)
        requires
            self.wf(),
        ensures
            b == self@.contains(v),
    {
        proof {
            self.items@.to_set_ensures();
        }
        match self.binary_search(v) {
            Ok(i) => {
                assert(self.items@[i as int] == v);
                true
            },
            Err(i) => {
                assert(!self.items@.contains(v));
                false
            },
        }
    }

    /// Inserts `v`; returns `true` iff it was not already present.
    ///
    /// Time: O(n) - binary search + shift.
    pub fn insert(&mut self, v: u64) -> (added: bool)
        requires
            old(self).wf(),
        ensures
            final(self).wf(),
            added == !old(self)@.contains(v),
            final(self)@ == old(self)@.insert(v),
    {
        proof {
            self.items@.to_set_ensures();
        }
        match self.binary_search(v) {
            Ok(i) => {
                assert(self.items@[i as int] == v);
                assert(self@ =~= old(self)@.insert(v));
                false
            },
            Err(i) => {
                assert(!self.items@.contains(v));
                let ghost before = self.items@;
                self.items.insert(i, v);
                let ghost after = self.items@;
                assert(after == before.insert(i as int, v));
                // wf is preserved: everything left of `i` is < v, everything
                // right of it is > v, and both halves were already sorted.
                assert forall|a: int, b: int| 0 <= a < b < after.len() implies after[a]
                    < after[b] by {
                    if b < i {
                    } else if b == i {
                    } else if a < i {
                    } else if a == i {
                    } else {
                        assert(before[a - 1] < before[b - 1]);
                    }
                }
                // The abstract set gained exactly `v`.
                assert forall|x: u64| #[trigger] after.contains(x) <==> (before.contains(x) || x
                    == v) by {
                    if before.contains(x) {
                        let k = choose|k: int| 0 <= k < before.len() && before[k] == x;
                        if k < i {
                            assert(after[k] == x);
                        } else {
                            assert(after[k + 1] == x);
                        }
                    }
                    if x == v {
                        assert(after[i as int] == v);
                    }
                    if after.contains(x) {
                        let j = choose|j: int| 0 <= j < after.len() && after[j] == x;
                        if j < i {
                            assert(before[j] == x);
                        } else if j > i {
                            assert(before[j - 1] == x);
                        }
                    }
                }
                proof {
                    after.to_set_ensures();
                }
                assert(self@ =~= old(self)@.insert(v));
                true
            },
        }
    }

    /// Removes `v`; returns `true` iff it was present.
    ///
    /// Time: O(n) - binary search + shift.
    pub fn remove(&mut self, v: u64) -> (removed: bool)
        requires
            old(self).wf(),
        ensures
            final(self).wf(),
            removed == old(self)@.contains(v),
            final(self)@ == old(self)@.remove(v),
    {
        proof {
            self.items@.to_set_ensures();
        }
        match self.binary_search(v) {
            Err(i) => {
                assert(!self.items@.contains(v));
                assert(self@ =~= old(self)@.remove(v));
                false
            },
            Ok(i) => {
                assert(self.items@[i as int] == v);
                let ghost before = self.items@;
                let _ = self.items.remove(i);
                let ghost after = self.items@;
                assert(after == before.remove(i as int));
                assert forall|a: int, b: int| 0 <= a < b < after.len() implies after[a]
                    < after[b] by {
                    if b < i {
                    } else if a < i {
                        assert(before[a] < before[b + 1]);
                    } else {
                        assert(before[a + 1] < before[b + 1]);
                    }
                }
                // The abstract set lost exactly `v` (and only `v`: strict
                // ordering means `v` appeared once).
                assert forall|x: u64| #[trigger] after.contains(x) <==> (before.contains(x) && x
                    != v) by {
                    if after.contains(x) {
                        let j = choose|j: int| 0 <= j < after.len() && after[j] == x;
                        if j < i {
                            assert(before[j] == x);
                            assert(before[j] < before[i as int]);
                        } else {
                            assert(before[j + 1] == x);
                            assert(before[i as int] < before[j + 1]);
                        }
                    }
                    if before.contains(x) && x != v {
                        let k = choose|k: int| 0 <= k < before.len() && before[k] == x;
                        if k < i {
                            assert(after[k] == x);
                        } else {
                            assert(k != i);
                            assert(after[k - 1] == x);
                        }
                    }
                }
                proof {
                    after.to_set_ensures();
                }
                assert(self@ =~= old(self)@.remove(v));
                true
            },
        }
    }

    /// `true` iff the set is empty. Time: O(1).
    pub fn is_empty(&self) -> (b: bool)
        requires
            self.wf(),
        ensures
            b == (self@ == Set::<u64>::empty()),
    {
        proof {
            self.items@.to_set_ensures();
        }
        if self.items.len() == 0 {
            assert(self@ =~= Set::<u64>::empty());
            true
        } else {
            assert(self@.contains(self.items@[0]));
            false
        }
    }

    /// Number of elements. Time: O(1).
    ///
    /// The spec is about the abstract *set*: its cardinality equals the vector
    /// length only because the invariant rules out duplicates.
    pub fn len(&self) -> (n: usize)
        requires
            self.wf(),
        ensures
            n == self@.len(),
    {
        proof {
            lemma_strictly_sorted_no_duplicates(self.items@);
            self.items@.unique_seq_to_set();
        }
        self.items.len()
    }

    /// The elements in strictly increasing order, each exactly once.
    pub fn as_slice(&self) -> (s: &[u64])
        requires
            self.wf(),
        ensures
            s@ == self.elems(),
            strictly_sorted(s@),
            s@.no_duplicates(),
            forall|x: u64| #[trigger] self@.contains(x) <==> s@.contains(x),
    {
        proof {
            lemma_strictly_sorted_no_duplicates(self.items@);
            self.items@.to_set_ensures();
        }
        self.items.as_slice()
    }
}

} // verus!

#[cfg(test)]
mod tests {
    use super::SortedSet;
    use proptest::prelude::*;
    use std::collections::BTreeSet;

    #[test]
    fn test_examples() {
        let mut s = SortedSet::new();
        assert!(s.is_empty());
        assert!(s.insert(5));
        assert!(s.insert(1));
        assert!(s.insert(9));
        assert!(!s.insert(5));
        assert_eq!(s.as_slice(), &[1, 5, 9]);
        assert_eq!(s.len(), 3);
        assert!(s.contains(9));
        assert!(!s.contains(4));
        assert_eq!(s.binary_search(5), Ok(1));
        assert_eq!(s.binary_search(4), Err(1));
        assert!(s.remove(1));
        assert!(!s.remove(1));
        assert_eq!(s.as_slice(), &[5, 9]);
    }

    #[test]
    fn test_extremes() {
        let mut s = SortedSet::new();
        assert!(s.insert(u64::MAX));
        assert!(s.insert(0));
        assert_eq!(s.as_slice(), &[0, u64::MAX]);
        assert_eq!(s.binary_search(u64::MAX), Ok(1));
        assert_eq!(s.binary_search(1), Err(1));
        assert!(s.remove(u64::MAX));
        assert!(s.remove(0));
        assert!(s.is_empty());
        assert_eq!(s.binary_search(7), Err(0));
    }

    #[derive(Debug, Clone, Copy)]
    enum Op {
        Insert(u64),
        Remove(u64),
        Contains(u64),
    }

    fn op() -> impl Strategy<Value = Op> {
        // A small domain makes inserts, removes and lookups collide.
        prop_oneof![
            (0u64..16).prop_map(Op::Insert),
            (0u64..16).prop_map(Op::Remove),
            (0u64..16).prop_map(Op::Contains),
            any::<u64>().prop_map(Op::Insert),
        ]
    }

    proptest! {
        /// Model-based test: the verified set agrees with `BTreeSet` on every
        /// return value and on its final contents.
        #[test]
        fn pt_matches_btreeset_model(ops in prop::collection::vec(op(), 0..128)) {
            let mut real = SortedSet::new();
            let mut model = BTreeSet::new();
            for op in ops {
                match op {
                    Op::Insert(v) => prop_assert_eq!(real.insert(v), model.insert(v)),
                    Op::Remove(v) => prop_assert_eq!(real.remove(v), model.remove(&v)),
                    Op::Contains(v) => prop_assert_eq!(real.contains(v), model.contains(&v)),
                }
                prop_assert_eq!(real.len(), model.len());
                prop_assert_eq!(real.is_empty(), model.is_empty());
            }
            prop_assert!(real.as_slice().iter().eq(model.iter()));
        }

        /// The `binary_search` contract, checked at runtime.
        #[test]
        fn pt_binary_search_contract(values in prop::collection::vec(any::<u64>(), 0..64), probe: u64) {
            let mut s = SortedSet::new();
            for v in values {
                s.insert(v);
            }
            let items = s.as_slice();
            prop_assert!(items.windows(2).all(|w| w[0] < w[1]));
            match s.binary_search(probe) {
                Ok(i) => prop_assert_eq!(items[i], probe),
                Err(i) => {
                    prop_assert!(items[..i].iter().all(|&x| x < probe));
                    prop_assert!(items[i..].iter().all(|&x| x > probe));
                }
            }
            prop_assert_eq!(s.binary_search(probe), items.binary_search(&probe));
        }
    }
}
