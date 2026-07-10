//! # 981. Time Based Key-Value Store
//!
//! Difficulty: Medium
//! Link: <https://leetcode.com/problems/time-based-key-value-store>/
//!
//! This problem is a natural fit for Rust's `std::collections::BTreeMap` and demonstrates why iterator adapters eliminate off-by-one errors.
//! It teaches how to compose collections (`HashMap` containing `BTreeMap` or `Vec`) and how to leverage `range` queries or `partition_point` for efficient O(log N) lookups without manual binary search implementation.
//!
//! Note: design problem with two idiomatic variants (`TimeMapBTree`, `TimeMapVec`); the brute/optimized/optimal progression does not apply here.

use std::collections::{BTreeMap, HashMap};

/// Approach 1: `BTreeMap` (Straightforward & Idiomatic)
///
/// Time Complexity:
///   - `set`: O(log N) for `BTreeMap` insertion.
///   - `get`: O(log N) for `BTreeMap` range query.
///
/// Space Complexity: O(K * N) where K is number of keys and N is number of timestamps.
///
/// Why this is idiomatic Rust:
/// Instead of manually implementing binary search and worrying about off-by-one errors
/// or loop conditions (as common in C++/Java), we leverage `BTreeMap::range`.
/// The double-ended iterator allows us to simply call `.next_back()` to get the largest
/// timestamp less than or equal to our target.
#[derive(Default)]
pub struct TimeMapBTree {
    // RUST INSIGHT: Composing collections is safe and ergonomic.
    // The outer HashMap gives O(1) key lookup, and inner BTreeMap gives O(log N) time lookup.
    store: HashMap<String, BTreeMap<i32, String>>,
}

impl TimeMapBTree {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set(&mut self, key: String, value: String, timestamp: i32) {
        // RUST INSIGHT: `entry` API prevents double-lookups.
        // `or_default` inserts an empty BTreeMap if the key doesn't exist.
        self.store.entry(key).or_default().insert(timestamp, value);
    }

    #[must_use]
    pub fn get(&self, key: &str, timestamp: i32) -> String {
        // GOTCHA: We must handle the case where the key doesn't exist,
        // AND the case where no valid timestamp exists for the key.
        self.store
            .get(key)
            .and_then(|tree| {
                // RUST INSIGHT: `range(..=timestamp)` gets all entries up to `timestamp`.
                // `.next_back()` effectively gets the maximum key <= timestamp.
                // This completely eliminates manual binary search logic and off-by-one bugs.
                tree.range(..=timestamp).next_back()
            })
            .map(|(_, v)| v.clone())
            .unwrap_or_default()
    }
}

/// Approach 2: Vector + Binary Search (Optimized & Cache Friendly)
///
/// Time Complexity:
///   - `set`: O(1) amortized, since timestamps are strictly increasing per `LeetCode` constraints.
///   - `get`: O(log N) using binary search (`partition_point`).
///
/// Space Complexity: O(K * N)
///
/// Why prefer this over `BTreeMap`?
/// `BTreeMap` nodes are heap-allocated individually, which can cause memory fragmentation.
/// If we know timestamps arrive in strictly increasing order (as the problem states),
/// a `Vec` is much more cache-friendly and `set` becomes O(1) instead of O(log N).
#[derive(Default)]
pub struct TimeMapVec {
    store: HashMap<String, Vec<(i32, String)>>,
}

impl TimeMapVec {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set(&mut self, key: String, value: String, timestamp: i32) {
        self.store.entry(key).or_default().push((timestamp, value));
    }

    #[must_use]
    pub fn get(&self, key: &str, timestamp: i32) -> String {
        let Some(values) = self.store.get(key) else {
            return String::new();
        };

        // RUST INSIGHT: `partition_point` is an incredibly elegant binary search method.
        // It finds the index of the first element that does NOT satisfy the predicate.
        // By looking for the first timestamp > our target, we know the previous element
        // (if any) is the largest timestamp <= our target.
        let idx = values.partition_point(|&(ts, _)| ts <= timestamp);

        if idx == 0 {
            String::new()
        } else {
            // GOTCHA: Since idx is the first element > timestamp, idx - 1 is the element we want.
            values[idx - 1].1.clone()
        }
    }
}

/// Alternative approaches:
/// 1. `binary_search_by_key`: You could use `values.binary_search_by_key(&timestamp, |&(ts, _)| ts)`.
///    However, it returns `Result<usize, usize>`, which requires a `match` to handle `Ok` (exact match)
///    and `Err` (insertion point). `partition_point` expresses the "less than or equal to" intent more cleanly.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_btree_map_approach_happy_path() {
        let mut time_map = TimeMapBTree::new();
        time_map.set("foo".to_string(), "bar".to_string(), 1);
        assert_eq!(time_map.get("foo", 1), "bar");
        assert_eq!(time_map.get("foo", 3), "bar");

        time_map.set("foo".to_string(), "bar2".to_string(), 4);
        assert_eq!(time_map.get("foo", 4), "bar2");
        assert_eq!(time_map.get("foo", 5), "bar2");
    }

    #[test]
    fn test_vec_approach_happy_path() {
        let mut time_map = TimeMapVec::new();
        time_map.set("foo".to_string(), "bar".to_string(), 1);
        assert_eq!(time_map.get("foo", 1), "bar");
        assert_eq!(time_map.get("foo", 3), "bar");

        time_map.set("foo".to_string(), "bar2".to_string(), 4);
        assert_eq!(time_map.get("foo", 4), "bar2");
        assert_eq!(time_map.get("foo", 5), "bar2");
    }

    #[test]
    fn test_edge_cases() {
        let mut time_map = TimeMapVec::new();

        // 1. Getting a key that doesn't exist
        assert_eq!(time_map.get("missing", 10), "");

        // 2. Getting a timestamp earlier than the first entry
        time_map.set("foo".to_string(), "bar".to_string(), 5);
        assert_eq!(time_map.get("foo", 2), "");

        // 3. Exact match at the beginning
        assert_eq!(time_map.get("foo", 5), "bar");
    }

    #[test]
    fn test_stress_multiple_keys() {
        let mut time_map = TimeMapVec::new();

        for i in 1..=100 {
            time_map.set(format!("key{}", i % 5), format!("val{}", i), i);
        }

        assert_eq!(time_map.get("key0", 50), "val50");
        assert_eq!(time_map.get("key0", 51), "val50");
        assert_eq!(time_map.get("key0", 49), "val45"); // 45 is the largest multiple of 5 <= 49
    }
}
