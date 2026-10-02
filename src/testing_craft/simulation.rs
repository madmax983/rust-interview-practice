//! # Deterministic Simulation Testing (DST)
//!
//! The `FoundationDB` / `TigerBeetle` approach to testing distributed systems: run
//! the *whole cluster* in one thread, with every source of nondeterminism —
//! network delay, message loss and duplication, crashes, restarts, partitions,
//! client traffic — drawn from **one seeded RNG**. Then:
//!
//! - every run is **reproducible**: a failing seed replays the exact same
//!   execution, as many times as you like, under a debugger if you want;
//! - thousands of seeds explore thousands of distinct interleavings per second;
//! - an **oracle** checks the protocol's safety invariants after every tick.
//!
//! The system under test is the functional-core Raft in
//! [`crate::systems::raft`]: it does no I/O, it only reacts to `step(msg)` and
//! `tick()` and leaves outgoing messages in a buffer. That shape — the "sans-IO"
//! design — is what makes DST possible. The simulator plays the network, the
//! disks, the clock and the clients.
//!
//! ## Invariants checked every tick
//!
//! | Invariant | Meaning |
//! |-----------|---------|
//! | [`Invariant::ElectionSafety`] | At most one leader per term, ever. |
//! | [`Invariant::LogMatching`] | An `(index, term)` pair identifies one entry *and* its whole prefix, for all time. |
//! | [`Invariant::StateMachineSafety`] | No two nodes ever commit different entries at the same index. |
//! | [`Invariant::CommitMonotonic`] | A running node's commit index never moves backwards. |
//! | [`Invariant::Liveness`] | After all faults heal, the cluster commits new entries. |
//!
//! ## Bugs found
//!
//! Its first sweep found a real **livelock** in `systems::raft`: stepping down
//! on a higher term reset the election timer even when the vote was refused,
//! so a node with a stale log and a short timeout could block every election
//! forever (seed 27 of [`SimConfig::chaos`]). The fix and its regression test
//! live in `systems::raft`; the seed sweep below keeps it fixed.
//!
//! ## Finding a planted bug
//!
//! [`Durability::ForgetVoteBuggy`] restarts nodes without their persisted
//! `voted_for` — the classic Raft durability bug. A restarted node can vote
//! twice in one term, so two candidates can both win it. A seed sweep finds a
//! counterexample, and replaying that seed reproduces it exactly:
//!
//! ```
//! use rust_interview_practice::testing_craft::simulation::{
//!     find_violation, run, Durability, Invariant, SimConfig,
//! };
//!
//! let config = SimConfig { durability: Durability::ForgetVoteBuggy, ..SimConfig::flapping() };
//! let violation = find_violation(0..50, config).expect("the sweep finds the bug");
//! assert_eq!(violation.invariant, Invariant::ElectionSafety);
//!
//! // Same seed, same execution, same failure.
//! assert_eq!(run(violation.seed, config).unwrap_err(), violation);
//!
//! // With correct durability, the same seed is clean.
//! assert!(run(violation.seed, SimConfig::flapping()).is_ok());
//! ```

use std::cmp::Ordering;
use std::collections::hash_map::DefaultHasher;
use std::collections::{BTreeMap, BTreeSet, BinaryHeap, HashMap, VecDeque};
use std::fmt;
use std::hash::{Hash, Hasher};

use super::sim_rng::SimRng;
use crate::systems::raft::{Consensus, Envelope, LogEntry, Message, RaftNode, Role};

/// Node identifier.
pub type NodeId = u64;
/// A client command (unique per simulation, so commits are distinguishable).
pub type Command = u64;

/// What survives a node crash.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Durability {
    /// Correct: `current_term`, `voted_for` and `log` survive a restart
    /// (Raft's "persistent state on all servers").
    Persistent,
    /// BUGGY on purpose: `voted_for` is not persisted. After a restart a node
    /// may grant a second vote in a term it already voted in.
    ForgetVoteBuggy,
}

