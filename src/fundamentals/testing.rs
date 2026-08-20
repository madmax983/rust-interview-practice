//! # Testing Patterns
//!
//! Comprehensive testing strategies for Rust: unit tests, integration tests,
//! property-based testing, and TDD workflows.

// ============================================================================
// Unit Test Fundamentals
// ============================================================================

/// Basic function to test.
#[allow(dead_code)]
const fn add(a: i32, b: i32) -> i32 {
    a + b
}

/// Function that might panic.
#[allow(dead_code)]
fn divide(a: i32, b: i32) -> i32 {
    assert!(b != 0, "Division by zero!");
    a / b
}

#[cfg(test)]
mod basic_tests {
    // test-code: this module demonstrates assertion/panic/ignore syntax itself, so bare
    // `assert!(true)`, `#[should_panic]`, and `#[ignore]` are intentional illustrations.
    #![allow(
        clippy::assertions_on_constants,
        clippy::should_panic_without_expect,
        clippy::ignore_without_reason
    )]

    use super::*;

    #[test]
    fn test_add() {
        assert_eq!(add(2, 2), 4);
        assert_eq!(add(-1, 1), 0);
        assert_eq!(add(0, 0), 0);
    }

    #[test]
    // This test intentionally contrasts `assert!` with `assert_eq!`, so keep the boolean form.
    #[allow(unknown_lints, clippy::manual_assert)]
    fn test_assertions() {
        // assert! - boolean condition
        assert!(2 + 2 == 4);

        // assert_eq! - equality with debug output
        assert_eq!(2 + 2, 4);

        // assert_ne! - inequality
        assert_ne!(2 + 2, 5);

        // Custom message
        assert_eq!(2 + 2, 4, "Math is broken!");
    }

    #[test]
    #[should_panic]
    fn test_divide_by_zero() {
        divide(10, 0); // Should panic
    }

    #[test]
    #[should_panic(expected = "Division by zero")]
    fn test_divide_by_zero_with_message() {
        divide(10, 0); // Must panic with specific message
    }

    #[test]
    #[ignore]
    fn expensive_test() {
        // Run with: cargo test -- --ignored
        // This test is skipped by default
        assert!(true);
    }

    #[test]
    fn test_result_return() -> Result<(), String> {
        // Tests can return Result
        if 2 + 2 == 4 {
            Ok(())
        } else {
            Err(String::from("Math is broken"))
        }
    }
}

// ============================================================================
// Test Organization
// ============================================================================

#[allow(dead_code)]
mod calculator {
    pub const fn add(a: i32, b: i32) -> i32 {
        a + b
    }

    pub const fn multiply(a: i32, b: i32) -> i32 {
        a * b
    }

    // Private function - only testable from tests module
    const fn internal_helper(x: i32) -> i32 {
        x * 2
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn test_add() {
            assert_eq!(add(2, 3), 5);
        }

        #[test]
        fn test_multiply() {
            assert_eq!(multiply(2, 3), 6);
        }

        #[test]
        fn test_private_function() {
            // Can test private functions from same module
            assert_eq!(internal_helper(5), 10);
        }
    }
}

// ============================================================================
// Test Fixtures and Setup
// ============================================================================

#[cfg(test)]
mod fixture_tests {
    // Setup function - called manually
    fn setup() -> Vec<i32> {
        vec![1, 2, 3, 4, 5]
    }

    // Teardown function
    fn teardown() {
        // Cleanup code here
        println!("Cleaning up...");
    }

    #[test]
    fn test_with_fixture() {
        let data = setup();
        assert_eq!(data.len(), 5);
        teardown();
    }

    // Helper to create test data
    fn create_user(name: &str, age: u32) -> User {
        User {
            name: name.to_string(),
            age,
        }
    }

    #[derive(Debug, PartialEq)]
    struct User {
        name: String,
        age: u32,
    }

    #[test]
    fn test_user_creation() {
        let user = create_user("Alice", 30);
        assert_eq!(user.name, "Alice");
        assert_eq!(user.age, 30);
    }

    // Struct-based fixture for complex setup
    struct TestContext {
        data: Vec<i32>,
        // demonstrates a fixture field held for RAII/teardown; not read in the example.
        #[allow(dead_code)]
        temp_file: String,
    }

    impl TestContext {
        fn new() -> Self {
            Self {
                data: vec![1, 2, 3],
                temp_file: "/tmp/test.txt".to_string(),
            }
        }
    }

    impl Drop for TestContext {
        fn drop(&mut self) {
            // Automatic cleanup when context goes out of scope
            println!("Cleaning up test context");
        }
    }

    #[test]
    fn test_with_context() {
        let ctx = TestContext::new();
        assert_eq!(ctx.data.len(), 3);
        // Automatic cleanup via Drop
    }
}

