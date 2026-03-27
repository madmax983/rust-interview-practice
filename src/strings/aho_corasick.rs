//! # Aho-Corasick Automaton
//!
//! Implements the Aho-Corasick algorithm for highly efficient multiple string searching.
//! It builds a finite state machine (a Trie with failure links) to search for multiple patterns
//! simultaneously in `O(N + M + Z)` time, where `N` is text length, `M` is the sum of pattern lengths,
//! and `Z` is the number of matches.
//!
//! **Replaces Crates:** `aho-corasick`
//!
//! **Real-world Usage:**
//! - Intrusion Detection Systems (Snort, Suricata) for matching thousands of virus signatures.
//! - Fast substring replacement (`String::replace` with multiple patterns).
//! - Text editors and IDEs (Find in Files).
//!
//! **Why build it yourself?**
//! The Aho-Corasick algorithm is a masterclass in dynamic programming applied to trees (Tries).
//! Building it from scratch teaches you how to construct deterministic finite automata (DFA),
//! how failure links act as a fast-forward mechanism to avoid backtracking, and how to safely
//! manage graph-like structures using arena-based indices (`Vec<State>`) to satisfy Rust's borrow checker.

use std::collections::{HashMap, VecDeque};

// =========================================================================================
// Architecture
// =========================================================================================
//
// Data Structure:
//
//      Trie (Prefix Tree) augmented with "Failure Links"
//
//          (0) Root
//         /   \
//      h /     \ s
//       /       \
//     (1)       (3)
//      |         |
//      | e       | h
//      |         |
//     (2)*      (4)
//      ^         |
//      |.........| e   <-- Failure Link from (5) to (2)
//                |
//               (5)*
//
// * denotes an accepting state (match found).
//
// Invariants:
// 1. **Index-based Arena**: Graph nodes are stored in a flat `Vec<State>`. Pointers are `usize` indices.
//    This prevents cyclical reference issues with `Rc<RefCell<T>>`.
// 2. **Failure Link Rule**: The failure link of a node `V` (reached by char `C` from `U`) points
//    to the node reached by taking the transition `C` from the failure link of `U`.
// 3. **Output Merging**: If a failure link points to an accepting state, the current state
//    must also accept that pattern.
//
// Complexity:
// ┌───────────────┬──────────────┬─────────────┐
// │ Operation     │ Time         │ Space       │
// ├───────────────┼──────────────┼─────────────┤
// │ Build Trie    │ O(M)         │ O(M)        │
// │ Build Links   │ O(M)         │ O(M)        │
// │ Search        │ O(N + Z)     │ O(1) aux    │
// └───────────────┴──────────────┴─────────────┘
// M = Sum of pattern lengths. N = Text length. Z = Number of matches.
//
// Design Decisions:
// - **Sparse Transitions**: We use `HashMap<u8, usize>` for state transitions.
//   - *Tradeoff*: Saves massive amounts of memory compared to `[usize; 256]` per state, but
//     adds hashing overhead to traversal.
//   - *Alternative*: Production systems often use a hybrid: dense arrays for the root and
//     first few levels (where branching is high), and sparse structures for deeper nodes.
// - **Output Collection**: We clone `Vec<usize>` for matching overlapping patterns.
//   - *Alternative*: Production crates use a linked list of matching states to avoid allocation during build.

#[derive(Default, Debug)]
struct State {
    transitions: HashMap<u8, usize>,
    fail: usize,
    output: Vec<usize>,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct Match {
    pub pattern_idx: usize,
    pub start: usize,
    pub end: usize,
}

/// A trait defining a multi-pattern searcher.
/// This allows swapping out implementations (e.g., Naive vs Aho-Corasick)
/// without changing the consumer code.
pub trait MultiPatternSearch {
    /// Searches the given text and returns all matches.
    fn search(&self, text: &[u8]) -> Vec<Match>;
}

pub struct AhoCorasick {
    // RUST INSIGHT: The Arena Pattern
    // Instead of fighting the borrow checker with `Rc<RefCell<State>>` or `unsafe` pointers,
    // we use a `Vec` as an arena allocator. "Pointers" are simply `usize` indices into this `Vec`.
    // This provides zero-cost abstractions, excellent cache locality, and safe memory management.
    states: Vec<State>,
    pattern_lengths: Vec<usize>,
}

impl AhoCorasick {
    /// Creates a new Aho-Corasick automaton from an iterator of patterns.
    pub fn new<I, P>(patterns: I) -> Self
    where
        I: IntoIterator<Item = P>,
        P: AsRef<[u8]>,
    {
        let mut ac = Self {
            states: vec![State::default()], // State 0 is the root
            pattern_lengths: Vec::new(),
        };

        for (i, pattern) in patterns.into_iter().enumerate() {
            let pat = pattern.as_ref();
            ac.add_pattern(pat, i);
            ac.pattern_lengths.push(pat.len());
        }

        ac.build_failure_links();
        ac
    }

