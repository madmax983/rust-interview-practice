//! # Reading MIR
//!
//! MIR (Mid-level IR) is the control-flow graph rustc borrow-checks. Every
//! borrow-checker error is a statement about *this* program, not about your
//! source code, so the fastest way to understand a confusing error is to look
//! at the MIR the checker actually saw.
//!
//! ## Getting MIR out of rustc
//!
//! ```bash
//! # Built MIR, before optimizations (closest to what borrowck sees):
//! cargo +nightly rustc --lib -- -Zunpretty=mir -Zmir-opt-level=0
//! # Borrowck's own view, with region variables and constraints:
//! cargo +nightly rustc --lib -- -Zdump-mir=nll -Zdump-mir-dir=mir_dump
//! # On a stable toolchain (debugging only, never in a build script):
//! RUSTC_BOOTSTRAP=1 rustc --crate-type=lib -Zunpretty=mir -Zmir-opt-level=0 file.rs
//! ```
//!
//! The Rust Playground's "MIR" button shows the optimized flavour.
//!
//! ## An annotated dump
//!
//! [`FIRST_OR_PUSH_MIR`] is the real output of rustc 1.97 for:
//!
//! ```
//! pub fn first_or_push(v: &mut Vec<i32>) -> i32 {
//!     let first = &v[0];
//!     let x = *first;
//!     v.push(x);
//!     x
//! }
//! # let mut v = vec![7];
//! # assert_eq!(first_or_push(&mut v), 7);
//! # assert_eq!(v, [7, 7]);
//! ```
//!
//! ```text
//! fn first_or_push(_1: &mut Vec<i32>) -> i32 {   // args are _1.._n
//!     debug v => _1;                     // source name -> MIR local
//!     let mut _0: i32;                   // _0 is ALWAYS the return place
//!     let _2: &i32;                      // `first`
//!     let _3: &i32;                      // temp: result of Index::index
//!     let mut _4: &std::vec::Vec<i32>;   // temp: the autoref `&*v`
//!     ...
//!     scope 1 { debug first => _2; ... } // lexical scopes, for debuginfo only
//!
//!     bb0: {                             // basic block: straight-line code
//!         StorageLive(_2);               // _2's stack slot becomes valid
//!         _4 = &(*_1);                   // reborrow through the &mut: shared
//!         _3 = <Vec<i32> as Index<usize>>::index(move _4, const 0_usize)
//!              -> [return: bb1, unwind continue];   // a call TERMINATES a block
//!     }
//!     bb1: {
//!         StorageDead(_4);               // slot invalid again; no drop runs
//!         _2 = &(*_3);                   // `&v[0]` is `&*Index::index(&*v, 0)`
//!         _5 = copy (*_2);               // `copy` = bitwise read of a Copy type
//!         _7 = &mut (*_1);               // autoref for `v.push`
//!         _6 = Vec::<i32>::push(move _7, move _8) -> [return: bb2, unwind continue];
//!     }
//!     bb2: { ...; _0 = copy _5; ...; return; }    // `return` reads _0
//! }
//! ```
//!
//! Reading checklist:
//!
//! - **Locals** — `_0` return place, `_1.._n` arguments, then user variables and
//!   temporaries. `debug name => _N` maps a source name onto its local.
//! - **Places** — `(*_1)` is "deref `_1`", `(_1.0: T)` is field 0. A borrow of
//!   `(*_1)` is a *reborrow* through the reference in `_1`.
//! - **Operands** — `copy _5` (read, source still usable), `move _4` (source is
//!   dead afterwards), `const 0_usize`.
//! - **Points** — `bb1[6]` is statement 6 of `bb1`; the terminator sits at index
//!   `statements.len()`. Region values in NLL dumps are sets of these points.
//! - **Terminators** — `goto`, `switchInt`, `return`, `drop`, calls. Edges in
//!   brackets: `return:` (normal), `unwind:` (panic path to a `(cleanup)` block).
//!   `unwind continue` means "no cleanup needed here, keep unwinding".
//! - **`FakeRead`** — appears in borrowck MIR only; it pins a `let` binding as
//!   "used" so patterns like `let _x = &mut y;` behave as written.
//!
//! [`MirFn::parse`] reads that text format into a small structure so the drills
//! below can answer questions about it programmatically: which local is
//! `first`, where every borrow is issued, what the CFG edges are.

use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;

/// Real rustc 1.97 `-Zunpretty=mir -Zmir-opt-level=0` output for `first_or_push`
/// (see the module docs for the source).
pub const FIRST_OR_PUSH_MIR: &str = "\
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
        _3 = <Vec<i32> as Index<usize>>::index(move _4, const 0_usize) -> [return: bb1, unwind continue];
    }

    bb1: {
        StorageDead(_4);
        _2 = &(*_3);
        StorageLive(_5);
        _5 = copy (*_2);
        StorageLive(_6);
        StorageLive(_7);
        _7 = &mut (*_1);
        StorageLive(_8);
        _8 = copy _5;
        _6 = Vec::<i32>::push(move _7, move _8) -> [return: bb2, unwind continue];
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
}
";

