//! # Crash Injection
//!
//! "Does my storage code survive power loss?" cannot be answered by pulling
//! the plug a few times. Instead, run the code against a **simulated disk**
//! that knows which bytes are durable, then crash it at *every* I/O operation
//! and check the recovery invariant after each crash.
//!
//! ## The pieces
//!
//! - [`SimDisk`] — an in-memory filesystem. Each file and the directory keep a
//!   queue of operations not yet made durable by `fsync`. On [`SimDisk::crash`],
//!   a seeded-random *prefix* of each queue survives, and the last surviving
//!   append may be **torn** (only some of its bytes land). Data and directory
//!   entries persist independently — exactly the reordering that bites real
//!   programs that `rename` without `fsync`.
//! - [`Fault`] — deterministic fault plans: crash at operation N, or return an
//!   I/O error at operation N.
//! - [`FailPoints`] + [`fail_point!`](crate::fail_point) — named failpoints
//!   (the `fail` crate pattern) for injecting errors at specific code locations.
//! - [`explore_crash_points`] — runs a workload once per (crash point, seed)
//!   pair and reports the first state that violates the invariant.
//!
//! ## Subjects under test
//!
//! - [`replace_file`] with three [`ReplaceStrategy`]s. Only
//!   [`ReplaceStrategy::TempFsyncRename`] is crash-safe; the harness finds a
//!   counterexample for the other two.
//! - [`LogStore`] — an append-only log of CRC-framed records whose recovery
//!   truncates a torn tail. Every committed record must survive.
//!
//! ```
//! use rust_interview_practice::testing_craft::crash_injection::{replace_file, Fault, ReplaceStrategy, SimDisk};
//!
//! let mut disk = SimDisk::new(1);
//! replace_file(&mut disk, "cfg", b"v1", ReplaceStrategy::TempFsyncRename).unwrap();
//! disk.set_fault(Fault::CrashAt(2)); // power loss on the 3rd I/O op
//! assert!(replace_file(&mut disk, "cfg", b"v2", ReplaceStrategy::TempFsyncRename).is_err());
//! disk.restart();
//! assert_eq!(disk.read("cfg").unwrap(), b"v1"); // old contents, intact
//! ```

use std::collections::{BTreeMap, HashMap};
use std::fmt;

use super::sim_rng::SimRng;

/// Errors from [`SimDisk`] and the code under test.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IoError {
    /// The simulated machine lost power; every op fails until [`SimDisk::restart`].
    Crashed,
    /// An injected I/O error (operation index or failpoint name).
    Injected(String),
    /// No such file.
    NotFound(String),
    /// On-disk data failed validation.
    Corrupt(String),
}

impl fmt::Display for IoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Crashed => write!(f, "simulated crash"),
            Self::Injected(what) => write!(f, "injected I/O error at {what}"),
            Self::NotFound(name) => write!(f, "file not found: {name}"),
            Self::Corrupt(why) => write!(f, "corrupt data: {why}"),
        }
    }
}

impl std::error::Error for IoError {}

/// A deterministic fault plan, keyed on the global operation counter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fault {
    /// No faults.
    None,
    /// Lose power just before operation `n` (0-based) executes.
    CrashAt(usize),
    /// Operation `n` fails with [`IoError::Injected`] and has no effect.
    ErrorAt(usize),
}

type FileId = u64;

#[derive(Debug, Clone)]
enum DataOp {
    Append(Vec<u8>),
    Truncate,
}

#[derive(Debug, Clone)]
enum DirOp {
    Link(String, FileId),
    Unlink(String),
}

#[derive(Debug, Clone, Default)]
struct SimFile {
    durable: Vec<u8>,
    pending: Vec<DataOp>,
}

impl SimFile {
    fn current(&self) -> Vec<u8> {
        let mut data = self.durable.clone();
        for op in &self.pending {
            apply_data(&mut data, op);
        }
        data
    }
}

fn apply_data(data: &mut Vec<u8>, op: &DataOp) {
    match op {
        DataOp::Append(bytes) => data.extend_from_slice(bytes),
        DataOp::Truncate => data.clear(),
    }
}

fn apply_dir(names: &mut BTreeMap<String, FileId>, op: &DirOp) {
    match op {
        DirOp::Link(name, id) => {
            names.insert(name.clone(), *id);
        }
        DirOp::Unlink(name) => {
            names.remove(name);
        }
    }
}