// ============================================================================
// Parameterized Tests
// ============================================================================

#[cfg(test)]
mod parameterized_tests {
    fn is_even(n: i32) -> bool {
        n % 2 == 0
    }

    #[test]
    fn test_is_even_table_driven() {
        let test_cases = vec![
            (0, true),
            (2, true),
            (4, true),
            (1, false),
            (3, false),
            (-2, true),
            (-1, false),
        ];

        for (input, expected) in test_cases {
            assert_eq!(is_even(input), expected, "Failed for input: {input}");
        }
    }

    // More complex table-driven test
    fn add(a: i32, b: i32) -> i32 {
        a + b
    }

    #[test]
    fn test_add_table() {
        struct TestCase {
            a: i32,
            b: i32,
            expected: i32,
        }

        let tests = vec![
            TestCase {
                a: 1,
                b: 2,
                expected: 3,
            },
            TestCase {
                a: 0,
                b: 0,
                expected: 0,
            },
            TestCase {
                a: -1,
                b: 1,
                expected: 0,
            },
            TestCase {
                a: -5,
                b: -3,
                expected: -8,
            },
        ];

        for test in tests {
            let result = add(test.a, test.b);
            assert_eq!(
                result, test.expected,
                "add({}, {}) = {}, expected {}",
                test.a, test.b, result, test.expected
            );
        }
    }
}

// ============================================================================
// Property-Based Testing (requires proptest feature)
// ============================================================================

#[cfg(all(test, feature = "testing-extras"))]
mod property_tests {
    use proptest::prelude::*;

    fn reverse<T: Clone>(xs: &[T]) -> Vec<T> {
        let mut rev = xs.to_vec();
        rev.reverse();
        rev
    }

    proptest! {
        #[test]
        fn test_reverse_twice_is_identity(ref v in prop::collection::vec(any::<i32>(), 0..100)) {
            let reversed_twice = reverse(&reverse(v));
            prop_assert_eq!(&reversed_twice, v);
        }

        #[test]
        fn test_reverse_length(ref v in prop::collection::vec(any::<i32>(), 0..100)) {
            let rev = reverse(v);
            prop_assert_eq!(rev.len(), v.len());
        }

        #[test]
        fn test_add_commutative(a in any::<i32>(), b in any::<i32>()) {
            // Test that addition is commutative
            // Using wrapping to avoid overflow panics in property tests
            prop_assert_eq!(a.wrapping_add(b), b.wrapping_add(a));
        }

        #[test]
        fn test_string_concat_length(ref s1 in ".*", ref s2 in ".*") {
            let combined = format!("{s1}{s2}");
            prop_assert_eq!(combined.len(), s1.len() + s2.len());
        }
    }

    // Custom strategy for generating test data
    proptest! {
        #[test]
        fn test_even_numbers(n in (0..1000i32).prop_map(|x| x * 2)) {
            prop_assert_eq!(n % 2, 0, "{} should be even", n);
        }
    }

    // Testing with multiple properties
    fn sort<T: Ord + Clone>(xs: &[T]) -> Vec<T> {
        let mut sorted = xs.to_vec();
        sorted.sort();
        sorted
    }

    proptest! {
        #[test]
        fn test_sort_properties(ref v in prop::collection::vec(any::<i32>(), 0..100)) {
            let sorted = sort(v);

            // Property 1: Same length
            prop_assert_eq!(sorted.len(), v.len());

            // Property 2: Is sorted
            for i in 1..sorted.len() {
                prop_assert!(sorted[i - 1] <= sorted[i]);
            }

            // Property 3: Contains same elements (idempotence)
            let sorted_again = sort(&sorted);
            prop_assert_eq!(sorted, sorted_again);
        }
    }
}

// ============================================================================
// Mocking Without External Crates
// ============================================================================

#[cfg(test)]
mod mock_tests {
    // Define a trait for the dependency
    trait DataStore {
        fn get(&self, key: &str) -> Option<String>;
        fn set(&mut self, key: &str, value: String);
    }

    // Real implementation
    // demonstrates the production impl contrasted with the mock; only the mock is exercised.
    #[allow(dead_code)]
    struct RealDataStore;

    impl DataStore for RealDataStore {
        fn get(&self, _key: &str) -> Option<String> {
            // Real database lookup
            None
        }

        fn set(&mut self, _key: &str, _value: String) {
            // Real database write
        }
    }

    // Mock implementation for testing
    struct MockDataStore {
        data: std::collections::HashMap<String, String>,
    }

    impl MockDataStore {
        fn new() -> Self {
            Self {
                data: std::collections::HashMap::new(),
            }
        }
    }

    impl DataStore for MockDataStore {
        fn get(&self, key: &str) -> Option<String> {
            self.data.get(key).cloned()
        }