/// Fault rates, in events per thousand ticks (or per thousand messages).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FaultConfig {
    /// Chance (per mille) that a message is lost.
    pub drop_per_mille: usize,
    /// Chance (per mille) that a message is delivered twice.
    pub duplicate_per_mille: usize,
    /// Messages take `1..=max_delay` ticks; different delays reorder them.
    pub max_delay: usize,
    /// Chance (per mille, per tick) that a running node crashes.
    pub crash_per_mille: usize,
    /// Chance (per mille, per tick) that a crashed node restarts.
    pub restart_per_mille: usize,
    /// Chance (per mille, per tick) that one node is cut off from the rest.
    pub partition_per_mille: usize,
    /// Chance (per mille, per tick) that all partitions heal.
    pub heal_per_mille: usize,
}

impl FaultConfig {
    /// A perfect network and immortal nodes.
    pub const NONE: Self = Self {
        drop_per_mille: 0,
        duplicate_per_mille: 0,
        max_delay: 1,
        crash_per_mille: 0,
        restart_per_mille: 0,
        partition_per_mille: 0,
        heal_per_mille: 0,
    };

    /// Everything at once, at rates high enough to matter in a few hundred ticks.
    pub const CHAOS: Self = Self {
        drop_per_mille: 50,
        duplicate_per_mille: 30,
        max_delay: 6,
        crash_per_mille: 15,
        restart_per_mille: 60,
        partition_per_mille: 8,
        heal_per_mille: 40,
    };

    /// Nodes that crash and come back within an election round. Bugs in what
    /// survives a restart only show up when a restart lands *inside* the
    /// protocol step it breaks, so the fault timescale must match the bug's.
    pub const FLAPPING: Self = Self {
        drop_per_mille: 50,
        duplicate_per_mille: 30,
        max_delay: 8,
        crash_per_mille: 100,
        restart_per_mille: 700,
        partition_per_mille: 8,
        heal_per_mille: 40,
    };

    /// **Swarm testing** (Groce et al., 2012): instead of enabling every fault
    /// at a fixed rate, each seed switches each fault off, low or high. Some
    /// bugs only appear when one fault is absent (e.g. no drops, so a
    /// duplicate is never masked), and a uniform mix rarely produces that.
    pub fn swarm(rng: &mut SimRng) -> Self {
        let mut level = |low: usize, high: usize| match rng.below(3) {
            0 => 0,
            1 => low,
            _ => high,
        };
        let drop_per_mille = level(20, 150);
        let duplicate_per_mille = level(20, 150);
        let crash_per_mille = level(5, 30);
        let partition_per_mille = level(3, 20);
        Self {
            drop_per_mille,
            duplicate_per_mille,
            max_delay: 1 + rng.below(8),
            crash_per_mille,
            restart_per_mille: 60,
            partition_per_mille,
            heal_per_mille: 40,
        }
    }
}

/// One simulation's shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SimConfig {
    /// Cluster size.
    pub nodes: usize,
    /// Ticks with faults enabled.
    pub ticks: u64,
    /// Fault-free ticks after healing everything; the cluster must commit
    /// something new in this window ([`Invariant::Liveness`]). `0` disables.
    pub quiesce_ticks: u64,
    /// Chance (per mille, per tick) that a client sends a command to a random
    /// node that believes it is leader.
    pub propose_per_mille: usize,
    /// Fault rates. Ignored when [`SimConfig::swarm`] is set.
    pub faults: FaultConfig,
    /// Derive fault rates per seed with [`FaultConfig::swarm`].
    pub swarm: bool,
    /// What survives a crash.
    pub durability: Durability,
}

impl SimConfig {
    /// Three nodes, perfect network.
    #[must_use]
    pub const fn calm() -> Self {
        Self {
            nodes: 3,
            ticks: 300,
            quiesce_ticks: 200,
            propose_per_mille: 300,
            faults: FaultConfig::NONE,
            swarm: false,
            durability: Durability::Persistent,
        }
    }

    /// Five nodes under [`FaultConfig::CHAOS`].
    #[must_use]
    pub const fn chaos() -> Self {
        Self {
            nodes: 5,
            ticks: 400,
            quiesce_ticks: 300,
            propose_per_mille: 300,
            faults: FaultConfig::CHAOS,
            swarm: false,
            durability: Durability::Persistent,
        }
    }

