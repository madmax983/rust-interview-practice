//! # Valgrind: Callgrind, Cachegrind and DHAT
//!
//! The other modules in this category *model* performance (virtual clocks, a
//! counting allocator, a cache simulator). Valgrind *measures* it, by running
//! the real binary on a synthetic CPU. Its tools give exact, repeatable counts,
//! which makes them ideal for CI regression gates:
//!
//! | Tool | Counts | Use it for | This module |
//! |------|--------|------------|-------------|
//! | `callgrind` | Instructions (`Ir`) per function, plus the call graph | "Where does the time go?" and instruction-count gates | [`CostProfile`] |
//! | `cachegrind` | `Ir` and simulated I1/D1/LL cache refs and misses | Checking a locality fix on a real cache model | [`CostProfile`] |
//! | `dhat` | Heap blocks/bytes per allocation site, lifetimes, peak | "Who allocates, and how much?" | [`DhatProfile`] |
//!
//! ## Running them
//!
//! ```bash
//! # Symbols and line numbers, without giving up optimizations:
//! export CARGO_PROFILE_RELEASE_DEBUG=true
//! cargo build --release --bin perf_drills
//! BIN=target/release/perf_drills
//!
//! valgrind --tool=callgrind --callgrind-out-file=cg.out $BIN run transpose_naive
//! callgrind_annotate cg.out | head -30        # or: kcachegrind cg.out
//! $BIN callgrind cg.out                       # same report via CostProfile
//!
//! valgrind --tool=cachegrind --cache-sim=yes --cachegrind-out-file=cache.out $BIN run column_major
//! $BIN cachegrind cache.out
//!
//! valgrind --tool=dhat --dhat-out-file=dhat.out $BIN run join_naive
//! $BIN dhat dhat.out                          # or load it in dh_view.html
//!
//! $BIN valgrind-check                         # runs all three on paired workloads and gates them
//! ```
//!
//! Notes that bite:
//!
//! - Valgrind slows a program down ~5-100x (callgrind is the slowest), so profile
//!   a small, representative workload, not the full test suite.
//! - Since Valgrind 3.21, cachegrind's cache simulation is **off by default**;
//!   `--cache-sim=yes` is required to get `D1mr`/`DLmr` columns at all.
//! - Cachegrind simulates a cache shaped like the host's, with its own
//!   simplifications (no prefetcher). Its miss counts are a *model*, which is
//!   exactly why they are stable enough to gate on.
//! - Cachegrind copies the host's cache geometry, so miss counts differ between
//!   machines. Pin it with [`PINNED_CACHE_GEOMETRY`] before gating on them.
//! - `callgrind --toggle-collect='*workload*'` restricts collection to one
//!   function and its callees, which removes start-up noise from the totals.
//! - DHAT counts every `realloc` as a new block; so does [`DhatProfile`].
//! - **Pick the event that shows the claim.** A naive transpose reads
//!   sequentially and scatters its *writes*: `D1mr` barely moves, `D1mw`
//!   explodes. And presizing a `String` cuts heap blocks (DHAT) while executing
//!   slightly *more* instructions (callgrind) than amortized growth. Both
//!   surprises came out of `perf_drills valgrind-check`; that is why you measure.
//!
//! ## DHAT without Valgrind: `dhat-rs`
//!
//! The [`dhat`](https://docs.rs/dhat) crate reimplements DHAT's heap profiling
//! in-process, as a `#[global_allocator]`. Its *testing* mode turns heap
//! counts into assertions (see `tests/dhat_heap.rs`, run with
//! `cargo test --features dhat-heap --test dhat_heap`). It writes the same JSON
//! format as `valgrind --tool=dhat`, so [`DhatProfile`] reads both.
//!
//! ```
//! use rust_interview_practice::performance::profilers::CostProfile;
//!
//! let text = "events: Ir\nfn=(1) main\n3 10\ncfn=(2) work\ncalls=1 7\n7 90\nfn=(2)\n7 90\nsummary: 100\n";
//! let p = CostProfile::parse(text).unwrap();
//! assert_eq!(p.self_cost("main", "Ir"), Some(10));
//! assert_eq!(p.inclusive_cost("main", "Ir"), Some(100));
//! assert_eq!(p.total("Ir"), Some(100));
//! assert_eq!(p.summary("Ir"), Some(100));
//! ```