/// An in-memory disk that models volatile vs. durable state.
///
/// Semantics (a simplified POSIX + ext4 `data=writeback` model):
/// - `append`/`truncate` change a file's data; durable only after `fsync(file)`.
/// - `create`/`rename`/`remove` change the directory; durable only after `fsync_dir`.
/// - On crash, for each file and for the directory, a random prefix of the
///   pending ops survives; a surviving append may be torn.
#[derive(Debug)]
pub struct SimDisk {
    files: HashMap<FileId, SimFile>,
    durable_names: BTreeMap<String, FileId>,
    pending_dir: Vec<DirOp>,
    next_id: FileId,
    ops: usize,
    fault: Fault,
    crashed: bool,
    rng: SimRng,
}

impl SimDisk {
    /// An empty disk. `seed` drives which unsynced writes survive a crash.
    #[must_use]
    pub fn new(seed: u64) -> Self {
        Self {
            files: HashMap::new(),
            durable_names: BTreeMap::new(),
            pending_dir: Vec::new(),
            next_id: 0,
            ops: 0,
            fault: Fault::None,
            crashed: false,
            rng: SimRng::new(seed),
        }
    }

    /// Installs a fault plan. Operation numbering restarts at 0.
    pub const fn set_fault(&mut self, fault: Fault) {
        self.fault = fault;
        self.ops = 0;
    }

    /// Operations executed since the last [`SimDisk::set_fault`].
    #[must_use]
    pub const fn op_count(&self) -> usize {
        self.ops
    }

    /// `true` between a crash and [`SimDisk::restart`].
    #[must_use]
    pub const fn is_crashed(&self) -> bool {
        self.crashed
    }

    /// Gate every operation: count it and apply the fault plan.
    fn tick(&mut self) -> Result<(), IoError> {
        if self.crashed {
            return Err(IoError::Crashed);
        }
        let n = self.ops;
        self.ops += 1;
        match self.fault {
            Fault::CrashAt(at) if at == n => {
                self.crash();
                Err(IoError::Crashed)
            }
            Fault::ErrorAt(at) if at == n => Err(IoError::Injected(format!("op {n}"))),
            _ => Ok(()),
        }
    }

    fn names(&self) -> BTreeMap<String, FileId> {
        let mut names = self.durable_names.clone();
        for op in &self.pending_dir {
            apply_dir(&mut names, op);
        }
        names
    }

    fn lookup(&self, name: &str) -> Result<FileId, IoError> {
        self.names()
            .get(name)
            .copied()
            .ok_or_else(|| IoError::NotFound(name.to_string()))
    }

    fn file_mut(&mut self, name: &str) -> Result<&mut SimFile, IoError> {
        let id = self.lookup(name)?;
        Ok(self.files.entry(id).or_default())
    }

    /// Creates (or replaces with) an empty file named `name`.
    ///
    /// # Errors
    ///
    /// [`IoError::Crashed`] / [`IoError::Injected`] per the fault plan.
    pub fn create(&mut self, name: &str) -> Result<(), IoError> {
        self.tick()?;
        let id = self.next_id;
        self.next_id += 1;
        self.files.insert(id, SimFile::default());
        self.pending_dir.push(DirOp::Link(name.to_string(), id));
        Ok(())
    }

    /// Appends `bytes` to `name` (volatile until [`SimDisk::fsync`]).
    ///
    /// # Errors
    ///
    /// Fault-plan errors or [`IoError::NotFound`].
    pub fn append(&mut self, name: &str, bytes: &[u8]) -> Result<(), IoError> {
        self.tick()?;
        self.file_mut(name)?
            .pending
            .push(DataOp::Append(bytes.to_vec()));
        Ok(())
    }

    /// Truncates `name` to zero length (volatile until [`SimDisk::fsync`]).
    ///
    /// # Errors
    ///
    /// Fault-plan errors or [`IoError::NotFound`].
    pub fn truncate(&mut self, name: &str) -> Result<(), IoError> {
        self.tick()?;
        self.file_mut(name)?.pending.push(DataOp::Truncate);
        Ok(())
    }