    /// Five nodes under [`FaultConfig::FLAPPING`].
    #[must_use]
    pub const fn flapping() -> Self {
        Self {
            faults: FaultConfig::FLAPPING,
            ..Self::chaos()
        }
    }
}

/// The kind of a Raft message (for traces).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MsgKind {
    /// `RequestVote`
    Vote,
    /// `RequestVoteResponse`
    VoteReply,
    /// `AppendEntries`
    Append,
    /// `AppendEntriesResponse`
    AppendReply,
}

impl MsgKind {
    const fn of(msg: &Message<Command>) -> Self {
        match msg {
            Message::RequestVote { .. } => Self::Vote,
            Message::RequestVoteResponse { .. } => Self::VoteReply,
            Message::AppendEntries { .. } => Self::Append,
            Message::AppendEntriesResponse { .. } => Self::AppendReply,
        }
    }
}

/// Everything that happens in a simulation, in order. Hashed into the run's
/// fingerprint and kept (the last few) for failure reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Event {
    /// A client command reached a node that believes it leads `term`.
    Propose {
        /// Receiving leader.
        leader: NodeId,
        /// Its term.
        term: u64,
        /// The command.
        cmd: Command,
    },
    /// A message arrived.
    Deliver {
        /// Sender.
        from: NodeId,
        /// Receiver.
        to: NodeId,
        /// Message kind.
        kind: MsgKind,
    },
    /// A message was lost (random drop, partition, or dead receiver).
    Drop {
        /// Sender.
        from: NodeId,
        /// Intended receiver.
        to: NodeId,
    },
    /// A message was put on the wire twice.
    Duplicate {
        /// Sender.
        from: NodeId,
        /// Receiver.
        to: NodeId,
    },
    /// A node lost power.
    Crash(NodeId),
    /// A node came back with its durable state.
    Restart(NodeId),
    /// A node was cut off from every other node.
    Isolate(NodeId),
    /// All partitions healed.
    Heal,
    /// A node became leader.
    Elected {
        /// The new leader.
        id: NodeId,
        /// Its term.
        term: u64,
    },
    /// The cluster-wide committed prefix grew.
    Commit {
        /// Log index.
        index: usize,
        /// Entry term.
        term: u64,
    },
}

/// The safety or liveness property that failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Invariant {
    /// Two leaders in one term.
    ElectionSafety,
    /// One `(index, term)` pair named two different entries or prefixes.
    LogMatching,
    /// Two nodes committed different entries at one index.
    StateMachineSafety,
    /// A running node's commit index went backwards.
    CommitMonotonic,
    /// The healed cluster made no progress.
    Liveness,
}

/// A failed run: everything needed to reproduce and understand it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Violation {
    /// Replay with `run(seed, config)`.
    pub seed: u64,
    /// Simulated time of the failure.
    pub tick: u64,
    /// What broke.
    pub invariant: Invariant,
    /// Human-readable specifics.
    pub detail: String,
    /// The last events before the failure, oldest first.
    pub recent: Vec<(u64, Event)>,
}

impl fmt::Display for Violation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "{:?} violated at tick {} (seed {}): {}",
            self.invariant, self.tick, self.seed, self.detail
        )?;
        for (tick, event) in &self.recent {
            writeln!(f, "  t={tick:>4} {event:?}")?;
        }
        Ok(())
    }
}

impl std::error::Error for Violation {}

/// Summary of a clean run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    /// The seed that produced this run.
    pub seed: u64,
    /// Total simulated ticks.
    pub ticks: u64,
    /// Hash of the full event sequence: equal fingerprints, equal executions.
    pub fingerprint: u64,
    /// Events recorded.
    pub events: usize,
    /// Leaders elected (distinct terms with a leader).
    pub elections: usize,
    /// Length of the cluster-wide committed prefix.
    pub committed: usize,
    /// Crashes injected.
    pub crashes: usize,
    /// Messages lost.
    pub drops: usize,
    /// Partitions injected.
    pub partitions: usize,
}