        fn set(&mut self, key: &str, value: String) {
            self.data.insert(key.to_string(), value);
        }
    }

    // Code under test
    fn process_data<D: DataStore>(store: &mut D, key: &str, value: String) {
        store.set(key, value);
    }

    #[test]
    fn test_with_mock() {
        let mut mock = MockDataStore::new();
        process_data(&mut mock, "key1", "value1".to_string());

        assert_eq!(mock.get("key1"), Some("value1".to_string()));
    }
}

// ============================================================================
// Testing Async Code
// ============================================================================

#[cfg(test)]
mod async_tests {
    // Async function to test
    // demonstrates testing an async fn; the `async` is the subject under test, not incidental.
    #[allow(clippy::unused_async, dead_code)]
    async fn fetch_data(id: u32) -> String {
        // Simulate async operation
        format!("Data for ID {id}")
    }

    #[tokio::test]
    #[cfg(feature = "async-parallel")]
    async fn test_async_function() {
        let result = fetch_data(42).await;
        assert_eq!(result, "Data for ID 42");
    }

    // Test with timeout
    #[tokio::test]
    #[cfg(feature = "async-parallel")]
    async fn test_with_timeout() {
        use tokio::time::{Duration, timeout};

        let result = timeout(Duration::from_secs(1), fetch_data(42)).await;
        assert!(result.is_ok());
    }
}

// ============================================================================
// Benchmark Patterns (doc examples - use criterion for real benchmarks)
// ============================================================================

/// Benchmark pattern using `std::time`.
/// For production, use criterion crate.
#[cfg(test)]
mod benchmark_patterns {
    use std::time::Instant;

    fn fibonacci(n: u32) -> u32 {
        match n {
            0 => 0,
            1 => 1,
            n => fibonacci(n - 1) + fibonacci(n - 2),
        }
    }

    #[test]
    #[ignore = "benchmarks are slow"]
    fn bench_fibonacci() {
        let iterations = 100;
        let start = Instant::now();

        for _ in 0..iterations {
            let _ = fibonacci(20);
        }

        let duration = start.elapsed();
        println!("Average time: {:?}", duration / iterations);
    }
}

// ============================================================================
// Golden File Testing (Snapshot Testing)
// ============================================================================

#[cfg(test)]
mod golden_file_tests {
    use std::fs;

    fn render_output(data: &[i32]) -> String {
        format!(
            "Numbers: {data:?}\nCount: {}\nSum: {}",
            data.len(),
            data.iter().sum::<i32>()
        )
    }

    #[test]
    #[ignore = "requires golden file"]
    fn test_golden_file() {
        let data = vec![1, 2, 3, 4, 5];
        let output = render_output(&data);

        // Read expected output from file
        let expected =
            fs::read_to_string("tests/golden/output.txt").expect("Golden file not found");

        assert_eq!(output, expected);

        // To update golden file:
        // fs::write("tests/golden/output.txt", output).unwrap();
    }
}

// ============================================================================
// Test-Driven Development (TDD) Workflow
// ============================================================================

/// TDD Example: Implementing a stack
///
/// RED -> GREEN -> REFACTOR cycle
#[cfg(test)]
mod tdd_example {
    // Step 1: Write failing tests (RED)
    #[test]
    fn test_stack_new() {
        let stack: Stack<i32> = Stack::new();
        assert!(stack.is_empty());
        assert_eq!(stack.len(), 0);
    }

    #[test]
    fn test_stack_push() {
        let mut stack = Stack::new();
        stack.push(1);
        assert_eq!(stack.len(), 1);
        assert!(!stack.is_empty());
    }

    #[test]
    fn test_stack_pop() {
        let mut stack = Stack::new();
        stack.push(1);
        stack.push(2);
        assert_eq!(stack.pop(), Some(2));
        assert_eq!(stack.pop(), Some(1));
        assert_eq!(stack.pop(), None);
    }

    #[test]
    fn test_stack_peek() {
        let mut stack = Stack::new();
        stack.push(1);
        assert_eq!(stack.peek(), Some(&1));
        assert_eq!(stack.len(), 1); // Peek doesn't remove
    }

    // Step 2: Minimal implementation to pass tests (GREEN)
    struct Stack<T> {
        items: Vec<T>,
    }

    impl<T> Stack<T> {
        fn new() -> Self {
            Self { items: Vec::new() }
        }

        fn push(&mut self, item: T) {
            self.items.push(item);
        }

        fn pop(&mut self) -> Option<T> {
            self.items.pop()
        }

        fn peek(&self) -> Option<&T> {
            self.items.last()
        }

        fn is_empty(&self) -> bool {
            self.items.is_empty()
        }

        fn len(&self) -> usize {
            self.items.len()
        }
    }