/// Real rustc 1.97 MIR for `pub fn two_phase(v: &mut Vec<usize>) { v.push(v.len()); }`.
///
/// Read it carefully: the `&mut (*_1)` for the receiver (`bb0[2]`) is created
/// **before** the shared `&(*_1)` for `v.len()` (`bb0[5]`). That only
/// type-checks because the receiver borrow is a *two-phase borrow*: reserved at
/// `bb0[2]`, activated at the `push` call in `bb1[1]`. Between the two it
/// behaves like a shared borrow. See
/// [`crate::compiler_literacy::borrowck_case_studies`] for the version that
/// fails (`Vec::push(&mut *v, v.len())`, where the borrow is not two-phase).
pub const TWO_PHASE_MIR: &str = "\
fn two_phase(_1: &mut Vec<usize>) -> () {
    debug v => _1;
    let mut _0: ();
    let _2: ();
    let mut _3: &mut std::vec::Vec<usize>;
    let mut _4: usize;
    let mut _5: &std::vec::Vec<usize>;

    bb0: {
        StorageLive(_2);
        StorageLive(_3);
        _3 = &mut (*_1);
        StorageLive(_4);
        StorageLive(_5);
        _5 = &(*_1);
        _4 = Vec::<usize>::len(move _5) -> [return: bb1, unwind continue];
    }

    bb1: {
        StorageDead(_5);
        _2 = Vec::<usize>::push(move _3, move _4) -> [return: bb2, unwind continue];
    }

    bb2: {
        StorageDead(_4);
        StorageDead(_3);
        StorageDead(_2);
        _0 = const ();
        return;
    }
}
";

// =========================================================================================
// Core identifiers
// =========================================================================================

/// A MIR local: `_0` is the return place, `_1.._n` are the arguments.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Local(pub usize);

impl Local {
    /// The return place `_0`.
    pub const RETURN_PLACE: Self = Self(0);
}

impl fmt::Display for Local {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "_{}", self.0)
    }
}

impl FromStr for Local {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.strip_prefix('_')
            .and_then(|digits| digits.parse().ok())
            .map(Self)
            .ok_or_else(|| ParseError::new(0, format!("expected a local like `_3`, got `{s}`")))
    }
}

/// A basic block id: `bb0`, `bb1`, ...
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BasicBlock(pub usize);

impl BasicBlock {
    /// The entry block of every body.
    pub const START: Self = Self(0);
}

impl fmt::Display for BasicBlock {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "bb{}", self.0)
    }
}

impl FromStr for BasicBlock {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.strip_prefix("bb")
            .and_then(|digits| digits.parse().ok())
            .map(Self)
            .ok_or_else(|| ParseError::new(0, format!("expected a block like `bb3`, got `{s}`")))
    }
}

/// A program point: statement `statement_index` of `block`.
///
/// The terminator of a block with `n` statements lives at index `n`.
/// Ordering is by block, then index, which is how rustc prints region values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Location {
    /// The basic block.
    pub block: BasicBlock,
    /// Index into the block's statements (`len` = the terminator).
    pub statement_index: usize,
}

impl Location {
    /// Shorthand constructor: `Location::new(1, 4)` is `bb1[4]`.
    #[must_use]
    pub const fn new(block: usize, statement_index: usize) -> Self {
        Self {
            block: BasicBlock(block),
            statement_index,
        }
    }

    /// The next point in the same block.
    #[must_use]
    pub const fn successor_within_block(self) -> Self {
        Self {
            block: self.block,
            statement_index: self.statement_index + 1,
        }
    }
}

impl fmt::Display for Location {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}[{}]", self.block, self.statement_index)
    }
}

impl FromStr for Location {
    type Err = ParseError;

    /// Parses `bb1[4]`.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let err = || ParseError::new(0, format!("expected a location like `bb1[4]`, got `{s}`"));
        let (block, rest) = s.split_once('[').ok_or_else(err)?;
        let index = rest.strip_suffix(']').ok_or_else(err)?;
        Ok(Self {
            block: block.parse()?,
            statement_index: index.parse().map_err(|_| err())?,
        })
    }
}

/// Error from parsing MIR or NLL dump text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    /// 1-based line number, or 0 when the error is not tied to a line.
    pub line: usize,
    /// What went wrong.
    pub message: String,
}

impl ParseError {
    /// Creates an error at `line` (1-based; 0 = unknown).
    pub fn new(line: usize, message: impl Into<String>) -> Self {
        Self {
            line,
            message: message.into(),
        }
    }

    const fn at_line(mut self, line: usize) -> Self {
        if self.line == 0 {
            self.line = line;
        }
        self
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.line == 0 {
            write!(f, "{}", self.message)
        } else {
            write!(f, "line {}: {}", self.line, self.message)
        }
    }
}

impl std::error::Error for ParseError {}

// =========================================================================================
// Parsed structure
// =========================================================================================

/// A `let` declaration in the MIR header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalDecl {
    /// The local being declared.
    pub local: Local,
    /// Declared `let mut` (rustc marks temporaries that get reassigned `mut`).
    pub mutable: bool,
    /// The type, verbatim.
    pub ty: String,
}