/// Runs one simulation.
///
/// # Errors
///
/// The first [`Violation`] the oracle detects.
pub fn run(seed: u64, config: SimConfig) -> Result<Report, Violation> {
    Simulation::new(seed, config).run()
}

/// Runs every seed in `seeds`; returns the first violation found.
#[must_use]
pub fn find_violation(seeds: std::ops::Range<u64>, config: SimConfig) -> Option<Violation> {
    seeds.into_iter().find_map(|seed| run(seed, config).err())
}

// ============================================================================
// The simulator
// ============================================================================

/// A message on the simulated wire, ordered by delivery time then send order.
struct InFlight {
    at: u64,
    seq: u64,
    env: Envelope<Command>,
}

impl PartialEq for InFlight {
    fn eq(&self, other: &Self) -> bool {
        (self.at, self.seq) == (other.at, other.seq)
    }
}

impl Eq for InFlight {}

impl PartialOrd for InFlight {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for InFlight {
    // Reversed so `BinaryHeap` (a max-heap) pops the earliest message first.
    fn cmp(&self, other: &Self) -> Ordering {
        (other.at, other.seq).cmp(&(self.at, self.seq))
    }
}

struct SimNode {
    raft: RaftNode<Command>,
    up: bool,
    election_timeout: usize,
    last_commit: usize,
}

const HEARTBEAT_TICKS: usize = 3;

/// Node ids are `0..nodes`, so an id is also its index into `nodes`.
#[allow(clippy::cast_possible_truncation)] // ids are < nodes.len() <= usize::MAX
const fn slot(id: NodeId) -> usize {
    id as usize
}
const RECENT_EVENTS: usize = 64;

/// A whole Raft cluster plus its environment, advanced one tick at a time.
pub struct Simulation {
    seed: u64,
    config: SimConfig,
    faults: FaultConfig,
    rng: SimRng,
    now: u64,
    seq: u64,
    nodes: Vec<SimNode>,
    wire: BinaryHeap<InFlight>,
    isolated: BTreeSet<NodeId>,
    next_cmd: Command,
    // ---- oracle state ----
    leaders: BTreeMap<u64, NodeId>,
    /// `(index, term)` -> `(command, previous entry's term)`, for all time.
    ledger: HashMap<(usize, u64), (Option<Command>, u64)>,
    /// The cluster-wide committed prefix (log indices 1..).
    committed: Vec<LogEntry<Command>>,
    // ---- trace ----
    recent: VecDeque<(u64, Event)>,
    hasher: DefaultHasher,
    events: usize,
    crashes: usize,
    drops: usize,
    partitions: usize,
}

impl Simulation {
    /// Builds a cluster for `seed`. Election timeouts are drawn per node from
    /// the seed, so elections differ between seeds but not between replays.
    #[must_use]
    pub fn new(seed: u64, config: SimConfig) -> Self {
        let mut rng = SimRng::new(seed);
        let faults = if config.swarm {
            FaultConfig::swarm(&mut rng)
        } else {
            config.faults
        };
        let ids: Vec<NodeId> = (0..config.nodes as u64).collect();
        let nodes = ids
            .iter()
            .map(|&id| {
                let peers = ids.iter().copied().filter(|&p| p != id).collect();
                let election_timeout = 10 + rng.below(15);
                SimNode {
                    raft: RaftNode::new(id, peers, election_timeout, HEARTBEAT_TICKS),
                    up: true,
                    election_timeout,
                    last_commit: 0,
                }
            })
            .collect();
        Self {
            seed,
            config,
            faults,
            rng,
            now: 0,
            seq: 0,
            nodes,
            wire: BinaryHeap::new(),
            isolated: BTreeSet::new(),
            next_cmd: 1,
            leaders: BTreeMap::new(),
            ledger: HashMap::new(),
            committed: Vec::new(),
            recent: VecDeque::with_capacity(RECENT_EVENTS),
            hasher: DefaultHasher::new(),
            events: 0,
            crashes: 0,
            drops: 0,
            partitions: 0,
        }
    }

