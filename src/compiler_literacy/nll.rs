//! # NLL: a Borrow Checker You Can Read in One Sitting
//!
//! A miniature of `rustc_borrowck` over a toy MIR. It runs the same pipeline,
//! in the same order, with the same vocabulary as the real thing:
//!
//! ```text
//!   Body ──► liveness ──► constraint generation ──► region solve ──► borrows in scope ──► access checks
//!            (backward      liveness: 'r live at P     (least fixpoint,   (forward from each     (each access vs
//!             dataflow)     outlives: 'a: 'b @ P       region_constraints) issue point while      every in-scope
//!                                                                          P ∈ region, minus      borrow → E0499,
//!                                                                          kills)                 E0502, ...)
//! ```
//!
//! What it models (each is exercised by a case study in
//! [`crate::compiler_literacy::borrowck_case_studies`]):
//!
//! - **Non-lexical lifetimes**: a borrow's region is just the points where a
//!   reference derived from it may still be used.
//! - **Flow sensitivity**: a borrow live on one branch does not block the other.
//! - **Reborrow constraints**: borrowing `*p` requires `p`'s region to outlive
//!   the new borrow, stopping at the first shared deref.
//! - **Kills**: overwriting `p` ends borrows of `*p` (NLL problem case #4).
//! - **Two-phase borrows**: `&two_phase mut` acts shared until activated.
//! - **Drop-liveness**: a value with a `Drop` impl keeps its regions live
//!   until the drop.
//! - **Universal regions**: returning data that outlives the wrong lifetime is
//!   *"lifetime may not live long enough"*.
//!
//! What it leaves out: types beyond references/structs/opaque blobs, generic
//! function signatures (calls state which arguments the result borrows from),
//! moves-out-of-uninitialized checks, mutability checks and `#[may_dangle]`.
//!
//! ## Example
//!
//! ```
//! use rust_interview_practice::compiler_literacy::nll::*;
//!
//! // let mut v = vec![..]; let first = &v; v = vec![]; use(first);
//! let mut b = BodyBuilder::new("overwrite_while_borrowed");
//! let v = b.var("v", Ty::opaque("Vec<i32>"));
//! let r = b.ref_ty(Mutability::Not, Ty::opaque("Vec<i32>"));
//! let first = b.var("first", r);
//! let tmp = b.temp(Ty::opaque("Vec<i32>"));
//! let bb0 = b.new_block();
//! let borrow = b.borrow(BorrowKind::Shared, Place::from(v));
//! b.assign(bb0, first, borrow);
//! b.assign(bb0, v, Rvalue::Use(Operand::Const(0)));
//! b.assign(bb0, tmp, Rvalue::Use(Operand::Copy(Place::from(first).deref())));
//! b.terminate(bb0, Terminator::Return);
//!
//! let result = borrowck(&b.build());
//! assert_eq!(result.codes(), vec![ErrorCode::E0506]);
//! assert_eq!(result.diagnostics[0].message, "cannot assign to `v` because it is borrowed");
//! ```

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use super::mir_reading::{BasicBlock, Local, Location};
use super::region_constraints::{
    ConstraintCategory, ConstraintLocation, RegionConstraintSet, RegionValues, RegionVid,
    UniversalKind,
};

// =========================================================================================
// Toy MIR
// =========================================================================================

/// Reference mutability.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mutability {
    /// `&T`
    Not,
    /// `&mut T`
    Mut,
}

/// The toy type system: just enough structure to carry regions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ty {
    /// `i32` (any `Copy` scalar).
    Int,
    /// `()`.
    Unit,
    /// A region-free type known only by name (`Vec<i32>`, `HashMap<..>`).
    Opaque(String),
    /// `&'r T` / `&'r mut T`.
    Ref {
        /// The reference's region.
        region: RegionVid,
        /// Shared or mutable.
        mutbl: Mutability,
        /// The referent type.
        pointee: Box<Self>,
    },
    /// A struct with positional fields.
    Struct {
        /// Type name.
        name: String,
        /// Field types.
        fields: Vec<Self>,
        /// Has a user `Drop` impl (its destructor may use every region).
        has_drop: bool,
    },
}

impl Ty {
    /// Shorthand for [`Ty::Opaque`].
    pub fn opaque(name: impl Into<String>) -> Self {
        Self::Opaque(name.into())
    }

    /// Every region mentioned by the type, outermost first.
    #[must_use]
    pub fn regions(&self) -> Vec<RegionVid> {
        let mut out = Vec::new();
        self.collect_regions(&mut out);
        out
    }

    fn collect_regions(&self, out: &mut Vec<RegionVid>) {
        match self {
            Self::Ref {
                region, pointee, ..
            } => {
                out.push(*region);
                pointee.collect_regions(out);
            }
            Self::Struct { fields, .. } => {
                for f in fields {
                    f.collect_regions(out);
                }
            }
            Self::Int | Self::Unit | Self::Opaque(_) => {}
        }
    }

    /// `true` if dropping a value of this type runs user code that may touch
    /// its regions (so the value is *drop-live* at its `drop`).
    #[must_use]
    pub fn drop_uses_regions(&self) -> bool {
        match self {
            Self::Struct {
                fields, has_drop, ..
            } => *has_drop || fields.iter().any(Self::drop_uses_regions),
            Self::Int | Self::Unit | Self::Opaque(_) | Self::Ref { .. } => false,
        }
    }

    /// The type of `self` after one projection.
    #[must_use]
    pub fn project(&self, elem: ProjectionElem) -> Option<&Self> {
        match (self, elem) {
            (Self::Ref { pointee, .. }, ProjectionElem::Deref) => Some(pointee),
            (Self::Struct { fields, .. }, ProjectionElem::Field(i)) => fields.get(i),
            _ => None,
        }
    }
}

impl fmt::Display for Ty {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Int => f.write_str("i32"),
            Self::Unit => f.write_str("()"),
            Self::Opaque(name) => f.write_str(name),
            Self::Ref {
                region,
                mutbl,
                pointee,
            } => match mutbl {
                Mutability::Not => write!(f, "&{region} {pointee}"),
                Mutability::Mut => write!(f, "&{region} mut {pointee}"),
            },
            Self::Struct { name, .. } => {
                let regions = self.regions();
                if regions.is_empty() {
                    f.write_str(name)
                } else {
                    let list: Vec<String> = regions.iter().map(ToString::to_string).collect();
                    write!(f, "{name}<{}>", list.join(", "))
                }
            }
        }
    }
}

/// One step of a place path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectionElem {
    /// `*p`
    Deref,
    /// `p.i`
    Field(usize),
}

/// A memory location: a local plus a projection path, e.g. `(*_1).0`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Place {
    /// Base local.
    pub local: Local,
    /// Projections, innermost (closest to the local) first.
    pub projection: Vec<ProjectionElem>,
}

impl Place {
    /// `*self`.
    #[must_use]
    pub fn deref(mut self) -> Self {
        self.projection.push(ProjectionElem::Deref);
        self
    }

