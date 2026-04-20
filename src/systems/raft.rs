//! # Raft Consensus Algorithm (Functional Core)
//!
//! Implements a deterministic state machine for the Raft consensus algorithm.
//! It isolates the core logic from networking and storage, making it fully testable.
//!
//! **Replaces Crates:** `raft` (TiKV), `async-raft`, `openraft`
//!
//! **Real-world Usage:**
//! - Distributed key-value stores (etcd, TiKV, Consul).
//! - Database replication and leader election (CockroachDB).
//! - Message queues and event streaming (Kafka/KRaft).
//!
//! **Why build it yourself?**
//! Raft is the industry standard for distributed consensus. Implementing the core state machine
//! teaches you how to model complex distributed state deterministically. By separating the consensus
//! logic from the network IO, you learn how to build highly testable, "pure" system components.

use std::cmp;
use std::collections::{HashMap, HashSet};

// =========================================================================================
// Architecture
// =========================================================================================
//
// Data Structure:
//
//       [ Incoming Messages / Ticks ]
//                 │
//                 ▼
//        ┌─────────────────┐
//        │   Raft Node     │──► [ Outgoing Messages ]
//        │ (State Machine) │
//        └─────────────────┘
//                 │
//                 ▼
//          [ State Changes ]
//       (Log Append, Commits)
//
// Invariants:
// - Election Safety: At most one leader can be elected in a given term.
// - Leader Append-Only: A leader never overwrites or deletes entries in its log.
// - Log Matching: If two logs contain an entry with the same index and term, the logs are identical up to that point.
// - Leader Completeness: If a log entry is committed, it will be present in the logs of all future leaders.
// - State Machine Safety: If a server has applied a log entry at a given index to its state machine, no other server will ever apply a different log entry for the same index.
//
// Complexity:
// - Step (Message Processing): O(E) where E is the number of entries in `AppendEntries`.
// - Tick (Time Progression): O(N) where N is peer count (for generating RequestVote messages).
//
// Design Decisions:
// - Functional Core, Imperative Shell: The node does not do IO. It takes `msg` and `tick` inputs and produces a queue of `messages` to be sent by the caller.

#[derive(Debug, Clone, PartialEq)]
pub struct LogEntry<T> {
    pub term: u64,
    pub data: Option<T>, // Option allows us to have dummy entries or no-op commands
}

