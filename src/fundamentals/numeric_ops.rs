//! # Numeric Operations and Bit Manipulation
//!
//! Common numeric patterns and bit manipulation techniques that frequently
//! appear in algorithm problems. Master these for interview success.

// intentional bit/byte/word manipulation for the demo
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap
)]

// ============================================================================
// Bit Manipulation Basics
// ============================================================================

#[allow(dead_code)]
fn demonstrate_bit_basics() {
    let mut n = 0b1010; // Binary: 1010 (decimal: 10)

    // Setting a bit at position pos (0-indexed from right)
    let pos = 0;
    n |= 1 << pos; // Set bit 0: 1010 -> 1011
    println!("After setting bit {pos}: {n:b} ({n})");

    // Clearing a bit at position pos
    let pos = 1;
    n &= !(1 << pos); // Clear bit 1: 1011 -> 1001
    println!("After clearing bit {pos}: {n:b} ({n})");

    // Toggling a bit at position pos
    let pos = 3;
    n ^= 1 << pos; // Toggle bit 3: 1001 -> 0001
    println!("After toggling bit {pos}: {n:b} ({n})");

    // Checking if a bit is set
    let pos = 0;
    let is_set = (n & (1 << pos)) != 0;
    println!("Bit {pos} is set: {is_set}");

    // Counting set bits (population count)
    let n = 0b10_1101u32; // 6 bits, 4 are set
    let count = n.count_ones();
    println!("Number of set bits in {n:b}: {count}");

    // Count trailing zeros
    let n = 0b10_1000u32; // 3 trailing zeros
    println!("Trailing zeros in {n:b}: {}", n.trailing_zeros());

    // Count leading zeros (in 32-bit representation)
    let n: u32 = 0b1010;
    println!("Leading zeros in {n:b}: {}", n.leading_zeros());

    // Left shift (multiply by 2^n)
    let n = 5;
    let shifted = n << 2; // 5 * 2^2 = 20
    println!("{n} << 2 = {shifted}");

    // Right shift (divide by 2^n)
    let n = 20;
    let shifted = n >> 2; // 20 / 2^2 = 5
    println!("{n} >> 2 = {shifted}");
}

// ============================================================================
// Bit Manipulation Tricks
// ============================================================================

/// Check if a number is a power of 2.
/// Powers of 2 have exactly one bit set: 8 = 1000, 16 = 10000
/// n & (n-1) clears the rightmost set bit.
#[allow(dead_code)]
const fn is_power_of_two(n: i32) -> bool {
    n > 0 && (n & (n - 1)) == 0
}

/// Find the rightmost set bit (isolate it).
/// n & -n isolates the lowest set bit.
/// Example: 12 = 1100, 12 & -12 = 0100 (bit at position 2)
#[allow(dead_code)]
const fn rightmost_set_bit(n: i32) -> i32 {
    n & -n
}

/// Clear the rightmost set bit.
/// n & (n-1) clears the rightmost set bit.
#[allow(dead_code)]
const fn clear_rightmost_set_bit(n: i32) -> i32 {
    n & (n - 1)
}

/// XOR trick: Find the unique element in array where all others appear twice.
/// XOR properties: a ^ a = 0, a ^ 0 = a, XOR is commutative and associative.
#[allow(dead_code)]
fn find_unique_element(nums: &[i32]) -> i32 {
    nums.iter().fold(0, |acc, &x| acc ^ x)
}

/// Create a bit mask with n bits set.
/// Example: n=3 creates 0b111 (7)
#[allow(dead_code)]
const fn create_mask(n: u32) -> i32 {
    (1 << n) - 1
}

/// Get bits in range [start, end).
/// Extract bits from position start to end (exclusive).
#[allow(dead_code)]
const fn get_bits_in_range(n: i32, start: u32, end: u32) -> i32 {
    let mask = (1 << (end - start)) - 1;
    (n >> start) & mask
}

/// Set bits in range [start, end) to value.
#[allow(dead_code)]
const fn set_bits_in_range(n: i32, start: u32, end: u32, value: i32) -> i32 {
    let mask = ((1 << (end - start)) - 1) << start;
    (n & !mask) | ((value << start) & mask)
}