    /// Step 1: Add patterns to construct a standard Trie.
    fn add_pattern(&mut self, pattern: &[u8], pattern_idx: usize) {
        let mut current_state = 0;
        for &byte in pattern {
            // Check if transition exists. We avoid holding a mutable reference to `self.states`
            // if we don't need to insert, satisfying the borrow checker cleanly.
            let next_state = self.states[current_state].transitions.get(&byte).copied();

            match next_state {
                Some(state) => current_state = state,
                None => {
                    let new_state_idx = self.states.len();
                    self.states.push(State::default());
                    self.states[current_state].transitions.insert(byte, new_state_idx);
                    current_state = new_state_idx;
                }
            }
        }
        // Mark the final state as an accepting state for this pattern index.
        self.states[current_state].output.push(pattern_idx);
    }

    /// Step 2: Build failure links using Breadth-First Search (BFS).
    fn build_failure_links(&mut self) {
        let mut queue = VecDeque::new();

        // 1. Root's children have fail link = 0 (root). Push them to the queue.
        // We collect them to avoid borrowing issues when pushing to the queue.
        let root_transitions: Vec<(u8, usize)> = self.states[0]
            .transitions
            .iter()
            .map(|(&k, &v)| (k, v))
            .collect();

        for (_, next_state) in root_transitions {
            self.states[next_state].fail = 0;
            queue.push_back(next_state);
        }

        // 2. Perform BFS to set failure links for deeper levels.
        while let Some(current_state) = queue.pop_front() {
            let transitions: Vec<(u8, usize)> = self.states[current_state]
                .transitions
                .iter()
                .map(|(&k, &v)| (k, v))
                .collect();

            for (byte, next_state) in transitions {
                queue.push_back(next_state);

                let mut fail_state = self.states[current_state].fail;

                // Follow failure links until we find a state that has a transition for `byte`,
                // or we reach the root.
                while fail_state != 0 && !self.states[fail_state].transitions.contains_key(&byte) {
                    fail_state = self.states[fail_state].fail;
                }

                // If a valid failure transition was found, set it. Otherwise, point to root.
                if let Some(&valid_fail_state) = self.states[fail_state].transitions.get(&byte) {
                    self.states[next_state].fail = valid_fail_state;

                    // GOTCHA: Output Merging
                    // We must merge the outputs of the fail state into the current state's outputs!
                    // This ensures overlapping patterns (e.g., "he" and "she" matching in "she")
                    // are both successfully reported when we reach the end of "she".
                    let mut fail_outputs = self.states[valid_fail_state].output.clone();
                    self.states[next_state].output.append(&mut fail_outputs);
                } else {
                    self.states[next_state].fail = 0;
                }
            }
        }
    }

}

impl MultiPatternSearch for AhoCorasick {
    /// Searches the text for all pattern matches.
    fn search(&self, text: &[u8]) -> Vec<Match> {
        let mut results = Vec::new();
        let mut current_state = 0;

        for (i, &byte) in text.iter().enumerate() {
            // Follow failure links if there is no direct transition.
            while current_state != 0 && !self.states[current_state].transitions.contains_key(&byte) {
                current_state = self.states[current_state].fail;
            }

            // Transition to the next state if possible, else reset to root.
            if let Some(&next_state) = self.states[current_state].transitions.get(&byte) {
                current_state = next_state;
            } else {
                current_state = 0;
            }

            // Record any matches associated with the current state.
            // PRODUCTION NOTE: Allocating a `Vec` for results is fine for small texts,
            // but production crates return an `Iterator` to yield matches lazily, avoiding allocations.
            for &pattern_idx in &self.states[current_state].output {
                results.push(Match {
                    pattern_idx,
                    start: i + 1 - self.pattern_lengths[pattern_idx],
                    end: i + 1,
                });
            }
        }

        results
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `aho-corasick`: The official crate uses heavily optimized DFAs, pre-multiplied state IDs,
//   and SIMD acceleration (e.g., `memchr` for fast-forwarding to the first byte of any pattern).
//   It also offers true iterator-based streaming search without allocating a `Vec` for results.
//
// Missing vs. Production:
// - **SIMD Fast-Forward**: No `memchr` usage to skip unmatching text quickly.
// - **DFA Conversion**: We built an NFA (Non-deterministic Finite Automaton) with failure links.
//   Production systems often compile this into a full DFA to avoid the `while` loop during search.
// - **Streaming Iterators**: Our `search` method allocates and returns all matches at once.
//
// Next Steps:
// 1. Implement an `Iterator` for `search` to yield matches lazily.
// 2. Replace `HashMap` with a `[usize; 256]` for a fully dense, high-speed DFA.
// 3. Integrate `memchr` to scan for starting bytes.
//
// Benchmarking Note:
// To benchmark this implementation against standard string searching or `aho-corasick`:
// 1. Use `criterion` to measure latency and throughput.
// 2. Create diverse workloads (e.g., short texts vs large corpora, few long patterns vs many short patterns).
// 3. Prevent compiler optimization using `std::hint::black_box(ac.search(black_box(text)))`.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_match() {
        let patterns = vec!["apple"];
        let ac = AhoCorasick::new(patterns);
        let text = b"I like apple pie.";
        let matches = ac.search(text);

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0], Match { pattern_idx: 0, start: 7, end: 12 });
    }

    #[test]
    fn test_multiple_non_overlapping() {
        let patterns = vec!["cat", "dog"];
        let ac = AhoCorasick::new(patterns);
        let text = b"The cat and the dog.";
        let matches = ac.search(text);

        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0], Match { pattern_idx: 0, start: 4, end: 7 });
        assert_eq!(matches[1], Match { pattern_idx: 1, start: 16, end: 19 });
    }

    #[test]
    fn test_overlapping_matches() {
        // Classic Aho-Corasick edge case: failure links and output merging
        let patterns = vec!["he", "she", "his", "hers"];
        let ac = AhoCorasick::new(patterns);
        let text = b"ushers";
        let matches = ac.search(text);

        // "ushers" contains:
        // - "she" at 1..4
        // - "he" at 2..4  <-- This must be found due to output merging!
        // - "hers" at 2..6
        assert_eq!(matches.len(), 3);
        assert!(matches.contains(&Match { pattern_idx: 1, start: 1, end: 4 })); // she
        assert!(matches.contains(&Match { pattern_idx: 0, start: 2, end: 4 })); // he
        assert!(matches.contains(&Match { pattern_idx: 3, start: 2, end: 6 })); // hers
    }

    #[test]
    fn test_no_matches() {
        let patterns = vec!["rust", "c++"];
        let ac = AhoCorasick::new(patterns);
        let text = b"Python is great.";
        let matches = ac.search(text);

        assert_eq!(matches.len(), 0);
    }

    #[test]
    fn test_empty_pattern() {
        let patterns = vec![""];
        let ac = AhoCorasick::new(patterns);
        let text = b"abc";
        let matches = ac.search(text);

        // An empty pattern conceptually matches everywhere, including before the first char.
        // Our implementation registers the root as matching the empty string.
        // It will match at every single step, including start (index 0) and each character.
        // The implementation finds matches at index 0 (before 'a'), 1, and 2.
        assert_eq!(matches.len(), 3);
        assert_eq!(matches[0], Match { pattern_idx: 0, start: 1, end: 1 });
        assert_eq!(matches[1], Match { pattern_idx: 0, start: 2, end: 2 });
        assert_eq!(matches[2], Match { pattern_idx: 0, start: 3, end: 3 });
    }

    #[test]
    fn test_identical_patterns() {
        let patterns = vec!["test", "test"];
        let ac = AhoCorasick::new(patterns);
        let text = b"this is a test.";
        let matches = ac.search(text);

        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0], Match { pattern_idx: 0, start: 10, end: 14 });
        assert_eq!(matches[1], Match { pattern_idx: 1, start: 10, end: 14 });
    }
}
