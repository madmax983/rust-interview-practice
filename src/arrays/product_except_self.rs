//! # 238. Product of Array Except Self
//!
//! Given an integer array `nums`, return an array `answer` such that `answer[i]` is equal to
//! the product of all the elements of `nums` except `nums[i]`.
//!
//! The product of any prefix or suffix of `nums` is guaranteed to fit in a 32-bit integer.
//!
//! You must write an algorithm that runs in O(n) time and without using the division operation.
//!
//! ## Why This Matters in Rust
//!
//! This problem is excellent for demonstrating the difference between **functional** (iterator-based)
//! and **imperative** (index-based) styles in Rust.
//!
//! 1.  **Iterators**: The "prefix/suffix" pattern maps naturally to `Iterator::scan`, allowing us to
//!     express the logic as a stream of transformations. This is readable and safe but may require
//!     extra allocation for intermediate steps.
//! 2.  **Memory Layout**: The O(1) space constraint forces us to think about how we can reuse the
//!     output `Vec` as a buffer for intermediate results, carefuly managing mutation steps to avoid
//!     reading what we just overwrote (or ensuring the overwrite is valid).
//! 3.  **Safety**: Rust's bounds checking protects us from the common off-by-one errors in the
//!     imperative approach, while iterators eliminate bounds checks entirely.

/// Brute force approach: Functional style using `scan` and `zip`.
///
/// This approach prioritizes readability and "declarative" logic. We construct the prefix products
/// and suffix products as separate streams and multiply them. It is the "brute force" of the two
/// implementations only in the sense that it uses more auxiliary space; the technique is the
/// functional/iterator (`scan` + `zip`) style.
///
/// Time: O(n) - two linear scans plus a zip pass.
/// Space: O(n) - the `suffix` vector requires separate allocation (excluding the output).
///
/// # Arguments
///
/// * `nums` - A vector of integers.
///
/// # Returns
///
/// * A vector where the element at index `i` is the product of all elements in `nums` except `nums[i]`.
#[must_use]
pub fn product_except_self_brute_force(nums: &[i32]) -> Vec<i32> {
    // RUST INSIGHT: `scan` maintains internal state during iteration.
    // We use it here to compute running products.
    // The closure returns the value *before* the update to get exclusive prefix products.
    // Example: [1, 2, 3, 4] -> init state 1
    // 1. state=1, x=1 -> yield 1, state becomes 1*1=1
    // 2. state=1, x=2 -> yield 1, state becomes 1*2=2
    // 3. state=2, x=3 -> yield 2, state becomes 2*3=6
    // 4. state=6, x=4 -> yield 6, state becomes 6*4=24
    // Result: [1, 1, 2, 6]
    let prefix_products = nums.iter().scan(1, |state, &x| {
        let val = *state;
        *state *= x;
        Some(val)
    });

    // For suffixes, we iterate in reverse.
    // We collect into a Vec because `scan` is not a DoubleEndedIterator,
    // so we can't reverse the result iterator directly without buffering.
    // GOTCHA: `rev()` on the input slice works, but we must reverse the result
    // to match indices with `prefix_products`.
    let mut suffix_products: Vec<i32> = nums
        .iter()
        .rev()
        .scan(1, |state, &x| {
            let val = *state;
            *state *= x;
            Some(val)
        })
        .collect();

    // We must reverse the suffixes to align with the original indices.
    // Original: [1, 2, 3, 4]
    // Rev Scan produces (from 4 backwards): [1, 4, 12, 24]
    // Reversed: [24, 12, 4, 1]
    suffix_products.reverse();

    // RUST INSIGHT: `zip` combines two iterators into pairs.
    // If they were of different lengths, it would stop at the shorter one.
    // Here, we know they are equal length.
    prefix_products
        .zip(suffix_products)
        .map(|(prefix, suffix)| prefix * suffix)
        .collect()
}