#[allow(dead_code)]
fn demonstrate_bit_tricks() {
    // Power of 2 check
    println!("Is 16 power of 2? {}", is_power_of_two(16)); // true
    println!("Is 18 power of 2? {}", is_power_of_two(18)); // false

    // Rightmost set bit
    let n = 12; // 1100
    println!("Rightmost set bit of {n}: {}", rightmost_set_bit(n)); // 4 (0100)

    // Clear rightmost set bit
    let n = 12; // 1100
    println!("Clear rightmost bit of {n}: {}", clear_rightmost_set_bit(n)); // 8 (1000)

    // Find unique element
    let nums = [2, 3, 2, 4, 4];
    println!("Unique element: {}", find_unique_element(&nums)); // 3

    // Create mask
    println!("Mask with 4 bits: {:b}", create_mask(4)); // 1111

    // Get bits in range
    let n = 0b1101_1010;
    let bits = get_bits_in_range(n, 2, 5); // Get bits [2,5)
    println!("Bits [2,5) of {n:b}: {bits:b}"); // 110

    // Set bits in range
    let n = 0b1101_1010;
    let result = set_bits_in_range(n, 2, 5, 0b101);
    println!("After setting bits [2,5): {result:b}"); // 11010110
}

// ============================================================================
// Safe Arithmetic Operations
// ============================================================================

#[allow(dead_code)]
fn demonstrate_safe_arithmetic() {
    let a: i32 = 2_000_000_000;
    let b: i32 = 2_000_000_000;

    // wrapping_* - Wraps on overflow (like C)
    let sum = a.wrapping_add(b);
    println!("Wrapping add: {a} + {b} = {sum}");

    let diff = 0i32.wrapping_sub(1);
    println!("Wrapping sub: 0 - 1 = {diff}"); // -1

    let product = a.wrapping_mul(2);
    println!("Wrapping mul: {a} * 2 = {product}");

    // saturating_* - Clamps at min/max bounds
    let sum = a.saturating_add(b);
    println!("Saturating add: {a} + {b} = {sum}"); // i32::MAX

    let diff = 0i32.saturating_sub(1);
    println!("Saturating sub: 0 - 1 = {diff}"); // 0 for unsigned, -1 for signed

    // checked_* - Returns None on overflow
    match a.checked_add(b) {
        Some(sum) => println!("Sum: {sum}"),
        None => println!("Overflow occurred"),
    }

    match 10i32.checked_div(0) {
        Some(result) => println!("Result: {result}"),
        None => println!("Division by zero"),
    }

    // overflowing_* - Returns (result, overflow_flag)
    let (sum, overflowed) = a.overflowing_add(b);
    println!("Overflowing add: {sum}, overflowed: {overflowed}");

    // abs - Absolute value
    let n = -42i32;
    println!("abs({n}) = {}", n.abs());

    // abs_diff - Absolute difference (never overflows)
    let diff = 10u32.abs_diff(20);
    println!("abs_diff(10, 20) = {diff}"); // 10
}

// ============================================================================
// Common Numeric Operations
// ============================================================================

#[allow(dead_code)]
fn demonstrate_numeric_operations() {
    // min, max, clamp
    let a = 5;
    let b = 10;
    println!("min({a}, {b}) = {}", a.min(b));
    println!("max({a}, {b}) = {}", a.max(b));
    println!("clamp(15, {a}, {b}) = {}", 15.clamp(a, b)); // 10

    // pow - Exponentiation
    let base = 2i32;
    let exp = 10;
    println!("{base}^{exp} = {}", base.pow(exp));

    // Integer square root
    let n = 17;
    let sqrt = f64::from(n).sqrt() as i32;
    println!("floor(sqrt({n})) = {sqrt}");

    // Division methods
    let a = 17i32;
    let b = 5i32;
    println!("{a} / {b} = {}", a / b); // 3 (truncates toward zero)
    println!("{a} % {b} = {}", a % b); // 2 (remainder)

    // Euclidean division (always positive remainder)
    println!("{a}.div_euclid({b}) = {}", a.div_euclid(b));
    println!("{a}.rem_euclid({b}) = {}", a.rem_euclid(b));

    // For negative numbers
    let a = -17i32;
    let b = 5i32;
    println!("{a} / {b} = {}", a / b); // -3
    println!("{a} % {b} = {}", a % b); // -2
    println!("{a}.div_euclid({b}) = {}", a.div_euclid(b)); // -4
    println!("{a}.rem_euclid({b}) = {}", a.rem_euclid(b)); // 3 (always positive!)
}

// ============================================================================
// Number Algorithms
// ============================================================================