    /// `self.i`.
    #[must_use]
    pub fn field(mut self, i: usize) -> Self {
        self.projection.push(ProjectionElem::Field(i));
        self
    }

    /// `true` if any projection is a deref (the place is not owned by the frame).
    #[must_use]
    pub fn has_deref(&self) -> bool {
        self.projection.contains(&ProjectionElem::Deref)
    }

    /// `true` if `self` is `other` or a path `other` extends.
    #[must_use]
    pub fn is_prefix_of(&self, other: &Self) -> bool {
        self.local == other.local && other.projection.starts_with(&self.projection)
    }
}

impl From<Local> for Place {
    fn from(local: Local) -> Self {
        Self {
            local,
            projection: Vec::new(),
        }
    }
}

impl fmt::Display for Place {
    /// rustc MIR style: `(*_1)`, `(_2.0)`, `(*(_2.0))`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut s = self.local.to_string();
        for elem in &self.projection {
            s = match elem {
                ProjectionElem::Deref => format!("(*{s})"),
                ProjectionElem::Field(i) => format!("({s}.{i})"),
            };
        }
        f.write_str(&s)
    }
}

/// An operand.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Operand {
    /// Read; the source stays usable.
    Copy(Place),
    /// Read and invalidate the source.
    Move(Place),
    /// A constant.
    Const(i64),
}

impl Operand {
    const fn place(&self) -> Option<&Place> {
        match self {
            Self::Copy(p) | Self::Move(p) => Some(p),
            Self::Const(_) => None,
        }
    }
}

impl fmt::Display for Operand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Copy(p) => write!(f, "copy {p}"),
            Self::Move(p) => write!(f, "move {p}"),
            Self::Const(c) => write!(f, "const {c}"),
        }
    }
}

/// Borrow flavour.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BorrowKind {
    /// `&place`
    Shared,
    /// `&mut place`
    Mut,
    /// `&mut place` that rustc created for an autoref (method receiver):
    /// *reserved* here, *activated* at the first use of the reference.
    TwoPhaseMut,
}

/// Right-hand side of an assignment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Rvalue {
    /// `copy p` / `move p` / `const c`.
    Use(Operand),
    /// `&'r place` / `&'r mut place`.
    Ref {
        /// The borrow's region.
        region: RegionVid,
        /// Flavour.
        kind: BorrowKind,
        /// Borrowed place.
        place: Place,
    },
    /// Struct construction: `Name(op0, op1, ..)`.
    Aggregate {
        /// Struct name (for printing).
        name: String,
        /// Field values in order.
        operands: Vec<Operand>,
    },
}

impl fmt::Display for Rvalue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Use(op) => write!(f, "{op}"),
            Self::Ref {
                region,
                kind,
                place,
            } => match kind {
                BorrowKind::Shared => write!(f, "&{region} {place}"),
                BorrowKind::Mut | BorrowKind::TwoPhaseMut => write!(f, "&{region} mut {place}"),
            },
            Self::Aggregate { name, operands } => {
                let ops: Vec<String> = operands.iter().map(ToString::to_string).collect();
                write!(f, "{name}({})", ops.join(", "))
            }
        }
    }
}

/// A statement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Statement {
    /// `place = rvalue`
    Assign(Place, Rvalue),
    /// The local's storage becomes valid.
    StorageLive(Local),
    /// The local's storage becomes invalid; borrows of it must be dead.
    StorageDead(Local),
}

impl fmt::Display for Statement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Assign(
                p,
                Rvalue::Ref {
                    kind: BorrowKind::TwoPhaseMut,
                    region,
                    place,
                },
            ) => write!(f, "{p} = &{region} mut {place}; // two-phase"),
            Self::Assign(p, rv) => write!(f, "{p} = {rv};"),
            Self::StorageLive(l) => write!(f, "StorageLive({l});"),
            Self::StorageDead(l) => write!(f, "StorageDead({l});"),
        }
    }
}

/// A block terminator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Terminator {
    /// Unconditional jump.
    Goto(BasicBlock),
    /// Branch on an integer: `targets[i]` for value `i`, last is `otherwise`.
    SwitchInt {
        /// Scrutinee.
        discr: Operand,
        /// Targets; the last one is the `otherwise` arm.
        targets: Vec<BasicBlock>,
    },
    /// `dest = func(args) -> target`.
    Call {
        /// Callee name (for printing).
        func: String,
        /// Arguments.
        args: Vec<Operand>,
        /// Where the result goes.
        dest: Place,
        /// Indices of reference arguments the result borrows from, i.e. the
        /// signature is `fn<'x>(&'x .., ..) -> &'x ..` for those arguments.
        returns_borrow_from: Vec<usize>,
        /// Return edge.
        target: BasicBlock,
    },
    /// Run the destructor of `place`, then continue.
    Drop {
        /// What to drop.
        place: Place,
        /// Continue here.
        target: BasicBlock,
    },
    /// Return `_0` to the caller.
    Return,
}

impl Terminator {
    /// Successor blocks.
    #[must_use]
    pub fn successors(&self) -> Vec<BasicBlock> {
        match self {
            Self::Goto(t) | Self::Call { target: t, .. } | Self::Drop { target: t, .. } => {
                vec![*t]
            }
            Self::SwitchInt { targets, .. } => targets.clone(),
            Self::Return => Vec::new(),
        }
    }
}

impl fmt::Display for Terminator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Goto(t) => write!(f, "goto -> {t};"),
            Self::SwitchInt { discr, targets } => {
                let mut arms: Vec<String> = Vec::new();
                for (i, t) in targets.iter().enumerate() {
                    if i + 1 == targets.len() {
                        arms.push(format!("otherwise: {t}"));
                    } else {
                        arms.push(format!("{i}: {t}"));
                    }
                }
                write!(f, "switchInt({discr}) -> [{}];", arms.join(", "))
            }
            Self::Call {
                func,
                args,
                dest,
                target,
                ..
            } => {
                let args: Vec<String> = args.iter().map(ToString::to_string).collect();
                write!(
                    f,
                    "{dest} = {func}({}) -> [return: {target}, unwind continue];",
                    args.join(", ")
                )
            }
            Self::Drop { place, target } => {
                write!(f, "drop({place}) -> [return: {target}, unwind continue];")
            }
            Self::Return => f.write_str("return;"),
        }
    }
}

/// A local's declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalDecl {
    /// Source name (for diagnostics), if any.
    pub name: Option<String>,
    /// Its type.
    pub ty: Ty,
}

/// A basic block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockData {
    /// Straight-line statements.
    pub statements: Vec<Statement>,
    /// The terminator.
    pub terminator: Terminator,
}

/// A toy MIR body plus its region variables.
#[derive(Debug, Clone)]
pub struct Body {
    /// Function name.
    pub name: String,
    /// `_0` (return place), then `arg_count` arguments, then the rest.
    pub locals: Vec<LocalDecl>,
    /// Number of arguments.
    pub arg_count: usize,
    /// Basic blocks, indexed by id.
    pub blocks: Vec<BlockData>,
    /// Region variables, universal regions and assumed bounds (no constraints yet).
    pub regions: RegionConstraintSet,
}

