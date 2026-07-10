//! Borrow checker patterns for Rust coding interviews
//!
//! This module contains common borrowing patterns that trip people up
//! when they don't have IDE assistance.

#![allow(clippy::doc_markdown)] // Type names in docs are clear without backticks

/// Pattern: Immutable borrow - multiple readers allowed
#[must_use]
pub fn immutable_borrow_example(nums: &[i32]) -> i32 {
    let first = &nums[0]; // First immutable borrow
    let second = &nums[1]; // Second immutable borrow - OK!
    first + second // Multiple immutable borrows can coexist
}

/// Pattern: Mutable borrow - exclusive access required
pub fn mutable_borrow_example(nums: &mut Vec<i32>) {
    // Only ONE mutable borrow allowed at a time
    nums.push(1); // Mutable borrow starts and ends here
    nums.push(2); // New mutable borrow - OK because previous ended
}

/// Pattern: Cannot borrow as mutable while immutable borrow exists
/// This shows the CORRECT way - immutable borrow ends before mutable
pub fn borrow_scope_example(nums: &mut Vec<i32>) -> i32 {
    let len = nums.len(); // Immutable borrow (for len())
    // Immutable borrow ends here (len is Copy)

    nums.push(42); // Mutable borrow - OK, no active immutable borrows

    len as i32 // Using len (no borrow involved, it's a usize value)
}

/// Pattern: Splitting borrows - borrow different parts simultaneously
pub fn split_borrow_example(nums: &mut [i32]) {
    // Can't have two mutable borrows to same array...
    // But CAN split it into non-overlapping parts!
    let (left, right) = nums.split_at_mut(nums.len() / 2);

    left[0] = 1; // Mutable access to left half
    right[0] = 2; // Mutable access to right half - OK!
}

/// Pattern: Reborrowing - creating a new borrow from existing one
pub fn reborrow_example(nums: &mut Vec<i32>) {
    helper_function(&mut *nums); // &mut *nums creates a new reborrow
    nums.push(1); // Original borrow still valid after helper returns
}

const fn helper_function(_nums: &mut Vec<i32>) {
    // Does something with the reborrow
}

/// Pattern: Lifetime basics - return reference tied to input lifetime
/// The 'a says: returned reference lives as long as input reference
#[must_use]
pub fn return_reference(nums: &[i32]) -> &i32 {
    &nums[0] // Returned reference borrows from nums
}

/// Pattern: Multiple lifetimes - when inputs have different lifetimes
/// Return type must pick one of the input lifetimes
#[must_use]
pub const fn longest<'a>(s1: &'a str, s2: &'a str) -> &'a str {
    // Both inputs must live at least as long as 'a
    if s1.len() > s2.len() {
        s1 // Could return either - both have lifetime 'a
    } else {
        s2
    }
}

/// Pattern: Lifetime elision - compiler infers lifetimes
/// This function has implicit lifetime annotations
#[must_use]
pub fn first_word(s: &str) -> &str {
    // Compiler infers: fn first_word<'a>(s: &'a str) -> &'a str
    s.split_whitespace().next().unwrap_or("")
}

/// Pattern: Clone to avoid borrow issues
#[must_use]
pub fn clone_to_avoid_borrow(nums: &[i32]) -> Vec<i32> {
    let mut result = nums.to_vec(); // Clone the slice to owned Vec
    result.push(1); // Now we own it, can mutate freely
    result
}

/// Pattern: Copy types don't have borrow issues
#[must_use]
pub const fn copy_types_example(x: i32) -> i32 {
    let y = x; // x is Copy, so it's copied not moved
    let z = x; // Can use x again - it wasn't moved!
    y + z + x // All three are valid
}

/// Pattern: Taking ownership vs borrowing
#[must_use]
pub fn take_ownership(mut nums: Vec<i32>) -> Vec<i32> {
    // Function takes ownership of nums
    nums.push(1); // Can mutate because we own it
    nums // Return ownership to caller
}

/// Pattern: Borrowing instead of taking ownership
pub fn borrow_instead(nums: &mut Vec<i32>) {
    // Function borrows nums mutably
    nums.push(1); // Can mutate the borrowed value
    // Ownership stays with caller
}

/// Pattern: Iterator borrowing - iter() vs into_iter()
#[must_use]
pub fn iter_vs_into_iter(nums: Vec<i32>) -> (Vec<i32>, i32) {
    // iter() borrows, into_iter() takes ownership

    let sum: i32 = nums.iter().sum(); // Borrows each element as &i32
    // nums is still valid here!

    (nums, sum) // Can return nums because we only borrowed it
}

/// Pattern: Dereferencing with *
pub const fn deref_example(x: &mut i32) {
    *x += 1; // Dereference to access/modify the value
    // x is &mut i32, *x is i32
}

/// Pattern: Reference in struct requires lifetime
pub struct Borrowed<'a> {
    data: &'a [i32], // Reference in struct needs lifetime annotation
}

impl<'a> Borrowed<'a> {
    /// Create a new Borrowed that holds a reference
    #[must_use]
    pub const fn new(data: &'a [i32]) -> Self {
        Self { data } // Lifetime ensures data outlives this struct
    }