use std::collections::{BTreeMap, HashMap};
use std::fmt;

use crate::serialization::json::{self, JsonValue};

// ============================================================================
// Building Valgrind command lines
// ============================================================================

/// A Valgrind tool this module can read the output of.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValgrindTool {
    /// Instruction counts and call graph.
    Callgrind,
    /// Instruction counts and simulated cache behaviour.
    Cachegrind,
    /// Heap profiling.
    Dhat,
}

impl ValgrindTool {
    /// The value for `--tool=`.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Callgrind => "callgrind",
            Self::Cachegrind => "cachegrind",
            Self::Dhat => "dhat",
        }
    }

    /// Tool-specific flags, writing results to `out_file`.
    #[must_use]
    pub fn flags(self, out_file: &str) -> Vec<String> {
        let mut flags = vec![format!("--tool={}", self.name())];
        match self {
            Self::Callgrind => flags.push(format!("--callgrind-out-file={out_file}")),
            Self::Cachegrind => {
                flags.push("--cache-sim=yes".to_owned());
                flags.push(format!("--cachegrind-out-file={out_file}"));
            }
            Self::Dhat => flags.push(format!("--dhat-out-file={out_file}")),
        }
        flags
    }
}

/// Cachegrind flags that pin the simulated cache to a fixed geometry (32 KiB
/// 8-way I1 and D1, 8 MiB 16-way LL, 64-byte lines).
///
/// By default cachegrind copies the *host's* cache sizes, so the same binary
/// reports different miss counts on a laptop and a CI runner. Pinning makes the
/// counts a property of the code alone, which is what a regression gate needs.
pub const PINNED_CACHE_GEOMETRY: [&str; 3] =
    ["--I1=32768,8,64", "--D1=32768,8,64", "--LL=8388608,16,64"];

/// The full argument vector (`valgrind` excluded) to profile `program` with
/// `tool`.
///
/// Order: tool flags, then `extra_flags` (e.g. [`PINNED_CACHE_GEOMETRY`]),
/// then the program and its arguments. Valgrind treats the first non-flag
/// argument as the program, so every flag must come before it.
#[must_use]
pub fn valgrind_args(
    tool: ValgrindTool,
    out_file: &str,
    extra_flags: &[&str],
    program: &str,
    program_args: &[&str],
) -> Vec<String> {
    let mut args = tool.flags(out_file);
    args.extend(extra_flags.iter().map(|&f| f.to_owned()));
    args.push(program.to_owned());
    args.extend(program_args.iter().map(|&a| a.to_owned()));
    args
}

// ============================================================================
// Errors
// ============================================================================

/// Why a profile could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProfileError {
    /// A line of a callgrind/cachegrind file was malformed.
    Syntax {
        /// 1-based line number.
        line: usize,
        /// What was wrong.
        reason: String,
    },
    /// The file never declared its `events:`.
    MissingEvents,
    /// A DHAT file was not valid JSON.
    Json(String),
    /// A DHAT file lacked a required field or had the wrong type.
    Field(&'static str),
}

impl fmt::Display for ProfileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Syntax { line, reason } => write!(f, "line {line}: {reason}"),
            Self::MissingEvents => f.write_str("no `events:` line"),
            Self::Json(e) => write!(f, "invalid DHAT JSON: {e}"),
            Self::Field(name) => write!(f, "DHAT field {name:?} missing or mistyped"),
        }
    }
}

impl std::error::Error for ProfileError {}

// ============================================================================
// Callgrind / Cachegrind cost files
// ============================================================================

/// Costs attributed to one function name (summed over every file it appears
/// in, as `callgrind_annotate` does by default).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FunctionCost {
    /// Cost of the function's own instructions, one entry per event.
    pub self_cost: Vec<u64>,
    /// Inclusive cost of the calls it makes (callgrind only), per event.
    pub callee_cost: Vec<u64>,
    /// Number of calls it makes (callgrind only).
    pub calls_made: u64,
}