/// Mutability of a borrow rvalue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefKind {
    /// `&place`
    Shared,
    /// `&mut place`
    Mut,
    /// `&fake shallow place` and friends (match guards); borrowck-only.
    Fake,
    /// `&raw const place` / `&raw mut place`; not tracked by borrowck.
    Raw,
}

/// Coarse classification of an assignment's right-hand side.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RvalueText {
    /// A borrow: `&(*_1)`, `&mut _3`.
    Ref {
        /// Shared / mut / fake / raw.
        kind: RefKind,
        /// The borrowed place, verbatim, e.g. `(*_1)`.
        place: String,
    },
    /// `copy p`, `move p`, `const c`.
    Use(String),
    /// Anything else (casts, binary ops, aggregates, ...), verbatim.
    Other(String),
}

/// One MIR statement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StatementText {
    /// `place = rvalue;`
    Assign {
        /// Destination place, verbatim.
        place: String,
        /// Classified right-hand side.
        rvalue: RvalueText,
    },
    /// `StorageLive(_N);`
    StorageLive(Local),
    /// `StorageDead(_N);`
    StorageDead(Local),
    /// `FakeRead(cause, place);`
    FakeRead(String),
    /// Anything else, verbatim (without the trailing `;`).
    Other(String),
}

/// What kind of terminator ends a block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminatorKind {
    /// `goto -> bbN`
    Goto,
    /// `switchInt(op) -> [...]`
    SwitchInt,
    /// `return`
    Return,
    /// `resume` (continue unwinding out of a cleanup block)
    Resume,
    /// `unreachable`
    Unreachable,
    /// `drop(place) -> [...]`
    Drop,
    /// `assert(...) -> [...]`
    Assert,
    /// `dest = func(args) -> [...]`
    Call,
    /// Anything else (`falseEdge`, `yield`, ...).
    Other,
}

/// A labelled CFG edge out of a terminator, e.g. `return: bb1` or `unwind: bb3`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edge {
    /// `return`, `unwind`, `success`, `otherwise`, a switch value, or `goto`.
    pub label: String,
    /// Target block.
    pub target: BasicBlock,
}

/// A block terminator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminatorText {
    /// Classification.
    pub kind: TerminatorKind,
    /// The full terminator text (without the trailing `;`).
    pub text: String,
    /// Outgoing edges, in printed order.
    pub edges: Vec<Edge>,
}

/// A basic block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockText {
    /// This block's id.
    pub id: BasicBlock,
    /// `true` for `bbN (cleanup): {` blocks, which only run while unwinding.
    pub cleanup: bool,
    /// Straight-line statements.
    pub statements: Vec<StatementText>,
    /// The single terminator.
    pub terminator: TerminatorText,
}

/// A borrow found in the MIR: `dest = &kind place` at `location`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BorrowSite {
    /// Where the borrow is issued.
    pub location: Location,
    /// The place receiving the reference, verbatim (usually a bare local).
    pub dest: String,
    /// Shared / mut / fake / raw.
    pub kind: RefKind,
    /// The borrowed place, verbatim.
    pub place: String,
}

/// One parsed MIR function.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MirFn {
    /// Function name from the `fn` line.
    pub name: String,
    /// Arguments as `(local, type)`.
    pub args: Vec<(Local, String)>,
    /// Return type from the `fn` line.
    pub return_ty: String,
    /// All `let` declarations (in printed order; `_0` first).
    pub decls: Vec<LocalDecl>,
    /// `debug name => place` entries, in printed order.
    pub debug_vars: Vec<(String, String)>,
    /// Basic blocks indexed by id.
    pub blocks: Vec<BlockText>,
}

impl MirFn {
    /// Parses the first function in `text`.
    ///
    /// # Errors
    ///
    /// Returns a [`ParseError`] if `text` contains no function or the first one
    /// is malformed.
    pub fn parse(text: &str) -> Result<Self, ParseError> {
        Self::parse_all(text)?
            .into_iter()
            .next()
            .ok_or_else(|| ParseError::new(0, "no `fn` found"))
    }

    /// Parses every function in a `-Zunpretty=mir` dump. Comment lines (`//`)
    /// and NLL-dump header lines (`|`) are skipped; trailing `// ...` span
    /// comments are stripped.
    ///
    /// Time: O(L) in the length of the text.
    ///
    /// # Errors
    ///
    /// Returns a [`ParseError`] naming the first malformed line.
    pub fn parse_all(text: &str) -> Result<Vec<Self>, ParseError> {
        let mut parser = Parser::default();
        for (index, raw) in text.lines().enumerate() {
            let line_no = index + 1;
            parser
                .line(strip_comment(raw).trim())
                .map_err(|e| e.at_line(line_no))?;
        }
        parser.finish()
    }

    /// The block with id `bb`.
    #[must_use]
    pub fn block(&self, bb: BasicBlock) -> Option<&BlockText> {
        self.blocks.get(bb.0)
    }

    /// Number of program points in `bb` (statements + terminator).
    #[must_use]
    pub fn points_in(&self, bb: BasicBlock) -> usize {
        self.block(bb).map_or(0, |b| b.statements.len() + 1)
    }

