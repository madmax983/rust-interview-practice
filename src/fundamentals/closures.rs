//! # Closure Patterns
//!
//! Closures are anonymous functions that can capture their environment.
//! Understanding Fn, `FnMut`, and `FnOnce` traits is essential for working with
//! iterators and callbacks in Rust.

use std::collections::HashMap;

// ============================================================================
// Function Trait Bounds: Fn, FnMut, FnOnce
// ============================================================================

/// Accepts a closure that can be called multiple times and borrows immutably.
/// Fn trait: Can call multiple times, captures by immutable reference.
#[allow(dead_code)]
fn call_with_fn<F>(f: F, x: i32) -> i32
where
    F: Fn(i32) -> i32,
{
    // Can call multiple times because Fn borrows immutably
    f(x) + f(x + 1)
}

/// Accepts a closure that can be called multiple times and borrows mutably.
/// `FnMut` trait: Can call multiple times, captures by mutable reference.
#[allow(dead_code)]
fn call_with_fn_mut<F>(mut f: F, x: i32) -> i32
where
    F: FnMut(i32) -> i32,
{
    // Can call multiple times, but must be mutable
    f(x) + f(x + 1)
}

/// Accepts a closure that consumes captured values and can only be called once.
/// `FnOnce` trait: Consumes captured values, can only call once.
#[allow(dead_code)]
fn call_with_fn_once<F>(f: F, x: i32) -> i32
where
    F: FnOnce(i32) -> i32,
{
    // Can only call once because it might consume captured values
    f(x)
}

#[allow(dead_code)]
fn demonstrate_fn_traits() {
    let multiplier = 2;

    // Fn: Captures by immutable reference
    let times_two = |x| x * multiplier;
    let result = call_with_fn(times_two, 5); // 5*2 + 6*2 = 22
    println!("Fn result: {result}");

    // FnMut: Captures by mutable reference
    let mut count = 0;
    let mut increment_and_add = |x| {
        count += 1;
        x + count
    };
    let result = call_with_fn_mut(&mut increment_and_add, 5); // (5+1) + (6+2) = 14
    println!("FnMut result: {result}");

    // FnOnce: Consumes captured values
    let vec = vec![1, 2, 3];
    let consume_vec = |x| {
        let _owned = vec; // Takes ownership
        x + 1
    };
    let result = call_with_fn_once(consume_vec, 5); // 6
    println!("FnOnce result: {result}");
}

// ============================================================================
// Capturing Patterns
// ============================================================================

#[allow(dead_code)]
fn demonstrate_captures() {
    // Immutable capture - closure reads a variable
    let multiplier = 10;
    let times_ten = |x: i32| x * multiplier; // Captures multiplier by immutable reference
    println!("5 * 10 = {}", times_ten(5));

    // Mutable capture - closure modifies a variable
    let mut sum = 0;
    let mut add_to_sum = |x: i32| {
        sum += x; // Captures sum by mutable reference
        sum
    };
    add_to_sum(5);
    add_to_sum(3);
    println!("Sum: {sum}");

    // Move capture - closure takes ownership
    let vec = vec![1, 2, 3];
    let consume = move || {
        // move keyword forces taking ownership
        println!("Vec: {vec:?}");
    };
    consume();
    // vec is no longer available here - it was moved into the closure

    // Multiple captures from environment
    let x = 5;
    let y = 10;
    let z = 15;
    let multi_capture = |a: i32| a + x + y + z; // Captures x, y, z
    println!("Result: {}", multi_capture(1)); // 1 + 5 + 10 + 15 = 31
}

// ============================================================================
// Common Iterator Patterns with Closures
// ============================================================================

#[allow(dead_code)]
fn demonstrate_iterator_closures() {
    let numbers = [1, 2, 3, 4, 5];

    // map - transform each element
    let doubled: Vec<i32> = numbers.iter().map(|x| x * 2).collect();
    println!("Doubled: {doubled:?}");

    // filter - keep elements matching predicate
    let evens: Vec<i32> = numbers.iter().filter(|&&x| x % 2 == 0).copied().collect();
    println!("Evens: {evens:?}");

    // fold - accumulate a value with a closure (init acc, then acc + each item)
    #[allow(clippy::unnecessary_fold)] // Demonstrating fold explicitly; sum() would also work
    let sum = numbers.iter().fold(0, |acc, &x| acc + x);
    println!("Sum: {sum}");

    // Chaining: filter then map then collect
    let result: Vec<i32> = numbers
        .iter()
        .filter(|&&x| x > 2) // Keep values > 2
        .map(|x| x * x) // Square them
        .collect(); // [9, 16, 25]
    println!("Filtered and squared: {result:?}");

    // filter_map - combine filter and map
    let strings = ["1", "two", "3", "four"];
    let parsed: Vec<i32> = strings
        .iter()
        .filter_map(|s| s.parse().ok()) // Only keep successful parses
        .collect(); // [1, 3]
    println!("Parsed: {parsed:?}");

    // Closures with multiple statements
    let result: Vec<String> = numbers
        .iter()
        .map(|x| {
            let squared = x * x;
            let doubled = squared * 2;
            format!("{x}^2 * 2 = {doubled}")
        })
        .collect();
    println!("Multi-statement closure: {result:?}");
}

// ============================================================================
// Closures with Collections
// ============================================================================