/// A parsed callgrind or cachegrind output file.
///
/// Supports the parts of the [callgrind format] both tools emit: `events:`,
/// `positions:`, `summary:`/`totals:`, `fn=`/`cfn=` with name compression
/// (`(id) name` defines, `(id)` refers back), `calls=` followed by an inclusive
/// call-cost line, and relative / repeated positions (`+n`, `-n`, `*`).
///
/// [callgrind format]: https://valgrind.org/docs/manual/cl-format.html
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CostProfile {
    events: Vec<String>,
    functions: BTreeMap<String, FunctionCost>,
    summary: Option<Vec<u64>>,
}

fn add_into(acc: &mut Vec<u64>, costs: &[u64]) {
    if acc.len() < costs.len() {
        acc.resize(costs.len(), 0);
    }
    for (a, c) in acc.iter_mut().zip(costs) {
        *a += c;
    }
}

/// A position token: a decimal or `0x` hex number, optionally `+`/`-`
/// relative to the previous line, or `*` for "same as before".
fn is_position(token: &str) -> bool {
    if token == "*" {
        return true;
    }
    let body = token.strip_prefix(['+', '-']).unwrap_or(token);
    body.strip_prefix("0x").map_or_else(
        || !body.is_empty() && body.bytes().all(|b| b.is_ascii_digit()),
        |hex| !hex.is_empty() && hex.bytes().all(|b| b.is_ascii_hexdigit()),
    )
}

impl CostProfile {
    /// Parses callgrind or cachegrind output.
    ///
    /// # Errors
    ///
    /// Returns [`ProfileError::MissingEvents`] if no `events:` line precedes the
    /// first cost line (or none exists), and [`ProfileError::Syntax`] for a cost
    /// line outside any function, an undefined compressed name, or a malformed
    /// number.
    pub fn parse(text: &str) -> Result<Self, ProfileError> {
        let mut profile = Self::default();
        let mut positions = 1_usize;
        let mut names: HashMap<String, String> = HashMap::new();
        let mut current: Option<String> = None;
        let mut pending_call: Option<u64> = None;

        for (idx, raw) in text.lines().enumerate() {
            let line_no = idx + 1;
            let line = raw.trim();
            let syntax = |reason: String| ProfileError::Syntax {
                line: line_no,
                reason,
            };
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let first = line.split_whitespace().next().unwrap_or_default();
            if is_position(first) {
                let tokens: Vec<&str> = line.split_whitespace().collect();
                if profile.events.is_empty() {
                    return Err(ProfileError::MissingEvents);
                }
                let costs = tokens
                    .get(positions..)
                    .unwrap_or_default()
                    .iter()
                    .map(|t| {
                        t.parse::<u64>()
                            .map_err(|_| syntax(format!("cost {t:?} is not a number")))
                    })
                    .collect::<Result<Vec<u64>, _>>()?;
                if costs.len() > profile.events.len() {
                    return Err(syntax(format!(
                        "{} costs for {} events",
                        costs.len(),
                        profile.events.len()
                    )));
                }
                let Some(func) = &current else {
                    return Err(syntax("cost line before any fn=".to_owned()));
                };
                let entry = profile.functions.entry(func.clone()).or_default();
                if let Some(count) = pending_call.take() {
                    entry.calls_made += count;
                    add_into(&mut entry.callee_cost, &costs);
                } else {
                    add_into(&mut entry.self_cost, &costs);
                }
                continue;
            }
            if let Some((key, value)) = line.split_once(':')
                && !key.contains('=')
            {
                let value = value.trim();
                match key {
                    "events" => {
                        profile.events = value.split_whitespace().map(str::to_owned).collect();
                    }
                    "positions" => positions = value.split_whitespace().count().max(1),
                    "summary" | "totals" => {
                        let totals = value
                            .split_whitespace()
                            .map(|t| {
                                t.parse::<u64>()
                                    .map_err(|_| syntax(format!("total {t:?} is not a number")))
                            })
                            .collect::<Result<Vec<u64>, _>>()?;
                        profile.summary = Some(totals);
                    }
                    _ => {} // version, creator, cmd, desc, pid, part, thread, ...
                }
                continue;
            }
            if let Some((key, value)) = line.split_once('=') {
                match key {
                    "fn" | "cfn" => {
                        let name = resolve_name(&mut names, value).map_err(syntax)?;
                        if key == "fn" {
                            current = Some(name);
                        }
                    }
                    "calls" => {
                        let count = value.split_whitespace().next().unwrap_or_default();
                        pending_call = Some(
                            count
                                .parse()
                                .map_err(|_| syntax(format!("call count {count:?}")))?,
                        );
                    }
                    _ => {} // ob, cob, fl, fi, fe, cfi, cfl, jump, jcnd, ...
                }
                continue;
            }
            return Err(syntax(format!("unrecognized line {line:?}")));
        }
        if profile.events.is_empty() {
            return Err(ProfileError::MissingEvents);
        }
        Ok(profile)
    }