/// Calculate greatest common divisor using Euclidean algorithm.
#[allow(dead_code)]
const fn gcd(mut a: i32, mut b: i32) -> i32 {
    while b != 0 {
        let temp = b;
        b = a % b;
        a = temp;
    }
    a.abs()
}

/// Calculate least common multiple.
/// LCM(a, b) = (a * b) / GCD(a, b)
#[allow(dead_code)]
const fn lcm(a: i32, b: i32) -> i32 {
    (a * b).abs() / gcd(a, b)
}

/// Fast exponentiation using binary exponentiation.
/// Computes base^exp in O(log exp) time.
#[allow(dead_code)]
const fn fast_pow(mut base: i64, mut exp: u32) -> i64 {
    let mut result = 1i64;
    while exp > 0 {
        if exp % 2 == 1 {
            result = result.wrapping_mul(base);
        }
        base = base.wrapping_mul(base);
        exp /= 2;
    }
    result
}

/// Fast modular exponentiation: (base^exp) % modulo.
/// Useful for large number computations.
#[allow(dead_code)]
const fn mod_pow(mut base: i64, mut exp: u32, modulo: i64) -> i64 {
    let mut result = 1i64;
    base %= modulo;
    while exp > 0 {
        if exp % 2 == 1 {
            result = (result * base) % modulo;
        }
        base = (base * base) % modulo;
        exp /= 2;
    }
    result
}

/// Check if a number is prime (basic trial division).
/// Optimized: only check up to sqrt(n).
#[allow(dead_code)]
fn is_prime(n: i32) -> bool {
    if n < 2 {
        return false;
    }
    if n == 2 {
        return true;
    }
    if n % 2 == 0 {
        return false;
    }

    let limit = f64::from(n).sqrt() as i32;
    for i in (3..=limit).step_by(2) {
        if n % i == 0 {
            return false;
        }
    }
    true
}

#[allow(dead_code)]
fn demonstrate_number_algorithms() {
    // GCD
    println!("gcd(48, 18) = {}", gcd(48, 18)); // 6

    // LCM
    println!("lcm(12, 18) = {}", lcm(12, 18)); // 36

    // Fast exponentiation
    println!("2^20 = {}", fast_pow(2, 20)); // 1048576

    // Modular exponentiation
    println!("(2^20) % 1000 = {}", mod_pow(2, 20, 1000)); // 576

    // Prime check
    println!("Is 17 prime? {}", is_prime(17)); // true
    println!("Is 18 prime? {}", is_prime(18)); // false
}

// ============================================================================
// Parsing and Conversions
// ============================================================================

#[allow(dead_code)]
fn demonstrate_parsing() {
    // Parse string to number
    let s = "42";
    match s.parse::<i32>() {
        Ok(n) => println!("Parsed: {n}"),
        Err(e) => println!("Parse error: {e}"),
    }

    // Parse with turbofish syntax
    let n: i32 = "42".parse().unwrap();
    println!("Parsed: {n}");

    // Parse different radix (binary, hex, octal)
    let binary = i32::from_str_radix("1010", 2).unwrap();
    println!("Binary 1010 = {binary}"); // 10

    let hex = i32::from_str_radix("FF", 16).unwrap();
    println!("Hex FF = {hex}"); // 255

    let octal = i32::from_str_radix("77", 8).unwrap();
    println!("Octal 77 = {octal}"); // 63

    // Extract digits from number
    let mut n = 12345;
    let mut digits = Vec::new();
    while n > 0 {
        digits.push(n % 10); // Get last digit
        n /= 10; // Remove last digit
    }
    digits.reverse();
    println!("Digits: {digits:?}"); // [1, 2, 3, 4, 5]

    // Build number from digits
    let digits = [1, 2, 3, 4, 5];
    let n = digits.iter().fold(0, |acc, &d| acc * 10 + d);
    println!("Number: {n}"); // 12345

    // Number to string conversions
    let n = 42;
    let s1 = n.to_string();
    let s2 = format!("{n}");
    let s3 = format!("{n:b}"); // Binary
    let s4 = format!("{n:x}"); // Hex
    println!("Decimal: {s1}, Format: {s2}, Binary: {s3}, Hex: {s4}");
}

// ============================================================================
// Practical Interview Patterns
// ============================================================================

/// Reverse the bits of a 32-bit integer.
#[allow(dead_code)]
fn reverse_bits(mut n: u32) -> u32 {
    let mut result = 0u32;
    for _ in 0..32 {
        result <<= 1;
        result |= n & 1;
        n >>= 1;
    }
    result
}