impl Body {
    /// The type of `place`, if the projections fit the types.
    #[must_use]
    pub fn place_ty(&self, place: &Place) -> Option<&Ty> {
        let mut ty = &self.locals.get(place.local.0)?.ty;
        for &elem in &place.projection {
            ty = ty.project(elem)?;
        }
        Some(ty)
    }

    /// Every program point, in block order.
    #[must_use]
    pub fn locations(&self) -> Vec<Location> {
        self.blocks
            .iter()
            .enumerate()
            .flat_map(|(b, data)| (0..=data.statements.len()).map(move |i| Location::new(b, i)))
            .collect()
    }

    /// Points control can reach right after `loc`.
    #[must_use]
    pub fn successor_points(&self, loc: Location) -> Vec<Location> {
        let Some(block) = self.blocks.get(loc.block.0) else {
            return Vec::new();
        };
        if loc.statement_index < block.statements.len() {
            vec![loc.successor_within_block()]
        } else {
            block
                .terminator
                .successors()
                .into_iter()
                .map(|bb| Location {
                    block: bb,
                    statement_index: 0,
                })
                .collect()
        }
    }

    /// Source-level description of a place, the way rustc words diagnostics:
    /// `v`, `*map`, `p.0`, falling back to MIR names for temporaries.
    #[must_use]
    pub fn describe(&self, place: &Place) -> String {
        let mut s = self
            .locals
            .get(place.local.0)
            .and_then(|d| d.name.clone())
            .unwrap_or_else(|| place.local.to_string());
        for elem in &place.projection {
            s = match elem {
                ProjectionElem::Deref => format!("*{s}"),
                ProjectionElem::Field(i) => format!("{s}.{i}"),
            };
        }
        s
    }
}

impl fmt::Display for Body {
    /// Prints the body like `-Zunpretty=mir` with `-Zverbose` regions.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let args: Vec<String> = (1..=self.arg_count)
            .filter_map(|i| self.locals.get(i).map(|d| format!("_{i}: {}", d.ty)))
            .collect();
        let ret = self.locals.first().map_or(Ty::Unit, |d| d.ty.clone());
        writeln!(f, "fn {}({}) -> {ret} {{", self.name, args.join(", "))?;
        for (i, decl) in self.locals.iter().enumerate() {
            if let Some(name) = &decl.name {
                writeln!(f, "    debug {name} => _{i};")?;
            }
        }
        for (i, decl) in self.locals.iter().enumerate().skip(self.arg_count + 1) {
            writeln!(f, "    let _{i}: {};", decl.ty)?;
        }
        if let Some(d) = self.locals.first() {
            writeln!(f, "    // return place: _0: {}", d.ty)?;
        }
        for (b, block) in self.blocks.iter().enumerate() {
            writeln!(f)?;
            writeln!(f, "    bb{b}: {{")?;
            for stmt in &block.statements {
                writeln!(f, "        {stmt}")?;
            }
            writeln!(f, "        {}", block.terminator)?;
            writeln!(f, "    }}")?;
        }
        writeln!(f, "}}")
    }
}

// =========================================================================================
// Builder
// =========================================================================================

/// Builds a [`Body`]. Declare the return type and arguments before other locals.
#[derive(Debug)]
pub struct BodyBuilder {
    body: Body,
}

impl BodyBuilder {
    /// A body whose return type is `()` until [`Self::return_ty`] is called.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            body: Body {
                name: name.into(),
                locals: vec![LocalDecl {
                    name: None,
                    ty: Ty::Unit,
                }],
                arg_count: 0,
                blocks: Vec::new(),
                regions: RegionConstraintSet::default(),
            },
        }
    }

    /// A named lifetime parameter (`'a`), or `'static` if `name == "'static"`.
    pub fn universal(&mut self, name: &str) -> RegionVid {
        let kind = if name == "'static" {
            UniversalKind::Global
        } else {
            UniversalKind::Local
        };
        self.body.regions.add_universal(kind, name)
    }

    /// Records a `where 'longer: 'shorter` clause.
    pub fn assume_outlives(&mut self, longer: RegionVid, shorter: RegionVid) {
        self.body.regions.assume_outlives(longer, shorter);
    }

    /// A fresh inference region (what rustc's *renumber* pass gives every
    /// region in the body).
    pub const fn fresh_region(&mut self) -> RegionVid {
        self.body.regions.fresh_region()
    }

    /// `&'fresh T` / `&'fresh mut T`.
    pub fn ref_ty(&mut self, mutbl: Mutability, pointee: Ty) -> Ty {
        Ty::Ref {
            region: self.fresh_region(),
            mutbl,
            pointee: Box::new(pointee),
        }
    }

    /// A borrow rvalue with a fresh region.
    pub const fn borrow(&mut self, kind: BorrowKind, place: Place) -> Rvalue {
        Rvalue::Ref {
            region: self.fresh_region(),
            kind,
            place,
        }
    }

    /// Sets `_0`'s type and returns `_0`.
    pub fn return_ty(&mut self, ty: Ty) -> Local {
        self.body.locals[0].ty = ty;
        Local::RETURN_PLACE
    }

    /// Declares the next argument.
    pub fn arg(&mut self, name: &str, ty: Ty) -> Local {
        debug_assert_eq!(
            self.body.locals.len(),
            self.body.arg_count + 1,
            "declare arguments before other locals"
        );
        self.body.arg_count += 1;
        self.push_local(Some(name), ty)
    }

    /// Declares a user variable.
    pub fn var(&mut self, name: &str, ty: Ty) -> Local {
        self.push_local(Some(name), ty)
    }

    /// Declares a compiler temporary.
    pub fn temp(&mut self, ty: Ty) -> Local {
        self.push_local(None, ty)
    }

    fn push_local(&mut self, name: Option<&str>, ty: Ty) -> Local {
        self.body.locals.push(LocalDecl {
            name: name.map(str::to_string),
            ty,
        });
        Local(self.body.locals.len() - 1)
    }

    /// A new, empty block ending in `return` until terminated.
    pub fn new_block(&mut self) -> BasicBlock {
        self.body.blocks.push(BlockData {
            statements: Vec::new(),
            terminator: Terminator::Return,
        });
        BasicBlock(self.body.blocks.len() - 1)
    }

    /// Appends a statement.
    ///
    /// # Panics
    ///
    /// If `bb` was not created by this builder's [`Self::new_block`].
    pub fn push(&mut self, bb: BasicBlock, stmt: Statement) {
        self.body.blocks[bb.0].statements.push(stmt);
    }

    /// Appends `place = rvalue`.
    ///
    /// # Panics
    ///
    /// If `bb` was not created by this builder's [`Self::new_block`].
    pub fn assign(&mut self, bb: BasicBlock, place: impl Into<Place>, rvalue: Rvalue) {
        self.push(bb, Statement::Assign(place.into(), rvalue));
    }

    /// Sets the block's terminator.
    ///
    /// # Panics
    ///
    /// If `bb` was not created by this builder's [`Self::new_block`].
    pub fn terminate(&mut self, bb: BasicBlock, terminator: Terminator) {
        self.body.blocks[bb.0].terminator = terminator;
    }

    /// Finishes the body.
    #[must_use]
    pub fn build(self) -> Body {
        self.body
    }
}

