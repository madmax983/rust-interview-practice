//! # Region Constraints
//!
//! NLL does not reason about lifetimes as scopes. It reasons about **regions as
//! sets of program points**, and it infers them by solving constraints:
//!
//! 1. **Liveness constraints** — `'r live at P`: if a variable whose type
//!    mentions `'r` may be used at or after `P`, then `P ∈ 'r`.
//! 2. **Outlives constraints** — `'a: 'b`: produced by subtyping at
//!    assignments, calls and reborrows. Solved as `'a ⊇ 'b`.
//! 3. **Universal regions** — named lifetimes from the signature (`'a`, `'static`)
//!    contain every point of the body plus a symbolic `end('a)` element ("and
//!    beyond, into the caller").
//!
//! The solution is the *least* fixpoint: start every region at its liveness
//! points, then propagate along `'a: 'b` edges until nothing changes. rustc's
//! solver is location-insensitive in exactly this way (Polonius is the
//! location-sensitive successor).
//!
//! After solving, rustc checks each universal region: if `end('b)` ended up
//! inside `'a` but the signature never promised `'a: 'b`, that is
//! *"lifetime may not live long enough"*. The blamed constraints are the path
//! from `'a` to `'b` through the constraint graph.
//!
//! ## Reading an NLL dump
//!
//! `-Zdump-mir=nll` prints a header above the MIR. [`FIRST_OR_PUSH_NLL`] is the
//! real rustc 1.97 header for the `first_or_push` example from
//! [`crate::compiler_literacy::mir_reading`] (spans and `DefId`s elided):
//!
//! ```text
//! | Free Region Mapping
//! | '?0 | Global | ['?0, '?2, '?1]     <- '?0 is 'static; it outlives every universal
//! | '?1 | Local | ['?2, '?1]           <- '?1 is the anonymous lifetime of `v: &mut Vec`
//! | Inferred Region Values
//! | '?3 | U0 | {bb0[3..=4], bb1[0..=4]}   <- the SOLUTION: points where the
//! |                                         borrow of `*v` for `v[0]` is live
//! | Inference Constraints
//! | '?3 live at {bb0[3]}                  <- liveness constraint
//! | '?3: '?9 due to Boring at Single(bb0[3])   <- outlives constraint + why
//! | Borrows
//! | bw0: issued at bb0[3] in '?3          <- borrow bw0's region is '?3
//! ```
//!
//! [`NllDump`] parses that header and [`RegionConstraintSet::solve`]
//! reproduces rustc's own "Inferred Region Values" from its "Inference
//! Constraints" — see the `solver_reproduces_rustc_inferred_values` test.
//!
//! ## Building constraints by hand
//!
//! ```
//! use rust_interview_practice::compiler_literacy::mir_reading::Location;
//! use rust_interview_practice::compiler_literacy::region_constraints::{
//!     ConstraintCategory, ConstraintLocation, RegionConstraintSet, UniversalKind,
//! };
//!
//! // fn pick<'a, 'b>(x: &'a i32, y: &'b i32) -> &'a i32 { y }
//! let points = [Location::new(0, 0), Location::new(0, 1)];
//! let mut cs = RegionConstraintSet::new(points);
//! let a = cs.add_universal(UniversalKind::Local, "'a");
//! let b = cs.add_universal(UniversalKind::Local, "'b");
//! // Returning `y` requires `&'b i32 <: &'a i32`, i.e. 'b: 'a.
//! cs.add_outlives(b, a, ConstraintCategory::Return, ConstraintLocation::Single(points[0]));
//!
//! let values = cs.solve();
//! assert!(values.get(b).contains_universal(a)); // end('a) flowed into 'b
//! let errors = cs.check_universal_regions(&values);
//! assert_eq!(errors.len(), 1);
//! assert_eq!(errors[0].to_string(), "lifetime may not live long enough: 'b must outlive 'a");
//!
//! // Declaring `'b: 'a` in the signature fixes it.
//! cs.assume_outlives(b, a);
//! assert_eq!(cs.check_universal_regions(&cs.solve()), vec![]);
//! ```

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;
use std::str::FromStr;

use super::mir_reading::{BasicBlock, Location, MirFn, ParseError};

