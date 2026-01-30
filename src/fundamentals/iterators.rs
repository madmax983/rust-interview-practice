//! Common iterator patterns for Rust coding interviews
//!
//! This module contains frequently-used iterator idioms that appear in
//! competitive programming and technical interviews.

#![allow(clippy::doc_markdown)] // Type names in docs are clear without backticks

/// Pattern: map + collect to transform a Vec
#[must_use]
pub fn map_collect_example(nums: Vec<i32>) -> Vec<i32> {
    nums.iter().map(|&x| x * 2).collect()
}

/// Pattern: filter + collect to select elements
#[must_use]
pub fn filter_collect_example(nums: Vec<i32>) -> Vec<i32> {
    nums.iter().filter(|&&x| x > 0).copied().collect()
}

/// Pattern: filter_map to transform and filter in one pass
#[must_use]
pub fn filter_map_example(nums: Vec<i32>) -> Vec<i32> {
    nums.iter()
        .filter_map(|&x| if x > 0 { Some(x * 2) } else { None })
        .collect()
}

/// Pattern: enumerate for index + value iteration
#[must_use]
pub fn enumerate_example(nums: Vec<i32>) -> Vec<(usize, i32)> {
    nums.iter().enumerate().map(|(i, &x)| (i, x)).collect()
}

/// Pattern: zip to iterate two collections together
#[must_use]
pub fn zip_example(nums1: Vec<i32>, nums2: Vec<i32>) -> Vec<i32> {
    nums1
        .iter()
        .zip(nums2.iter())
        .map(|(&a, &b)| a + b)
        .collect()
}

/// Pattern: chain to concatenate iterators
#[must_use]
pub fn chain_example(nums1: Vec<i32>, nums2: Vec<i32>) -> Vec<i32> {
    nums1.iter().chain(nums2.iter()).copied().collect()
}

/// Pattern: fold for custom accumulation
#[must_use]
pub fn fold_example(nums: Vec<i32>) -> i32 {
    nums.iter().fold(0, |acc, &x| acc + x)
}

/// Pattern: sum/product for simple aggregation
#[must_use]
pub fn sum_example(nums: Vec<i32>) -> i32 {
    nums.iter().sum()
}

/// Pattern: max/min for finding extremes
#[must_use]
pub fn max_example(nums: Vec<i32>) -> Option<i32> {
    nums.iter().copied().max()
}

/// Pattern: any/all for boolean checks
#[must_use]
pub fn any_all_example(nums: Vec<i32>) -> (bool, bool) {
    let has_positive = nums.iter().any(|&x| x > 0);
    let all_positive = nums.iter().all(|&x| x > 0);
    (has_positive, all_positive)
}

/// Pattern: take/skip for subsequences
#[must_use]
pub fn take_skip_example(nums: Vec<i32>) -> Vec<i32> {
    nums.iter().skip(2).take(3).copied().collect()
}

/// Pattern: windows for sliding window
#[must_use]
pub fn windows_example(nums: Vec<i32>) -> Vec<i32> {
    nums.windows(2).map(|w| w[0] + w[1]).collect()
}

/// Pattern: collect into different collection types (e.g., `HashSet`)
#[must_use]
pub fn collect_hashset_example(nums: Vec<i32>) -> std::collections::HashSet<i32> {
    nums.iter().copied().collect()
}

/// Pattern: partition to split based on predicate
#[must_use]
pub fn partition_example(nums: Vec<i32>) -> (Vec<i32>, Vec<i32>) {
    nums.iter().copied().partition(|&x| x > 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_map_collect() {
        assert_eq!(map_collect_example(vec![1, 2, 3]), vec![2, 4, 6]);
    }

    #[test]
    fn test_filter_collect() {
        assert_eq!(filter_collect_example(vec![-1, 2, -3, 4]), vec![2, 4]);
    }

    #[test]
    fn test_filter_map() {
        assert_eq!(filter_map_example(vec![-1, 2, -3, 4]), vec![4, 8]);
    }

    #[test]
    fn test_enumerate() {
        assert_eq!(
            enumerate_example(vec![10, 20, 30]),
            vec![(0, 10), (1, 20), (2, 30)]
        );
    }

    #[test]
    fn test_zip() {
        assert_eq!(zip_example(vec![1, 2, 3], vec![4, 5, 6]), vec![5, 7, 9]);
    }

    #[test]
    fn test_chain() {
        assert_eq!(chain_example(vec![1, 2], vec![3, 4]), vec![1, 2, 3, 4]);
    }

    #[test]
    fn test_fold() {
        assert_eq!(fold_example(vec![1, 2, 3, 4]), 10);
    }

    #[test]
    fn test_sum() {
        assert_eq!(sum_example(vec![1, 2, 3, 4]), 10);
    }

    #[test]
    fn test_max() {
        assert_eq!(max_example(vec![1, 5, 3, 2]), Some(5));
        assert_eq!(max_example(vec![]), None);
    }

    #[test]
    fn test_any_all() {
        assert_eq!(any_all_example(vec![1, 2, 3]), (true, true));
        assert_eq!(any_all_example(vec![-1, 2, 3]), (true, false));
        assert_eq!(any_all_example(vec![-1, -2]), (false, false));
    }

    #[test]
    fn test_take_skip() {
        assert_eq!(take_skip_example(vec![1, 2, 3, 4, 5]), vec![3, 4, 5]);
    }

    #[test]
    fn test_windows() {
        assert_eq!(windows_example(vec![1, 2, 3, 4]), vec![3, 5, 7]);
    }

    #[test]
    fn test_collect_hashset() {
        let result = collect_hashset_example(vec![1, 2, 2, 3]);
        assert_eq!(result.len(), 3);
        assert!(result.contains(&1));
        assert!(result.contains(&2));
        assert!(result.contains(&3));
    }

    #[test]
    fn test_partition() {
        let (pos, neg) = partition_example(vec![1, -2, 3, -4]);
        assert_eq!(pos, vec![1, 3]);
        assert_eq!(neg, vec![-2, -4]);
    }
}
