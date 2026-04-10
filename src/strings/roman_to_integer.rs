//! # 13. Roman to Integer
//!
//! Difficulty: Easy
//! Link: <https://leetcode.com/problems/roman-to-integer/>
//!
//! Roman numerals are represented by seven different symbols: `I`, `V`, `X`, `L`, `C`, `D`, and `M`.
//! For example, `2` is written as `II` in Roman numeral, just two ones added together. `12` is written as `XII`, which is simply `X + II`.
//!
//! ## Why this matters in Rust
//! This problem is an excellent showcase for Rust's **Enums**, **Pattern Matching**, and **Iterators**.
//! Instead of using a `HashMap` mapping characters to integers as you might in Python or Java, we can
//! encode the symbols directly into a zero-cost abstraction using a strongly-typed `enum`.
//! Additionally, handling the subtraction cases (like `IV` for `4`) naturally demonstrates the power
//! of Rust's `Peekable` iterator, allowing us to cleanly look ahead without manual index manipulation
//! or bounds checking.
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::strings::roman_to_integer::roman_to_int;
//!
//! assert_eq!(roman_to_int("III".to_string()), 3);
//! assert_eq!(roman_to_int("LVIII".to_string()), 58);
//! assert_eq!(roman_to_int("MCMXCIV".to_string()), 1994);
//! ```

/// Defines the valid Roman numeral symbols.
///
/// # Rust Insight
/// By making this an `enum`, we constrain the possible values at compile-time.
/// We implement `TryFrom<char>` to safely convert raw characters into our strongly-typed enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RomanSymbol {
    I,
    V,
    X,
    L,
    C,
    D,
    M,
}

impl RomanSymbol {
    /// Returns the integer value associated with the Roman symbol.
    const fn value(self) -> i32 {
        // RUST INSIGHT: Exhaustive pattern matching ensures we never forget a symbol.
        match self {
            Self::I => 1,
            Self::V => 5,
            Self::X => 10,
            Self::L => 50,
            Self::C => 100,
            Self::D => 500,
            Self::M => 1000,
        }
    }
}

impl TryFrom<char> for RomanSymbol {
    type Error = &'static str;

    fn try_from(c: char) -> Result<Self, Self::Error> {
        match c {
            'I' => Ok(Self::I),
            'V' => Ok(Self::V),
            'X' => Ok(Self::X),
            'L' => Ok(Self::L),
            'C' => Ok(Self::C),
            'D' => Ok(Self::D),
            'M' => Ok(Self::M),
            _ => Err("Invalid Roman numeral character"),
        }
    }
}

/// Brute Force / HashMap Approach (for comparison)
///
/// Time: O(N) where N is the length of the string.
/// Space: O(1) as the mapping is a constant size.
///
/// This is how you might solve it in Java or Python. While valid in Rust,
/// using a HashMap adds overhead (hashing, heap allocation) that we don't need
/// for a fixed set of characters.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn roman_to_int_hashmap(s: String) -> i32 {
    let mut map = std::collections::HashMap::with_capacity(7);
    map.insert('I', 1);
    map.insert('V', 5);
    map.insert('X', 10);
    map.insert('L', 50);
    map.insert('C', 100);
    map.insert('D', 500);
    map.insert('M', 1000);

    let chars: Vec<char> = s.chars().collect();
    let mut total = 0;
    let mut i = 0;

    while i < chars.len() {
        let current = *map.get(&chars[i]).unwrap_or(&0);
        let next = if i + 1 < chars.len() {
            *map.get(&chars[i + 1]).unwrap_or(&0)
        } else {
            0
        };

        if current < next {
            total += next - current;
            i += 2;
        } else {
            total += current;
            i += 1;
        }
    }

    total
}

/// Optimized Approach using `Peekable` Iterator
///
/// Time: O(N) where N is the length of the string.
/// Space: O(1)
///
/// Uses a functional approach with a `peekable` iterator. This avoids manual indexing
/// and bounds checking.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn roman_to_int_peekable(s: String) -> i32 {
    let mut total = 0;

    // RUST INSIGHT: `filter_map` safely ignores invalid characters and unwraps `Ok` values,
    // translating characters to our strongly-typed `RomanSymbol` enum.
    let mut iter = s
        .chars()
        .filter_map(|c| RomanSymbol::try_from(c).ok())
        .peekable();

    while let Some(current) = iter.next() {
        let current_val = current.value();

        // RUST INSIGHT: `.peek()` borrows the next item without consuming it.
        // We can check if the next symbol is larger (a subtraction case) and handle it.
        if let Some(next) = iter.peek()
            && current_val < next.value() {
                // We consume the next element since we're using it in this step
                total += next.value() - current_val;
                iter.next(); // Consume next
                continue;
            }

        total += current_val;
    }

    total
}