    /// Runs the fault phase, heals everything, runs the quiesce phase, and
    /// checks liveness.
    ///
    /// # Errors
    ///
    /// The first [`Violation`] found.
    pub fn run(mut self) -> Result<Report, Violation> {
        for _ in 0..self.config.ticks {
            self.step(true)?;
        }
        if self.config.quiesce_ticks > 0 {
            self.heal_all();
            let before = self.committed.len();
            for _ in 0..self.config.quiesce_ticks {
                self.step(false)?;
            }
            if self.committed.len() == before {
                let detail = format!(
                    "no new commits in {} fault-free ticks (committed prefix stuck at {before})",
                    self.config.quiesce_ticks
                );
                return Err(self.violation(Invariant::Liveness, detail));
            }
        }
        Ok(Report {
            seed: self.seed,
            ticks: self.now,
            fingerprint: self.hasher.finish(),
            events: self.events,
            elections: self.leaders.len(),
            committed: self.committed.len(),
            crashes: self.crashes,
            drops: self.drops,
            partitions: self.partitions,
        })
    }

    /// One tick: faults, client traffic, deliveries, timers, sends, oracle.
    fn step(&mut self, faults: bool) -> Result<(), Violation> {
        self.now += 1;
        if faults {
            self.inject_faults();
        }
        self.maybe_propose();
        self.deliver_due();
        for node in self.nodes.iter_mut().filter(|n| n.up) {
            node.raft.tick();
        }
        self.flush_outboxes(faults);
        self.check_invariants()
    }

    fn record(&mut self, event: Event) {
        event.hash(&mut self.hasher);
        self.now.hash(&mut self.hasher);
        self.events += 1;
        if self.recent.len() == RECENT_EVENTS {
            self.recent.pop_front();
        }
        self.recent.push_back((self.now, event));
    }

    fn violation(&self, invariant: Invariant, detail: String) -> Violation {
        Violation {
            seed: self.seed,
            tick: self.now,
            invariant,
            detail,
            recent: self.recent.iter().copied().collect(),
        }
    }

    // ---- environment ----

    fn pick_node(&mut self, up: bool) -> Option<usize> {
        let candidates: Vec<usize> = (0..self.nodes.len())
            .filter(|&i| self.nodes[i].up == up)
            .collect();
        self.rng.pick(&candidates).copied()
    }

    fn inject_faults(&mut self) {
        if self.rng.chance(self.faults.crash_per_mille, 1000)
            && let Some(i) = self.pick_node(true)
        {
            self.crash(i);
        }
        if self.rng.chance(self.faults.restart_per_mille, 1000)
            && let Some(i) = self.pick_node(false)
        {
            self.restart(i);
        }
        if self.rng.chance(self.faults.partition_per_mille, 1000) {
            let id = self.rng.below(self.nodes.len()) as NodeId;
            if self.isolated.insert(id) {
                self.partitions += 1;
                self.record(Event::Isolate(id));
            }
        }
        if !self.isolated.is_empty() && self.rng.chance(self.faults.heal_per_mille, 1000) {
            self.isolated.clear();
            self.record(Event::Heal);
        }
    }

    fn crash(&mut self, i: usize) {
        let node = &mut self.nodes[i];
        node.up = false;
        node.raft.messages.clear(); // unsent output dies with the process
        self.crashes += 1;
        self.record(Event::Crash(i as NodeId));
    }

    /// Rebuilds the node from scratch, copying back only what was durable.
    fn restart(&mut self, i: usize) {
        let durability = self.config.durability;
        let node = &mut self.nodes[i];
        let old = &node.raft;
        let mut fresh = RaftNode::new(
            old.id,
            old.peers.clone(),
            node.election_timeout,
            HEARTBEAT_TICKS,
        );
        fresh.current_term = old.current_term;
        fresh.log.clone_from(&old.log);
        if durability == Durability::Persistent {
            fresh.voted_for = old.voted_for;
        }
        node.raft = fresh;
        node.up = true;
        node.last_commit = 0; // commit_index is volatile
        self.record(Event::Restart(i as NodeId));
    }