    /// Makes `name`'s data durable.
    ///
    /// # Errors
    ///
    /// Fault-plan errors or [`IoError::NotFound`].
    pub fn fsync(&mut self, name: &str) -> Result<(), IoError> {
        self.tick()?;
        let file = self.file_mut(name)?;
        file.durable = file.current();
        file.pending.clear();
        Ok(())
    }

    /// Atomically points `to` at `from`'s file and unlinks `from`
    /// (volatile until [`SimDisk::fsync_dir`]).
    ///
    /// # Errors
    ///
    /// Fault-plan errors or [`IoError::NotFound`].
    pub fn rename(&mut self, from: &str, to: &str) -> Result<(), IoError> {
        self.tick()?;
        let id = self.lookup(from)?;
        self.pending_dir.push(DirOp::Link(to.to_string(), id));
        self.pending_dir.push(DirOp::Unlink(from.to_string()));
        Ok(())
    }

    /// Removes `name` (volatile until [`SimDisk::fsync_dir`]).
    ///
    /// # Errors
    ///
    /// Fault-plan errors or [`IoError::NotFound`].
    pub fn remove(&mut self, name: &str) -> Result<(), IoError> {
        self.tick()?;
        self.lookup(name)?;
        self.pending_dir.push(DirOp::Unlink(name.to_string()));
        Ok(())
    }

    /// Makes directory changes durable.
    ///
    /// # Errors
    ///
    /// Fault-plan errors.
    pub fn fsync_dir(&mut self) -> Result<(), IoError> {
        self.tick()?;
        self.durable_names = self.names();
        self.pending_dir.clear();
        Ok(())
    }

    /// Reads the current (possibly volatile) contents of `name`.
    ///
    /// # Errors
    ///
    /// Fault-plan errors or [`IoError::NotFound`].
    pub fn read(&mut self, name: &str) -> Result<Vec<u8>, IoError> {
        self.tick()?;
        let id = self.lookup(name)?;
        Ok(self
            .files
            .get(&id)
            .map(SimFile::current)
            .unwrap_or_default())
    }

    /// `true` if `name` exists. Does not count as an operation.
    #[must_use]
    pub fn exists(&self, name: &str) -> bool {
        self.lookup(name).is_ok()
    }

    /// Loses power: for each file and the directory, keep a random prefix of
    /// pending operations (tearing the last surviving append), drop the rest.
    pub fn crash(&mut self) {
        let mut ids: Vec<FileId> = self.files.keys().copied().collect();
        ids.sort_unstable(); // HashMap order is random; keep crashes reproducible
        for id in ids {
            let Some(file) = self.files.get_mut(&id) else {
                continue;
            };
            let keep = self.rng.below(file.pending.len() + 1);
            let pending = std::mem::take(&mut file.pending);
            for (i, op) in pending.into_iter().take(keep).enumerate() {
                match op {
                    DataOp::Append(bytes) if i + 1 == keep => {
                        let torn = self.rng.below(bytes.len() + 1);
                        file.durable.extend_from_slice(&bytes[..torn]);
                    }
                    op => apply_data(&mut file.durable, &op),
                }
            }
        }
        let keep = self.rng.below(self.pending_dir.len() + 1);
        for op in std::mem::take(&mut self.pending_dir).iter().take(keep) {
            apply_dir(&mut self.durable_names, op);
        }
        self.crashed = true;
    }

    /// Powers back on: only durable state remains. Clears the fault plan.
    pub fn restart(&mut self) {
        self.crashed = false;
        self.fault = Fault::None;
        self.ops = 0;
        let live: Vec<FileId> = self.durable_names.values().copied().collect();
        self.files.retain(|id, _| live.contains(id));
    }
}

// ============================================================================
// Named failpoints (the `fail` crate pattern, without globals)
// ============================================================================

/// What an armed failpoint does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailAction {
    /// Return [`IoError::Injected`] every time.
    Error,
    /// Return [`IoError::Injected`] once, then disarm.
    ErrorOnce,
}

/// A registry of named failpoints. Passed explicitly (not a global) so tests
/// stay parallel-safe.
#[derive(Debug, Default)]
pub struct FailPoints {
    armed: HashMap<&'static str, FailAction>,
    hits: HashMap<&'static str, usize>,
}

impl FailPoints {
    /// No failpoints armed.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Arms `name`.
    pub fn arm(&mut self, name: &'static str, action: FailAction) {
        self.armed.insert(name, action);
    }