/// Real rustc 1.97 `-Zdump-mir=nll` output for `first_or_push`.
///
/// See [`crate::compiler_literacy::mir_reading`] for the source. Source spans, `// scope`
/// comments and the `DefId` inside `CallArgument` were trimmed; nothing else
/// was edited. Note the borrowck-only `FakeRead`s and the `unwind: bb3` edges
/// to a cleanup block, which the post-borrowck MIR no longer shows.
pub const FIRST_OR_PUSH_NLL: &str = "\
| Free Region Mapping
| '?0 | Global | ['?0, '?2, '?1]
| '?1 | Local | ['?2, '?1]
| '?2 | Local | ['?2]
|
| Inferred Region Values
| '?0 | U0 | {bb0[0..=4], bb1[0..=11], bb2[0..=7], bb3[0], '?0, '?1, '?2}
| '?1 | U0 | {bb0[0..=4], bb1[0..=11], bb2[0..=7], bb3[0], '?1}
| '?2 | U0 | {bb0[0..=4], bb1[0..=11], bb2[0..=7], bb3[0], '?2}
| '?3 | U0 | {bb0[3..=4], bb1[0..=4]}
| '?4 | U0 | {bb1[1..=4]}
| '?5 | U0 | {bb1[8..=11]}
| '?6 | U0 | {bb0[0..=4], bb1[0..=11], bb2[0..=7], bb3[0], '?1}
| '?7 | U0 | {bb1[2..=4]}
| '?8 | U0 | {bb1[0..=4]}
| '?9 | U0 | {bb0[4], bb1[0..=4]}
| '?10 | U0 | {bb1[9..=11]}
| '?11 | U0 | {bb0[4], bb1[0..=4]}
| '?12 | U0 | {bb1[11]}
|
| Inference Constraints
| '?0 live at {bb0[0..=4], bb1[0..=11], bb2[0..=7], bb3[0]}
| '?1 live at {bb0[0..=4], bb1[0..=11], bb2[0..=7], bb3[0]}
| '?2 live at {bb0[0..=4], bb1[0..=11], bb2[0..=7], bb3[0]}
| '?3 live at {bb0[3]}
| '?4 live at {bb1[1]}
| '?5 live at {bb1[8]}
| '?7 live at {bb1[2..=4]}
| '?8 live at {bb1[0..=1]}
| '?9 live at {bb0[4]}
| '?10 live at {bb1[9..=11]}
| '?11 live at {bb0[4]}
| '?12 live at {bb1[11]}
| '?1: '?6 due to BoringNoLocation at All
| '?3: '?9 due to Boring at Single(bb0[3])
| '?4: '?7 due to Assignment at Single(bb1[1])
| '?5: '?10 due to Boring at Single(bb1[8])
| '?6: '?1 due to BoringNoLocation at All
| '?6: '?3 due to Boring at Single(bb0[3])
| '?6: '?5 due to Boring at Single(bb1[8])
| '?8: '?4 due to Boring at Single(bb1[1])
| '?9: '?11 due to Boring at Single(bb0[4])
| '?10: '?12 due to CallArgument(Some(FnDef(Vec::<i32>::push))) at Single(bb1[11])
| '?11: '?8 due to Boring at Single(bb0[4])
|
| Borrows
| bw0: issued at bb0[3] in '?3
| bw1: issued at bb1[8] in '?5
|
fn first_or_push(_1: &mut Vec<i32>) -> i32 {
    debug v => _1;
    let mut _0: i32;
    let _2: &i32;
    let _3: &i32;
    let mut _4: &std::vec::Vec<i32>;
    let _6: ();
    let mut _7: &mut std::vec::Vec<i32>;
    let mut _8: i32;
    scope 1 {
        debug first => _2;
        let _5: i32;
        scope 2 {
            debug x => _5;
        }
    }

    bb0: {
        StorageLive(_2);
        StorageLive(_3);
        StorageLive(_4);
        _4 = &(*_1);
        _3 = <Vec<i32> as Index<usize>>::index(move _4, const 0_usize) -> [return: bb1, unwind: bb3];
    }

    bb1: {
        StorageDead(_4);
        _2 = &(*_3);
        FakeRead(ForLet(None), _2);
        StorageLive(_5);
        _5 = copy (*_2);
        FakeRead(ForLet(None), _5);
        StorageLive(_6);
        StorageLive(_7);
        _7 = &mut (*_1);
        StorageLive(_8);
        _8 = copy _5;
        _6 = Vec::<i32>::push(move _7, move _8) -> [return: bb2, unwind: bb3];
    }

    bb2: {
        StorageDead(_8);
        StorageDead(_7);
        StorageDead(_6);
        _0 = copy _5;
        StorageDead(_5);
        StorageDead(_3);
        StorageDead(_2);
        return;
    }

    bb3 (cleanup): {
        resume;
    }
}
";

// =========================================================================================
// Regions and region values
// =========================================================================================

/// A region inference variable, printed like rustc: `'?3`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RegionVid(pub usize);

impl fmt::Display for RegionVid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "'?{}", self.0)
    }
}

impl FromStr for RegionVid {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.strip_prefix("'?")
            .and_then(|digits| digits.parse().ok())
            .map(Self)
            .ok_or_else(|| ParseError::new(0, format!("expected a region like `'?3`, got `{s}`")))
    }
}

/// A region's value: the points it contains plus the `end('u)` elements of
/// universal regions it must outlive.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RegionValue {
    points: BTreeSet<Location>,
    universals: BTreeSet<RegionVid>,
}

impl RegionValue {
    /// The empty region.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            points: BTreeSet::new(),
            universals: BTreeSet::new(),
        }
    }

    /// `true` if the region contains point `p`.
    #[must_use]
    pub fn contains_point(&self, p: Location) -> bool {
        self.points.contains(&p)
    }

    /// `true` if the region contains `end(u)`.
    #[must_use]
    pub fn contains_universal(&self, u: RegionVid) -> bool {
        self.universals.contains(&u)
    }

    /// The points, in order.
    #[must_use]
    pub const fn points(&self) -> &BTreeSet<Location> {
        &self.points
    }

    /// The `end(u)` elements, in order.
    #[must_use]
    pub const fn universals(&self) -> &BTreeSet<RegionVid> {
        &self.universals
    }

    /// Adds a point; returns `true` if it was new.
    pub fn insert_point(&mut self, p: Location) -> bool {
        self.points.insert(p)
    }

    /// Adds `end(u)`; returns `true` if it was new.
    pub fn insert_universal(&mut self, u: RegionVid) -> bool {
        self.universals.insert(u)
    }

    /// `self ⊇ other` after this call. Returns `true` if `self` grew.
    pub fn union_with(&mut self, other: &Self) -> bool {
        let before = self.points.len() + self.universals.len();
        self.points.extend(other.points.iter().copied());
        self.universals.extend(other.universals.iter().copied());
        before != self.points.len() + self.universals.len()
    }

    /// `true` if every element of `other` is in `self`.
    #[must_use]
    pub fn is_superset(&self, other: &Self) -> bool {
        self.points.is_superset(&other.points) && self.universals.is_superset(&other.universals)
    }
}