// =========================================================================================
// 1. Liveness
// =========================================================================================

/// Locals live on entry to each point.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Liveness {
    live: BTreeMap<Location, BTreeSet<Local>>,
}

impl Liveness {
    /// Locals live on entry to `loc`.
    #[must_use]
    pub fn live_at(&self, loc: Location) -> BTreeSet<Local> {
        self.live.get(&loc).cloned().unwrap_or_default()
    }

    /// `true` if `local` may be used at or after `loc` before being overwritten.
    #[must_use]
    pub fn is_live(&self, local: Local, loc: Location) -> bool {
        self.live.get(&loc).is_some_and(|s| s.contains(&local))
    }
}

/// Def/use summary of one point, rustc's `DefUse` categorisation.
#[derive(Debug, Default)]
struct DefUse {
    defs: Vec<Local>,
    uses: Vec<Local>,
}

fn place_def_use(place: &Place, du: &mut DefUse) {
    if place.projection.is_empty() {
        du.defs.push(place.local);
    } else if place.has_deref() {
        // Writing through `*p` reads `p`.
        du.uses.push(place.local);
    }
    // A store to a field of a local is neither a def nor a use.
}

fn def_use_at(body: &Body, loc: Location) -> DefUse {
    let mut du = DefUse::default();
    let block = &body.blocks[loc.block.0];
    if let Some(stmt) = block.statements.get(loc.statement_index) {
        match stmt {
            Statement::Assign(place, rv) => {
                place_def_use(place, &mut du);
                match rv {
                    Rvalue::Use(op) => du.uses.extend(op.place().map(|p| p.local)),
                    Rvalue::Ref { place, .. } => du.uses.push(place.local),
                    Rvalue::Aggregate { operands, .. } => {
                        du.uses
                            .extend(operands.iter().filter_map(Operand::place).map(|p| p.local));
                    }
                }
            }
            Statement::StorageLive(l) | Statement::StorageDead(l) => du.defs.push(*l),
        }
    } else {
        match &block.terminator {
            Terminator::Goto(_) => {}
            Terminator::SwitchInt { discr, .. } => du.uses.extend(discr.place().map(|p| p.local)),
            Terminator::Call { args, dest, .. } => {
                place_def_use(dest, &mut du);
                du.uses
                    .extend(args.iter().filter_map(Operand::place).map(|p| p.local));
            }
            Terminator::Drop { place, .. } => {
                // Drop-live only if the destructor can observe the regions.
                if body.place_ty(place).is_some_and(Ty::drop_uses_regions) {
                    du.uses.push(place.local);
                }
            }
            Terminator::Return => du.uses.push(Local::RETURN_PLACE),
        }
    }
    du
}

/// Backward "may be used later" dataflow, iterated to a fixpoint.
///
/// `live_in(P) = (live_out(P) − defs(P)) ∪ uses(P)` and `live_out(P)` is the
/// union of `live_in` over successor points.
///
/// Time: O(I · P · L) for I iterations (bounded by loop nesting + 2), P
/// points and L locals. Space: O(P · L).
#[must_use]
pub fn compute_liveness(body: &Body) -> Liveness {
    let points = body.locations();
    let def_uses: BTreeMap<Location, DefUse> =
        points.iter().map(|&p| (p, def_use_at(body, p))).collect();
    let mut live: BTreeMap<Location, BTreeSet<Local>> =
        points.iter().map(|&p| (p, BTreeSet::new())).collect();
    let mut changed = true;
    while changed {
        changed = false;
        for &p in points.iter().rev() {
            let mut set: BTreeSet<Local> = body
                .successor_points(p)
                .iter()
                .filter_map(|s| live.get(s))
                .flatten()
                .copied()
                .collect();
            let du = &def_uses[&p];
            for d in &du.defs {
                set.remove(d);
            }
            set.extend(du.uses.iter().copied());
            if live.get(&p) != Some(&set) {
                live.insert(p, set);
                changed = true;
            }
        }
    }
    Liveness { live }
}

// =========================================================================================
// 2. Constraint generation
// =========================================================================================

/// Relates `sub <: sup`, recording outlives constraints.
fn relate(
    cs: &mut RegionConstraintSet,
    sub: &Ty,
    sup: &Ty,
    category: &ConstraintCategory,
    at: ConstraintLocation,
) {
    match (sub, sup) {
        (
            Ty::Ref {
                region: r1,
                mutbl,
                pointee: p1,
            },
            Ty::Ref {
                region: r2,
                pointee: p2,
                ..
            },
        ) => {
            // &'r1 T <: &'r2 U requires 'r1: 'r2.
            cs.add_outlives(*r1, *r2, category.clone(), at);
            relate(cs, p1, p2, category, at);
            if *mutbl == Mutability::Mut {
                // &mut T is invariant in T.
                relate(cs, p2, p1, category, at);
            }
        }
        (Ty::Struct { fields: f1, .. }, Ty::Struct { fields: f2, .. }) => {
            for (a, b) in f1.iter().zip(f2) {
                relate(cs, a, b, category, at);
            }
        }
        _ => {}
    }
}

/// Borrowing `place` with region `borrow_region` requires every reference
/// dereferenced on the way to outlive the new borrow. Walks from the outermost
/// deref inwards and stops after the first *shared* reference: data behind
/// `&T` is frozen anyway, so outer references cannot be invalidated.
fn add_reborrow_constraints(
    body: &Body,
    cs: &mut RegionConstraintSet,
    place: &Place,
    borrow_region: RegionVid,
    at: ConstraintLocation,
) {
    for i in (0..place.projection.len()).rev() {
        if place.projection[i] != ProjectionElem::Deref {
            continue;
        }
        let base = Place {
            local: place.local,
            projection: place.projection[..i].to_vec(),
        };
        if let Some(Ty::Ref { region, mutbl, .. }) = body.place_ty(&base) {
            cs.add_outlives(*region, borrow_region, ConstraintCategory::Boring, at);
            if *mutbl == Mutability::Not {
                break;
            }
        }
    }
}

const fn category_for(dest: &Place) -> ConstraintCategory {
    if dest.local.0 == 0 && dest.projection.is_empty() {
        ConstraintCategory::Return
    } else {
        ConstraintCategory::Assignment
    }
}