    /// The event names, in column order (e.g. `["Ir"]` or `["Ir", "I1mr", ...]`).
    #[must_use]
    pub fn events(&self) -> &[String] {
        &self.events
    }

    fn event_index(&self, event: &str) -> Option<usize> {
        self.events.iter().position(|e| e == event)
    }

    /// Every function name seen with a cost or as a caller.
    pub fn functions(&self) -> impl Iterator<Item = (&str, &FunctionCost)> {
        self.functions.iter().map(|(k, v)| (k.as_str(), v))
    }

    /// Sum of every function's self cost for `event` (`None` for an unknown
    /// event). Matches `summary:` for a complete profile.
    #[must_use]
    pub fn total(&self, event: &str) -> Option<u64> {
        let i = self.event_index(event)?;
        Some(
            self.functions
                .values()
                .map(|f| f.self_cost.get(i).copied().unwrap_or(0))
                .sum(),
        )
    }

    /// The total the tool itself reported on its `summary:`/`totals:` line.
    #[must_use]
    pub fn summary(&self, event: &str) -> Option<u64> {
        let i = self.event_index(event)?;
        self.summary.as_ref()?.get(i).copied()
    }

    /// `function`'s own cost for `event`.
    #[must_use]
    pub fn self_cost(&self, function: &str, event: &str) -> Option<u64> {
        let i = self.event_index(event)?;
        let f = self.functions.get(function)?;
        Some(f.self_cost.get(i).copied().unwrap_or(0))
    }

    /// `function`'s inclusive cost: its own plus that of everything it calls.
    /// (Recursive cycles are counted once per call edge, like callgrind's
    /// raw data; `callgrind_annotate --inclusive=yes` does the same.)
    #[must_use]
    pub fn inclusive_cost(&self, function: &str, event: &str) -> Option<u64> {
        let i = self.event_index(event)?;
        let f = self.functions.get(function)?;
        Some(f.self_cost.get(i).copied().unwrap_or(0) + f.callee_cost.get(i).copied().unwrap_or(0))
    }

    /// The `k` functions with the highest self cost for `event`, highest first
    /// (ties broken by name).
    #[must_use]
    pub fn top_self(&self, event: &str, k: usize) -> Vec<(&str, u64)> {
        let Some(i) = self.event_index(event) else {
            return Vec::new();
        };
        let mut rows: Vec<(&str, u64)> = self
            .functions
            .iter()
            .map(|(name, f)| (name.as_str(), f.self_cost.get(i).copied().unwrap_or(0)))
            .filter(|&(_, cost)| cost > 0)
            .collect();
        rows.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
        rows.truncate(k);
        rows
    }

    /// Sum of self cost for `event` over functions whose name contains `needle`.
    #[must_use]
    pub fn self_cost_matching(&self, needle: &str, event: &str) -> Option<u64> {
        let i = self.event_index(event)?;
        Some(
            self.functions
                .iter()
                .filter(|(name, _)| name.contains(needle))
                .map(|(_, f)| f.self_cost.get(i).copied().unwrap_or(0))
                .sum(),
        )
    }
}