impl fmt::Display for RegionValue {
    /// rustc's compact form: `{bb0[0..=4], bb1[2], '?1}`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut parts = Vec::new();
        let mut run: Option<(BasicBlock, usize, usize)> = None;
        let flush = |run: Option<(BasicBlock, usize, usize)>, parts: &mut Vec<String>| {
            if let Some((bb, start, end)) = run {
                parts.push(if start == end {
                    format!("{bb}[{start}]")
                } else {
                    format!("{bb}[{start}..={end}]")
                });
            }
        };
        for p in &self.points {
            match run {
                Some((bb, start, end)) if bb == p.block && end + 1 == p.statement_index => {
                    run = Some((bb, start, p.statement_index));
                }
                _ => {
                    flush(run, &mut parts);
                    run = Some((p.block, p.statement_index, p.statement_index));
                }
            }
        }
        flush(run, &mut parts);
        parts.extend(self.universals.iter().map(ToString::to_string));
        write!(f, "{{{}}}", parts.join(", "))
    }
}

impl FromStr for RegionValue {
    type Err = ParseError;

    /// Parses rustc's compact form, e.g. `{bb0[3..=4], bb1[0], '?1}`.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let inner = s
            .trim()
            .strip_prefix('{')
            .and_then(|r| r.strip_suffix('}'))
            .ok_or_else(|| ParseError::new(0, format!("expected `{{...}}`, got `{s}`")))?;
        let mut value = Self::new();
        for item in inner.split(", ").map(str::trim).filter(|i| !i.is_empty()) {
            if item.starts_with('\'') {
                value.insert_universal(item.parse()?);
                continue;
            }
            let (block, range) = item
                .split_once('[')
                .ok_or_else(|| ParseError::new(0, format!("bad region element `{item}`")))?;
            let block: BasicBlock = block.parse()?;
            let range = range
                .strip_suffix(']')
                .ok_or_else(|| ParseError::new(0, format!("bad region element `{item}`")))?;
            let num = |t: &str| {
                t.parse::<usize>()
                    .map_err(|_| ParseError::new(0, format!("bad index in `{item}`")))
            };
            let (start, end) = match range.split_once("..=") {
                Some((a, b)) => (num(a)?, num(b)?),
                None => (num(range)?, num(range)?),
            };
            for i in start..=end {
                value.insert_point(Location {
                    block,
                    statement_index: i,
                });
            }
        }
        Ok(value)
    }
}

// =========================================================================================
// Constraints
// =========================================================================================

/// How rustc classifies a universal region in the "Free Region Mapping".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UniversalKind {
    /// `'static`: outlives every other universal region.
    Global,
    /// Declared on an enclosing item (closures see their parent's lifetimes).
    External,
    /// Declared on this function (named or elided).
    Local,
}

impl FromStr for UniversalKind {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Global" => Ok(Self::Global),
            "External" => Ok(Self::External),
            "Local" => Ok(Self::Local),
            other => Err(ParseError::new(
                0,
                format!("unknown universal kind `{other}`"),
            )),
        }
    }
}

/// A named lifetime from the signature.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UniversalRegion {
    /// Its region variable.
    pub vid: RegionVid,
    /// Global / external / local.
    pub kind: UniversalKind,
    /// Display name, e.g. `'a`.
    pub name: String,
}

/// Why an outlives constraint exists (rustc's `ConstraintCategory`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConstraintCategory {
    /// Writing a value into `_0`.
    Return,
    /// `place = value` subtyping.
    Assignment,
    /// Passing an argument / relating a call's signature.
    CallArgument,
    /// Uninteresting (reborrows, temporaries); never blamed if avoidable.
    Boring,
    /// Like `Boring`, with no location (implied bounds from the signature).
    BoringNoLocation,
    /// Any other rustc category, verbatim.
    Other(String),
}

impl ConstraintCategory {
    /// rustc prefers to blame "interesting" constraints; lower is better.
    #[must_use]
    pub const fn blame_rank(&self) -> u8 {
        match self {
            Self::Return => 0,
            Self::CallArgument => 1,
            Self::Assignment => 2,
            Self::Other(_) => 3,
            Self::Boring => 4,
            Self::BoringNoLocation => 5,
        }
    }
}

impl fmt::Display for ConstraintCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Return => f.write_str("Return"),
            Self::Assignment => f.write_str("Assignment"),
            Self::CallArgument => f.write_str("CallArgument"),
            Self::Boring => f.write_str("Boring"),
            Self::BoringNoLocation => f.write_str("BoringNoLocation"),
            Self::Other(s) => f.write_str(s),
        }
    }
}

impl From<&str> for ConstraintCategory {
    /// Parses rustc's debug text; payloads like `CallArgument(Some(..))` are dropped.
    fn from(s: &str) -> Self {
        let name = s.split(['(', ' ']).next().unwrap_or(s);
        match name {
            "Return" => Self::Return,
            "Assignment" => Self::Assignment,
            "CallArgument" => Self::CallArgument,
            "Boring" => Self::Boring,
            "BoringNoLocation" => Self::BoringNoLocation,
            other => Self::Other(other.to_string()),
        }
    }
}