    /// Times execution passed through `name` (armed or not).
    #[must_use]
    pub fn hits(&self, name: &str) -> usize {
        self.hits.get(name).copied().unwrap_or(0)
    }

    /// Evaluates failpoint `name`.
    ///
    /// # Errors
    ///
    /// [`IoError::Injected`] when armed.
    pub fn eval(&mut self, name: &'static str) -> Result<(), IoError> {
        *self.hits.entry(name).or_insert(0) += 1;
        match self.armed.get(name).copied() {
            None => Ok(()),
            Some(FailAction::Error) => Err(IoError::Injected(name.to_string())),
            Some(FailAction::ErrorOnce) => {
                self.armed.remove(name);
                Err(IoError::Injected(name.to_string()))
            }
        }
    }
}

/// `fail_point!(fp, "name")` returns early with the injected error when armed.
#[macro_export]
macro_rules! fail_point {
    ($fp:expr, $name:literal) => {
        $fp.eval($name)?
    };
}

// ============================================================================
// Subject under test #1: replacing a file's contents
// ============================================================================

/// How [`replace_file`] updates a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplaceStrategy {
    /// BUGGY: truncate + write + fsync in place. A crash can leave an empty or
    /// half-written file.
    InPlace,
    /// BUGGY: write temp file, rename over target, fsync dir — but never fsync
    /// the temp file. The rename can be durable while the data is not.
    TempRenameNoFsync,
    /// CORRECT: write temp, fsync temp, rename, fsync dir.
    TempFsyncRename,
}

/// Replaces `name`'s contents with `contents`.
///
/// # Errors
///
/// Any [`IoError`] from the disk (including injected faults).
pub fn replace_file(
    disk: &mut SimDisk,
    name: &str,
    contents: &[u8],
    strategy: ReplaceStrategy,
) -> Result<(), IoError> {
    match strategy {
        ReplaceStrategy::InPlace => {
            if !disk.exists(name) {
                disk.create(name)?;
                disk.fsync_dir()?;
            }
            disk.truncate(name)?;
            disk.append(name, contents)?;
            disk.fsync(name)
        }
        ReplaceStrategy::TempRenameNoFsync | ReplaceStrategy::TempFsyncRename => {
            let tmp = format!("{name}.tmp");
            disk.create(&tmp)?;
            disk.append(&tmp, contents)?;
            if strategy == ReplaceStrategy::TempFsyncRename {
                disk.fsync(&tmp)?;
            }
            disk.rename(&tmp, name)?;
            disk.fsync_dir()
        }
    }
}

// ============================================================================
// Subject under test #2: a CRC-framed append-only log
// ============================================================================

const RECORD_HEADER: usize = 8; // u32 len + u32 crc

/// An append-only log: `[len u32 LE][crc32 u32 LE][payload]` per record.
#[derive(Debug)]
pub struct LogStore {
    name: String,
    /// Named failpoints inside `commit`.
    pub fail_points: FailPoints,
}

impl LogStore {
    /// Opens (creating if needed) the log `name`.
    ///
    /// # Errors
    ///
    /// Any [`IoError`] from the disk.
    pub fn open(disk: &mut SimDisk, name: &str) -> Result<Self, IoError> {
        if !disk.exists(name) {
            disk.create(name)?;
            disk.fsync_dir()?;
        }
        Ok(Self {
            name: name.to_string(),
            fail_points: FailPoints::new(),
        })
    }

    /// Appends `payload` and fsyncs. When this returns `Ok`, the record is
    /// durable.
    ///
    /// # Errors
    ///
    /// Any [`IoError`]; on error the record may or may not survive a crash,
    /// but recovery never returns a partial record.
    pub fn commit(&mut self, disk: &mut SimDisk, payload: &[u8]) -> Result<(), IoError> {
        let len = u32::try_from(payload.len())
            .map_err(|_| IoError::Corrupt("record too large".into()))?;
        let mut record = Vec::with_capacity(RECORD_HEADER + payload.len());
        record.extend_from_slice(&len.to_le_bytes());
        record.extend_from_slice(&crc32fast::hash(payload).to_le_bytes());
        record.extend_from_slice(payload);
        fail_point!(self.fail_points, "log::before_append");
        disk.append(&self.name, &record)?;
        fail_point!(self.fail_points, "log::before_fsync");
        disk.fsync(&self.name)
    }