    // Step 3: Refactor while keeping tests green
    // (Implementation is already clean for this example)
}

// ============================================================================
// Testing Error Handling
// ============================================================================

#[cfg(test)]
mod error_tests {
    use std::num::ParseIntError;

    fn parse_and_double(s: &str) -> Result<i32, ParseIntError> {
        let n: i32 = s.parse()?;
        Ok(n * 2)
    }

    #[test]
    fn test_successful_parse() {
        assert_eq!(parse_and_double("5").unwrap(), 10);
    }

    #[test]
    fn test_parse_error() {
        assert!(parse_and_double("not a number").is_err());
    }

    #[test]
    fn test_error_type() {
        let result = parse_and_double("not a number");
        match result {
            Ok(_) => panic!("Expected error"),
            Err(e) => {
                // Check error message or type
                assert!(e.to_string().contains("invalid"));
            }
        }
    }
}

// ============================================================================
// Doc Tests (tests in documentation comments)
// ============================================================================

/// Adds two numbers together.
///
/// # Examples
///
/// ```
/// # // Hidden setup line (starts with #)
/// # fn add(a: i32, b: i32) -> i32 { a + b }
/// assert_eq!(add(2, 2), 4);
/// assert_eq!(add(-1, 1), 0);
/// ```
///
/// # Panics
///
/// This function doesn't panic, but if it did:
///
/// ```should_panic
/// # fn add(a: i32, b: i32) -> i32 { a + b }
/// # // This example expects a panic
/// # panic!("example panic");
/// ```
///
/// # Errors
///
/// Example with Result:
///
/// ```
/// # fn parse_number(s: &str) -> Result<i32, std::num::ParseIntError> {
/// #     s.parse()
/// # }
/// assert!(parse_number("42").is_ok());
/// assert!(parse_number("not a number").is_err());
/// ```
#[allow(dead_code)]
const fn documented_add(a: i32, b: i32) -> i32 {
    a + b
}

// ============================================================================
// Integration Test Patterns (documented here, run from tests/ directory)
// ============================================================================

// Integration tests live in tests/ directory:
//
// ```text
// tests/
// ├── common/
// │   └── mod.rs      # Shared test utilities
// ├── integration_test.rs
// └── another_test.rs
// ```
//
// Example integration test:
//
// ```rust,ignore
// // tests/integration_test.rs
// use my_crate::public_function;
//
// #[test]
// fn test_public_api() {
//     assert_eq!(public_function(), expected_value);
// }
// ```
//
// Common module pattern:
//
// ```rust,ignore
// // tests/common/mod.rs
// pub fn setup() -> TestContext {
//     // Shared setup code
// }
// ```
//
// ```rust,ignore
// // tests/integration_test.rs
// mod common;
//
// #[test]
// fn test_with_common() {
//     let ctx = common::setup();
//     // Use ctx
// }
// ```

// ============================================================================
// Testing Best Practices
// ============================================================================

#[cfg(test)]
mod best_practices {
    /// Best practice: One assertion per test (when possible)
    #[test]
    fn test_single_assertion() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }

    /// Best practice: Clear test names describe what they test
    #[test]
    fn test_add_returns_sum_of_two_positive_numbers() {
        assert_eq!(2 + 3, 5);
    }

    /// Best practice: Arrange-Act-Assert pattern
    #[test]
    fn test_with_aaa_pattern() {
        // Arrange: Set up test data
        let x = 5;
        let y = 3;

        // Act: Execute the code under test
        let result = x + y;

        // Assert: Verify the result
        assert_eq!(result, 8);
    }

    /// Best practice: Test edge cases
    #[test]
    fn test_edge_cases() {
        // Empty
        assert_eq!(Vec::<i32>::new().len(), 0);

        // Single element
        assert_eq!(vec![1].len(), 1);

        // Boundary values
        assert_eq!(i32::MAX.wrapping_add(1), i32::MIN);
    }
}

// ============================================================================
// Usage Notes
// ============================================================================

/// Run tests:
/// ```bash
/// cargo test                    # All tests
/// cargo test --lib              # Library tests only
/// cargo test --doc              # Doc tests only
/// cargo test module_name        # Specific module
/// cargo test test_name          # Specific test
/// cargo test -- --ignored       # Run ignored tests
/// cargo test -- --nocapture     # Show println! output
/// cargo test -- --test-threads=1  # Run tests sequentially
/// ```
///
/// With features:
/// ```bash
/// cargo test --features testing-extras   # Enable proptest
/// cargo test --all-features              # All features
/// ```
///
/// Coverage (requires tarpaulin):
/// ```bash
/// cargo install cargo-tarpaulin
/// cargo tarpaulin --out Html
/// ```
#[allow(dead_code)]
const TESTING_USAGE: &str = "See module docs for usage";