/// Generates the full region constraint set for `body`.
#[must_use]
pub fn generate_constraints(body: &Body, liveness: &Liveness) -> RegionConstraintSet {
    let points = body.locations();
    let mut cs = body.regions.clone();
    cs.set_body_points(points.iter().copied());

    // Liveness constraints: each region in a live local's type contains P.
    for &p in &points {
        for local in liveness.live_at(p) {
            for r in body.locals[local.0].ty.regions() {
                cs.add_live(r, p);
            }
        }
    }

    for &p in &points {
        let at = ConstraintLocation::Single(p);
        let block = &body.blocks[p.block.0];
        if let Some(Statement::Assign(dest, rv)) = block.statements.get(p.statement_index) {
            let Some(dest_ty) = body.place_ty(dest) else {
                continue;
            };
            let category = category_for(dest);
            match rv {
                Rvalue::Use(op) => {
                    if let Some(src_ty) = op.place().and_then(|src| body.place_ty(src)) {
                        relate(&mut cs, src_ty, dest_ty, &category, at);
                    }
                }
                Rvalue::Ref {
                    region,
                    kind,
                    place,
                } => {
                    // The borrow is live where it is created.
                    cs.add_live(*region, p);
                    if let Some(pointee) = body.place_ty(place) {
                        let mutbl = match kind {
                            BorrowKind::Shared => Mutability::Not,
                            BorrowKind::Mut | BorrowKind::TwoPhaseMut => Mutability::Mut,
                        };
                        let rv_ty = Ty::Ref {
                            region: *region,
                            mutbl,
                            pointee: Box::new(pointee.clone()),
                        };
                        relate(&mut cs, &rv_ty, dest_ty, &category, at);
                    }
                    add_reborrow_constraints(body, &mut cs, place, *region, at);
                }
                Rvalue::Aggregate { operands, .. } => {
                    if let Ty::Struct { fields, .. } = dest_ty {
                        for (op, field_ty) in operands.iter().zip(fields) {
                            if let Some(src_ty) = op.place().and_then(|src| body.place_ty(src)) {
                                relate(&mut cs, src_ty, field_ty, &category, at);
                            }
                        }
                    }
                }
            }
        } else if let Terminator::Call {
            args,
            dest,
            returns_borrow_from,
            ..
        } = &block.terminator
        {
            if p.statement_index != block.statements.len() {
                continue;
            }
            let dest_regions = body.place_ty(dest).map(Ty::regions).unwrap_or_default();
            for &i in returns_borrow_from {
                let arg_region = args
                    .get(i)
                    .and_then(Operand::place)
                    .and_then(|a| body.place_ty(a))
                    .and_then(|t| t.regions().first().copied());
                if let Some(ar) = arg_region {
                    for &dr in &dest_regions {
                        cs.add_outlives(ar, dr, ConstraintCategory::CallArgument, at);
                    }
                }
            }
        }
    }
    cs
}

// =========================================================================================
// 3. Borrows and their scopes
// =========================================================================================

/// One borrow in the body (rustc's `BorrowData`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BorrowData {
    /// Issue point.
    pub location: Location,
    /// Flavour.
    pub kind: BorrowKind,
    /// Borrowed place.
    pub place: Place,
    /// The borrow's region.
    pub region: RegionVid,
    /// The local that receives the reference.
    pub assigned_to: Local,
    /// Points where the borrow is in scope (on entry).
    pub in_scope: BTreeSet<Location>,
    /// For two-phase borrows: points where the reference is first used.
    pub activations: BTreeSet<Location>,
    /// For two-phase borrows: in-scope points at or after activation.
    pub activated: BTreeSet<Location>,
}

/// `true` if the point reads `local` (operands, borrowed places, `*local` writes).
fn uses_local(body: &Body, loc: Location, local: Local) -> bool {
    let block = &body.blocks[loc.block.0];
    let op_uses = |op: &Operand| op.place().is_some_and(|p| p.local == local);
    match block.statements.get(loc.statement_index) {
        Some(Statement::Assign(dest, rv)) => {
            (dest.local == local && dest.has_deref())
                || match rv {
                    Rvalue::Use(op) => op_uses(op),
                    Rvalue::Ref { place, .. } => place.local == local,
                    Rvalue::Aggregate { operands, .. } => operands.iter().any(op_uses),
                }
        }
        Some(_) => false,
        None => match &block.terminator {
            Terminator::SwitchInt { discr, .. } => op_uses(discr),
            Terminator::Call { args, dest, .. } => {
                args.iter().any(op_uses) || (dest.local == local && dest.has_deref())
            }
            Terminator::Drop { place, .. } => place.local == local,
            Terminator::Return => local == Local::RETURN_PLACE,
            Terminator::Goto(_) => false,
        },
    }
}

/// `true` if the point's effect ends `borrow` (overwrites or frees its root).
fn kills(body: &Body, loc: Location, borrow: &Place) -> bool {
    let block = &body.blocks[loc.block.0];
    match block.statements.get(loc.statement_index) {
        Some(Statement::Assign(dest, _)) => dest.is_prefix_of(borrow),
        Some(Statement::StorageDead(l)) => *l == borrow.local,
        Some(Statement::StorageLive(_)) => false,
        None => match &block.terminator {
            Terminator::Call { dest, .. } => dest.is_prefix_of(borrow),
            _ => false,
        },
    }
}

/// Forward reachability from `starts`, staying inside `allowed` and not
/// continuing past points that `stop_after` rejects.
fn reach(
    body: &Body,
    starts: Vec<Location>,
    allowed: impl Fn(Location) -> bool,
    stop_after: impl Fn(Location) -> bool,
) -> BTreeSet<Location> {
    let mut seen = BTreeSet::new();
    let mut stack = starts;
    while let Some(p) = stack.pop() {
        if !allowed(p) || !seen.insert(p) {
            continue;
        }
        if !stop_after(p) {
            stack.extend(body.successor_points(p));
        }
    }
    seen
}

fn collect_borrows(body: &Body, values: &RegionValues) -> Vec<BorrowData> {
    let mut borrows = Vec::new();
    for (b, block) in body.blocks.iter().enumerate() {
        for (i, stmt) in block.statements.iter().enumerate() {
            let Statement::Assign(
                dest,
                Rvalue::Ref {
                    region,
                    kind,
                    place,
                },
            ) = stmt
            else {
                continue;
            };
            let location = Location::new(b, i);
            let value = values.get(*region);
            let in_scope = reach(
                body,
                body.successor_points(location),
                |p| value.contains_point(p),
                |p| kills(body, p, place),
            );
            let (activations, activated) = if *kind == BorrowKind::TwoPhaseMut {
                let activations: BTreeSet<Location> = in_scope
                    .iter()
                    .copied()
                    .filter(|&p| uses_local(body, p, dest.local))
                    .collect();
                let activated = reach(
                    body,
                    activations
                        .iter()
                        .flat_map(|&a| body.successor_points(a))
                        .collect(),
                    |p| in_scope.contains(&p),
                    |_| false,
                );
                (activations, activated)
            } else {
                (BTreeSet::new(), BTreeSet::new())
            };
            borrows.push(BorrowData {
                location,
                kind: *kind,
                place: place.clone(),
                region: *region,
                assigned_to: dest.local,
                in_scope,
                activations,
                activated,
            });
        }
    }
    borrows
}

// =========================================================================================
// 4. Access checks
// =========================================================================================