/// Optimal Approach using `as_bytes()` and windowing/iteration
///
/// Time: O(N)
/// Space: O(1)
///
/// Since Roman numerals are guaranteed to be ASCII characters, we can iterate over the
/// byte slice `&[u8]`. This avoids the slight overhead of `char` iteration (UTF-8 decoding)
/// and allows us to use a fast reverse iterator. By iterating from right to left, we
/// avoid looking ahead altogether.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn roman_to_int_optimal(s: String) -> i32 {
    let mut total = 0;
    let mut prev_value = 0;

    // RUST INSIGHT: Iterating over bytes `.bytes().rev()` is faster than `.chars().rev()`
    // when we know the string is pure ASCII. We process from right to left.
    for b in s.bytes().rev() {
        // We use a simple match here. It's essentially a fast jump table.
        let val = match b {
            b'I' => 1,
            b'V' => 5,
            b'X' => 10,
            b'L' => 50,
            b'C' => 100,
            b'D' => 500,
            b'M' => 1000,
            // GOTCHA: For LeetCode constraints, invalid characters aren't present.
            // But we must handle them for Rust's exhaustive matching.
            _ => 0,
        };

        if val < prev_value {
            // Subtraction case (e.g., 'I' before 'V')
            total -= val;
        } else {
            // Addition case
            total += val;
        }

        prev_value = val;
    }

    total
}

/// Main entry point - uses the optimal reverse byte iteration.
#[must_use]
pub fn roman_to_int(s: String) -> i32 {
    roman_to_int_optimal(s)
}

// =========================================================================================
// Alternative Approaches
// =========================================================================================
// 1. **Slice `windows(2)`**: You can convert the string to a `Vec<i32>` of values, then use
//    `.windows(2)` to compare adjacent elements. While expressive, it allocates a `Vec`.
// 2. **Recursive approach**: You could write a recursive function that parses tokens.
//    This might be overkill for this problem, but is foundational for more complex parsers.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_enum_try_from() {
        assert_eq!(RomanSymbol::try_from('I'), Ok(RomanSymbol::I));
        assert_eq!(RomanSymbol::try_from('M'), Ok(RomanSymbol::M));
        assert!(RomanSymbol::try_from('Z').is_err());
    }

    #[test]
    fn test_happy_path() {
        let cases = [("III", 3), ("LVIII", 58), ("MCMXCIV", 1994)];

        for (s, expected) in cases {
            assert_eq!(
                roman_to_int_hashmap(s.to_string()),
                expected,
                "Hashmap failed on {}",
                s
            );
            assert_eq!(
                roman_to_int_peekable(s.to_string()),
                expected,
                "Peekable failed on {}",
                s
            );
            assert_eq!(
                roman_to_int_optimal(s.to_string()),
                expected,
                "Optimal failed on {}",
                s
            );
        }
    }

    #[test]
    fn test_edge_cases() {
        // Single characters
        assert_eq!(roman_to_int("I".to_string()), 1);
        assert_eq!(roman_to_int("M".to_string()), 1000);

        // Consecutive subtractions (not strictly valid Roman numerals, but tested by some)
        assert_eq!(roman_to_int("IV".to_string()), 4);
        assert_eq!(roman_to_int("IX".to_string()), 9);
        assert_eq!(roman_to_int("XL".to_string()), 40);
        assert_eq!(roman_to_int("XC".to_string()), 90);
        assert_eq!(roman_to_int("CD".to_string()), 400);
        assert_eq!(roman_to_int("CM".to_string()), 900);
    }

    #[test]
    fn test_invalid_characters_ignored() {
        // The peekable approach ignores invalid characters due to filter_map.
        // The optimal approach currently treats invalid characters as 0 value.
        // I will omit the test for optimal on invalid string, as it's undefined behavior in LeetCode constraints.
        // But for peekable, let's verify it ignores correctly.
        assert_eq!(roman_to_int_peekable("I Z V".to_string()), 4); // Z is ignored, space is ignored, so IV -> 4.
    }
}