/// Count number of 1 bits (Hamming weight).
#[allow(dead_code)]
const fn hamming_weight(mut n: u32) -> i32 {
    let mut count = 0;
    while n != 0 {
        n &= n - 1; // Clear rightmost set bit
        count += 1;
    }
    count
}

/// Check if a number is palindrome without converting to string.
#[allow(dead_code)]
const fn is_palindrome(mut x: i32) -> bool {
    if x < 0 {
        return false;
    }

    let mut reversed = 0;
    let original = x;

    while x > 0 {
        reversed = reversed * 10 + x % 10;
        x /= 10;
    }

    original == reversed
}

/// Compute integer square root using binary search.
#[allow(dead_code)]
fn int_sqrt(x: i32) -> i32 {
    if x < 2 {
        return x;
    }

    let mut left = 1;
    let mut right = x / 2;

    while left <= right {
        let mid = left + (right - left) / 2;
        let mid_squared = i64::from(mid) * i64::from(mid);
        let x_i64 = i64::from(x);

        match mid_squared.cmp(&x_i64) {
            std::cmp::Ordering::Equal => return mid,
            std::cmp::Ordering::Less => left = mid + 1,
            std::cmp::Ordering::Greater => right = mid - 1,
        }
    }

    right
}

#[allow(dead_code)]
fn demonstrate_interview_patterns() {
    // Reverse bits
    let n = 0b0000_0000_0000_0000_0000_0000_0000_1011_u32; // 11
    let reversed = reverse_bits(n);
    println!("Reverse bits of {n:b}: {reversed:b}");

    // Hamming weight
    let n = 0b0000_1011_u32; // 11
    println!("Hamming weight of {n:b}: {}", hamming_weight(n)); // 3

    // Palindrome check
    println!("Is 121 palindrome? {}", is_palindrome(121)); // true
    println!("Is 123 palindrome? {}", is_palindrome(123)); // false

    // Integer square root
    println!("int_sqrt(8) = {}", int_sqrt(8)); // 2
    println!("int_sqrt(16) = {}", int_sqrt(16)); // 4
}

#[cfg(test)]
mod tests {
    use super::*;

    // ------------------------------------------------------------------------
    // Bit manipulation basics (values demonstrated in `demonstrate_bit_basics`)
    // ------------------------------------------------------------------------

    #[test]
    fn test_set_clear_toggle_bit() {
        let mut n = 0b1010; // 10

        n |= 1 << 0; // set bit 0
        assert_eq!(n, 0b1011); // 11

        n &= !(1 << 1); // clear bit 1
        assert_eq!(n, 0b1001); // 9

        n ^= 1 << 3; // toggle bit 3
        assert_eq!(n, 0b0001); // 1
    }

    #[test]
    fn test_check_bit_set() {
        let n = 0b0001;
        assert!((n & (1 << 0)) != 0);
        assert!((n & (1 << 1)) == 0);
    }

    #[test]
    fn test_bit_counts() {
        assert_eq!(0b10_1101_u32.count_ones(), 4);
        assert_eq!(0b10_1000_u32.trailing_zeros(), 3);
        assert_eq!(0b1010_u32.leading_zeros(), 28); // 32-bit representation
    }

    #[test]
    fn test_shifts() {
        assert_eq!(5 << 2, 20); // multiply by 2^2
        assert_eq!(20 >> 2, 5); // divide by 2^2
    }

    // ------------------------------------------------------------------------
    // Bit manipulation tricks
    // ------------------------------------------------------------------------

    #[test]
    fn test_is_power_of_two() {
        assert!(is_power_of_two(16));
        assert!(!is_power_of_two(18));
        assert!(is_power_of_two(1));
        assert!(!is_power_of_two(0));
        assert!(!is_power_of_two(-8)); // negatives are never powers of two
    }

    #[test]
    fn test_rightmost_set_bit() {
        assert_eq!(rightmost_set_bit(12), 4); // 1100 -> 0100
        assert_eq!(rightmost_set_bit(1), 1);
        assert_eq!(rightmost_set_bit(0b1000_0000), 0b1000_0000);
    }

    #[test]
    fn test_clear_rightmost_set_bit() {
        assert_eq!(clear_rightmost_set_bit(12), 8); // 1100 -> 1000
        assert_eq!(clear_rightmost_set_bit(0b1011), 0b1010);
        assert_eq!(clear_rightmost_set_bit(0), 0);
    }

