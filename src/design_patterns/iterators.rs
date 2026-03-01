//! # Iterator Adapter Chains
//!
//! Replaces: **Loops** (Imperative), **Streams** (Java), **LINQ** (C#)
//!
//! Real Rust usage: `std::iter::Iterator`, `itertools`, `rayon`
//!
//! ## Why this pattern exists in Rust
//! Rust's iterators are zero-cost abstractions. Unlike Java Streams or Python generators which often involve
//! overhead (boxing, virtual calls), Rust iterators compile down to state machines that are often optimized
//! into the same machine code as a manual C-style loop.
//!
//! ## Architecture
//!
//! ```text
//! [ Data Source ] -> [ Adapter (Map) ] -> [ Adapter (Filter) ] -> [ Consumer (Collect) ]
//! ```
//!
//! **Invariants:**
//! - Lazy evaluation: nothing happens until `.next()` or `.collect()` is called.
//! - State containment: Each adapter holds its own state (index, buffer, etc.).
//! - Sizedness: Iterators are `Sized` by default, allowing stack allocation.
//!
//! ## When to use
//! - Transformations of sequences (map, filter, fold).
//! - When you want to avoid intermediate allocations (lazy evaluation).
//! - To improve readability by expressing *what* to do rather than *how* to loop.

// ============================================================================
// Pattern 1: Windowed Iterator (Sliding Window on Slice)
// ============================================================================

/// A custom iterator that yields sliding windows of a slice.
///
/// **OWNERSHIP INSIGHT:** We hold a reference `'a` to the slice, and return references `'a` to sub-slices.
/// The iterator itself does not own the data, it just borrows it.
pub struct WindowedIterator<'a, T> {
    slice: &'a [T],
    window_size: usize,
    index: usize,
}

impl<'a, T> WindowedIterator<'a, T> {
    pub fn new(slice: &'a [T], window_size: usize) -> Self {
        // Handle invalid window size gracefully (or panic, but empty iterator is safer)
        if window_size == 0 {
            return WindowedIterator {
                slice,
                window_size,
                index: slice.len(), // Done immediately
            };
        }
        WindowedIterator {
            slice,
            window_size,
            index: 0,
        }
    }
}

impl<'a, T> Iterator for WindowedIterator<'a, T> {
    // COMPILE-TIME WIN: The Item is a reference with the same lifetime as the original slice.
    // No copying occurs.
    type Item = &'a [T];

    fn next(&mut self) -> Option<Self::Item> {
        if self.index + self.window_size > self.slice.len() {
            return None;
        }

        // Zero-cost slice creation
        let window = &self.slice[self.index..self.index + self.window_size];
        self.index += 1;
        Some(window)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        if self.index + self.window_size > self.slice.len() {
            return (0, Some(0));
        }
        let remaining = self.slice.len() - self.window_size - self.index + 1;
        (remaining, Some(remaining))
    }
}

// ============================================================================
// Pattern 2: Chunked Iterator (Generic over any Iterator)
// ============================================================================

/// A custom iterator that groups elements into vectors of size `N`.
///
/// **TRADEOFF:** This requires allocation for each chunk (`Vec<T>`).
/// To avoid allocation, one would need `ArrayVec` or similar stack-based structures,
/// or yield a custom iterator-like type that borrows from the source (which is hard with `Iterator` trait).
pub struct ChunkedIterator<I>
where
    I: Iterator,
{
    iter: I,
    chunk_size: usize,
}

impl<I> ChunkedIterator<I>
where
    I: Iterator,
{
    pub fn new(iter: I, chunk_size: usize) -> Self {
        if chunk_size == 0 {
            panic!("Chunk size must be non-zero");
        }
        ChunkedIterator { iter, chunk_size }
    }
}

impl<I> Iterator for ChunkedIterator<I>
where
    I: Iterator,
{
    type Item = Vec<I::Item>;

    fn next(&mut self) -> Option<Self::Item> {
        let mut chunk = Vec::with_capacity(self.chunk_size);

        // Fill the chunk
        for _ in 0..self.chunk_size {
            match self.iter.next() {
                Some(item) => chunk.push(item),
                None => break,
            }
        }

        if chunk.is_empty() { None } else { Some(chunk) }
    }
}

// ============================================================================
// Pattern 3: Zero-Cost Extension (impl Iterator)
// ============================================================================

/// Extension trait to add our custom adapters to any slice.
pub trait SliceExt<T> {
    fn my_windows(&self, size: usize) -> WindowedIterator<'_, T>;
}

impl<T> SliceExt<T> for [T] {
    fn my_windows(&self, size: usize) -> WindowedIterator<'_, T> {
        WindowedIterator::new(self, size)
    }
}

/// Extension trait to add our custom adapters to any Iterator.
pub trait IteratorExt: Iterator {
    fn my_chunks(self, size: usize) -> ChunkedIterator<Self>
    where
        Self: Sized,
    {
        ChunkedIterator::new(self, size)
    }
}

// Blanket implementation for all Iterators
impl<I: Iterator> IteratorExt for I {}

// ============================================================================
// Pattern 4: The Collect Pattern & `FromIterator`
// ============================================================================

/// Replaces: Builder patterns for collections.
///
/// **OWNERSHIP INSIGHT:** `FromIterator` takes an iterator and consumes it
/// entirely, building a new collection. The items are moved into the collection.
#[derive(Debug, PartialEq, Eq)]
pub struct CustomList<T> {
    elements: Vec<T>,
}