    fn heal_all(&mut self) {
        self.isolated.clear();
        self.record(Event::Heal);
        for i in 0..self.nodes.len() {
            if !self.nodes[i].up {
                self.restart(i);
            }
        }
    }

    /// A client sends a command to a random node; it lands only if that node
    /// believes it is leader (stale leaders included — that is the point).
    fn maybe_propose(&mut self) {
        if !self.rng.chance(self.config.propose_per_mille, 1000) {
            return;
        }
        let Some(i) = self.pick_node(true) else {
            return;
        };
        let raft = &mut self.nodes[i].raft;
        if raft.role != Role::Leader {
            return;
        }
        let cmd = self.next_cmd;
        self.next_cmd += 1;
        let (leader, term) = (raft.id, raft.current_term);
        raft.log.push(LogEntry {
            term,
            data: Some(cmd),
        });
        self.record(Event::Propose { leader, term, cmd });
    }

    fn link_up(&self, from: NodeId, to: NodeId) -> bool {
        !self.isolated.contains(&from) && !self.isolated.contains(&to)
    }

    fn deliver_due(&mut self) {
        while self.wire.peek().is_some_and(|m| m.at <= self.now) {
            let Some(InFlight { env, .. }) = self.wire.pop() else {
                break;
            };
            let (from, to) = (env.from, env.to);
            let receiver_up = self.nodes.get(slot(to)).is_some_and(|n| n.up);
            if receiver_up && self.link_up(from, to) {
                self.record(Event::Deliver {
                    from,
                    to,
                    kind: MsgKind::of(&env.msg),
                });
                self.nodes[slot(to)].raft.step(env);
            } else {
                self.drops += 1;
                self.record(Event::Drop { from, to });
            }
        }
    }

    fn send(&mut self, env: Envelope<Command>) {
        let at = self.now + 1 + self.rng.below(self.faults.max_delay) as u64;
        self.seq += 1;
        self.wire.push(InFlight {
            at,
            seq: self.seq,
            env,
        });
    }

    /// Puts every node's outgoing messages on the wire. Loss and duplication
    /// apply only while `faults` is on; delays (and so reordering) always do.
    fn flush_outboxes(&mut self, faults: bool) {
        for i in 0..self.nodes.len() {
            let outbox = std::mem::take(&mut self.nodes[i].raft.messages);
            for env in outbox {
                if faults && self.rng.chance(self.faults.drop_per_mille, 1000) {
                    self.drops += 1;
                    self.record(Event::Drop {
                        from: env.from,
                        to: env.to,
                    });
                    continue;
                }
                if faults && self.rng.chance(self.faults.duplicate_per_mille, 1000) {
                    self.record(Event::Duplicate {
                        from: env.from,
                        to: env.to,
                    });
                    self.send(env.clone());
                }
                self.send(env);
            }
        }
    }

    // ---- oracle ----

    fn check_invariants(&mut self) -> Result<(), Violation> {
        for i in 0..self.nodes.len() {
            if !self.nodes[i].up {
                continue;
            }
            self.check_election_safety(i)?;
            self.check_log_matching(i)?;
            self.check_commits(i)?;
        }
        Ok(())
    }

    fn check_election_safety(&mut self, i: usize) -> Result<(), Violation> {
        let raft = &self.nodes[i].raft;
        if raft.role != Role::Leader {
            return Ok(());
        }
        let (id, term) = (raft.id, raft.current_term);
        match self.leaders.get(&term) {
            Some(&other) if other != id => Err(self.violation(
                Invariant::ElectionSafety,
                format!("nodes {other} and {id} both lead term {term}"),
            )),
            Some(_) => Ok(()),
            None => {
                self.leaders.insert(term, id);
                self.record(Event::Elected { id, term });
                Ok(())
            }
        }
    }