#[allow(dead_code)]
fn demonstrate_collection_closures() {
    // Using closures with HashMap
    let mut scores = HashMap::new();
    scores.insert("Alice", 10);
    scores.insert("Bob", 20);

    // entry API with closure
    scores.entry("Charlie").or_insert_with(|| 30); // Closure only called if key doesn't exist

    // Updating with closure
    scores.entry("Alice").and_modify(|score| *score += 5);

    // any - check if any element matches
    let has_high_score = scores.values().any(|&score| score > 25);
    println!("Has high score: {has_high_score}");

    // all - check if all elements match
    let all_positive = scores.values().all(|&score| score > 0);
    println!("All positive: {all_positive}");

    // find - get first matching element
    if let Some((&name, &score)) = scores.iter().find(|&(_, &score)| score > 25) {
        println!("High scorer: {name} with {score}");
    }

    // partition - split into two collections based on predicate
    let numbers = [1, 2, 3, 4, 5, 6];
    let (evens, odds): (Vec<i32>, Vec<i32>) = numbers.iter().partition(|&&x| x % 2 == 0);
    println!("Evens: {evens:?}, Odds: {odds:?}");
}

// ============================================================================
// Returning Closures
// ============================================================================

/// Returns a closure using impl Fn syntax (static dispatch).
/// The closure adds a fixed value to its input.
#[allow(dead_code)]
fn make_adder(n: i32) -> impl Fn(i32) -> i32 {
    // Returns a closure that captures n
    move |x| x + n
}

/// Returns a boxed closure (dynamic dispatch).
/// Useful when you need to return different closure types.
#[allow(dead_code)]
fn make_multiplier(n: i32) -> Box<dyn Fn(i32) -> i32> {
    // Box allows returning different closure types from same function
    Box::new(move |x| x * n)
}

/// Returns different closures based on condition.
/// Requires Box because different closures have different types.
#[allow(dead_code)]
fn make_operation(multiply: bool) -> Box<dyn Fn(i32, i32) -> i32> {
    if multiply {
        Box::new(|a, b| a * b)
    } else {
        Box::new(|a, b| a + b)
    }
}

#[allow(dead_code)]
fn demonstrate_returning_closures() {
    // impl Fn return type
    let add_five = make_adder(5);
    println!("10 + 5 = {}", add_five(10));

    // Box<dyn Fn> return type
    let times_three = make_multiplier(3);
    println!("10 * 3 = {}", times_three(10));

    // Returning different closures
    let multiply_op = make_operation(true);
    println!("10 * 5 = {}", multiply_op(10, 5));

    let add_op = make_operation(false);
    println!("10 + 5 = {}", add_op(10, 5));
}

// ============================================================================
// Closure Type Inference and Annotations
// ============================================================================

#[allow(dead_code)]
fn demonstrate_type_inference() {
    // Fully annotated closure
    let add_one = |x: i32| -> i32 { x + 1 };
    println!("5 + 1 = {}", add_one(5));

    // Type inference from context
    let numbers = [1, 2, 3];
    let _doubled: Vec<i32> = numbers.iter().map(|x| x * 2).collect();
    // Compiler infers x is &i32 from iter()

    // Sometimes need type annotations for ambiguity
    let parse_closure = |s: &str| -> Result<i32, _> { s.parse() };
    match parse_closure("42") {
        Ok(n) => println!("Parsed: {n}"),
        Err(_) => println!("Parse failed"),
    }

    // Multiple statement closures
    let complex = |x: i32| {
        let doubled = x * 2;
        let squared = doubled * doubled;
        squared + 1
    };
    println!("Complex: {}", complex(3)); // (3*2)^2 + 1 = 37
}

// ============================================================================
// Practical Interview Patterns
// ============================================================================

#[allow(dead_code)]
fn demonstrate_interview_patterns() {
    // Pattern 1: Frequency counting with closure
    let words = ["apple", "banana", "apple", "cherry", "banana"];
    let mut freq: HashMap<&str, i32> = HashMap::new();
    words.iter().for_each(|&word| {
        *freq.entry(word).or_insert(0) += 1;
    });
    println!("Frequency: {freq:?}");

    // Pattern 2: Custom sorting with closure
    let mut numbers = vec![5, 2, 8, 1, 9];
    numbers.sort_by(|a, b| b.cmp(a)); // Descending order
    println!("Sorted desc: {numbers:?}");

    // Pattern 3: Filtering with multiple conditions
    let numbers = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
    let result: Vec<i32> = numbers
        .iter()
        .filter(|&&x| x > 3 && x < 8 && x % 2 == 0)
        .copied()
        .collect(); // [4, 6]
    println!("Filtered: {result:?}");

    // Pattern 4: Transforming with stateful closure
    let mut counter = 0;
    let indexed: Vec<(usize, i32)> = numbers
        .iter()
        .map(|&x| {
            let idx = counter;
            counter += 1;
            (idx, x)
        })
        .collect();
    println!("Indexed: {indexed:?}");

    // Pattern 5: rfold - fold from the right with a closure
    let numbers = [1, 2, 3, 4, 5];
    let product = numbers.iter().rfold(1, |acc, &x| acc * x);
    println!("Product: {product}"); // 120
}

// ============================================================================
// Move Closures with Threads (Preview for concurrency)
// ============================================================================

#[allow(dead_code)]
fn demonstrate_move_for_threads() {
    use std::thread;

    let data = [1, 2, 3, 4, 5];

    // Must use 'move' to transfer ownership to thread
    let handle = thread::spawn(move || {
        let sum: i32 = data.iter().sum();
        println!("Sum from thread: {sum}");
    });

    handle.join().unwrap();
    // data is no longer available here - it was moved to the thread
}