    #[test]
    fn test_find_unique_element() {
        assert_eq!(find_unique_element(&[2, 3, 2, 4, 4]), 3);
        assert_eq!(find_unique_element(&[1, 1, 5]), 5);
        assert_eq!(find_unique_element(&[42]), 42);
        assert_eq!(find_unique_element(&[]), 0);
    }

    #[test]
    fn test_create_mask() {
        assert_eq!(create_mask(4), 0b1111); // 15
        assert_eq!(create_mask(3), 0b111); // 7
        assert_eq!(create_mask(0), 0);
        assert_eq!(create_mask(1), 1);
    }

    #[test]
    fn test_get_bits_in_range() {
        assert_eq!(get_bits_in_range(0b1101_1010, 2, 5), 0b110); // 6
        assert_eq!(get_bits_in_range(0b1111, 0, 4), 0b1111);
        assert_eq!(get_bits_in_range(0b1010, 1, 2), 1);
    }

    #[test]
    fn test_set_bits_in_range() {
        assert_eq!(set_bits_in_range(0b1101_1010, 2, 5, 0b101), 0b1101_0110); // 214
                                                                              // Setting a range and reading it back yields the written value.
        let modified = set_bits_in_range(0, 4, 8, 0b1010);
        assert_eq!(get_bits_in_range(modified, 4, 8), 0b1010);
    }

    // ------------------------------------------------------------------------
    // Safe arithmetic
    // ------------------------------------------------------------------------

    #[test]
    fn test_wrapping_arithmetic() {
        let a: i32 = 2_000_000_000;
        let b: i32 = 2_000_000_000;
        assert_eq!(a.wrapping_add(b), -294_967_296);
        assert_eq!(0i32.wrapping_sub(1), -1);
        assert_eq!(a.wrapping_mul(2), -294_967_296);
    }

    #[test]
    fn test_saturating_arithmetic() {
        let a: i32 = 2_000_000_000;
        assert_eq!(a.saturating_add(a), i32::MAX);
        assert_eq!(0i32.saturating_sub(1), -1); // signed floor is i32::MIN, not 0
        assert_eq!(0u32.saturating_sub(1), 0); // unsigned clamps at 0
    }

    #[test]
    fn test_checked_arithmetic() {
        let a: i32 = 2_000_000_000;
        assert_eq!(a.checked_add(a), None); // overflow
        assert_eq!(100i32.checked_add(1), Some(101));
        assert_eq!(10i32.checked_div(0), None); // division by zero
        assert_eq!(10i32.checked_div(2), Some(5));
    }

    #[test]
    fn test_overflowing_arithmetic() {
        let a: i32 = 2_000_000_000;
        assert_eq!(a.overflowing_add(a), (-294_967_296, true));
        assert_eq!(1i32.overflowing_add(1), (2, false));
    }

    #[test]
    fn test_abs_operations() {
        assert_eq!((-42i32).abs(), 42);
        assert_eq!(10u32.abs_diff(20), 10);
        assert_eq!(20u32.abs_diff(10), 10);
    }

    // ------------------------------------------------------------------------
    // Common numeric operations
    // ------------------------------------------------------------------------

    #[test]
    fn test_min_max_clamp() {
        assert_eq!(5.min(10), 5);
        assert_eq!(5.max(10), 10);
        assert_eq!(15.clamp(5, 10), 10);
        assert_eq!(3.clamp(5, 10), 5);
    }

    #[test]
    fn test_pow_and_sqrt() {
        assert_eq!(2i32.pow(10), 1024);
        assert_eq!((17.0_f64).sqrt() as i32, 4);
    }

    #[test]
    fn test_division_and_remainder() {
        assert_eq!(17i32 / 5, 3);
        assert_eq!(17i32 % 5, 2);
        assert_eq!(17i32.div_euclid(5), 3);
        assert_eq!(17i32.rem_euclid(5), 2);
    }

    #[test]
    fn test_euclidean_division_negative() {
        assert_eq!(-17i32 / 5, -3); // truncates toward zero
        assert_eq!(-17i32 % 5, -2);
        assert_eq!((-17i32).div_euclid(5), -4);
        assert_eq!((-17i32).rem_euclid(5), 3); // always non-negative
    }

    // ------------------------------------------------------------------------
    // Number algorithms
    // ------------------------------------------------------------------------