    /// Log Matching, checked inductively: if every `(index, term)` pair maps to
    /// a single `(command, previous term)` for all time, then equal pairs imply
    /// equal entries *and* equal prefixes. O(log length) per node per tick,
    /// instead of comparing every pair of logs.
    fn check_log_matching(&mut self, i: usize) -> Result<(), Violation> {
        let log = &self.nodes[i].raft.log;
        for idx in 1..log.len() {
            let key = (idx, log[idx].term);
            let value = (log[idx].data, log[idx - 1].term);
            match self.ledger.get(&key) {
                Some(seen) if *seen != value => {
                    let detail = format!(
                        "node {i} has {value:?} at (index {idx}, term {}), but {seen:?} was seen there before",
                        key.1
                    );
                    return Err(self.violation(Invariant::LogMatching, detail));
                }
                Some(_) => {}
                None => {
                    self.ledger.insert(key, value);
                }
            }
        }
        Ok(())
    }

    fn check_commits(&mut self, i: usize) -> Result<(), Violation> {
        let commit = self.nodes[i].raft.commit_index;
        if commit < self.nodes[i].last_commit {
            let detail = format!(
                "node {i} commit index fell from {} to {commit}",
                self.nodes[i].last_commit
            );
            return Err(self.violation(Invariant::CommitMonotonic, detail));
        }
        self.nodes[i].last_commit = commit;
        for idx in 1..=commit {
            let Some(entry) = self.nodes[i].raft.log.get(idx).cloned() else {
                let detail = format!("node {i} committed index {idx} beyond its log");
                return Err(self.violation(Invariant::StateMachineSafety, detail));
            };
            match self.committed.get(idx - 1) {
                Some(agreed) if *agreed != entry => {
                    let detail = format!(
                        "node {i} committed {entry:?} at index {idx}, cluster committed {agreed:?}"
                    );
                    return Err(self.violation(Invariant::StateMachineSafety, detail));
                }
                Some(_) => {}
                None => {
                    self.record(Event::Commit {
                        index: idx,
                        term: entry.term,
                    });
                    self.committed.push(entry);
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calm_cluster_elects_and_commits() {
        let report = run(1, SimConfig::calm()).unwrap();
        assert!(report.elections >= 1);
        assert!(report.committed > 20, "{report:?}");
        assert_eq!(report.crashes, 0);
        assert_eq!(report.drops, 0);
    }

    #[test]
    fn test_same_seed_same_execution() {
        let a = run(42, SimConfig::chaos()).unwrap();
        let b = run(42, SimConfig::chaos()).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn test_different_seeds_explore_different_executions() {
        let fingerprints: BTreeSet<u64> = (0..8)
            .map(|seed| run(seed, SimConfig::chaos()).unwrap().fingerprint)
            .collect();
        assert_eq!(fingerprints.len(), 8);
    }

    #[test]
    fn test_raft_is_safe_and_live_under_chaos() {
        let mut crashes = 0;
        let mut partitions = 0;
        for seed in 0..40 {
            let report = run(seed, SimConfig::chaos()).unwrap_or_else(|v| panic!("{v}"));
            crashes += report.crashes;
            partitions += report.partitions;
        }
        // The sweep actually exercised the faults it claims to survive.
        assert!(crashes > 40, "crashes: {crashes}");
        assert!(partitions > 10, "partitions: {partitions}");
    }

    #[test]
    fn test_swarm_sweep() {
        let config = SimConfig {
            swarm: true,
            ..SimConfig::chaos()
        };
        if let Some(v) = find_violation(0..40, config) {
            panic!("{v}");
        }
    }

    #[test]
    fn test_swarm_varies_fault_mix_per_seed() {
        let mixes: BTreeSet<(usize, usize, usize)> = (0..30)
            .map(|seed| {
                let f = FaultConfig::swarm(&mut SimRng::new(seed));
                (f.drop_per_mille, f.duplicate_per_mille, f.crash_per_mille)
            })
            .collect();
        assert!(mixes.len() > 5);
        assert!(
            mixes.iter().any(|&(d, _, _)| d == 0),
            "some seeds disable drops"
        );
    }

    #[test]
    fn test_forgotten_vote_is_found_and_replays_exactly() {
        let buggy = SimConfig {
            durability: Durability::ForgetVoteBuggy,
            ..SimConfig::flapping()
        };
        let violation = find_violation(0..50, buggy).expect("double voting must be found");
        assert_eq!(
            violation.invariant,
            Invariant::ElectionSafety,
            "{violation}"
        );
        assert!(
            violation
                .recent
                .iter()
                .any(|(_, e)| matches!(e, Event::Restart(_)))
        );
        assert_eq!(run(violation.seed, buggy).unwrap_err(), violation);
        assert!(run(violation.seed, SimConfig::flapping()).is_ok());
    }

    #[test]
    fn test_raft_is_safe_and_live_while_flapping() {
        if let Some(v) = find_violation(0..30, SimConfig::flapping()) {
            panic!("{v}");
        }
    }

    #[test]
    fn test_regression_seed_27_livelock_stays_fixed() {
        // Seed 27 livelocked before the election-timer fix in systems::raft.
        let report = run(27, SimConfig::chaos()).unwrap_or_else(|v| panic!("{v}"));
        assert!(report.committed > 18, "{report:?}");
    }

    #[test]
    fn test_liveness_violation_is_reported() {
        // A cluster that never receives client commands cannot commit, so the
        // liveness oracle must fire (this tests the oracle, not Raft).
        let idle = SimConfig {
            propose_per_mille: 0,
            ..SimConfig::calm()
        };
        let v = run(3, idle).unwrap_err();
        assert_eq!(v.invariant, Invariant::Liveness);
        assert!(v.to_string().contains("seed 3"));
        assert_ne!(v.recent.len(), 0);
    }

    #[test]
    fn test_oracle_catches_log_matching_and_commit_violations() {
        // Tamper with node state directly to prove each oracle check can fire.
        let mut sim = Simulation::new(0, SimConfig::calm());
        sim.nodes[0].raft.log.push(LogEntry {
            term: 1,
            data: Some(7),
        });
        sim.check_invariants().unwrap();
        sim.nodes[1].raft.log.push(LogEntry {
            term: 1,
            data: Some(8),
        });
        assert_eq!(
            sim.check_invariants().unwrap_err().invariant,
            Invariant::LogMatching
        );

        let mut sim = Simulation::new(0, SimConfig::calm());
        sim.nodes[0].raft.log.push(LogEntry {
            term: 1,
            data: Some(7),
        });
        sim.nodes[0].raft.commit_index = 1;
        sim.check_invariants().unwrap();
        sim.nodes[0].raft.commit_index = 0;
        assert_eq!(
            sim.check_invariants().unwrap_err().invariant,
            Invariant::CommitMonotonic
        );

        let mut sim = Simulation::new(0, SimConfig::calm());
        sim.nodes[0].raft.commit_index = 1;
        assert_eq!(
            sim.check_invariants().unwrap_err().invariant,
            Invariant::StateMachineSafety
        );

        let mut sim = Simulation::new(0, SimConfig::calm());
        sim.nodes[0].raft.log.push(LogEntry {
            term: 1,
            data: Some(7),
        });
        sim.nodes[0].raft.commit_index = 1;
        sim.nodes[1].raft.log.push(LogEntry {
            term: 2,
            data: Some(9),
        });
        sim.nodes[1].raft.commit_index = 1;
        assert_eq!(
            sim.check_invariants().unwrap_err().invariant,
            Invariant::StateMachineSafety
        );
    }

    #[test]
    fn test_wire_orders_by_time_then_send_order() {
        let env = |to| Envelope {
            to,
            from: 0,
            msg: Message::RequestVoteResponse {
                term: 0,
                vote_granted: true,
            },
        };
        let mut wire = BinaryHeap::new();
        wire.push(InFlight {
            at: 5,
            seq: 1,
            env: env(1),
        });
        wire.push(InFlight {
            at: 3,
            seq: 3,
            env: env(2),
        });
        wire.push(InFlight {
            at: 3,
            seq: 2,
            env: env(3),
        });
        let order: Vec<u64> = std::iter::from_fn(|| wire.pop().map(|m| m.env.to)).collect();
        assert_eq!(order, vec![3, 2, 1]);
    }
}