/// Borrowck error codes this checker can produce.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ErrorCode {
    /// Two mutable borrows overlap.
    E0499,
    /// A mutable and a shared borrow overlap.
    E0502,
    /// A use of a place that is mutably borrowed.
    E0503,
    /// A move out of a borrowed place.
    E0505,
    /// An assignment to a borrowed place.
    E0506,
    /// Returning a reference to a local.
    E0515,
    /// A borrowed local goes out of scope (or is dropped) while borrowed.
    E0597,
    /// "lifetime may not live long enough" (no error code in rustc).
    LifetimeMayNotLiveLongEnough,
}

impl ErrorCode {
    /// The rustc code, e.g. `Some("E0502")`.
    #[must_use]
    pub const fn code(self) -> Option<&'static str> {
        match self {
            Self::E0499 => Some("E0499"),
            Self::E0502 => Some("E0502"),
            Self::E0503 => Some("E0503"),
            Self::E0505 => Some("E0505"),
            Self::E0506 => Some("E0506"),
            Self::E0515 => Some("E0515"),
            Self::E0597 => Some("E0597"),
            Self::LifetimeMayNotLiveLongEnough => None,
        }
    }
}

/// One borrowck error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    /// Which error.
    pub code: ErrorCode,
    /// Where the offending access happens (for region errors: the blamed
    /// constraint's location, if any).
    pub location: Option<Location>,
    /// Where the conflicting borrow was issued.
    pub borrow_location: Option<Location>,
    /// rustc-style message.
    pub message: String,
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.code.code() {
            Some(code) => write!(f, "error[{code}]: {}", self.message)?,
            None => write!(f, "error: {}", self.message)?,
        }
        if let Some(loc) = self.location {
            write!(f, " at {loc}")?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AccessKind {
    CopyRead,
    SharedBorrow,
    MutBorrow,
    Reservation,
    Activation,
    Move,
    Assign,
    StorageDead,
    Drop,
}

impl AccessKind {
    const fn is_read(self) -> bool {
        matches!(
            self,
            Self::CopyRead | Self::SharedBorrow | Self::Reservation
        )
    }

    /// Shallow accesses do not reach through a deref of the accessed place.
    const fn is_shallow(self) -> bool {
        matches!(self, Self::Assign | Self::StorageDead)
    }
}

/// rustc's `places_conflict`: could `borrowed` and `accessed` overlap?
fn places_conflict(borrowed: &Place, accessed: &Place, shallow: bool) -> bool {
    if borrowed.local != accessed.local {
        return false;
    }
    for (b, a) in borrowed.projection.iter().zip(&accessed.projection) {
        if let (ProjectionElem::Field(x), ProjectionElem::Field(y)) = (b, a)
            && x != y
        {
            return false; // disjoint fields
        }
    }
    if borrowed.projection.len() > accessed.projection.len() {
        // The borrow is of something *inside* the accessed place. A shallow
        // access (e.g. overwriting a reference) does not touch what lies
        // behind a deref.
        let extension = &borrowed.projection[accessed.projection.len()..];
        return !(shallow && extension.contains(&ProjectionElem::Deref));
    }
    true
}

/// How an in-scope borrow behaves at a point.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Effective {
    Shared,
    Mut,
}

fn conflict_code(access: AccessKind, borrow: Effective) -> Option<ErrorCode> {
    if borrow == Effective::Shared && access.is_read() {
        return None;
    }
    Some(match access {
        AccessKind::MutBorrow | AccessKind::Reservation | AccessKind::Activation => {
            if borrow == Effective::Mut {
                ErrorCode::E0499
            } else {
                ErrorCode::E0502
            }
        }
        AccessKind::SharedBorrow => ErrorCode::E0502,
        AccessKind::CopyRead => ErrorCode::E0503,
        AccessKind::Move => ErrorCode::E0505,
        AccessKind::Assign => ErrorCode::E0506,
        AccessKind::StorageDead | AccessKind::Drop => ErrorCode::E0597,
    })
}

fn message(body: &Body, code: ErrorCode, accessed: &Place, borrow: Effective) -> String {
    let p = body.describe(accessed);
    match code {
        ErrorCode::E0499 => format!("cannot borrow `{p}` as mutable more than once at a time"),
        ErrorCode::E0502 => {
            let (now, before) = if borrow == Effective::Mut {
                ("immutable", "mutable")
            } else {
                ("mutable", "immutable")
            };
            format!("cannot borrow `{p}` as {now} because it is also borrowed as {before}")
        }
        ErrorCode::E0503 => format!("cannot use `{p}` because it was mutably borrowed"),
        ErrorCode::E0505 => format!("cannot move out of `{p}` because it is borrowed"),
        ErrorCode::E0506 => format!("cannot assign to `{p}` because it is borrowed"),
        ErrorCode::E0515 => format!("cannot return reference to local variable `{p}`"),
        ErrorCode::E0597 => format!("`{p}` does not live long enough"),
        ErrorCode::LifetimeMayNotLiveLongEnough => "lifetime may not live long enough".to_string(),
    }
}

/// One access at a point. `borrow` is the borrow the access belongs to (the
/// one being created or activated), if any.
#[derive(Debug, Clone)]
struct Access {
    place: Place,
    kind: AccessKind,
    borrow: Option<usize>,
}

/// Accesses performed at `loc`, in evaluation order: activations, operands,
/// then the write to the destination.
fn accesses_at(body: &Body, loc: Location, borrows: &[BorrowData]) -> Vec<Access> {
    let mut out = Vec::new();
    let op_access = |op: &Operand, out: &mut Vec<Access>| match op {
        Operand::Copy(p) => out.push(Access {
            place: p.clone(),
            kind: AccessKind::CopyRead,
            borrow: None,
        }),
        Operand::Move(p) => out.push(Access {
            place: p.clone(),
            kind: AccessKind::Move,
            borrow: None,
        }),
        Operand::Const(_) => {}
    };
    // Two-phase activations happen where the reference is used.
    for (i, b) in borrows.iter().enumerate() {
        if b.activations.contains(&loc) {
            out.push(Access {
                place: b.place.clone(),
                kind: AccessKind::Activation,
                borrow: Some(i),
            });
        }
    }
    let block = &body.blocks[loc.block.0];
    let assign = |dest: &Place| Access {
        place: dest.clone(),
        kind: AccessKind::Assign,
        borrow: None,
    };
    match block.statements.get(loc.statement_index) {
        Some(Statement::Assign(dest, rv)) => {
            match rv {
                Rvalue::Use(op) => op_access(op, &mut out),
                Rvalue::Ref { kind, place, .. } => out.push(Access {
                    place: place.clone(),
                    kind: match kind {
                        BorrowKind::Shared => AccessKind::SharedBorrow,
                        BorrowKind::Mut => AccessKind::MutBorrow,
                        BorrowKind::TwoPhaseMut => AccessKind::Reservation,
                    },
                    borrow: borrows.iter().position(|b| b.location == loc),
                }),
                Rvalue::Aggregate { operands, .. } => {
                    for op in operands {
                        op_access(op, &mut out);
                    }
                }
            }
            out.push(assign(dest));
        }
        Some(Statement::StorageDead(l)) => out.push(Access {
            place: Place::from(*l),
            kind: AccessKind::StorageDead,
            borrow: None,
        }),
        Some(Statement::StorageLive(_)) => {}
        None => match &block.terminator {
            Terminator::SwitchInt { discr, .. } => op_access(discr, &mut out),
            Terminator::Call { args, dest, .. } => {
                for a in args {
                    op_access(a, &mut out);
                }
                out.push(assign(dest));
            }
            Terminator::Drop { place, .. } => out.push(Access {
                place: place.clone(),
                kind: AccessKind::Drop,
                borrow: None,
            }),
            Terminator::Goto(_) | Terminator::Return => {}
        },
    }
    out
}