#[derive(Debug, Clone, PartialEq)]
pub enum Message<T> {
    RequestVote {
        term: u64,
        candidate_id: u64,
        last_log_index: usize,
        last_log_term: u64,
    },
    RequestVoteResponse {
        term: u64,
        vote_granted: bool,
    },
    AppendEntries {
        term: u64,
        leader_id: u64,
        prev_log_index: usize,
        prev_log_term: u64,
        entries: Vec<LogEntry<T>>,
        leader_commit: usize,
    },
    AppendEntriesResponse {
        term: u64,
        success: bool,
        match_index: usize,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Envelope<T> {
    pub to: u64,
    pub from: u64,
    pub msg: Message<T>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Follower,
    Candidate,
    Leader,
}

pub struct RaftNode<T> {
    pub id: u64,
    pub peers: Vec<u64>,

    // Persistent state on all servers
    pub current_term: u64,
    pub voted_for: Option<u64>,
    pub log: Vec<LogEntry<T>>, // log[0] is a dummy entry to simplify 1-based indexing

    // Volatile state on all servers
    pub commit_index: usize,
    pub last_applied: usize,

    // Volatile state on leaders
    pub next_index: HashMap<u64, usize>,
    pub match_index: HashMap<u64, usize>,

    // Volatile state on candidates
    pub votes_received: HashSet<u64>,

    // Role
    pub role: Role,

    // Output buffer for messages to be sent over the network
    pub messages: Vec<Envelope<T>>,

    // Timers
    pub election_elapsed: usize,
    pub heartbeat_elapsed: usize,

    // Configuration
    pub election_timeout: usize,
    pub heartbeat_timeout: usize,
}

pub trait Consensus<T> {
    fn step(&mut self, env: Envelope<T>);
    fn tick(&mut self);
}

impl<T: Clone> Consensus<T> for RaftNode<T> {
    fn tick(&mut self) {
        self.tick_internal();
    }

    fn step(&mut self, env: Envelope<T>) {
        self.step_internal(env);
    }
}

impl<T: Clone> RaftNode<T> {
    /// Creates a new Raft node.
    pub fn new(
        id: u64,
        peers: Vec<u64>,
        election_timeout: usize,
        heartbeat_timeout: usize,
    ) -> Self {
        // PRODUCTION NOTE: In a real implementation, persistent state (current_term, voted_for, log)
        // would be loaded from stable storage (e.g., WAL) here.
        Self {
            id,
            peers,
            current_term: 0,
            voted_for: None,
            log: vec![LogEntry {
                term: 0,
                data: None,
            }], // Dummy entry
            commit_index: 0,
            last_applied: 0,
            next_index: HashMap::new(),
            match_index: HashMap::new(),
            votes_received: HashSet::new(),
            role: Role::Follower,
            messages: Vec::new(),
            election_elapsed: 0,
            heartbeat_elapsed: 0,
            election_timeout,
            heartbeat_timeout,
        }
    }
}

impl<T: Clone> RaftNode<T> {
    /// Progresses the logical clock by one tick.
    fn tick_internal(&mut self) {
        match self.role {
            Role::Follower | Role::Candidate => {
                self.election_elapsed += 1;
                if self.election_elapsed >= self.election_timeout {
                    self.election_elapsed = 0;
                    self.become_candidate();
                }
            }
            Role::Leader => {
                self.heartbeat_elapsed += 1;
                if self.heartbeat_elapsed >= self.heartbeat_timeout {
                    self.heartbeat_elapsed = 0;
                    self.bcast_append_entries();
                }
            }
        }
    }

    /// Steps the state machine with an incoming message.
    fn step_internal(&mut self, env: Envelope<T>) {
        let from = env.from;

        // Extract term from message to check if we need to step down
        let msg_term = match &env.msg {
            Message::RequestVote { term, .. } => *term,
            Message::RequestVoteResponse { term, .. } => *term,
            Message::AppendEntries { term, .. } => *term,
            Message::AppendEntriesResponse { term, .. } => *term,
        };

        // RUST INSIGHT: We handle the common term-checking logic up front.
        // If the message has a higher term, we always become a follower.
        if msg_term > self.current_term {
            self.current_term = msg_term;
            self.voted_for = None;
            self.become_follower();
        }

        match env.msg {
            Message::RequestVote {
                term,
                candidate_id,
                last_log_index,
                last_log_term,
            } => {
                self.handle_request_vote(from, term, candidate_id, last_log_index, last_log_term);
            }
            Message::RequestVoteResponse { term, vote_granted } => {
                if term == self.current_term && self.role == Role::Candidate {
                    self.handle_request_vote_response(from, vote_granted);
                }
            }
            Message::AppendEntries {
                term,
                leader_id,
                prev_log_index,
                prev_log_term,
                entries,
                leader_commit,
            } => {
                self.handle_append_entries(
                    from,
                    term,
                    leader_id,
                    prev_log_index,
                    prev_log_term,
                    entries,
                    leader_commit,
                );
            }
            Message::AppendEntriesResponse {
                term,
                success,
                match_index,
            } => {
                if term == self.current_term && self.role == Role::Leader {
                    self.handle_append_entries_response(from, success, match_index);
                }
            }
        }
    }

    fn become_follower(&mut self) {
        self.role = Role::Follower;
        self.votes_received.clear();
        self.election_elapsed = 0;
    }

    fn become_candidate(&mut self) {
        self.role = Role::Candidate;
        self.current_term += 1;
        self.voted_for = Some(self.id);
        self.votes_received.clear();
        self.votes_received.insert(self.id);
        self.election_elapsed = 0;

        if self.votes_received.len() >= (self.peers.len() + 1) / 2 + 1 {
            self.become_leader();
            return;
        }

        let last_log_index = self.log.len() - 1;
        let last_log_term = self.log[last_log_index].term;

        for peer in self.peers.clone() {
            self.send(
                peer,
                Message::RequestVote {
                    term: self.current_term,
                    candidate_id: self.id,
                    last_log_index,
                    last_log_term,
                },
            );
        }
    }

    fn become_leader(&mut self) {
        self.role = Role::Leader;
        let last_log_index = self.log.len() - 1;

        for peer in self.peers.clone() {
            self.next_index.insert(peer, last_log_index + 1);
            self.match_index.insert(peer, 0);
        }

        // Send initial heartbeat
        self.bcast_append_entries();
    }

    fn handle_request_vote(
        &mut self,
        from: u64,
        term: u64,
        _candidate_id: u64,
        last_log_index: usize,
        last_log_term: u64,
    ) {
        let mut vote_granted = false;

        // Reply false if term < currentTerm
        if term >= self.current_term {
            let my_last_log_index = self.log.len() - 1;
            let my_last_log_term = self.log[my_last_log_index].term;

            // GOTCHA: Raft determines which of two logs is more up-to-date by comparing the index and term of the last entries in the logs.
            let is_up_to_date = last_log_term > my_last_log_term
                || (last_log_term == my_last_log_term && last_log_index >= my_last_log_index);

            let can_vote = match self.voted_for {
                None => true,
                Some(id) if id == from => true,
                _ => false,
            };

            if can_vote && is_up_to_date {
                vote_granted = true;
                self.voted_for = Some(from);
                self.election_elapsed = 0; // Reset election timer if vote granted
            }
        }

        self.send(
            from,
            Message::RequestVoteResponse {
                term: self.current_term,
                vote_granted,
            },
        );
    }

    fn handle_request_vote_response(&mut self, from: u64, vote_granted: bool) {
        if vote_granted {
            self.votes_received.insert(from);
            // Majority requires self + strictly greater than half peers
            let majority = (self.peers.len() + 1) / 2 + 1;
            if self.votes_received.len() >= majority {
                self.become_leader();
            }
        }
    }

    fn handle_append_entries(
        &mut self,
        from: u64,
        term: u64,
        _leader_id: u64,
        prev_log_index: usize,
        prev_log_term: u64,
        entries: Vec<LogEntry<T>>,
        leader_commit: usize,
    ) {
        // If we are a candidate and we receive an AppendEntries with a term >= current_term,
        // we must step down to a follower. The > case is handled in step(), but == is handled here.
        if term == self.current_term && self.role == Role::Candidate {
            self.become_follower();
        }

        if term < self.current_term {
            self.send(
                from,
                Message::AppendEntriesResponse {
                    term: self.current_term,
                    success: false,
                    match_index: 0,
                },
            );
            return;
        }

        // Reset election timer since we heard from a valid leader
        self.election_elapsed = 0;

        // Reply false if log doesn’t contain an entry at prevLogIndex whose term matches prevLogTerm
        if prev_log_index >= self.log.len() || self.log[prev_log_index].term != prev_log_term {
            self.send(
                from,
                Message::AppendEntriesResponse {
                    term: self.current_term,
                    success: false,
                    match_index: 0,
                },
            );
            return;
        }

        // If an existing entry conflicts with a new one (same index but different terms),
        // delete the existing entry and all that follow it
        let mut i = 0;
        while i < entries.len() {
            let idx = prev_log_index + 1 + i;
            if idx < self.log.len() {
                if self.log[idx].term != entries[i].term {
                    self.log.truncate(idx);
                    break;
                }
            } else {
                break;
            }
            i += 1;
        }

        // Append any new entries not already in the log
        if i < entries.len() {
            self.log.extend(entries[i..].iter().cloned());
        }

        let match_index = prev_log_index + entries.len();

        // If leaderCommit > commitIndex, set commitIndex = min(leaderCommit, index of last new entry)
        if leader_commit > self.commit_index {
            self.commit_index = cmp::min(leader_commit, match_index);
        }

        self.send(
            from,
            Message::AppendEntriesResponse {
                term: self.current_term,
                success: true,
                match_index,
            },
        );
    }

    fn handle_append_entries_response(&mut self, from: u64, success: bool, match_index: usize) {
        if success {
            // Update nextIndex and matchIndex for follower
            if let Some(m) = self.match_index.get_mut(&from) {
                *m = cmp::max(*m, match_index);
            }
            if let Some(n) = self.next_index.get_mut(&from) {
                *n = match_index + 1;
            }

            // If there exists an N such that N > commitIndex, a majority
            // of matchIndex[i] >= N, and log[N].term == currentTerm:
            // set commitIndex = N
            for n in (self.commit_index + 1..self.log.len()).rev() {
                if self.log[n].term == self.current_term {
                    let mut count = 1; // Self
                    for peer in self.peers.clone() {
                        if self.match_index.get(&peer).copied().unwrap_or(0) >= n {
                            count += 1;
                        }
                    }

                    let majority = (self.peers.len() + 1) / 2 + 1;
                    if count >= majority {
                        self.commit_index = n;
                        // Once we update commit_index, broadcast AppendEntries to update followers
                        self.bcast_append_entries();
                        break;
                    }
                }
            }
        } else {
            // If AppendEntries fails because of log inconsistency: decrement nextIndex and retry
            if let Some(n) = self.next_index.get_mut(&from) {
                if *n > 1 {
                    *n -= 1;
                }
                self.send_append_entries(from);
            }
        }
    }

    fn bcast_append_entries(&mut self) {
        for peer in self.peers.clone() {
            self.send_append_entries(peer);
        }
    }

    fn send_append_entries(&mut self, peer: u64) {
        let next_idx = *self.next_index.get(&peer).unwrap_or(&1);
        let prev_log_index = next_idx - 1;
        let prev_log_term = self.log[prev_log_index].term;
        let entries = if next_idx < self.log.len() {
            self.log[next_idx..].to_vec()
        } else {
            Vec::new()
        };

        self.send(
            peer,
            Message::AppendEntries {
                term: self.current_term,
                leader_id: self.id,
                prev_log_index,
                prev_log_term,
                entries,
                leader_commit: self.commit_index,
            },
        );
    }

    fn send(&mut self, to: u64, msg: Message<T>) {
        self.messages.push(Envelope {
            to,
            from: self.id,
            msg,
        });
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Canonical Crates:
// - `raft` (from TiKV): Highly optimized, production-grade Raft in Rust.
// - `openraft`: An async, ergonomic Raft implementation.
//
// Missing vs Production:
// - Log Compaction (Snapshots): The log grows infinitely here.
// - Membership Changes: Dynamic peer addition/removal (joint consensus).
// - Stable Storage Interface: No traits provided here for persisting `term`, `vote`, `log`.
// - Read-Only Queries: No ReadIndex/LeaseRead for local linearizable reads.
//
// Next Steps:
// - Implement a `Storage` trait to read/write state durably.
// - Implement Snapshot handling (`InstallSnapshot` message).
// - Wrap this functional core in an asynchronous actor that performs real network IO.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_election_timeout_triggers_candidacy() {
        use super::Consensus;
        let mut node: RaftNode<String> = RaftNode::new(1, vec![2, 3], 10, 5);
        assert_eq!(node.role, Role::Follower);

        for _ in 0..9 {
            node.tick();
        }
        assert_eq!(node.role, Role::Follower);

        node.tick();
        assert_eq!(node.role, Role::Candidate);
        assert_eq!(node.current_term, 1);
        assert_eq!(node.voted_for, Some(1));
        assert!(node.votes_received.contains(&1));

        // Should have sent out RequestVote to peers 2 and 3
        assert_eq!(node.messages.len(), 2);
    }

    #[test]
    fn test_candidate_becomes_leader_on_majority() {
        use super::Consensus;
        let mut node: RaftNode<String> = RaftNode::new(1, vec![2, 3, 4, 5], 10, 5);
        node.become_candidate();
        node.messages.clear();

        // Needs 3 votes total (1 self + 2 others)
        node.step(Envelope {
            to: 1,
            from: 2,
            msg: Message::RequestVoteResponse {
                term: node.current_term,
                vote_granted: true,
            },
        });
        assert_eq!(node.role, Role::Candidate);

        node.step(Envelope {
            to: 1,
            from: 3,
            msg: Message::RequestVoteResponse {
                term: node.current_term,
                vote_granted: true,
            },
        });
        assert_eq!(node.role, Role::Leader);

        // Sending heartbeats
        assert_eq!(node.messages.len(), 4);
    }

    #[test]
    fn test_follower_rejects_append_entries_with_stale_term() {
        use super::Consensus;
        let mut node: RaftNode<String> = RaftNode::new(1, vec![2], 10, 5);
        node.current_term = 2;

        node.step(Envelope {
            to: 1,
            from: 2,
            msg: Message::AppendEntries {
                term: 1, // Stale
                leader_id: 2,
                prev_log_index: 0,
                prev_log_term: 0,
                entries: vec![],
                leader_commit: 0,
            },
        });

        assert_eq!(node.messages.len(), 1);
        match &node.messages[0].msg {
            Message::AppendEntriesResponse { success, .. } => {
                assert!(!success);
            }
            _ => panic!("Expected AppendEntriesResponse"),
        }
    }

    #[test]
    fn test_follower_appends_entries() {
        use super::Consensus;
        let mut node: RaftNode<String> = RaftNode::new(1, vec![2], 10, 5);

        node.step(Envelope {
            to: 1,
            from: 2,
            msg: Message::AppendEntries {
                term: 1,
                leader_id: 2,
                prev_log_index: 0,
                prev_log_term: 0,
                entries: vec![LogEntry {
                    term: 1,
                    data: Some("cmd1".to_string()),
                }],
                leader_commit: 0,
            },
        });

        assert_eq!(node.log.len(), 2);
        assert_eq!(node.log[1].data, Some("cmd1".to_string()));
    }

    #[test]
    fn test_steps_down_on_higher_term() {
        use super::Consensus;
        let mut node: RaftNode<String> = RaftNode::new(1, vec![2], 10, 5);
        node.become_candidate();
        assert_eq!(node.role, Role::Candidate);

        node.step(Envelope {
            to: 1,
            from: 2,
            msg: Message::AppendEntries {
                term: 5, // Higher term
                leader_id: 2,
                prev_log_index: 0,
                prev_log_term: 0,
                entries: vec![],
                leader_commit: 0,
            },
        });

        assert_eq!(node.role, Role::Follower);
        assert_eq!(node.current_term, 5);
    }
}