    /// Every program point, in block order.
    #[must_use]
    pub fn locations(&self) -> Vec<Location> {
        self.blocks
            .iter()
            .flat_map(|b| {
                (0..=b.statements.len()).map(move |i| Location {
                    block: b.id,
                    statement_index: i,
                })
            })
            .collect()
    }

    /// The statement at `loc`, or `None` for a terminator / out of range.
    #[must_use]
    pub fn statement_at(&self, loc: Location) -> Option<&StatementText> {
        self.block(loc.block)?.statements.get(loc.statement_index)
    }

    /// Successor blocks of `bb`, including unwind edges, de-duplicated.
    #[must_use]
    pub fn successors(&self, bb: BasicBlock) -> Vec<BasicBlock> {
        let mut out = Vec::new();
        if let Some(block) = self.block(bb) {
            for edge in &block.terminator.edges {
                if !out.contains(&edge.target) {
                    out.push(edge.target);
                }
            }
        }
        out
    }

    /// Predecessor map: for each block, the blocks that jump to it.
    #[must_use]
    pub fn predecessors(&self) -> BTreeMap<BasicBlock, Vec<BasicBlock>> {
        let mut preds: BTreeMap<BasicBlock, Vec<BasicBlock>> =
            self.blocks.iter().map(|b| (b.id, Vec::new())).collect();
        for block in &self.blocks {
            for succ in self.successors(block.id) {
                preds.entry(succ).or_default().push(block.id);
            }
        }
        preds
    }

    /// Reverse post-order from `bb0` — the iteration order forward dataflow
    /// analyses use so each block is visited after its (non-loop) predecessors.
    #[must_use]
    pub fn reverse_postorder(&self) -> Vec<BasicBlock> {
        let mut visited = vec![false; self.blocks.len()];
        let mut post = Vec::with_capacity(self.blocks.len());
        // Iterative DFS: (block, next-successor-index).
        let mut stack = vec![(BasicBlock::START, 0usize)];
        if let Some(v) = visited.get_mut(0) {
            *v = true;
        } else {
            return post;
        }
        while let Some((bb, next)) = stack.pop() {
            let succs = self.successors(bb);
            if let Some(&succ) = succs.get(next) {
                stack.push((bb, next + 1));
                if succ.0 < visited.len() && !visited[succ.0] {
                    visited[succ.0] = true;
                    stack.push((succ, 0));
                }
            } else {
                post.push(bb);
            }
        }
        post.reverse();
        post
    }

    /// The MIR local a source variable maps to, via `debug name => _N`.
    #[must_use]
    pub fn user_var(&self, name: &str) -> Option<Local> {
        self.debug_vars
            .iter()
            .find(|(n, _)| n == name)
            .and_then(|(_, place)| place.parse().ok())
    }

    /// The source name of `local`, if it has one.
    #[must_use]
    pub fn debug_name(&self, local: Local) -> Option<&str> {
        let printed = local.to_string();
        self.debug_vars
            .iter()
            .find(|(_, place)| *place == printed)
            .map(|(name, _)| name.as_str())
    }

    /// The declaration of `local`.
    #[must_use]
    pub fn decl(&self, local: Local) -> Option<&LocalDecl> {
        self.decls.iter().find(|d| d.local == local)
    }

    /// Every borrow statement in the body, in block order.
    #[must_use]
    pub fn borrows(&self) -> Vec<BorrowSite> {
        let mut out = Vec::new();
        for block in &self.blocks {
            for (i, stmt) in block.statements.iter().enumerate() {
                if let StatementText::Assign {
                    place: dest,
                    rvalue: RvalueText::Ref { kind, place },
                } = stmt
                {
                    out.push(BorrowSite {
                        location: Location {
                            block: block.id,
                            statement_index: i,
                        },
                        dest: dest.clone(),
                        kind: *kind,
                        place: place.clone(),
                    });
                }
            }
        }
        out
    }

    /// `(StorageLive locations, StorageDead locations)` of `local`.
    #[must_use]
    pub fn storage_markers(&self, local: Local) -> (Vec<Location>, Vec<Location>) {
        let mut live = Vec::new();
        let mut dead = Vec::new();
        for block in &self.blocks {
            for (i, stmt) in block.statements.iter().enumerate() {
                let loc = Location {
                    block: block.id,
                    statement_index: i,
                };
                match stmt {
                    StatementText::StorageLive(l) if *l == local => live.push(loc),
                    StatementText::StorageDead(l) if *l == local => dead.push(loc),
                    _ => {}
                }
            }
        }
        (live, dead)
    }

    /// Every point whose statement or terminator text mentions `local`,
    /// excluding storage markers. Useful for "where is `_5` used?".
    #[must_use]
    pub fn mentions(&self, local: Local) -> Vec<Location> {
        let mut out = Vec::new();
        for block in &self.blocks {
            for (i, stmt) in block.statements.iter().enumerate() {
                let text = match stmt {
                    StatementText::StorageLive(_) | StatementText::StorageDead(_) => continue,
                    StatementText::Assign { place, rvalue } => {
                        let rv = match rvalue {
                            RvalueText::Ref { place, .. } => place.as_str(),
                            RvalueText::Use(s) | RvalueText::Other(s) => s.as_str(),
                        };
                        format!("{place} {rv}")
                    }
                    StatementText::FakeRead(s) | StatementText::Other(s) => s.clone(),
                };
                if mentions_local(&text, local) {
                    out.push(Location {
                        block: block.id,
                        statement_index: i,
                    });
                }
            }
            if mentions_local(&block.terminator.text, local) {
                out.push(Location {
                    block: block.id,
                    statement_index: block.statements.len(),
                });
            }
        }
        out
    }
}