/// Where an outlives constraint was generated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConstraintLocation {
    /// Holds everywhere (signature-implied).
    All,
    /// Generated at one statement.
    Single(Location),
}

impl fmt::Display for ConstraintLocation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::All => f.write_str("All"),
            Self::Single(loc) => write!(f, "Single({loc})"),
        }
    }
}

/// `sup: sub` — region `sup` must outlive (be a superset of) region `sub`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutlivesConstraint {
    /// The longer region.
    pub sup: RegionVid,
    /// The shorter region.
    pub sub: RegionVid,
    /// Why.
    pub category: ConstraintCategory,
    /// Where.
    pub at: ConstraintLocation,
}

impl fmt::Display for OutlivesConstraint {
    /// rustc's dump form: `'?3: '?9 due to Boring at Single(bb0[3])`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}: {} due to {} at {}",
            self.sup, self.sub, self.category, self.at
        )
    }
}

/// A universal region was forced to outlive another without the signature
/// promising it: rustc's *"lifetime may not live long enough"*.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UniversalRegionError {
    /// The region that must outlive...
    pub longer: RegionVid,
    /// ...this one.
    pub shorter: RegionVid,
    /// Display name of `longer`.
    pub longer_name: String,
    /// Display name of `shorter`.
    pub shorter_name: String,
    /// Constraint path from `longer` to `shorter` that forced it.
    pub path: Vec<OutlivesConstraint>,
}

impl UniversalRegionError {
    /// The constraint rustc would point at: the most interesting one on the path.
    #[must_use]
    pub fn blamed(&self) -> Option<&OutlivesConstraint> {
        self.path.iter().min_by_key(|c| c.category.blame_rank())
    }
}

impl fmt::Display for UniversalRegionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "lifetime may not live long enough: {} must outlive {}",
            self.longer_name, self.shorter_name
        )
    }
}

/// The solved value of every region.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionValues {
    values: Vec<RegionValue>,
}

impl RegionValues {
    /// The value of `r`. Unknown regions read as empty.
    #[must_use]
    pub fn get(&self, r: RegionVid) -> &RegionValue {
        static EMPTY: RegionValue = RegionValue::new();
        self.values.get(r.0).unwrap_or(&EMPTY)
    }

    /// Number of regions.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.values.len()
    }

    /// `true` if there are no regions.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
}

/// All the inputs to region inference for one body.
#[derive(Debug, Clone, Default)]
pub struct RegionConstraintSet {
    num_regions: usize,
    universals: Vec<UniversalRegion>,
    body_points: BTreeSet<Location>,
    liveness: BTreeMap<RegionVid, BTreeSet<Location>>,
    outlives: Vec<OutlivesConstraint>,
    known_outlives: Vec<(RegionVid, RegionVid)>,
}

impl RegionConstraintSet {
    /// An empty constraint set over a body with the given points.
    pub fn new(body_points: impl IntoIterator<Item = Location>) -> Self {
        Self {
            body_points: body_points.into_iter().collect(),
            ..Self::default()
        }
    }

    /// A new existential (inferred) region variable.
    pub const fn fresh_region(&mut self) -> RegionVid {
        let vid = RegionVid(self.num_regions);
        self.num_regions += 1;
        vid
    }

    /// A new universal region named `name`.
    pub fn add_universal(&mut self, kind: UniversalKind, name: impl Into<String>) -> RegionVid {
        let vid = self.fresh_region();
        self.universals.push(UniversalRegion {
            vid,
            kind,
            name: name.into(),
        });
        vid
    }

    /// Replaces the set of program points of the body.
    pub fn set_body_points(&mut self, points: impl IntoIterator<Item = Location>) {
        self.body_points = points.into_iter().collect();
    }

    /// The program points of the body.
    #[must_use]
    pub const fn body_points(&self) -> &BTreeSet<Location> {
        &self.body_points
    }

    /// Makes sure region ids up to `vid` exist (used when importing a dump).
    pub const fn ensure_region(&mut self, vid: RegionVid) {
        if vid.0 >= self.num_regions {
            self.num_regions = vid.0 + 1;
        }
    }

    /// Records that the signature promises `longer: shorter` (a where-clause or
    /// implied bound). Used only when checking universal regions.
    pub fn assume_outlives(&mut self, longer: RegionVid, shorter: RegionVid) {
        self.known_outlives.push((longer, shorter));
    }

    /// Liveness constraint: `point ∈ region`.
    pub fn add_live(&mut self, region: RegionVid, point: Location) {
        self.ensure_region(region);
        self.liveness.entry(region).or_default().insert(point);
    }

    /// Outlives constraint `sup: sub`.
    pub fn add_outlives(
        &mut self,
        sup: RegionVid,
        sub: RegionVid,
        category: ConstraintCategory,
        at: ConstraintLocation,
    ) {
        self.ensure_region(sup);
        self.ensure_region(sub);
        self.outlives.push(OutlivesConstraint {
            sup,
            sub,
            category,
            at,
        });
    }

    /// Number of region variables.
    #[must_use]
    pub const fn num_regions(&self) -> usize {
        self.num_regions
    }

    /// The universal regions.
    #[must_use]
    pub fn universals(&self) -> &[UniversalRegion] {
        &self.universals
    }

    /// The outlives constraints, in insertion order.
    #[must_use]
    pub fn outlives(&self) -> &[OutlivesConstraint] {
        &self.outlives
    }

