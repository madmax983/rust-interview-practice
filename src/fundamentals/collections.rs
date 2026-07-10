//! Common collection patterns for Rust coding interviews
//!
//! This module contains frequently-used patterns for `HashMap`, `HashSet`,
//! `VecDeque`, and other standard collections.

#![allow(clippy::implicit_hasher)] // Using concrete HashMap for clarity in examples

use std::collections::{HashMap, HashSet, VecDeque};

/// Pattern: `HashMap` - counting frequency
#[must_use]
pub fn frequency_map(nums: Vec<i32>) -> HashMap<i32, usize> {
    let mut freq = HashMap::with_capacity(nums.len());
    for num in nums {
        // entry(key) gets Entry enum (occupied or vacant)
        // or_insert(default) inserts if vacant, returns &mut to value
        *freq.entry(num).or_insert(0) += 1; // Dereference to increment
    }
    freq
}

/// Pattern: `HashMap` - get with default
#[must_use]
pub fn get_or_default(map: &HashMap<i32, i32>, key: i32) -> i32 {
    *map.get(&key).unwrap_or(&0)
}

/// Pattern: `HashMap` - insert if absent
pub fn insert_if_absent(map: &mut HashMap<i32, String>, key: i32, value: String) {
    map.entry(key).or_insert(value);
}

/// Pattern: `HashMap` - update or insert
pub fn update_or_insert(map: &mut HashMap<i32, i32>, key: i32, value: i32) {
    map.entry(key)
        .and_modify(|v| *v += value) // If key exists, modify the value
        .or_insert(value); // If key doesn't exist, insert it
}

/// Pattern: `HashSet` - check membership
#[must_use]
pub fn has_duplicates(nums: Vec<i32>) -> bool {
    let mut seen = HashSet::with_capacity(nums.len());
    for num in nums {
        // insert() returns false if value was already present
        if !seen.insert(num) {
            return true; // Found a duplicate
        }
    }
    false // No duplicates found
}

/// Pattern: `HashSet` - set operations (union, intersection, difference)
#[must_use]
pub fn set_operations(nums1: Vec<i32>, nums2: Vec<i32>) -> (HashSet<i32>, HashSet<i32>) {
    let set1: HashSet<_> = nums1.iter().copied().collect();
    let set2: HashSet<_> = nums2.iter().copied().collect();

    let union: HashSet<_> = set1.union(&set2).copied().collect();
    let intersection: HashSet<_> = set1.intersection(&set2).copied().collect();

    (union, intersection)
}

/// Pattern: `VecDeque` - use as queue (FIFO)
#[must_use]
pub fn queue_example(nums: Vec<i32>) -> Vec<i32> {
    let mut queue = VecDeque::new();
    let len = nums.len();

    // Enqueue: add to back
    for num in nums {
        queue.push_back(num);
    }

    // Dequeue: remove from front (FIFO order)
    let mut result = Vec::with_capacity(len);
    while let Some(num) = queue.pop_front() {
        // pop_front() returns Option<T>
        result.push(num);
    }

    result
}

/// Pattern: `VecDeque` - use as stack (LIFO)
#[must_use]
pub fn stack_example(nums: Vec<i32>) -> Vec<i32> {
    let mut stack = VecDeque::new();
    let len = nums.len();

    // Push: add to back
    for num in nums {
        stack.push_back(num);
    }

    // Pop: remove from back (LIFO order - reversed)
    let mut result = Vec::with_capacity(len);
    while let Some(num) = stack.pop_back() {
        // pop_back() returns Option<T>
        result.push(num);
    }

    result
}