impl<T> std::iter::FromIterator<T> for CustomList<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        // COMPILE-TIME WIN: By implementing FromIterator, users can use `.collect()`.
        // We can manually utilize `size_hint` to pre-allocate capacity.
        let iter = iter.into_iter();
        let (lower, _upper) = iter.size_hint();
        let mut elements = Vec::with_capacity(lower);

        for item in iter {
            elements.push(item);
        }
        CustomList { elements }
    }
}

/// A custom implementation of `.collect::<Result<_, _>>()` short-circuiting.
///
/// **OWNERSHIP INSIGHT:** This implements `FromIterator` for a `Result` of `CustomList`.
/// When an iterator yields `Result<T, E>`, it will stop at the first `Err` and return it,
/// avoiding further processing or allocation.
// We cannot implement `FromIterator<Result<T, E>> for Result<CustomList<T>, E>` directly
// because of the orphan rule (both `Result` and `FromIterator` are from `std`).
// Instead, we implement it for `CustomList<T>` to accept `Result`, or we rely on
// `Result`'s own implementation of `FromIterator` which delegates to the inner type's
// `FromIterator` implementation.
//
// Because we already implemented `FromIterator<T> for CustomList<T>`,
// `Result<CustomList<T>, E>` will AUTOMATICALLY implement `FromIterator<Result<T, E>>`
// via the standard library's blanket implementation!
//
// This is exactly how `collect::<Result<Vec<_>, _>>()` works.

// ============================================================================
// Pattern 5: Fallible Iteration
// ============================================================================

/// Replaces: Exceptions inside loops, manual error checking.
///
/// **COMPILE-TIME WIN:** The `?` operator works perfectly inside `try_fold`
/// and `try_for_each`. The iterator chain will automatically short-circuit
/// upon encountering the first `Err`, returning it immediately.
pub fn parse_and_sum(strings: &[&str]) -> Result<i32, std::num::ParseIntError> {
    strings
        .iter()
        // Here, a closure returning a Result allows us to use `try_fold`
        .try_fold(0, |acc, &s| {
            let val: i32 = s.parse()?; // Short-circuits on failure
            Ok(acc + val)
        })
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_windowed_iterator() {
        let data = [1, 2, 3, 4, 5];
        let mut windows = WindowedIterator::new(&data, 3);

        assert_eq!(windows.next(), Some(&[1, 2, 3][..]));
        assert_eq!(windows.next(), Some(&[2, 3, 4][..]));
        assert_eq!(windows.next(), Some(&[3, 4, 5][..]));
        assert_eq!(windows.next(), None);
    }

    #[test]
    fn test_windowed_iterator_edge_cases() {
        let data = [1, 2];
        let mut windows = WindowedIterator::new(&data, 3);
        assert_eq!(windows.next(), None); // Window larger than data

        let mut windows_exact = WindowedIterator::new(&data, 2);
        assert_eq!(windows_exact.next(), Some(&[1, 2][..]));
        assert_eq!(windows_exact.next(), None);
    }

    #[test]
    fn test_chunked_iterator() {
        let data = vec![1, 2, 3, 4, 5];
        let mut chunks = ChunkedIterator::new(data.into_iter(), 2);

        assert_eq!(chunks.next(), Some(vec![1, 2]));
        assert_eq!(chunks.next(), Some(vec![3, 4]));
        assert_eq!(chunks.next(), Some(vec![5])); // Partial chunk
        assert_eq!(chunks.next(), None);
    }

    #[test]
    fn test_extension_methods() {
        let data = [1, 2, 3, 4, 5];

        // Test slice extension
        let windows: Vec<&[i32]> = data.my_windows(2).collect();
        assert_eq!(windows.len(), 4);
        assert_eq!(windows[0], &[1, 2]);

        // Test iterator extension
        let chunks: Vec<Vec<i32>> = data.iter().cloned().my_chunks(3).collect();
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0], vec![1, 2, 3]);
        assert_eq!(chunks[1], vec![4, 5]);
    }

    #[test]
    fn test_zero_cost_property() {
        // This test is more conceptual - verifying we can use it in a loop without easy-to-spot allocations
        let data = [10, 20, 30, 40];
        let mut sum = 0;
        for window in data.my_windows(2) {
            sum += window[0] + window[1];
        }
        // (10+20) + (20+30) + (30+40) = 30 + 50 + 70 = 150
        assert_eq!(sum, 150);
    }

    #[test]
    fn test_collect_pattern() {
        let nums = vec![1, 2, 3];
        // The turbofish `::<>` syntax is required because `.collect()` can build any type
        // that implements `FromIterator`.
        let custom: CustomList<i32> = nums.into_iter().collect();
        assert_eq!(
            custom,
            CustomList {
                elements: vec![1, 2, 3]
            }
        );
    }

    #[test]
    fn test_fallible_iteration() {
        let valid = vec!["1", "2", "3"];
        let invalid = vec!["1", "foo", "3"];

        // Returns Ok(6) since all parse successfully
        assert_eq!(parse_and_sum(&valid), Ok(6));

        // Returns Err(...) as it short-circuits on "foo"
        assert!(parse_and_sum(&invalid).is_err());

        // Bonus: Collect can also handle results and transpose them automatically
        let results: Result<Vec<i32>, _> = invalid.iter().map(|s| s.parse::<i32>()).collect();
        assert!(results.is_err());

        // Custom Collect Pattern short-circuiting test
        let custom_results: Result<CustomList<i32>, _> =
            invalid.iter().map(|s| s.parse::<i32>()).collect();
        assert!(custom_results.is_err());

        let custom_valid: Result<CustomList<i32>, _> =
            valid.iter().map(|s| s.parse::<i32>()).collect();
        assert_eq!(custom_valid.unwrap().elements, vec![1, 2, 3]);
    }
}