/// Resolves callgrind name compression: `(id) name` defines and returns
/// `name`; `(id)` looks it up; anything else is a literal name.
///
/// Compression ids are always numeric. That matters: cachegrind never
/// compresses, and emits literal names such as `(below main)`.
fn resolve_name(names: &mut HashMap<String, String>, value: &str) -> Result<String, String> {
    let value = value.trim();
    let Some(rest) = value.strip_prefix('(') else {
        return Ok(value.to_owned());
    };
    let id_len = rest.bytes().take_while(u8::is_ascii_digit).count();
    if id_len == 0 {
        return Ok(value.to_owned()); // a literal name that happens to start with '('
    }
    let (id, after) = rest.split_at(id_len);
    let Some(name) = after.strip_prefix(')') else {
        return Err(format!("unterminated compressed name {value:?}"));
    };
    let name = name.trim();
    if name.is_empty() {
        names
            .get(id)
            .cloned()
            .ok_or_else(|| format!("compressed name ({id}) used before definition"))
    } else {
        names.insert(id.to_owned(), name.to_owned());
        Ok(name.to_owned())
    }
}

// ============================================================================
// DHAT
// ============================================================================

/// One allocation site (a "program point") in a DHAT profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AllocSite {
    /// Bytes allocated here over the whole run.
    pub total_bytes: u64,
    /// Blocks allocated here over the whole run.
    pub total_blocks: u64,
    /// Most bytes live at once from this site.
    pub max_bytes: u64,
    /// Bytes live from this site at the global peak (`t-gmax`).
    pub bytes_at_peak: u64,
    /// The allocation stack, innermost frame first.
    pub frames: Vec<String>,
}

/// A parsed DHAT heap profile (`valgrind --tool=dhat` or `dhat-rs`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DhatProfile {
    /// `"heap"` from Valgrind, `"rust-heap"` from `dhat-rs`.
    pub mode: String,
    /// Every allocation site.
    pub sites: Vec<AllocSite>,
}

fn field<'a>(
    obj: &'a HashMap<String, JsonValue>,
    name: &'static str,
) -> Result<&'a JsonValue, ProfileError> {
    obj.get(name).ok_or(ProfileError::Field(name))
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // checked non-negative integral
fn as_u64(value: &JsonValue, name: &'static str) -> Result<u64, ProfileError> {
    match value {
        JsonValue::Number(n) if *n >= 0.0 && n.fract() == 0.0 && *n <= 9_007_199_254_740_992.0 => {
            Ok(*n as u64)
        }
        _ => Err(ProfileError::Field(name)),
    }
}

impl DhatProfile {
    /// Parses DHAT's JSON output.
    ///
    /// # Errors
    ///
    /// Returns [`ProfileError::Json`] for invalid JSON and
    /// [`ProfileError::Field`] when `mode`, `pps`, `ftbl` or a per-site count is
    /// missing, mistyped, or a frame index is out of range.
    pub fn parse(text: &str) -> Result<Self, ProfileError> {
        let root = json::parse(text).map_err(ProfileError::Json)?;
        let JsonValue::Object(root) = root else {
            return Err(ProfileError::Field("<root>"));
        };
        let JsonValue::String(mode) = field(&root, "mode")? else {
            return Err(ProfileError::Field("mode"));
        };
        let JsonValue::Array(frame_table) = field(&root, "ftbl")? else {
            return Err(ProfileError::Field("ftbl"));
        };
        let JsonValue::Array(pps) = field(&root, "pps")? else {
            return Err(ProfileError::Field("pps"));
        };
        let mut sites = Vec::with_capacity(pps.len());
        for pp in pps {
            let JsonValue::Object(pp) = pp else {
                return Err(ProfileError::Field("pps"));
            };
            let JsonValue::Array(fs) = field(pp, "fs")? else {
                return Err(ProfileError::Field("fs"));
            };
            let frames = fs
                .iter()
                .map(|i| {
                    let idx =
                        usize::try_from(as_u64(i, "fs")?).map_err(|_| ProfileError::Field("fs"))?;
                    match frame_table.get(idx) {
                        Some(JsonValue::String(s)) => Ok(s.clone()),
                        _ => Err(ProfileError::Field("fs")),
                    }
                })
                .collect::<Result<Vec<String>, _>>()?;
            sites.push(AllocSite {
                total_bytes: as_u64(field(pp, "tb")?, "tb")?,
                total_blocks: as_u64(field(pp, "tbk")?, "tbk")?,
                max_bytes: as_u64(field(pp, "mb")?, "mb")?,
                bytes_at_peak: as_u64(field(pp, "gb")?, "gb")?,
                frames,
            });
        }
        Ok(Self {
            mode: mode.clone(),
            sites,
        })
    }