fn check_accesses(body: &Body, borrows: &[BorrowData]) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    // Like rustc, report a two-phase borrow once: if its reservation already
    // conflicted, its activation is not reported again.
    let mut reported: BTreeSet<usize> = BTreeSet::new();
    for loc in body.locations() {
        for access in accesses_at(body, loc, borrows) {
            if access.kind == AccessKind::Activation
                && access.borrow.is_some_and(|b| reported.contains(&b))
            {
                continue;
            }
            let shallow = access.kind.is_shallow();
            let hit = borrows.iter().enumerate().find_map(|(i, b)| {
                // An activation never conflicts with its own reservation. A
                // fresh borrow *can* conflict with itself: in a loop, the
                // previous iteration's borrow may still be in scope.
                let own = access.kind == AccessKind::Activation && access.borrow == Some(i);
                if own || !b.in_scope.contains(&loc) {
                    return None;
                }
                if !places_conflict(&b.place, &access.place, shallow) {
                    return None;
                }
                let effective = match b.kind {
                    BorrowKind::Mut => Effective::Mut,
                    BorrowKind::TwoPhaseMut if b.activated.contains(&loc) => Effective::Mut,
                    // Reserved-but-not-activated two-phase borrows act shared.
                    BorrowKind::Shared | BorrowKind::TwoPhaseMut => Effective::Shared,
                };
                conflict_code(access.kind, effective).map(|code| (code, effective, b.location))
            });
            if let Some((code, effective, borrow_location)) = hit {
                reported.extend(access.borrow);
                diagnostics.push(Diagnostic {
                    code,
                    location: Some(loc),
                    borrow_location: Some(borrow_location),
                    message: message(body, code, &access.place, effective),
                });
            }
        }
        // Returning ends every local: borrows of frame-owned data must be dead.
        if body.blocks[loc.block.0].statements.len() == loc.statement_index
            && body.blocks[loc.block.0].terminator == Terminator::Return
            && let Some(b) = borrows
                .iter()
                .find(|b| b.in_scope.contains(&loc) && !b.place.has_deref())
        {
            diagnostics.push(Diagnostic {
                code: ErrorCode::E0515,
                location: Some(loc),
                borrow_location: Some(b.location),
                message: message(body, ErrorCode::E0515, &b.place, Effective::Shared),
            });
        }
    }
    diagnostics
}

// =========================================================================================
// Driver
// =========================================================================================

/// Everything the checker computed, for inspection in drills and tests.
#[derive(Debug, Clone)]
pub struct BorrowckResult {
    /// Errors: access conflicts in program order, then region errors.
    pub diagnostics: Vec<Diagnostic>,
    /// Locals live at each point.
    pub liveness: Liveness,
    /// The generated constraints.
    pub constraints: RegionConstraintSet,
    /// The solved regions.
    pub region_values: RegionValues,
    /// Every borrow with its scope.
    pub borrows: Vec<BorrowData>,
}

impl BorrowckResult {
    /// The error codes, in report order.
    #[must_use]
    pub fn codes(&self) -> Vec<ErrorCode> {
        self.diagnostics.iter().map(|d| d.code).collect()
    }

    /// `true` if the body borrow-checks.
    #[must_use]
    pub const fn is_ok(&self) -> bool {
        self.diagnostics.is_empty()
    }

    /// The borrow issued at `loc`.
    #[must_use]
    pub fn borrow_at(&self, loc: Location) -> Option<&BorrowData> {
        self.borrows.iter().find(|b| b.location == loc)
    }
}