/// `true` if `text` contains the token `_N` for `local` (not `_N0`, not `x_N`).
#[must_use]
pub fn mentions_local(text: &str, local: Local) -> bool {
    let needle = local.to_string();
    let bytes = text.as_bytes();
    let mut start = 0;
    while let Some(offset) = text[start..].find(&needle) {
        let at = start + offset;
        let end = at + needle.len();
        let before_ok = at == 0 || !is_ident_byte(bytes[at - 1]);
        let after_ok = end == bytes.len() || !is_ident_byte(bytes[end]);
        if before_ok && after_ok {
            return true;
        }
        start = at + 1;
    }
    false
}

const fn is_ident_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// Removes a trailing `// ...` comment. `//` never appears inside MIR syntax.
fn strip_comment(line: &str) -> &str {
    line.find("//").map_or(line, |at| &line[..at])
}

/// Splits `s` on `sep` at bracket depth 0 (`<>`, `()`, `[]`, `{}`).
fn split_top_level(s: &str, sep: char) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0i32;
    let mut start = 0;
    for (i, c) in s.char_indices() {
        match c {
            '<' | '(' | '[' | '{' => depth += 1,
            // `->` is not a closing bracket.
            '>' if i > 0 && s.as_bytes()[i - 1] == b'-' => {}
            '>' | ')' | ']' | '}' => depth -= 1,
            c if c == sep && depth == 0 => {
                parts.push(s[start..i].trim());
                start = i + c.len_utf8();
            }
            _ => {}
        }
    }
    let last = s[start..].trim();
    if !last.is_empty() {
        parts.push(last);
    }
    parts
}

// =========================================================================================
// Line-oriented parser
// =========================================================================================

#[derive(Default)]
struct Parser {
    done: Vec<MirFn>,
    current: Option<MirFn>,
    /// Inside `bbN: { ... }`: (id, cleanup, lines so far).
    block: Option<(BasicBlock, bool, Vec<String>)>,
}

impl Parser {
    fn line(&mut self, line: &str) -> Result<(), ParseError> {
        if line.is_empty() || line.starts_with('|') {
            return Ok(());
        }
        if let Some(rest) = line.strip_prefix("fn ") {
            if let Some(f) = self.current.take() {
                self.done.push(f);
            }
            self.current = Some(parse_fn_header(rest)?);
            return Ok(());
        }
        let Some(func) = self.current.as_mut() else {
            // Text before the first `fn` (warnings, headers) is ignored.
            return Ok(());
        };
        if let Some((id, cleanup, lines)) = self.block.as_mut() {
            if line == "}" {
                let block = build_block(*id, *cleanup, lines)?;
                if block.id.0 != func.blocks.len() {
                    return Err(ParseError::new(
                        0,
                        format!("block {} out of order", block.id),
                    ));
                }
                func.blocks.push(block);
                self.block = None;
            } else {
                lines.push(line.trim_end_matches(';').to_string());
            }
            return Ok(());
        }
        if line.starts_with("bb") && line.ends_with('{') {
            let head = line.trim_end_matches('{').trim().trim_end_matches(':');
            let (id, cleanup) = head
                .strip_suffix(" (cleanup)")
                .map_or((head, false), |id| (id, true));
            self.block = Some((id.parse()?, cleanup, Vec::new()));
        } else if let Some(rest) = line.strip_prefix("debug ") {
            let rest = rest.trim_end_matches(';');
            let (name, place) = rest
                .split_once(" => ")
                .ok_or_else(|| ParseError::new(0, "malformed debug line"))?;
            func.debug_vars.push((name.to_string(), place.to_string()));
        } else if let Some(rest) = line.strip_prefix("let ") {
            let rest = rest.trim_end_matches(';');
            let (mutable, rest) = rest
                .strip_prefix("mut ")
                .map_or((false, rest), |r| (true, r));
            let (local, ty) = rest
                .split_once(": ")
                .ok_or_else(|| ParseError::new(0, "malformed let line"))?;
            func.decls.push(LocalDecl {
                local: local.parse()?,
                mutable,
                ty: ty.to_string(),
            });
        }
        // `scope N {` and the `}` closing scopes / the fn carry no data we need.
        Ok(())
    }

    fn finish(mut self) -> Result<Vec<MirFn>, ParseError> {
        if self.block.is_some() {
            return Err(ParseError::new(0, "unterminated basic block"));
        }
        if let Some(f) = self.current.take() {
            self.done.push(f);
        }
        if self.done.is_empty() {
            return Err(ParseError::new(0, "no `fn` found"));
        }
        Ok(self.done)
    }
}