    /// Bytes allocated over the whole run.
    #[must_use]
    pub fn total_bytes(&self) -> u64 {
        self.sites.iter().map(|s| s.total_bytes).sum()
    }

    /// Blocks allocated over the whole run.
    #[must_use]
    pub fn total_blocks(&self) -> u64 {
        self.sites.iter().map(|s| s.total_blocks).sum()
    }

    /// Bytes live at the global peak (`t-gmax`).
    #[must_use]
    pub fn peak_bytes(&self) -> u64 {
        self.sites.iter().map(|s| s.bytes_at_peak).sum()
    }

    /// Sites whose stack has a frame containing `needle` (e.g. a function name).
    pub fn sites_matching<'a>(&'a self, needle: &'a str) -> impl Iterator<Item = &'a AllocSite> {
        self.sites
            .iter()
            .filter(move |s| s.frames.iter().any(|f| f.contains(needle)))
    }

    /// The `k` sites allocating the most blocks, most first.
    #[must_use]
    pub fn top_sites_by_blocks(&self, k: usize) -> Vec<&AllocSite> {
        let mut sites: Vec<&AllocSite> = self.sites.iter().collect();
        sites.sort_by_key(|s| std::cmp::Reverse(s.total_blocks));
        sites.truncate(k);
        sites
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Shaped like real `callgrind-3.22` output (trimmed): name compression
    /// defined on `cfn=` and reused by a bare `fn=(id)`, `*`/`+n`/`-n`
    /// positions, a call-cost line after `calls=`, and a trailing `totals:`.
    const CALLGRIND: &str = "\
# callgrind format
version: 1
creator: callgrind-3.22.0
pid: 1185
cmd:  ./t
part: 1

desc: Timerange: Basic block 0 - 191604
desc: Trigger: Program termination

positions: line
events: Ir
summary: 1000

ob=(6) /tmp/t
fl=(251) /src/t.rs
fn=(850) t::main
4 2
cfi=(254)
cfn=(846) t::row
calls=1 900
* 300
+1 5
cfn=(854) t::col
calls=1 900
* 500
-2 3

fl=(254) /src/hint.rs
fn=(846)
900 1
0 299
fn=(1112) leaf
0 100
fn=(854)
900 450
fi=(255) /src/other.rs
491 50
fn=(1112)
0 90

totals: 1000
";

    /// Shaped like real `cachegrind-3.22 --cache-sim=yes` output: one function
    /// listed under two files, and a trailing space on `events:`.
    const CACHEGRIND: &str = "\
desc: I1 cache:         32768 B, 64 B, 8-way associative
desc: D1 cache:         49152 B, 64 B, 12-way associative
desc: LL cache:         276824064 B, 64 B, 33-way associative
cmd: ./t
events: Ir I1mr ILmr Dr D1mr DLmr Dw D1mw DLmw
fl=/src/t.rs
fn=t::col
10 1000 1 1 400 300 10 0 0 0
fn=t::row
10 1000 1 1 400 50 10 0 0 0
fl=/src/hint.rs
fn=t::col
900 24 0 0 8 8 0
summary: 2024 2 2 808 358 20 0 0 0
";

    #[test]
    fn test_callgrind_self_and_inclusive_costs() {
        let p = CostProfile::parse(CALLGRIND).unwrap();
        assert_eq!(p.events(), ["Ir"]);
        assert_eq!(p.self_cost("t::main", "Ir"), Some(2 + 5 + 3));
        assert_eq!(p.inclusive_cost("t::main", "Ir"), Some(10 + 300 + 500));
        assert_eq!(p.self_cost("t::row", "Ir"), Some(300));
        assert_eq!(p.self_cost("t::col", "Ir"), Some(500));
        assert_eq!(p.self_cost("leaf", "Ir"), Some(190));
        let main = p.functions().find(|(n, _)| *n == "t::main").unwrap().1;
        assert_eq!(main.calls_made, 2);
    }

    #[test]
    fn test_callgrind_totals_match_summary() {
        let p = CostProfile::parse(CALLGRIND).unwrap();
        assert_eq!(p.total("Ir"), Some(1_000));
        assert_eq!(p.summary("Ir"), Some(1_000));
        assert_eq!(p.total("D1mr"), None);
    }

    #[test]
    fn test_callgrind_top_and_matching() {
        let p = CostProfile::parse(CALLGRIND).unwrap();
        assert_eq!(
            p.top_self("Ir", 3),
            [("t::col", 500), ("t::row", 300), ("leaf", 190)]
        );
        assert!(p.top_self("nope", 3).is_empty());
        assert_eq!(p.self_cost_matching("t::", "Ir"), Some(810));
    }

    #[test]
    fn test_cachegrind_aggregates_and_pads() {
        let p = CostProfile::parse(CACHEGRIND).unwrap();
        assert_eq!(p.events().len(), 9);
        // t::col appears under two files; short lines are zero-padded.
        assert_eq!(p.self_cost("t::col", "Ir"), Some(1_024));
        assert_eq!(p.self_cost("t::col", "D1mr"), Some(308));
        assert_eq!(p.self_cost("t::col", "DLmw"), Some(0));
        assert_eq!(p.self_cost("t::row", "D1mr"), Some(50));
        for event in ["Ir", "Dr", "D1mr", "DLmr"] {
            assert_eq!(p.total(event), p.summary(event), "{event}");
        }
    }

    #[test]
    fn test_cost_profile_errors() {
        assert_eq!(CostProfile::parse(""), Err(ProfileError::MissingEvents));
        assert_eq!(
            CostProfile::parse("fn=x\n1 2\n"),
            Err(ProfileError::MissingEvents)
        );
        let cases = [
            ("events: Ir\n1 2\n", 2, "before any fn"),
            ("events: Ir\nfn=(3)\n", 2, "before definition"),
            ("events: Ir\nfn=a\n1 2 3\n", 3, "2 costs for 1 events"),
            ("events: Ir\nfn=a\n1 x\n", 3, "\"x\""),
            ("events: Ir\nfn=(4 a\n", 2, "unterminated"),
            ("events: Ir\nfn=(4x) a\n", 2, "unterminated"),
            ("events: Ir\nwhat is this\n", 2, "unrecognized"),
            ("events: Ir\nfn=a\ncalls=many 3\n", 3, "call count"),
            ("events: Ir\nsummary: lots\n", 2, "total"),
        ];
        for (text, line, needle) in cases {
            match CostProfile::parse(text) {
                Err(ProfileError::Syntax { line: l, reason }) => {
                    assert_eq!(l, line, "{text:?}");
                    assert!(reason.contains(needle), "{text:?}: {reason}");
                }
                other => panic!("{text:?} -> {other:?}"),
            }
        }
    }

    #[test]
    fn test_literal_parenthesized_names_are_not_compression() {
        // Regression: real cachegrind output contains `fn=(below main)`.
        let text = "events: Ir\nfn=(below main)\n0 7\nfn=(1) (below main)\n0 3\nfn=(1)\n0 1\n";
        let p = CostProfile::parse(text).unwrap();
        assert_eq!(p.self_cost("(below main)", "Ir"), Some(11));
    }

    #[test]
    fn test_positions_header_with_instr_and_line() {
        let text = "positions: instr line\nevents: Ir\nfn=f\n0x401000 12 7\n+4 +1 3\n* * 2\n";
        let p = CostProfile::parse(text).unwrap();
        assert_eq!(p.self_cost("f", "Ir"), Some(12));
    }

    /// Shaped like real `valgrind --tool=dhat` output (trimmed).
    const DHAT: &str = r#"{"dhatFileVersion":2
,"mode":"heap","verb":"Allocated"
,"bklt":true,"bkacc":true
,"tu":"instrs","Mtu":"Minstr"
,"tuth":500
,"cmd":"./t"
,"pid":1189
,"te":1707523
,"tg":1699457
,"pps":
 [{"tb":472,"tbk":1,"tl":159972,"mb":472,"mbk":1,"gb":0,"gbk":0,"eb":0,"ebk":0
  ,"rb":2871,"wb":1342,"acc":[194,-3,143],"fs":[1,2,3]}
 ,{"tb":1200,"tbk":7,"tl":10,"mb":600,"mbk":1,"gb":600,"gbk":1,"eb":0,"ebk":0
  ,"rb":0,"wb":1200,"fs":[1,4,5,3]}
 ,{"tb":4096,"tbk":1,"tl":10,"mb":4096,"mbk":1,"gb":4096,"gbk":1,"eb":0,"ebk":0
  ,"rb":0,"wb":4096,"fs":[1,6,3]}
 ]
,"ftbl":
 ["[root]"
 ,"0x4846828: malloc (in /usr/libexec/valgrind/vgpreload_dhat-amd64-linux.so)"
 ,"0x4918E7E: __fopen_internal (iofopen.c:65)"
 ,"0x10A000: t::main (t.rs:4)"
 ,"0x10B000: <alloc::string::String>::push_str (string.rs:1100)"
 ,"0x10C000: t::join (t.rs:3)"
 ,"0x10D000: t::buffer (t.rs:9)"
 ]
}"#;