    /// Reads all intact records, stopping at the first torn or corrupt one.
    /// A torn tail is expected after a crash; it is ignored, not an error.
    ///
    /// # Errors
    ///
    /// Any [`IoError`] from the disk.
    pub fn recover(disk: &mut SimDisk, name: &str) -> Result<Vec<Vec<u8>>, IoError> {
        let data = match disk.read(name) {
            Ok(data) => data,
            Err(IoError::NotFound(_)) => return Ok(Vec::new()),
            Err(e) => return Err(e),
        };
        let mut records = Vec::new();
        let mut rest = data.as_slice();
        while let Some((header, body)) = rest.split_first_chunk::<RECORD_HEADER>() {
            let len = u32::from_le_bytes([header[0], header[1], header[2], header[3]]) as usize;
            let crc = u32::from_le_bytes([header[4], header[5], header[6], header[7]]);
            let Some(payload) = body.get(..len) else {
                break;
            }; // torn payload
            if crc32fast::hash(payload) != crc {
                break; // torn or corrupt
            }
            records.push(payload.to_vec());
            rest = &body[len..];
        }
        Ok(records)
    }
}

// ============================================================================
// The harness: crash at every point, check the invariant after each
// ============================================================================

/// A state that violated the recovery invariant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrashViolation {
    /// Operation index the crash was injected at.
    pub crash_point: usize,
    /// Disk seed (which pending writes survived).
    pub seed: u64,
    /// What the invariant check reported.
    pub message: String,
}

/// Outcome of a full exploration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Exploration {
    /// Operations in one crash-free run of the workload.
    pub ops_per_run: usize,
    /// Crash states checked.
    pub states_checked: usize,
}