fn parse_fn_header(rest: &str) -> Result<MirFn, ParseError> {
    let open = rest
        .find('(')
        .ok_or_else(|| ParseError::new(0, "fn line without `(`"))?;
    let name = rest[..open].trim().to_string();
    let after_name = &rest[open..];
    // Find the `)` matching the first `(`.
    let mut depth = 0i32;
    let mut close = None;
    for (i, c) in after_name.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    close = Some(i);
                    break;
                }
            }
            _ => {}
        }
    }
    let close = close.ok_or_else(|| ParseError::new(0, "unbalanced fn arguments"))?;
    let mut args = Vec::new();
    for arg in split_top_level(&after_name[1..close], ',') {
        let (local, ty) = arg
            .split_once(": ")
            .ok_or_else(|| ParseError::new(0, format!("malformed argument `{arg}`")))?;
        args.push((local.parse()?, ty.to_string()));
    }
    let tail = after_name[close + 1..].trim().trim_end_matches('{').trim();
    let return_ty = tail
        .strip_prefix("->")
        .map_or_else(|| "()".to_string(), |t| t.trim().to_string());
    Ok(MirFn {
        name,
        args,
        return_ty,
        decls: Vec::new(),
        debug_vars: Vec::new(),
        blocks: Vec::new(),
    })
}

fn build_block(id: BasicBlock, cleanup: bool, lines: &[String]) -> Result<BlockText, ParseError> {
    let (term, stmts) = lines
        .split_last()
        .ok_or_else(|| ParseError::new(0, format!("{id} has no terminator")))?;
    Ok(BlockText {
        id,
        cleanup,
        statements: stmts.iter().map(|s| parse_statement(s)).collect(),
        terminator: parse_terminator(term)?,
    })
}

fn parse_statement(s: &str) -> StatementText {
    if let Some(local) = s
        .strip_prefix("StorageLive(")
        .and_then(|r| r.strip_suffix(')'))
        .and_then(|l| l.parse().ok())
    {
        return StatementText::StorageLive(local);
    }
    if let Some(local) = s
        .strip_prefix("StorageDead(")
        .and_then(|r| r.strip_suffix(')'))
        .and_then(|l| l.parse().ok())
    {
        return StatementText::StorageDead(local);
    }
    if s.starts_with("FakeRead(") {
        return StatementText::FakeRead(s.to_string());
    }
    if let Some((place, rvalue)) = s.split_once(" = ") {
        return StatementText::Assign {
            place: place.to_string(),
            rvalue: parse_rvalue(rvalue),
        };
    }
    StatementText::Other(s.to_string())
}

fn parse_rvalue<'a>(rv: &'a str) -> RvalueText {
    if let Some(rest) = rv.strip_prefix('&') {
        // Optional region in verbose dumps: `&'?3 mut (*_1)`.
        let rest = if rest.starts_with('\'') {
            rest.split_once(' ').map_or(rest, |(_, r)| r)
        } else {
            rest
        };
        // `&raw const p` / `&fake shallow p` carry one more word before the place.
        let skip_word = |p: &'a str| p.split_once(' ').map_or(p, |(_, place)| place);
        let (kind, place) = match rest.split_once(' ') {
            Some(("mut", p)) => (RefKind::Mut, p),
            Some(("raw", p)) => (RefKind::Raw, skip_word(p)),
            Some(("fake", p)) => (RefKind::Fake, skip_word(p)),
            _ => (RefKind::Shared, rest),
        };
        return RvalueText::Ref {
            kind,
            place: place.to_string(),
        };
    }
    if rv.starts_with("copy ") || rv.starts_with("move ") || rv.starts_with("const ") {
        return RvalueText::Use(rv.to_string());
    }
    RvalueText::Other(rv.to_string())
}