    #[test]
    fn test_dhat_totals_and_sites() {
        let p = DhatProfile::parse(DHAT).unwrap();
        assert_eq!(p.mode, "heap");
        assert_eq!(p.sites.len(), 3);
        assert_eq!(p.total_bytes(), 472 + 1_200 + 4_096);
        assert_eq!(p.total_blocks(), 9);
        assert_eq!(p.peak_bytes(), 4_696);
        let join: Vec<&AllocSite> = p.sites_matching("t::join").collect();
        assert_eq!(join.len(), 1);
        assert_eq!(join[0].total_blocks, 7);
        assert!(join[0].frames[0].contains("malloc")); // innermost frame first
        assert_eq!(p.top_sites_by_blocks(1)[0].total_bytes, 1_200);
        assert_eq!(p.sites_matching("t::main").count(), 3);
    }

    #[test]
    fn test_dhat_errors() {
        assert!(matches!(
            DhatProfile::parse("{"),
            Err(ProfileError::Json(_))
        ));
        assert_eq!(DhatProfile::parse("[]"), Err(ProfileError::Field("<root>")));
        assert_eq!(
            DhatProfile::parse(r#"{"pps":[],"ftbl":[]}"#),
            Err(ProfileError::Field("mode"))
        );
        let bad_index =
            r#"{"mode":"heap","ftbl":["[root]"],"pps":[{"tb":1,"tbk":1,"mb":1,"gb":0,"fs":[9]}]}"#;
        assert_eq!(
            DhatProfile::parse(bad_index),
            Err(ProfileError::Field("fs"))
        );
        let negative =
            r#"{"mode":"heap","ftbl":["[root]"],"pps":[{"tb":-1,"tbk":1,"mb":1,"gb":0,"fs":[0]}]}"#;
        assert_eq!(DhatProfile::parse(negative), Err(ProfileError::Field("tb")));
        let empty = DhatProfile::parse(r#"{"mode":"rust-heap","ftbl":[],"pps":[]}"#).unwrap();
        assert_eq!(empty.total_blocks(), 0);
        assert_eq!(
            ProfileError::Field("tb").to_string(),
            "DHAT field \"tb\" missing or mistyped"
        );
    }

    #[test]
    fn test_valgrind_args() {
        assert_eq!(
            valgrind_args(
                ValgrindTool::Cachegrind,
                "c.out",
                &["--D1=32768,8,64"],
                "./bin",
                &["run", "x"]
            ),
            [
                "--tool=cachegrind",
                "--cache-sim=yes",
                "--cachegrind-out-file=c.out",
                "--D1=32768,8,64",
                "./bin",
                "run",
                "x"
            ]
        );
        assert_eq!(
            ValgrindTool::Callgrind.flags("cg.out"),
            ["--tool=callgrind", "--callgrind-out-file=cg.out"]
        );
        // Valgrind requires a power-of-two set count: size / (ways * line).
        for flag in PINNED_CACHE_GEOMETRY {
            let spec = flag.split_once('=').unwrap().1;
            let n: Vec<usize> = spec.split(',').map(|x| x.parse().unwrap()).collect();
            assert!((n[0] / (n[1] * n[2])).is_power_of_two(), "{flag}");
        }
        assert_eq!(
            ValgrindTool::Dhat.flags("d.out"),
            ["--tool=dhat", "--dhat-out-file=d.out"]
        );
    }
}