    /// The liveness constraints.
    #[must_use]
    pub const fn liveness(&self) -> &BTreeMap<RegionVid, BTreeSet<Location>> {
        &self.liveness
    }

    /// Display name of a region: its universal name, else `'?N`.
    #[must_use]
    pub fn region_name(&self, r: RegionVid) -> String {
        self.universals
            .iter()
            .find(|u| u.vid == r)
            .map_or_else(|| r.to_string(), |u| u.name.clone())
    }

    /// Each region's value before propagation: liveness points, plus (for a
    /// universal region) every body point and its own `end`, plus (for
    /// `'static`) the `end` of every universal region.
    #[must_use]
    pub fn initial_values(&self) -> Vec<RegionValue> {
        let mut values = vec![RegionValue::new(); self.num_regions];
        for u in &self.universals {
            let value = &mut values[u.vid.0];
            value.points.extend(self.body_points.iter().copied());
            value.insert_universal(u.vid);
            if u.kind == UniversalKind::Global {
                value
                    .universals
                    .extend(self.universals.iter().map(|other| other.vid));
            }
        }
        for (region, points) in &self.liveness {
            values[region.0].points.extend(points.iter().copied());
        }
        values
    }

    /// Least fixpoint of the constraints.
    ///
    /// Worklist propagation: when `'sub` grows, every `'sup` with a `'sup: 'sub`
    /// edge is re-unioned. Each union only adds elements, and there are finitely
    /// many, so it terminates.
    ///
    /// Time: O(C · E) for C constraints and E distinct elements (points +
    /// universals). Space: O(R · E) for R regions.
    #[must_use]
    pub fn solve(&self) -> RegionValues {
        let mut values = self.initial_values();
        // sub -> [indices of constraints with that sub]
        let mut by_sub: Vec<Vec<usize>> = vec![Vec::new(); self.num_regions];
        for (i, c) in self.outlives.iter().enumerate() {
            by_sub[c.sub.0].push(i);
        }
        let mut queued = vec![true; self.num_regions];
        let mut worklist: VecDeque<usize> = (0..self.num_regions).collect();
        while let Some(sub) = worklist.pop_front() {
            queued[sub] = false;
            for &ci in &by_sub[sub] {
                let sup = self.outlives[ci].sup.0;
                if sup == sub {
                    continue;
                }
                let sub_value = values[sub].clone();
                if values[sup].union_with(&sub_value) && !queued[sup] {
                    queued[sup] = true;
                    worklist.push_back(sup);
                }
            }
        }
        RegionValues { values }
    }

    /// `true` if the signature (transitively) promises `longer: shorter`.
    /// Every region outlives itself and `'static` outlives everything.
    #[must_use]
    pub fn is_known_outlives(&self, longer: RegionVid, shorter: RegionVid) -> bool {
        if longer == shorter
            || self
                .universals
                .iter()
                .any(|u| u.vid == longer && u.kind == UniversalKind::Global)
        {
            return true;
        }
        let mut seen = BTreeSet::from([longer]);
        let mut stack = vec![longer];
        while let Some(r) = stack.pop() {
            for &(l, s) in &self.known_outlives {
                if l == r && seen.insert(s) {
                    if s == shorter {
                        return true;
                    }
                    stack.push(s);
                }
            }
        }
        false
    }

    /// Shortest path of constraints `from: x1, x1: x2, ..., xn: to` (BFS).
    #[must_use]
    pub fn constraint_path(
        &self,
        from: RegionVid,
        to: RegionVid,
    ) -> Option<Vec<OutlivesConstraint>> {
        if from == to {
            return Some(Vec::new());
        }
        let mut came_by: BTreeMap<RegionVid, usize> = BTreeMap::new();
        let mut queue = VecDeque::from([from]);
        while let Some(r) = queue.pop_front() {
            for (i, c) in self.outlives.iter().enumerate() {
                if c.sup != r || c.sub == from || came_by.contains_key(&c.sub) {
                    continue;
                }
                came_by.insert(c.sub, i);
                if c.sub == to {
                    let mut path = Vec::new();
                    let mut cur = to;
                    while cur != from {
                        let ci = came_by[&cur];
                        path.push(self.outlives[ci].clone());
                        cur = self.outlives[ci].sup;
                    }
                    path.reverse();
                    return Some(path);
                }
                queue.push_back(c.sub);
            }
        }
        None
    }

    /// For every universal region `'u` and every `end('v)` in its solved value,
    /// require that the signature promises `'u: 'v`.
    #[must_use]
    pub fn check_universal_regions(&self, values: &RegionValues) -> Vec<UniversalRegionError> {
        let mut errors = Vec::new();
        for u in &self.universals {
            for &v in values.get(u.vid).universals() {
                if self.is_known_outlives(u.vid, v) {
                    continue;
                }
                errors.push(UniversalRegionError {
                    longer: u.vid,
                    shorter: v,
                    longer_name: self.region_name(u.vid),
                    shorter_name: self.region_name(v),
                    path: self.constraint_path(u.vid, v).unwrap_or_default(),
                });
            }
        }
        errors
    }
}

// =========================================================================================
// NLL dump parsing
// =========================================================================================

/// A row of the "Free Region Mapping" table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FreeRegionRow {
    /// The universal region.
    pub vid: RegionVid,
    /// Its kind.
    pub kind: UniversalKind,
    /// Regions it is known to outlive (including itself).
    pub outlives: Vec<RegionVid>,
}