fn parse_terminator(t: &str) -> Result<TerminatorText, ParseError> {
    let (head, targets) = t
        .split_once(" -> ")
        .map_or((t, None), |(h, r)| (h, Some(r)));
    let mut edges = Vec::new();
    if let Some(targets) = targets {
        if let Some(list) = targets.strip_prefix('[').and_then(|r| r.strip_suffix(']')) {
            for item in split_top_level(list, ',') {
                // `unwind continue` / `unwind unreachable` carry no edge.
                if let Some((label, target)) = item.split_once(": ")
                    && let Ok(target) = target.parse()
                {
                    edges.push(Edge {
                        label: label.to_string(),
                        target,
                    });
                }
            }
        } else {
            edges.push(Edge {
                label: "goto".to_string(),
                target: targets.trim().parse()?,
            });
        }
    }
    let kind = if head.starts_with("goto") {
        TerminatorKind::Goto
    } else if head.starts_with("switchInt(") {
        TerminatorKind::SwitchInt
    } else if head == "return" {
        TerminatorKind::Return
    } else if head == "resume" || head == "UnwindResume" {
        TerminatorKind::Resume
    } else if head == "unreachable" {
        TerminatorKind::Unreachable
    } else if head.starts_with("drop(") {
        TerminatorKind::Drop
    } else if head.starts_with("assert(") {
        TerminatorKind::Assert
    } else if head.contains(" = ") && head.ends_with(')') {
        TerminatorKind::Call
    } else {
        TerminatorKind::Other
    };
    Ok(TerminatorText {
        kind,
        text: t.to_string(),
        edges,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fop() -> MirFn {
        MirFn::parse(FIRST_OR_PUSH_MIR).expect("captured MIR parses")
    }

    #[test]
    fn header_signature_and_decls() {
        let f = fop();
        assert_eq!(f.name, "first_or_push");
        assert_eq!(f.args, vec![(Local(1), "&mut Vec<i32>".to_string())]);
        assert_eq!(f.return_ty, "i32");
        // _0 is always declared first and is the return place.
        assert_eq!(f.decls[0].local, Local::RETURN_PLACE);
        assert!(f.decls[0].mutable);
        assert_eq!(
            f.decl(Local(7)).map(|d| d.ty.as_str()),
            Some("&mut std::vec::Vec<i32>")
        );
        // `let _5` lives inside `scope 1`; scope nesting must not hide it.
        assert_eq!(f.decl(Local(5)).map(|d| d.ty.as_str()), Some("i32"));
    }

    #[test]
    fn debug_vars_map_source_names_to_locals() {
        let f = fop();
        assert_eq!(f.user_var("v"), Some(Local(1)));
        assert_eq!(f.user_var("first"), Some(Local(2)));
        assert_eq!(f.user_var("x"), Some(Local(5)));
        assert_eq!(f.user_var("nope"), None);
        assert_eq!(f.debug_name(Local(2)), Some("first"));
        assert_eq!(
            f.debug_name(Local(4)),
            None,
            "temporaries have no debug name"
        );
    }

    #[test]
    fn blocks_points_and_terminators() {
        let f = fop();
        assert_eq!(f.blocks.len(), 3);
        assert_eq!(f.points_in(BasicBlock(0)), 5); // 4 statements + terminator
        assert_eq!(f.points_in(BasicBlock(1)), 10);
        assert_eq!(f.points_in(BasicBlock(2)), 8);
        assert_eq!(f.locations().len(), 23);
        assert_eq!(f.blocks[0].terminator.kind, TerminatorKind::Call);
        assert_eq!(f.blocks[2].terminator.kind, TerminatorKind::Return);
        assert_eq!(
            f.statement_at(Location::new(1, 3)),
            Some(&StatementText::Assign {
                place: "_5".to_string(),
                rvalue: RvalueText::Use("copy (*_2)".to_string()),
            })
        );
        assert_eq!(
            f.statement_at(Location::new(0, 4)),
            None,
            "bb0[4] is the terminator"
        );
    }

    #[test]
    fn cfg_edges_ignore_unwind_continue() {
        let f = fop();
        assert_eq!(f.successors(BasicBlock(0)), vec![BasicBlock(1)]);
        assert_eq!(f.successors(BasicBlock(2)), Vec::<BasicBlock>::new());
        let preds = f.predecessors();
        assert_eq!(preds[&BasicBlock(2)], vec![BasicBlock(1)]);
        assert_eq!(
            f.reverse_postorder(),
            vec![BasicBlock(0), BasicBlock(1), BasicBlock(2)]
        );
    }

    #[test]
    fn borrows_are_found_with_kind_and_place() {
        let f = fop();
        let borrows = f.borrows();
        let summary: Vec<_> = borrows
            .iter()
            .map(|b| {
                (
                    b.location.to_string(),
                    b.dest.as_str(),
                    b.kind,
                    b.place.as_str(),
                )
            })
            .collect();
        assert_eq!(
            summary,
            vec![
                ("bb0[3]".to_string(), "_4", RefKind::Shared, "(*_1)"),
                ("bb1[1]".to_string(), "_2", RefKind::Shared, "(*_3)"),
                ("bb1[6]".to_string(), "_7", RefKind::Mut, "(*_1)"),
            ]
        );
    }

    #[test]
    fn storage_markers_and_mentions() {
        let f = fop();
        let (live, dead) = f.storage_markers(Local(5));
        assert_eq!(live, vec![Location::new(1, 2)]);
        assert_eq!(dead, vec![Location::new(2, 4)]);
        // `x` (_5) is written at bb1[3], copied at bb1[8] and into _0 at bb2[3].
        assert_eq!(
            f.mentions(Local(5)),
            vec![
                Location::new(1, 3),
                Location::new(1, 8),
                Location::new(2, 3)
            ]
        );
    }

    #[test]
    fn two_phase_receiver_borrow_precedes_shared_borrow() {
        let f = MirFn::parse(TWO_PHASE_MIR).expect("captured MIR parses");
        let borrows = f.borrows();
        assert_eq!(borrows.len(), 2);
        let (recv, len_arg) = (&borrows[0], &borrows[1]);
        assert_eq!((recv.kind, recv.place.as_str()), (RefKind::Mut, "(*_1)"));
        assert_eq!(
            (len_arg.kind, len_arg.place.as_str()),
            (RefKind::Shared, "(*_1)")
        );
        // The &mut is issued first, yet the program compiles: two-phase borrow.
        assert!(recv.location < len_arg.location);
        // ...and it is only *used* (activated) by the push call in bb1.
        assert_eq!(
            f.mentions(Local(3)),
            vec![Location::new(0, 2), Location::new(1, 1)]
        );
        assert_eq!(f.return_ty, "()");
    }

    #[test]
    fn parses_cleanup_blocks_switches_and_drops() {
        let text = "\
// a leading comment
fn demo(_1: bool, _2: HashMap<u8, (u8, u8)>) {
    let mut _0: ();
    bb0: {
        switchInt(copy _1) -> [0: bb2, otherwise: bb1];
    }
    bb1: {
        goto -> bb2;
    }
    bb2: {
        drop(_2) -> [return: bb3, unwind: bb4];  // scope 0 at x.rs:1:1
    }
    bb3: {
        return;
    }
    bb4 (cleanup): {
        resume;
    }
}
";
        let f = MirFn::parse(text).expect("parses");
        assert_eq!(f.args.len(), 2, "commas inside generics are not separators");
        assert_eq!(f.args[1].1, "HashMap<u8, (u8, u8)>");
        assert_eq!(f.return_ty, "()");
        assert_eq!(f.blocks[0].terminator.kind, TerminatorKind::SwitchInt);
        assert_eq!(
            f.blocks[0]
                .terminator
                .edges
                .iter()
                .map(|e| e.label.as_str())
                .collect::<Vec<_>>(),
            vec!["0", "otherwise"]
        );
        assert_eq!(
            f.successors(BasicBlock(0)),
            vec![BasicBlock(2), BasicBlock(1)]
        );
        assert_eq!(f.blocks[1].terminator.kind, TerminatorKind::Goto);
        assert_eq!(f.blocks[2].terminator.kind, TerminatorKind::Drop);
        assert_eq!(
            f.successors(BasicBlock(2)),
            vec![BasicBlock(3), BasicBlock(4)]
        );
        assert!(f.blocks[4].cleanup);
        assert_eq!(f.blocks[4].terminator.kind, TerminatorKind::Resume);
        let preds = f.predecessors();
        assert_eq!(preds[&BasicBlock(2)], vec![BasicBlock(0), BasicBlock(1)]);
        assert_eq!(
            f.reverse_postorder(),
            vec![
                BasicBlock(0),
                BasicBlock(1),
                BasicBlock(2),
                BasicBlock(4),
                BasicBlock(3)
            ]
        );
    }

    #[test]
    fn parse_all_handles_multiple_functions() {
        let both = format!("{FIRST_OR_PUSH_MIR}\n{TWO_PHASE_MIR}");
        let fns = MirFn::parse_all(&both).expect("parses");
        assert_eq!(
            fns.iter().map(|f| f.name.as_str()).collect::<Vec<_>>(),
            vec!["first_or_push", "two_phase"]
        );
    }

    #[test]
    fn verbose_region_and_raw_borrows_are_classified() {
        assert_eq!(
            parse_rvalue("&'?3 mut (*_1)"),
            RvalueText::Ref {
                kind: RefKind::Mut,
                place: "(*_1)".to_string()
            }
        );
        assert_eq!(
            parse_rvalue("&raw const _2"),
            RvalueText::Ref {
                kind: RefKind::Raw,
                place: "_2".to_string()
            }
        );
        assert_eq!(
            parse_rvalue("&fake shallow _1"),
            RvalueText::Ref {
                kind: RefKind::Fake,
                place: "_1".to_string()
            }
        );
        assert_eq!(
            parse_rvalue("Add(copy _1, const 1_i32)"),
            RvalueText::Other("Add(copy _1, const 1_i32)".to_string())
        );
    }

    #[test]
    fn malformed_input_reports_line() {
        let err = MirFn::parse("fn f() -> () {\n    bb0: {\n").expect_err("unterminated");
        assert!(err.message.contains("unterminated"));
        let err = MirFn::parse("fn f() {\n    let _x: i32;\n}\n").expect_err("bad local");
        assert_eq!(err.line, 2);
        let err = MirFn::parse("fn f() {\n    bb0: {\n    }\n}\n").expect_err("empty block");
        assert!(err.message.contains("no terminator"), "{err}");
        assert!(MirFn::parse("no functions here").is_err());
        let err = MirFn::parse("fn f() {\n    bb1: {\n        return;\n    }\n}\n")
            .expect_err("out of order");
        assert!(err.to_string().starts_with("line 4:"), "{err}");
    }

    #[test]
    fn location_and_ids_round_trip() {
        let loc: Location = "bb12[3]".parse().expect("parses");
        assert_eq!(loc, Location::new(12, 3));
        assert_eq!(loc.to_string(), "bb12[3]");
        assert_eq!(loc.successor_within_block(), Location::new(12, 4));
        assert!("bb1".parse::<Location>().is_err());
        assert!("bbx[1]".parse::<Location>().is_err());
        assert!("x".parse::<Local>().is_err());
        assert!(Location::new(0, 9) < Location::new(1, 0));
    }

    #[test]
    fn mentions_local_respects_token_boundaries() {
        assert!(mentions_local("copy (*_1)", Local(1)));
        assert!(!mentions_local("copy _10", Local(1)));
        assert!(!mentions_local("x_1", Local(1)));
        assert!(mentions_local("_1", Local(1)));
        assert!(mentions_local("f(_10, _1)", Local(1)));
    }
}