/// Pattern: `VecDeque` as a monotonic deque - sliding window maximum
///
/// Keeps a deque of *indices* whose corresponding values are in strictly
/// decreasing order, so the front is always the current window's maximum.
///
/// Time: O(n) - each index is pushed and popped at most once
/// Space: O(k) - the deque holds at most `k` indices
#[must_use]
pub fn sliding_window_max(nums: Vec<i32>, k: usize) -> Vec<i32> {
    if nums.is_empty() || k == 0 {
        return vec![];
    }

    let mut result = Vec::with_capacity(nums.len() + 1 - k.min(nums.len()));
    let mut window: VecDeque<usize> = VecDeque::new();

    for (i, &val) in nums.iter().enumerate() {
        // Drop the front index once it slides out of the window's left edge.
        if window.front().is_some_and(|&front| front + k <= i) {
            window.pop_front();
        }

        // Maintain decreasing values: pop indices whose value is <= the new one.
        while window.back().is_some_and(|&back| nums[back] <= val) {
            window.pop_back();
        }

        window.push_back(i);

        // Once the first full window is formed, the front holds its maximum.
        if let Some(&max_idx) = window.front().filter(|_| i + 1 >= k) {
            result.push(nums[max_idx]);
        }
    }

    result
}

/// Pattern: Vec - binary heap (priority queue)
#[must_use]
pub fn heap_example(nums: Vec<i32>) -> Vec<i32> {
    use std::collections::BinaryHeap;

    let mut heap = BinaryHeap::from(nums);
    let mut result = Vec::new();

    // Extract in sorted order (max heap by default)
    while let Some(num) = heap.pop() {
        result.push(num);
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frequency_map() {
        let freq = frequency_map(vec![1, 2, 2, 3, 3, 3]);
        assert_eq!(freq.get(&1), Some(&1));
        assert_eq!(freq.get(&2), Some(&2));
        assert_eq!(freq.get(&3), Some(&3));
    }

    #[test]
    fn test_get_or_default() {
        let mut map = HashMap::new();
        map.insert(1, 10);
        assert_eq!(get_or_default(&map, 1), 10);
        assert_eq!(get_or_default(&map, 2), 0);
    }

    #[test]
    fn test_insert_if_absent() {
        let mut map = HashMap::new();
        insert_if_absent(&mut map, 1, "first".to_string());
        insert_if_absent(&mut map, 1, "second".to_string());
        assert_eq!(map.get(&1), Some(&"first".to_string()));
    }

    #[test]
    fn test_update_or_insert() {
        let mut map = HashMap::new();
        update_or_insert(&mut map, 1, 5);
        assert_eq!(map.get(&1), Some(&5));
        update_or_insert(&mut map, 1, 3);
        assert_eq!(map.get(&1), Some(&8));
    }

    #[test]
    fn test_has_duplicates() {
        assert!(has_duplicates(vec![1, 2, 2, 3]));
        assert!(!has_duplicates(vec![1, 2, 3, 4]));
    }

    #[test]
    fn test_set_operations() {
        let (union, intersection) = set_operations(vec![1, 2, 3], vec![2, 3, 4]);
        assert_eq!(union.len(), 4);
        assert_eq!(intersection.len(), 2);
        assert!(intersection.contains(&2));
        assert!(intersection.contains(&3));
    }

    #[test]
    fn test_queue_example() {
        assert_eq!(queue_example(vec![1, 2, 3]), vec![1, 2, 3]);
    }

    #[test]
    fn test_stack_example() {
        assert_eq!(stack_example(vec![1, 2, 3]), vec![3, 2, 1]);
    }

    #[test]
    fn test_sliding_window_max() {
        assert_eq!(sliding_window_max(vec![1, 3, 2, 5, 4], 3), vec![3, 5, 5]);
    }

    #[test]
    fn test_sliding_window_max_leetcode_example() {
        assert_eq!(
            sliding_window_max(vec![1, 3, -1, -3, 5, 3, 6, 7], 3),
            vec![3, 3, 5, 5, 6, 7]
        );
    }

    #[test]
    fn test_sliding_window_max_edge_cases() {
        assert_eq!(sliding_window_max(vec![], 3), vec![] as Vec<i32>);
        assert_eq!(sliding_window_max(vec![1, 2, 3], 0), vec![] as Vec<i32>);
        assert_eq!(sliding_window_max(vec![4, 2, 1], 1), vec![4, 2, 1]);
        assert_eq!(sliding_window_max(vec![9, 8, 7], 3), vec![9]);
    }

    #[test]
    fn test_heap_example() {
        assert_eq!(heap_example(vec![3, 1, 4, 1, 5]), vec![5, 4, 3, 1, 1]);
    }
}