/// A row of the "Borrows" table: `bw0: issued at bb0[3] in '?3`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BorrowRow {
    /// rustc's borrow index name, e.g. `bw0`.
    pub name: String,
    /// Issue point.
    pub issued_at: Location,
    /// The borrow's region.
    pub region: RegionVid,
}

/// A parsed `-Zdump-mir=nll` file: the region header plus the MIR body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NllDump {
    /// "Free Region Mapping" rows.
    pub free_regions: Vec<FreeRegionRow>,
    /// "Inferred Region Values" — rustc's solution.
    pub inferred: BTreeMap<RegionVid, RegionValue>,
    /// `'r live at {...}` lines.
    pub live_at: BTreeMap<RegionVid, RegionValue>,
    /// Outlives constraints.
    pub outlives: Vec<OutlivesConstraint>,
    /// "Borrows" rows.
    pub borrows: Vec<BorrowRow>,
    /// The MIR below the header.
    pub body: MirFn,
}

#[derive(Clone, Copy)]
enum Section {
    None,
    FreeRegions,
    Inferred,
    Constraints,
    Borrows,
}

impl NllDump {
    /// Parses an NLL dump.
    ///
    /// # Errors
    ///
    /// Returns a [`ParseError`] for a malformed header line or MIR body.
    pub fn parse(text: &str) -> Result<Self, ParseError> {
        let mut dump = Self {
            free_regions: Vec::new(),
            inferred: BTreeMap::new(),
            live_at: BTreeMap::new(),
            outlives: Vec::new(),
            borrows: Vec::new(),
            body: MirFn::parse(text)?,
        };
        let mut section = Section::None;
        for (index, raw) in text.lines().enumerate() {
            let Some(line) = raw.trim().strip_prefix('|') else {
                continue;
            };
            let line = line.trim();
            section = match line {
                "" => continue,
                "Free Region Mapping" => Section::FreeRegions,
                "Inferred Region Values" => Section::Inferred,
                "Inference Constraints" => Section::Constraints,
                "Borrows" => Section::Borrows,
                _ => {
                    dump.header_line(section, line)
                        .map_err(|e| ParseError::new(index + 1, e.message))?;
                    section
                }
            };
        }
        Ok(dump)
    }

    fn header_line(&mut self, section: Section, line: &str) -> Result<(), ParseError> {
        let bad = || ParseError::new(0, format!("unrecognised header line `{line}`"));
        match section {
            Section::FreeRegions => {
                let cols: Vec<&str> = line.split(" | ").collect();
                let [vid, kind, list] = cols.as_slice() else {
                    return Err(bad());
                };
                let list = list
                    .trim()
                    .strip_prefix('[')
                    .and_then(|l| l.strip_suffix(']'))
                    .ok_or_else(bad)?;
                self.free_regions.push(FreeRegionRow {
                    vid: vid.trim().parse()?,
                    kind: kind.trim().parse()?,
                    outlives: list
                        .split(", ")
                        .filter(|s| !s.is_empty())
                        .map(str::parse)
                        .collect::<Result<_, _>>()?,
                });
            }
            Section::Inferred => {
                let cols: Vec<&str> = line.split(" | ").collect();
                let [vid, _universe, value] = cols.as_slice() else {
                    return Err(bad());
                };
                self.inferred.insert(vid.trim().parse()?, value.parse()?);
            }
            Section::Constraints => {
                if let Some((vid, set)) = line.split_once(" live at ") {
                    self.live_at.insert(vid.parse()?, set.parse()?);
                } else {
                    let (regions, rest) = line.split_once(" due to ").ok_or_else(bad)?;
                    let (sup, sub) = regions.split_once(": ").ok_or_else(bad)?;
                    let (category, at) = rest.rsplit_once(" at ").ok_or_else(bad)?;
                    let at = if at.starts_with("All") {
                        ConstraintLocation::All
                    } else {
                        let loc = at
                            .strip_prefix("Single(")
                            .and_then(|r| r.split(')').next())
                            .ok_or_else(bad)?;
                        ConstraintLocation::Single(loc.parse()?)
                    };
                    self.outlives.push(OutlivesConstraint {
                        sup: sup.parse()?,
                        sub: sub.parse()?,
                        category: ConstraintCategory::from(category),
                        at,
                    });
                }
            }
            Section::Borrows => {
                let (name, rest) = line.split_once(": issued at ").ok_or_else(bad)?;
                let (loc, region) = rest.split_once(" in ").ok_or_else(bad)?;
                self.borrows.push(BorrowRow {
                    name: name.to_string(),
                    issued_at: loc.parse()?,
                    region: region.parse()?,
                });
            }
            Section::None => return Err(bad()),
        }
        Ok(())
    }