/// Crashes `workload` at every operation and checks recovery after each.
///
/// For every crash point `k` in `0..=ops` and every seed in `seeds`:
/// build a fresh disk with `setup`, run `workload` with a crash injected at op
/// `k`, restart, and run `check`. `workload` returns what it believes was
/// committed before the crash (e.g. records whose commit returned `Ok`).
///
/// # Errors
///
/// The first [`CrashViolation`] found.
pub fn explore_crash_points<S, W, C, T>(
    seeds: std::ops::Range<u64>,
    setup: S,
    workload: W,
    check: C,
) -> Result<Exploration, CrashViolation>
where
    S: Fn(&mut SimDisk),
    W: Fn(&mut SimDisk) -> T,
    C: Fn(&mut SimDisk, &T) -> Result<(), String>,
{
    // Dry run: count the workload's operations.
    let mut dry = SimDisk::new(0);
    setup(&mut dry);
    dry.set_fault(Fault::None);
    let _ = workload(&mut dry);
    let ops_per_run = dry.op_count();

    let mut states_checked = 0;
    for crash_point in 0..=ops_per_run {
        for seed in seeds.clone() {
            let mut disk = SimDisk::new(seed);
            setup(&mut disk);
            disk.set_fault(Fault::CrashAt(crash_point));
            let committed = workload(&mut disk);
            if !disk.is_crashed() {
                disk.crash(); // crash point past the end: crash after completion
            }
            disk.restart();
            check(&mut disk, &committed).map_err(|message| CrashViolation {
                crash_point,
                seed,
                message,
            })?;
            states_checked += 1;
        }
    }
    Ok(Exploration {
        ops_per_run,
        states_checked,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const OLD: &[u8] = b"version-1 settings";
    const NEW: &[u8] = b"version-2 settings, longer";

    fn explore_replace(strategy: ReplaceStrategy) -> Result<Exploration, CrashViolation> {
        explore_crash_points(
            0..16,
            |disk| {
                replace_file(disk, "config", OLD, ReplaceStrategy::TempFsyncRename).unwrap();
            },
            |disk| replace_file(disk, "config", NEW, strategy).is_ok(),
            |disk, &completed| {
                let got = disk.read("config").map_err(|e| e.to_string())?;
                if completed && got != NEW {
                    return Err(format!("acknowledged write lost: {got:?}"));
                }
                if got == OLD || got == NEW {
                    Ok(())
                } else {
                    Err(format!(
                        "neither old nor new: {:?}",
                        String::from_utf8_lossy(&got)
                    ))
                }
            },
        )
    }

    // ---- SimDisk semantics ----

    #[test]
    fn test_unsynced_data_lost_on_crash_with_seed_that_keeps_nothing() {
        // Find a seed whose crash keeps no pending op, then assert the effect.
        let lost = (0..64).any(|seed| {
            let mut disk = SimDisk::new(seed);
            disk.create("f").unwrap();
            disk.fsync_dir().unwrap();
            disk.append("f", b"data").unwrap();
            disk.crash();
            disk.restart();
            disk.read("f").unwrap().is_empty()
        });
        assert!(lost);
    }

    #[test]
    fn test_synced_data_survives_every_crash() {
        for seed in 0..64 {
            let mut disk = SimDisk::new(seed);
            disk.create("f").unwrap();
            disk.fsync_dir().unwrap();
            disk.append("f", b"data").unwrap();
            disk.fsync("f").unwrap();
            disk.append("f", b"more").unwrap(); // unsynced tail
            disk.crash();
            disk.restart();
            let got = disk.read("f").unwrap();
            assert!(got.starts_with(b"data"), "seed {seed}: {got:?}");
            assert!(b"datamore".starts_with(&got), "seed {seed}: {got:?}");
        }
    }

    #[test]
    fn test_torn_writes_happen() {
        let torn = (0..256).any(|seed| {
            let mut disk = SimDisk::new(seed);
            disk.create("f").unwrap();
            disk.fsync_dir().unwrap();
            disk.append("f", b"abcdefgh").unwrap();
            disk.crash();
            disk.restart();
            let len = disk.read("f").unwrap().len();
            len > 0 && len < 8
        });
        assert!(torn, "the model must produce torn appends");
    }

    #[test]
    fn test_unsynced_create_can_vanish() {
        let vanished = (0..64).any(|seed| {
            let mut disk = SimDisk::new(seed);
            disk.create("f").unwrap();
            disk.crash();
            disk.restart();
            !disk.exists("f")
        });
        assert!(vanished);
    }

    #[test]
    fn test_crashed_disk_rejects_everything_until_restart() {
        let mut disk = SimDisk::new(0);
        disk.set_fault(Fault::CrashAt(1));
        disk.create("f").unwrap();
        assert_eq!(disk.fsync_dir(), Err(IoError::Crashed));
        assert!(disk.is_crashed());
        assert_eq!(disk.read("f"), Err(IoError::Crashed));
        disk.restart();
        assert!(!disk.is_crashed());
    }

    #[test]
    fn test_error_injection_has_no_effect() {
        let mut disk = SimDisk::new(0);
        disk.create("f").unwrap();
        disk.set_fault(Fault::ErrorAt(0));
        assert_eq!(
            disk.append("f", b"x"),
            Err(IoError::Injected("op 0".into()))
        );
        assert_eq!(disk.read("f").unwrap(), b"");
        disk.append("f", b"y").unwrap();
        assert_eq!(disk.read("f").unwrap(), b"y");
    }

    #[test]
    fn test_not_found_and_remove() {
        let mut disk = SimDisk::new(0);
        assert_eq!(disk.read("nope"), Err(IoError::NotFound("nope".into())));
        assert_eq!(disk.remove("nope"), Err(IoError::NotFound("nope".into())));
        disk.create("f").unwrap();
        disk.remove("f").unwrap();
        disk.fsync_dir().unwrap();
        assert!(!disk.exists("f"));
        assert!(IoError::Crashed.to_string().contains("crash"));
        assert!(IoError::Corrupt("x".into()).to_string().contains("corrupt"));
    }

    // ---- replace_file: the harness separates safe from unsafe ----

    #[test]
    fn test_all_strategies_work_without_crashes() {
        for strategy in [
            ReplaceStrategy::InPlace,
            ReplaceStrategy::TempRenameNoFsync,
            ReplaceStrategy::TempFsyncRename,
        ] {
            let mut disk = SimDisk::new(0);
            replace_file(&mut disk, "c", OLD, strategy).unwrap();
            replace_file(&mut disk, "c", NEW, strategy).unwrap();
            assert_eq!(disk.read("c").unwrap(), NEW, "{strategy:?}");
        }
    }

    #[test]
    fn test_in_place_replace_is_not_crash_safe() {
        let violation =
            explore_replace(ReplaceStrategy::InPlace).expect_err("must find a torn file");
        assert!(
            violation.message.contains("neither old nor new"),
            "{violation:?}"
        );
    }

    #[test]
    fn test_rename_without_fsync_is_not_crash_safe() {
        let violation = explore_replace(ReplaceStrategy::TempRenameNoFsync)
            .expect_err("rename can outrun the data");
        assert!(
            violation.message.contains("neither old nor new"),
            "{violation:?}"
        );
    }

    #[test]
    fn test_temp_fsync_rename_is_crash_safe() {
        let exploration = explore_replace(ReplaceStrategy::TempFsyncRename).unwrap();
        assert_eq!(exploration.ops_per_run, 5); // create, append, fsync, rename, fsync_dir
        assert_eq!(exploration.states_checked, 6 * 16);
    }

    // ---- LogStore: committed records survive, torn tails are dropped ----

    #[test]
    fn test_log_roundtrip() {
        let mut disk = SimDisk::new(0);
        let mut log = LogStore::open(&mut disk, "wal").unwrap();
        log.commit(&mut disk, b"a").unwrap();
        log.commit(&mut disk, b"").unwrap();
        log.commit(&mut disk, b"ccc").unwrap();
        assert_eq!(
            LogStore::recover(&mut disk, "wal").unwrap(),
            vec![b"a".to_vec(), vec![], b"ccc".to_vec()]
        );
        assert!(LogStore::recover(&mut disk, "missing").unwrap().is_empty());
    }

    #[test]
    fn test_log_survives_crash_at_every_point() {
        let payloads: Vec<Vec<u8>> = (0u8..5).map(|i| vec![i; 3 + usize::from(i) * 7]).collect();
        let exploration = explore_crash_points(
            0..32,
            |disk| {
                LogStore::open(disk, "wal").unwrap();
            },
            |disk| {
                let mut log = LogStore::open(disk, "wal").expect("exists after setup");
                payloads
                    .iter()
                    .take_while(|p| log.commit(disk, p).is_ok())
                    .count()
            },
            |disk, &acked| {
                let recovered = LogStore::recover(disk, "wal").map_err(|e| e.to_string())?;
                if recovered.len() < acked {
                    return Err(format!(
                        "acked {acked} records, recovered {}",
                        recovered.len()
                    ));
                }
                if recovered.as_slice() != &payloads[..recovered.len()] {
                    return Err("recovered records are not a prefix of what was written".into());
                }
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(exploration.ops_per_run, 10); // 5 x (append + fsync)
    }

    #[test]
    fn test_log_recovery_rejects_corrupt_record() {
        let mut disk = SimDisk::new(0);
        let mut log = LogStore::open(&mut disk, "wal").unwrap();
        log.commit(&mut disk, b"good").unwrap();
        // A record whose CRC does not match its payload.
        let mut bad = 3u32.to_le_bytes().to_vec();
        bad.extend_from_slice(&0xDEAD_BEEFu32.to_le_bytes());
        bad.extend_from_slice(b"bad");
        disk.append("wal", &bad).unwrap();
        log.commit(&mut disk, b"after").unwrap();
        assert_eq!(
            LogStore::recover(&mut disk, "wal").unwrap(),
            vec![b"good".to_vec()]
        );
    }

    #[test]
    fn test_failpoints_inject_errors() {
        let mut disk = SimDisk::new(0);
        let mut log = LogStore::open(&mut disk, "wal").unwrap();

        log.fail_points
            .arm("log::before_append", FailAction::ErrorOnce);
        assert_eq!(
            log.commit(&mut disk, b"x"),
            Err(IoError::Injected("log::before_append".into()))
        );
        log.commit(&mut disk, b"y").unwrap(); // disarmed after one hit

        log.fail_points.arm("log::before_fsync", FailAction::Error);
        assert!(log.commit(&mut disk, b"z").is_err());
        assert!(log.commit(&mut disk, b"z").is_err());
        assert_eq!(log.fail_points.hits("log::before_fsync"), 3);
        assert_eq!(log.fail_points.hits("never"), 0);

        // The un-fsynced "z" records are visible now but not guaranteed durable.
        disk.crash();
        disk.restart();
        let recovered = LogStore::recover(&mut disk, "wal").unwrap();
        assert_eq!(recovered[0], b"y");
        assert!(recovered.iter().skip(1).all(|r| r == b"z"));
    }
}