    /// Access the borrowed data
    #[must_use]
    pub fn get(&self, index: usize) -> Option<i32> {
        self.data.get(index).copied() // Get returns &i32, copied makes it i32
    }
}

/// Pattern: Returning owned data to avoid lifetime issues
#[must_use]
pub fn return_owned(nums: &[i32]) -> Vec<i32> {
    // Instead of returning &[i32] (borrow), return Vec (owned)
    nums.iter().map(|&x| x * 2).collect()
}

/// Pattern: as_ref() to convert owned to borrowed
#[must_use]
pub fn as_ref_example(opt: &Option<String>) -> Option<&str> {
    // Option<String> -> Option<&str>
    opt.as_ref().map(std::string::String::as_str) // as_ref() converts &Option<T> to Option<&T>
}

/// Pattern: Entry API avoids double borrow
pub fn entry_api_no_double_borrow(map: &mut std::collections::HashMap<i32, Vec<i32>>, key: i32) {
    // WRONG: if map.contains_key(&key) { map.get_mut(&key).push(1); }
    // (borrows twice - once for contains_key, once for get_mut)

    // RIGHT: Use entry API
    map.entry(key).or_default().push(1); // Single borrow
}

/// Pattern: NLL (Non-Lexical Lifetimes) - borrow ends at last use
pub fn nll_example(nums: &mut Vec<i32>) -> i32 {
    let len = nums.len(); // Immutable borrow

    // In old Rust, immutable borrow would last until end of scope
    // With NLL, it ends here (last use of len is next line)

    nums.push(1); // Mutable borrow OK - immutable borrow already ended

    len as i32
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_immutable_borrow() {
        let nums = vec![1, 2, 3];
        assert_eq!(immutable_borrow_example(&nums), 3);
    }

    #[test]
    fn test_mutable_borrow() {
        let mut nums = vec![];
        mutable_borrow_example(&mut nums);
        assert_eq!(nums, vec![1, 2]);
    }

    #[test]
    fn test_borrow_scope() {
        let mut nums = vec![1, 2, 3];
        let result = borrow_scope_example(&mut nums);
        assert_eq!(result, 3);
        assert_eq!(nums.len(), 4);
    }

    #[test]
    fn test_split_borrow() {
        let mut nums = vec![0, 0, 0, 0];
        split_borrow_example(&mut nums);
        assert_eq!(nums[0], 1);
        assert_eq!(nums[2], 2);
    }

    #[test]
    fn test_reborrow() {
        let mut nums = vec![];
        reborrow_example(&mut nums);
        assert_eq!(nums.len(), 1);
    }

    #[test]
    fn test_return_reference() {
        let nums = vec![42, 43];
        assert_eq!(*return_reference(&nums), 42);
    }

    #[test]
    fn test_longest() {
        assert_eq!(longest("hello", "hi"), "hello");
        assert_eq!(longest("hi", "hello"), "hello");
    }

    #[test]
    fn test_first_word() {
        assert_eq!(first_word("hello world"), "hello");
        assert_eq!(first_word(""), "");
    }

    #[test]
    fn test_clone_to_avoid_borrow() {
        let nums = vec![1, 2, 3];
        let result = clone_to_avoid_borrow(&nums);
        assert_eq!(result, vec![1, 2, 3, 1]);
    }

    #[test]
    fn test_copy_types() {
        assert_eq!(copy_types_example(5), 15);
    }

    #[test]
    fn test_take_ownership() {
        let nums = vec![1, 2, 3];
        let result = take_ownership(nums);
        assert_eq!(result, vec![1, 2, 3, 1]);
    }

    #[test]
    fn test_borrow_instead() {
        let mut nums = vec![1, 2, 3];
        borrow_instead(&mut nums);
        assert_eq!(nums, vec![1, 2, 3, 1]);
    }

    #[test]
    fn test_iter_vs_into_iter() {
        let nums = vec![1, 2, 3];
        let (returned, sum) = iter_vs_into_iter(nums);
        assert_eq!(sum, 6);
        assert_eq!(returned, vec![1, 2, 3]);
    }

    #[test]
    fn test_deref() {
        let mut x = 5;
        deref_example(&mut x);
        assert_eq!(x, 6);
    }

    #[test]
    fn test_borrowed_struct() {
        let data = vec![1, 2, 3];
        let borrowed = Borrowed::new(&data);
        assert_eq!(borrowed.get(0), Some(1));
        assert_eq!(borrowed.get(10), None);
    }

    #[test]
    fn test_return_owned() {
        let nums = vec![1, 2, 3];
        assert_eq!(return_owned(&nums), vec![2, 4, 6]);
    }

    #[test]
    fn test_as_ref() {
        let opt = Some("hello".to_string());
        assert_eq!(as_ref_example(&opt), Some("hello"));
        let none: Option<String> = None;
        assert_eq!(as_ref_example(&none), None);
    }

    #[test]
    fn test_entry_api() {
        let mut map = HashMap::new();
        entry_api_no_double_borrow(&mut map, 1);
        assert_eq!(map.get(&1), Some(&vec![1]));
    }

    #[test]
    fn test_nll() {
        let mut nums = vec![1, 2, 3];
        let result = nll_example(&mut nums);
        assert_eq!(result, 3);
        assert_eq!(nums.len(), 4);
    }
}