    /// Rebuilds the solver input from the dump: body points from the MIR,
    /// universal regions and their known relations from the free-region
    /// mapping, and the liveness / outlives constraints verbatim.
    #[must_use]
    pub fn to_constraint_set(&self) -> RegionConstraintSet {
        let mut cs = RegionConstraintSet::new(self.body.locations());
        for row in &self.free_regions {
            cs.ensure_region(row.vid);
            cs.universals.push(UniversalRegion {
                vid: row.vid,
                kind: row.kind,
                name: row.vid.to_string(),
            });
            for &shorter in &row.outlives {
                cs.assume_outlives(row.vid, shorter);
            }
        }
        for (&region, value) in &self.live_at {
            for &p in value.points() {
                cs.add_live(region, p);
            }
        }
        for c in &self.outlives {
            cs.add_outlives(c.sup, c.sub, c.category.clone(), c.at);
        }
        for &r in self.inferred.keys() {
            cs.ensure_region(r);
        }
        cs
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing_craft::sim_rng::SimRng;

    fn dump() -> NllDump {
        NllDump::parse(FIRST_OR_PUSH_NLL).expect("captured dump parses")
    }

    #[test]
    fn dump_header_is_parsed() {
        let d = dump();
        assert_eq!(d.free_regions.len(), 3);
        assert_eq!(d.free_regions[0].kind, UniversalKind::Global);
        assert_eq!(d.free_regions[1].outlives, vec![RegionVid(2), RegionVid(1)]);
        assert_eq!(d.inferred.len(), 13);
        assert_eq!(d.live_at.len(), 12, "'?6 has no liveness constraint");
        assert_eq!(d.outlives.len(), 11);
        assert_eq!(d.outlives[9].category, ConstraintCategory::CallArgument);
        assert_eq!(d.outlives[0].at, ConstraintLocation::All);
        assert_eq!(
            d.borrows,
            vec![
                BorrowRow {
                    name: "bw0".to_string(),
                    issued_at: Location::new(0, 3),
                    region: RegionVid(3)
                },
                BorrowRow {
                    name: "bw1".to_string(),
                    issued_at: Location::new(1, 8),
                    region: RegionVid(5)
                },
            ]
        );
        // The borrowck MIR has FakeReads, so bb1 has 12 points, not 10.
        assert_eq!(d.body.points_in(BasicBlock(1)), 12);
        assert!(d.body.blocks[3].cleanup);
    }

    #[test]
    fn solver_reproduces_rustc_inferred_values() {
        let d = dump();
        let cs = d.to_constraint_set();
        let values = cs.solve();
        for (&region, expected) in &d.inferred {
            assert_eq!(
                values.get(region),
                expected,
                "{region}: ours {} vs rustc {}",
                values.get(region),
                expected
            );
        }
        assert_eq!(cs.check_universal_regions(&values), Vec::new());
    }

    #[test]
    fn borrow_regions_match_reading_of_the_mir() {
        let d = dump();
        let values = d.to_constraint_set().solve();
        // bw0 (`&(*_1)` for `v[0]`) must stay live until `first` (_2) is last
        // used at bb1[4] — and not one point longer, so `v.push` is fine.
        let bw0 = values.get(d.borrows[0].region);
        assert_eq!(bw0.to_string(), "{bb0[3..=4], bb1[0..=4]}");
        let push_borrow = d.borrows[1].issued_at;
        assert!(!bw0.contains_point(push_borrow));
    }

    #[test]
    fn region_value_round_trips_rustc_format() {
        for text in [
            "{bb0[0..=4], bb1[0..=11], bb2[0..=7], bb3[0], '?0, '?1, '?2}",
            "{bb1[11]}",
            "{}",
            "{bb0[4], bb1[0..=4]}",
        ] {
            let v: RegionValue = text.parse().expect("parses");
            assert_eq!(v.to_string(), text);
        }
        assert!("bb0[1]".parse::<RegionValue>().is_err());
        assert!("{bb0}".parse::<RegionValue>().is_err());
        assert!("{bb0[x]}".parse::<RegionValue>().is_err());
    }

    #[test]
    fn constraint_display_matches_dump_lines() {
        let d = dump();
        assert_eq!(
            d.outlives[1].to_string(),
            "'?3: '?9 due to Boring at Single(bb0[3])"
        );
        assert_eq!(
            d.outlives[0].to_string(),
            "'?1: '?6 due to BoringNoLocation at All"
        );
    }

    #[test]
    fn raw_rustc_constraint_lines_with_spans_parse() {
        let text = format!(
            "| Inference Constraints\n\
             | '?3: '?9 due to Boring at Single(bb0[3]) (a.rs:2:18: 2:19 (#0)\n\
             | '?1: '?6 due to BoringNoLocation at All(a.rs:1:22: 1:23) (a.rs:1:22: 1:23 (#0)\n\
             {}",
            crate::compiler_literacy::mir_reading::FIRST_OR_PUSH_MIR
        );
        let d = NllDump::parse(&text).expect("parses");
        assert_eq!(
            d.outlives[0].at,
            ConstraintLocation::Single(Location::new(0, 3))
        );
        assert_eq!(d.outlives[1].at, ConstraintLocation::All);
    }

    #[test]
    fn malformed_header_line_is_reported() {
        let text = format!(
            "| Borrows\n| what is this\n{}",
            crate::compiler_literacy::mir_reading::FIRST_OR_PUSH_MIR
        );
        let err = NllDump::parse(&text).expect_err("bad borrow row");
        assert_eq!(err.line, 2);
        let text = format!(
            "| Free Region Mapping\n| '?0 | Weird | ['?0]\n{}",
            crate::compiler_literacy::mir_reading::FIRST_OR_PUSH_MIR
        );
        assert!(NllDump::parse(&text).is_err());
    }

    #[test]
    fn static_outlives_everything_and_known_bounds_are_transitive() {
        let mut cs = RegionConstraintSet::new([Location::new(0, 0)]);
        let st = cs.add_universal(UniversalKind::Global, "'static");
        let a = cs.add_universal(UniversalKind::Local, "'a");
        let b = cs.add_universal(UniversalKind::Local, "'b");
        let c = cs.add_universal(UniversalKind::Local, "'c");
        cs.assume_outlives(a, b);
        cs.assume_outlives(b, c);
        assert!(cs.is_known_outlives(a, c));
        assert!(!cs.is_known_outlives(c, a));
        assert!(cs.is_known_outlives(st, c));
        // x: &'a T returned as &'static T: 'a must outlive 'static.
        cs.add_outlives(a, st, ConstraintCategory::Return, ConstraintLocation::All);
        let values = cs.solve();
        // end('static) drags every universal's end into 'a, but 'a: 'b and
        // 'a: 'c are promised, so only 'a: 'static is reported.
        assert!(values.get(a).contains_universal(c));
        let errors = cs.check_universal_regions(&values);
        assert_eq!(
            errors.iter().map(ToString::to_string).collect::<Vec<_>>(),
            vec!["lifetime may not live long enough: 'a must outlive 'static"]
        );
        assert_eq!(cs.region_name(st), "'static");
        assert_eq!(cs.region_name(RegionVid(42)), "'?42");
    }

    #[test]
    fn blame_prefers_interesting_constraints_on_the_path() {
        let mut cs = RegionConstraintSet::new([Location::new(0, 0), Location::new(0, 1)]);
        let a = cs.add_universal(UniversalKind::Local, "'a");
        let b = cs.add_universal(UniversalKind::Local, "'b");
        let t1 = cs.fresh_region();
        let t2 = cs.fresh_region();
        let here = ConstraintLocation::Single(Location::new(0, 0));
        cs.add_outlives(b, t1, ConstraintCategory::Boring, here);
        cs.add_outlives(t1, t2, ConstraintCategory::Assignment, here);
        cs.add_outlives(t2, a, ConstraintCategory::Return, here);
        let errors = cs.check_universal_regions(&cs.solve());
        assert_eq!(errors.len(), 1);
        let e = &errors[0];
        assert_eq!((e.longer, e.shorter), (b, a));
        assert_eq!(e.path.len(), 3);
        assert_eq!(
            e.blamed().map(|c| &c.category),
            Some(&ConstraintCategory::Return)
        );
        assert_eq!(cs.constraint_path(a, b), None);
        assert_eq!(cs.constraint_path(a, a), Some(Vec::new()));
    }

    #[test]
    fn cycles_unify_regions() {
        let mut cs = RegionConstraintSet::new([]);
        let x = cs.fresh_region();
        let y = cs.fresh_region();
        cs.add_live(x, Location::new(0, 0));
        cs.add_live(y, Location::new(1, 0));
        cs.add_outlives(x, y, ConstraintCategory::Boring, ConstraintLocation::All);
        cs.add_outlives(y, x, ConstraintCategory::Boring, ConstraintLocation::All);
        cs.add_outlives(x, x, ConstraintCategory::Boring, ConstraintLocation::All);
        let v = cs.solve();
        assert_eq!(v.get(x), v.get(y));
        assert_eq!(v.get(x).to_string(), "{bb0[0], bb1[0]}");
        assert_eq!(v.len(), 2);
        assert!(!v.is_empty());
        assert_eq!(v.get(RegionVid(99)), &RegionValue::new());
    }

    /// Specification of the least solution: a region contains exactly its own
    /// initial elements plus those of every region reachable through
    /// `sup -> sub` edges. Checked against the worklist solver on random graphs
    /// (with cycles and self-loops).
    fn solve_by_reachability(cs: &RegionConstraintSet) -> Vec<RegionValue> {
        let init = cs.initial_values();
        (0..cs.num_regions())
            .map(|r| {
                let mut seen = BTreeSet::from([r]);
                let mut stack = vec![r];
                while let Some(cur) = stack.pop() {
                    for c in cs.outlives() {
                        if c.sup.0 == cur && seen.insert(c.sub.0) {
                            stack.push(c.sub.0);
                        }
                    }
                }
                let mut value = RegionValue::new();
                for s in seen {
                    value.union_with(&init[s]);
                }
                value
            })
            .collect()
    }

    #[test]
    fn property_solver_equals_reachability_spec() {
        for seed in 0..300 {
            let mut rng = SimRng::new(seed);
            let points: Vec<Location> = (0..4)
                .flat_map(|b| (0..3).map(move |i| Location::new(b, i)))
                .collect();
            let mut cs = RegionConstraintSet::new(points.iter().copied());
            for _ in 0..rng.below(3) {
                cs.add_universal(UniversalKind::Local, "'u");
            }
            if rng.chance(1, 4) {
                cs.add_universal(UniversalKind::Global, "'static");
            }
            let n = cs.num_regions() + 1 + rng.below(8);
            while cs.num_regions() < n {
                cs.fresh_region();
            }
            for _ in 0..rng.below(2 * n) {
                let r = RegionVid(rng.below(n));
                cs.add_live(r, points[rng.below(points.len())]);
            }
            for _ in 0..rng.below(3 * n) {
                let (a, b) = (RegionVid(rng.below(n)), RegionVid(rng.below(n)));
                cs.add_outlives(a, b, ConstraintCategory::Boring, ConstraintLocation::All);
            }
            let solved = cs.solve();
            let spec = solve_by_reachability(&cs);
            for (r, expected) in spec.iter().enumerate() {
                assert_eq!(
                    solved.get(RegionVid(r)),
                    expected,
                    "seed {seed}, region {r}"
                );
            }
            // Soundness restated: every constraint holds in the solution.
            for c in cs.outlives() {
                assert!(
                    solved.get(c.sup).is_superset(solved.get(c.sub)),
                    "seed {seed}"
                );
            }
        }
    }
}