/// Optimal approach: Imperative prefix/suffix products with O(1) extra space.
///
/// We reuse the result vector to store the prefix products first, then multiply by the suffix
/// products on the fly in a second reverse pass.
///
/// Time: O(n) - two linear passes.
/// Space: O(1) - only a single running-product accumulator (ignoring the output array).
#[must_use]
pub fn product_except_self_optimal(nums: &[i32]) -> Vec<i32> {
    let n = nums.len();
    if n == 0 {
        return vec![];
    }

    // Initialize result vector with 1s.
    // This will hold our prefix products initially.
    let mut result = vec![1; n];

    // Pass 1: Calculate prefix products
    // result[i] should contain product of nums[0..i]
    // We start from 1 because result[0] has no prefix (product is 1).
    for i in 1..n {
        // RUST INSIGHT: Direct indexing `[i]` and `[i-1]` is safe here due to the loop range,
        // but the compiler inserts bounds checks. Iterators would elide them.
        result[i] = result[i - 1] * nums[i - 1];
    }

    // Pass 2: Calculate suffix products and multiply into result
    // We track the running suffix product in a variable `right_product`.
    let mut right_product = 1;
    for i in (0..n).rev() {
        // Multiply the prefix product (already in result[i]) by the current suffix product
        result[i] *= right_product;

        // Update the suffix product for the next iteration (which moves left)
        right_product *= nums[i];
    }

    result
}

/// Main entry point.
#[must_use]
#[allow(clippy::needless_pass_by_value)] // To match LeetCode signature if needed, though we use &[i32]
pub fn product_except_self(nums: Vec<i32>) -> Vec<i32> {
    product_except_self_optimal(&nums)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_example() {
        let input = vec![1, 2, 3, 4];
        let expected = vec![24, 12, 8, 6];

        assert_eq!(product_except_self_brute_force(&input), expected);
        assert_eq!(product_except_self_optimal(&input), expected);
    }

    #[test]
    fn test_with_zeros() {
        // If there's one zero, everything else is 0 except the position of the zero.
        let input = vec![-1, 1, 0, -3, 3];
        let expected = vec![0, 0, 9, 0, 0];

        assert_eq!(product_except_self_brute_force(&input), expected);
        assert_eq!(product_except_self_optimal(&input), expected);
    }

    #[test]
    fn test_multiple_zeros() {
        // If there are multiple zeros, the product of "everything except self" will always include at least one zero.
        // So all results should be 0.
        let input = vec![0, 1, 2, 0, 4];
        let expected = vec![0, 0, 0, 0, 0];

        assert_eq!(product_except_self_brute_force(&input), expected);
        assert_eq!(product_except_self_optimal(&input), expected);
    }

    #[test]
    fn test_empty() {
        let input = vec![];
        let expected: Vec<i32> = vec![];

        assert_eq!(product_except_self_brute_force(&input), expected);
        assert_eq!(product_except_self_optimal(&input), expected);
    }

    #[test]
    fn test_single_element() {
        // Constraints say nums.length >= 2, but good to handle safely.
        // Product of everything except self for a single element is 1 (identity).
        let input = vec![5];
        // scan(1, ...) -> yield 1. Suffix -> yield 1. Result 1*1 = 1.
        let expected = vec![1];

        assert_eq!(product_except_self_brute_force(&input), expected);

        // For optimal:
        // Pass 1: 1..n is empty range. result = [1].
        // Pass 2: i=0. result[0] *= 1 -> 1. right_product *= 5.
        // Returns [1].
        assert_eq!(product_except_self_optimal(&input), expected);
    }

    #[test]
    fn test_main_entry_point() {
        assert_eq!(product_except_self(vec![1, 2, 3, 4]), vec![24, 12, 8, 6]);
    }

    #[test]
    fn test_all_approaches_agreement() {
        let inputs = vec![
            vec![1, 2, 3, 4],
            vec![-1, 1, 0, -3, 3],
            vec![0, 1, 2, 0, 4],
            vec![2, 3, 5, 7, 11],
            vec![-4, -3, -2, -1],
        ];

        for input in inputs {
            let brute = product_except_self_brute_force(&input);
            let optimal = product_except_self_optimal(&input);
            assert_eq!(brute, optimal, "mismatch for input {input:?}");
        }
    }
}