    #[test]
    fn test_gcd() {
        assert_eq!(gcd(48, 18), 6);
        assert_eq!(gcd(17, 5), 1);
        assert_eq!(gcd(0, 5), 5);
        assert_eq!(gcd(-48, 18), 6); // result is absolute
    }

    #[test]
    fn test_lcm() {
        assert_eq!(lcm(12, 18), 36);
        assert_eq!(lcm(4, 6), 12);
        assert_eq!(lcm(7, 3), 21);
    }

    #[test]
    fn test_fast_pow() {
        assert_eq!(fast_pow(2, 20), 1_048_576);
        assert_eq!(fast_pow(2, 0), 1);
        assert_eq!(fast_pow(3, 4), 81);
        assert_eq!(fast_pow(5, 1), 5);
    }

    #[test]
    fn test_mod_pow() {
        assert_eq!(mod_pow(2, 20, 1000), 576);
        assert_eq!(mod_pow(2, 10, 1000), 24); // 1024 % 1000
        assert_eq!(mod_pow(3, 3, 7), 6); // 27 % 7
    }

    #[test]
    fn test_is_prime() {
        assert!(is_prime(17));
        assert!(!is_prime(18));
        assert!(is_prime(2));
        assert!(!is_prime(1));
        assert!(!is_prime(0));
        assert!(!is_prime(-5));
        assert!(is_prime(97));
        assert!(!is_prime(9));
    }

    // ------------------------------------------------------------------------
    // Parsing and conversions
    // ------------------------------------------------------------------------

    #[test]
    fn test_parse_and_radix() {
        assert_eq!("42".parse::<i32>(), Ok(42));
        assert!("abc".parse::<i32>().is_err());
        assert_eq!(i32::from_str_radix("1010", 2), Ok(10));
        assert_eq!(i32::from_str_radix("FF", 16), Ok(255));
        assert_eq!(i32::from_str_radix("77", 8), Ok(63));
    }

    #[test]
    fn test_extract_and_build_digits() {
        let mut n = 12345;
        let mut digits = Vec::new();
        while n > 0 {
            digits.push(n % 10);
            n /= 10;
        }
        digits.reverse();
        assert_eq!(digits, vec![1, 2, 3, 4, 5]);

        let rebuilt = digits.iter().fold(0, |acc, &d| acc * 10 + d);
        assert_eq!(rebuilt, 12345);
    }

    #[test]
    fn test_number_to_string_formats() {
        let n = 42;
        assert_eq!(n.to_string(), "42");
        assert_eq!(format!("{n:b}"), "101010");
        assert_eq!(format!("{n:x}"), "2a");
    }

    // ------------------------------------------------------------------------
    // Practical interview patterns
    // ------------------------------------------------------------------------

    #[test]
    fn test_reverse_bits() {
        assert_eq!(reverse_bits(11), 3_489_660_928); // 0b...1011 reversed
        assert_eq!(reverse_bits(0), 0);
        assert_eq!(reverse_bits(1), 1 << 31);
        assert_eq!(reverse_bits(u32::MAX), u32::MAX);
    }

    #[test]
    fn test_hamming_weight() {
        assert_eq!(hamming_weight(0b1011), 3);
        assert_eq!(hamming_weight(0), 0);
        assert_eq!(hamming_weight(u32::MAX), 32);
    }

    #[test]
    fn test_is_palindrome() {
        assert!(is_palindrome(121));
        assert!(!is_palindrome(123));
        assert!(!is_palindrome(-121)); // negatives are never palindromes
        assert!(is_palindrome(0));
        assert!(is_palindrome(7));
    }

    #[test]
    fn test_int_sqrt() {
        assert_eq!(int_sqrt(8), 2); // floor(sqrt(8))
        assert_eq!(int_sqrt(16), 4);
        assert_eq!(int_sqrt(0), 0);
        assert_eq!(int_sqrt(1), 1);
        assert_eq!(int_sqrt(15), 3);
        assert_eq!(int_sqrt(2_147_395_600), 46_340); // large value
    }

    // ------------------------------------------------------------------------
    // Smoke tests: demonstration functions must run without panicking.
    // ------------------------------------------------------------------------

    #[test]
    fn test_demonstrations_run() {
        demonstrate_bit_basics();
        demonstrate_bit_tricks();
        demonstrate_safe_arithmetic();
        demonstrate_numeric_operations();
        demonstrate_number_algorithms();
        demonstrate_parsing();
        demonstrate_interview_patterns();
    }
}