/// Runs the whole pipeline on `body`.
///
/// Time: dominated by liveness and region solving, polynomial in points ×
/// locals × regions; fine for drill-sized bodies.
#[must_use]
pub fn borrowck(body: &Body) -> BorrowckResult {
    let liveness = compute_liveness(body);
    let constraints = generate_constraints(body, &liveness);
    let region_values = constraints.solve();
    let borrows = collect_borrows(body, &region_values);
    let mut diagnostics = check_accesses(body, &borrows);
    for err in constraints.check_universal_regions(&region_values) {
        let location = err.blamed().and_then(|c| match c.at {
            ConstraintLocation::Single(l) => Some(l),
            ConstraintLocation::All => None,
        });
        diagnostics.push(Diagnostic {
            code: ErrorCode::LifetimeMayNotLiveLongEnough,
            location,
            borrow_location: None,
            message: format!(
                "lifetime may not live long enough: `{}` must outlive `{}`",
                err.longer_name, err.shorter_name
            ),
        });
    }
    BorrowckResult {
        diagnostics,
        liveness,
        constraints,
        region_values,
        borrows,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `let x = 1; let r = &x; let y = *r; x = 2;` — the borrow dies after its
    /// last use, so the assignment is fine (NLL, not lexical scopes).
    fn nll_basic() -> (Body, Local, Local) {
        let mut b = BodyBuilder::new("nll_basic");
        let x = b.var("x", Ty::Int);
        let rt = b.ref_ty(Mutability::Not, Ty::Int);
        let r = b.var("r", rt);
        let y = b.var("y", Ty::Int);
        let bb0 = b.new_block();
        b.assign(bb0, x, Rvalue::Use(Operand::Const(1)));
        let br = b.borrow(BorrowKind::Shared, Place::from(x));
        b.assign(bb0, r, br);
        b.assign(bb0, y, Rvalue::Use(Operand::Copy(Place::from(r).deref())));
        b.assign(bb0, x, Rvalue::Use(Operand::Const(2)));
        b.terminate(bb0, Terminator::Return);
        (b.build(), x, r)
    }

    #[test]
    fn liveness_ends_at_last_use() {
        let (body, _x, r) = nll_basic();
        let result = borrowck(&body);
        assert!(
            !result.liveness.is_live(r, Location::new(0, 1)),
            "r is defined here"
        );
        assert!(result.liveness.is_live(r, Location::new(0, 2)));
        assert!(!result.liveness.is_live(r, Location::new(0, 3)));
        assert!(
            result
                .liveness
                .live_at(Location::new(0, 4))
                .contains(&Local(0))
        );
        assert!(result.is_ok(), "{:?}", result.diagnostics);
        let borrow = result.borrow_at(Location::new(0, 1)).expect("borrow");
        assert_eq!(
            result.region_values.get(borrow.region).to_string(),
            "{bb0[1..=2]}"
        );
        assert_eq!(borrow.in_scope, BTreeSet::from([Location::new(0, 2)]));
    }

    #[test]
    fn using_the_borrow_after_the_write_is_e0506() {
        let (mut body, x, r) = nll_basic();
        let use_again = Statement::Assign(
            Place::from(Local(3)),
            Rvalue::Use(Operand::Copy(Place::from(r).deref())),
        );
        body.blocks[0].statements.push(use_again);
        let result = borrowck(&body);
        assert_eq!(result.codes(), vec![ErrorCode::E0506]);
        let d = &result.diagnostics[0];
        assert_eq!(d.location, Some(Location::new(0, 3)));
        assert_eq!(d.borrow_location, Some(Location::new(0, 1)));
        assert_eq!(
            d.to_string(),
            "error[E0506]: cannot assign to `x` because it is borrowed at bb0[3]"
        );
        assert_eq!(body.describe(&Place::from(x)), "x");
    }

    #[test]
    fn places_conflict_rules() {
        let p = |l: usize| Place::from(Local(l));
        // Same local, disjoint fields.
        assert!(!places_conflict(&p(1).field(0), &p(1).field(1), false));
        assert!(places_conflict(&p(1).field(0), &p(1), false));
        assert!(places_conflict(&p(1), &p(1).field(0), false));
        // Overwriting the reference `p` doesn't touch `*p` (shallow write).
        assert!(!places_conflict(&p(1).deref(), &p(1), true));
        assert!(places_conflict(&p(1).deref(), &p(1), false));
        // ...but it does conflict with a borrow of a field of `p` itself.
        assert!(places_conflict(&p(1).field(0), &p(1), true));
        assert!(!places_conflict(&p(1), &p(2), false));
    }

    #[test]
    fn reborrow_constraints_stop_at_shared_deref() {
        // x: &'?2 mut &'?1 &'?0 mut i32; y = &(***x)
        let mut b = BodyBuilder::new("reborrow");
        let inner = b.ref_ty(Mutability::Mut, Ty::Int);
        let mid = b.ref_ty(Mutability::Not, inner);
        let outer = b.ref_ty(Mutability::Mut, mid);
        let x = b.arg("x", outer);
        let rt = b.ref_ty(Mutability::Not, Ty::Int);
        let y = b.var("y", rt);
        let bb0 = b.new_block();
        let br = b.borrow(BorrowKind::Shared, Place::from(x).deref().deref().deref());
        b.assign(bb0, y, br);
        let body = b.build();
        let result = borrowck(&body);
        let from_reborrow: Vec<String> = result
            .constraints
            .outlives()
            .iter()
            .filter(|c| c.category == ConstraintCategory::Boring)
            .map(|c| format!("{}: {}", c.sup, c.sub))
            .collect();
        // Outermost deref goes through `&'?0 mut` (keep going), then `&'?1`
        // (shared: add and stop). `'?2` is never required to outlive the borrow.
        assert_eq!(from_reborrow, vec!["'?0: '?4", "'?1: '?4"]);
        assert!(result.is_ok());
    }

    #[test]
    fn field_store_is_neither_def_nor_use() {
        let mut b = BodyBuilder::new("fields");
        let s = b.var(
            "s",
            Ty::Struct {
                name: "P".into(),
                fields: vec![Ty::Int, Ty::Int],
                has_drop: false,
            },
        );
        let bb0 = b.new_block();
        b.assign(bb0, Place::from(s).field(0), Rvalue::Use(Operand::Const(1)));
        b.terminate(bb0, Terminator::Return);
        let body = b.build();
        let du = def_use_at(&body, Location::new(0, 0));
        assert_eq!((du.defs, du.uses), (Vec::new(), Vec::new()));
        assert_eq!(body.describe(&Place::from(s).field(1)), "s.1");
        assert_eq!(Place::from(s).field(1).to_string(), "(_1.1)");
        assert_eq!(body.place_ty(&Place::from(s).field(2)), None);
    }

    #[test]
    fn loop_carried_mut_borrow_is_e0499() {
        // loop { let r = &mut x; keep.push(r) } — `keep` outlives the loop, so the
        // borrow from the previous iteration is still in scope.
        let mut b = BodyBuilder::new("loop_borrow");
        let x = b.var("x", Ty::Int);
        let rt = b.ref_ty(Mutability::Mut, Ty::Int);
        let keep_inner = rt.clone();
        let keep = b.var(
            "keep",
            Ty::Struct {
                name: "Vec".into(),
                fields: vec![keep_inner],
                has_drop: false,
            },
        );
        let r = b.var("r", rt);
        let sink = b.var("sink", Ty::Int);
        let bb0 = b.new_block();
        let bb1 = b.new_block();
        let bb2 = b.new_block();
        b.terminate(bb0, Terminator::Goto(bb1));
        let br = b.borrow(BorrowKind::Mut, Place::from(x));
        b.assign(bb1, r, br);
        b.assign(
            bb1,
            Place::from(keep).field(0),
            Rvalue::Use(Operand::Move(Place::from(r))),
        );
        b.terminate(
            bb1,
            Terminator::SwitchInt {
                discr: Operand::Const(0),
                targets: vec![bb1, bb2],
            },
        );
        b.assign(
            bb2,
            sink,
            Rvalue::Use(Operand::Copy(Place::from(keep).field(0).deref())),
        );
        b.push(bb2, Statement::StorageDead(keep));
        let body = b.build();
        let result = borrowck(&body);
        assert_eq!(
            result.codes(),
            vec![ErrorCode::E0499],
            "{:?}",
            result.diagnostics
        );
        assert_eq!(result.diagnostics[0].location, Some(Location::new(1, 0)));
        assert_eq!(
            result.diagnostics[0].borrow_location,
            Some(Location::new(1, 0))
        );
        assert!(
            body.to_string()
                .contains("switchInt(const 0) -> [0: bb1, otherwise: bb2];")
        );
    }

    #[test]
    fn body_display_reads_like_rustc_mir() {
        let (body, ..) = nll_basic();
        let text = body.to_string();
        let expected = "\
fn nll_basic() -> () {
    debug x => _1;
    debug r => _2;
    debug y => _3;
    let _1: i32;
    let _2: &'?0 i32;
    let _3: i32;
    // return place: _0: ()

    bb0: {
        _1 = const 1;
        _2 = &'?1 _1;
        _3 = copy (*_2);
        _1 = const 2;
        return;
    }
}
";
        assert_eq!(text, expected);
    }

    #[test]
    fn error_codes_map_to_rustc() {
        assert_eq!(ErrorCode::E0597.code(), Some("E0597"));
        assert_eq!(ErrorCode::LifetimeMayNotLiveLongEnough.code(), None);
        let d = Diagnostic {
            code: ErrorCode::LifetimeMayNotLiveLongEnough,
            location: None,
            borrow_location: None,
            message: "lifetime may not live long enough".into(),
        };
        assert_eq!(d.to_string(), "error: lifetime may not live long enough");
    }
}
