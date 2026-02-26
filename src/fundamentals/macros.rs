//! # Macro Patterns
//!
//! Declarative macros (macro_rules!) for compile-time metaprogramming.
//! Master these patterns to write DSLs and eliminate boilerplate.

// ============================================================================
// Declarative Macro Basics
// ============================================================================

/// Simple replacement macro - no arguments.
macro_rules! say_hello {
    () => {
        println!("Hello from macro!")
    };
}

/// Macro with a single expression argument.
macro_rules! double {
    ($x:expr) => {
        $x * 2
    };
}

/// Macro demonstrating fragment specifiers.
macro_rules! fragment_types {
    // expr - expression
    (expr $e:expr) => {
        println!("Expression: {}", $e)
    };

    // ident - identifier
    (ident $i:ident) => {
        println!("Identifier: {}", stringify!($i))
    };

    // ty - type
    (ty $t:ty) => {
        println!("Type: {}", stringify!($t))
    };

    // pat - pattern
    (pat $p:pat) => {
        println!("Pattern: {}", stringify!($p))
    };

    // stmt - statement
    (stmt $s:stmt) => {
        $s
    };
}

#[allow(dead_code)]
fn demonstrate_basic_macros() {
    say_hello!();

    let x = 5;
    let doubled = double!(x);
    println!("Doubled: {doubled}");

    fragment_types!(expr 2 + 2);
    fragment_types!(ident my_variable);
    fragment_types!(ty i32);
    fragment_types!(pat Some(x));
    fragment_types!(stmt let _y = 10);
}

// ============================================================================
// Repetition Patterns
// ============================================================================

/// Zero or more repetition with $(...)*
macro_rules! sum {
    () => { 0 };
    ($($x:expr),*) => {
        {
            let mut total = 0;
            $(
                total += $x;
            )*
            total
        }
    };
}

/// One or more repetition with $(...)+
macro_rules! min {
    ($x:expr) => { $x };
    ($x:expr, $($rest:expr),+) => {
        {
            let temp = min!($($rest),+);
            if $x < temp { $x } else { temp }
        }
    };
}

/// vec!-like macro - comma-separated with optional trailing comma.
macro_rules! my_vec {
    () => {
        Vec::new()
    };
    ($($x:expr),+ $(,)?) => {
        {
            let mut temp_vec = Vec::new();
            $(
                temp_vec.push($x);
            )+
            temp_vec
        }
    };
}

/// Repeat with different separators.
macro_rules! create_struct {
    ($name:ident { $($field:ident: $type:ty),* $(,)? }) => {
        struct $name {
            $(
                $field: $type,
            )*
        }
    };
}

#[allow(dead_code)]
fn demonstrate_repetition() {
    // sum macro
    let total = sum!(1, 2, 3, 4, 5);
    println!("Sum: {total}");

    // min macro (recursive)
    let minimum = min!(5, 2, 8, 1, 9);
    println!("Min: {minimum}");

    // vec-like macro
    let v = my_vec![1, 2, 3, 4, 5];
    println!("Vec: {v:?}");

    // Trailing comma support
    let v2 = my_vec![1, 2, 3,];
    println!("Vec with trailing comma: {v2:?}");
}

// ============================================================================
// Multiple Branches and Pattern Matching
// ============================================================================

/// Macro with multiple match arms (like println!).
macro_rules! my_print {
    () => {
        println!()
    };
    ($msg:expr) => {
        println!("{}", $msg)
    };
    ($fmt:expr, $($arg:expr),+) => {
        println!($fmt, $($arg),+)
    };
}

/// Count arguments macro.
macro_rules! count {
    () => { 0 };
    ($head:expr) => { 1 };
    ($head:expr, $($tail:expr),+) => {
        1 + count!($($tail),+)
    };
}

/// Type-based dispatch in macros.
macro_rules! type_of {
    ($x:expr) => {{
        fn type_name_of<T>(_: &T) -> &'static str {
            std::any::type_name::<T>()
        }
        type_name_of(&$x)
    }};
}

/// Optional parameters with default values.
macro_rules! create_function {
    // With explicit return type
    ($name:ident ( $($param:ident : $ptype:ty),* ) -> $rtype:ty $body:block) => {
        fn $name($($param: $ptype),*) -> $rtype $body
    };

    // Without return type (defaults to unit)
    ($name:ident ( $($param:ident : $ptype:ty),* ) $body:block) => {
        fn $name($($param: $ptype),*) $body
    };
}

#[allow(dead_code)]
fn demonstrate_multiple_branches() {
    my_print!();
    my_print!("Hello");
    my_print!("Hello, {}!", "world");

    let num = count!(1, 2, 3, 4, 5);
    println!("Count: {num}");

    let x = 42;
    println!("Type: {}", type_of!(x));

    let s = "hello";
    println!("Type: {}", type_of!(s));
}

// ============================================================================
// Practical Macro Patterns
// ============================================================================

/// HashMap literal macro (like serde_json::json!).
macro_rules! hashmap {
    ($($key:expr => $value:expr),* $(,)?) => {
        {
            let mut map = std::collections::HashMap::new();
            $(
                map.insert($key, $value);
            )*
            map
        }
    };
}

/// Assert macro with custom message.
macro_rules! assert_custom {
    ($cond:expr, $msg:expr) => {
        if !$cond {
            panic!("Assertion failed: {} ({}:{})", $msg, file!(), line!());
        }
    };
    ($cond:expr) => {
        assert_custom!($cond, stringify!($cond))
    };
}

/// Measure execution time macro.
macro_rules! time_it {
    ($label:expr, $code:block) => {{
        let start = std::time::Instant::now();
        let result = $code;
        let duration = start.elapsed();
        println!("{} took: {:?}", $label, duration);
        result
    }};
}

/// Enum with automatic string conversion.
macro_rules! string_enum {
    ($name:ident { $($variant:ident),* $(,)? }) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        enum $name {
            $($variant),*
        }

        impl $name {
            fn as_str(&self) -> &'static str {
                match self {
                    $(
                        $name::$variant => stringify!($variant),
                    )*
                }
            }

            fn variants() -> &'static [$name] {
                &[$($name::$variant),*]
            }
        }
    };
}

#[allow(dead_code)]
fn demonstrate_practical_macros() {
    // HashMap literal
    let scores = hashmap! {
        "Alice" => 10,
        "Bob" => 20,
        "Charlie" => 30,
    };
    println!("Scores: {scores:?}");

    // Custom assert
    let x = 5;
    assert_custom!(x > 0, "x must be positive");

    // Time measurement
    let result = time_it!("Computing sum", { (0..1000).sum::<i32>() });
    println!("Result: {result}");
}

// ============================================================================
// DSL-Style Macros
// ============================================================================

/// Builder-style DSL macro.
macro_rules! build_struct {
    ($name:ident {
        $($field:ident : $value:expr),* $(,)?
    }) => {
        $name {
            $($field: $value),*
        }
    };
}

/// SQL-like query DSL (concept).
macro_rules! query {
    (SELECT $($field:ident),+ FROM $table:ident WHERE $condition:expr) => {
        {
            println!("Selecting {} from {}",
                stringify!($($field),+),
                stringify!($table)
            );
            println!("Where: {}", stringify!($condition));
        }
    };
}

/// HTML-like DSL.
macro_rules! html {
    ($tag:ident { $($child:tt)* }) => {
        format!("<{}>{}</{}>", stringify!($tag), html!($($child)*), stringify!($tag))
    };
    ($text:expr) => {
        $text
    };
}

#[allow(dead_code)]
fn demonstrate_dsl_macros() {
    // Builder DSL
    #[derive(Debug)]
    struct Config {
        name: String,
        port: u16,
        debug: bool,
    }

    let config = build_struct!(Config {
        name: "server".to_string(),
        port: 8080,
        debug: true,
    });
    println!("Config: {config:?}");

    // SQL-like DSL
    query!(SELECT id, name FROM users WHERE age > 18);
}

// ============================================================================
// Recursive Macros
// ============================================================================

/// Recursive macro for reversing arguments.
macro_rules! reverse {
    // Base case: single element
    ($x:expr) => { vec![$x] };

    // Recursive case: take last element, recurse on rest
    ($x:expr, $($rest:expr),+) => {{
        let mut v = reverse!($($rest),+);
        v.insert(0, $x);
        v
    }};
}

/// Recursive macro for compile-time computation.
macro_rules! factorial {
    (0) => {
        1
    };
    ($n:expr) => {
        $n * factorial!($n - 1)
    };
}

/// Tree-like recursion in macros.
macro_rules! tree {
    ($value:expr) => {
        Node::Leaf($value)
    };
    ($value:expr => { $($children:tt)* }) => {
        Node::Branch {
            value: $value,
            children: vec![$($children)*],
        }
    };
}

#[allow(dead_code)]
fn demonstrate_recursive_macros() {
    let reversed = reverse!(1, 2, 3, 4, 5);
    println!("Reversed: {reversed:?}");
}

// ============================================================================
// Macro Hygiene and Scope
// ============================================================================

/// Demonstrate macro hygiene - variables don't leak.
macro_rules! hygienic_macro {
    ($x:expr) => {{
        let temp = $x; // This 'temp' doesn't conflict with outer scope
        temp * 2
    }};
}

/// Break hygiene intentionally with $crate.
macro_rules! use_crate_item {
    () => {
        // $crate refers to the crate where macro is defined
        $crate::demonstrate_basic_macros()
    };
}

#[allow(dead_code)]
fn demonstrate_hygiene() {
    let temp = 5;
    let result = hygienic_macro!(10); // Uses internal temp, not outer
    println!("temp: {temp}, result: {result}");
}

// ============================================================================
// Debugging Macros
// ============================================================================

/// Macro that shows its expansion.
macro_rules! debug_macro {
    ($($x:expr),*) => {{
        println!("Debug macro called with: {}", stringify!($($x),*));
        vec![$($x),*]
    }};
}

/// Use case for cargo expand:
/// ```bash
/// cargo install cargo-expand
/// cargo expand --lib fundamentals::macros
/// ```

#[allow(dead_code)]
fn demonstrate_debugging() {
    let v = debug_macro!(1, 2, 3);
    println!("Result: {v:?}");

    // To see full expansion:
    // 1. Install: cargo install cargo-expand
    // 2. Run: cargo expand --lib fundamentals::macros
    // 3. See the actual generated code
}

// ============================================================================
// When to Use Macros vs Alternatives
// ============================================================================

/// Example: This could be a macro OR a generic function.
/// Use macro when you need:
/// - Compile-time evaluation
/// - Variable number of arguments
/// - Code generation based on types
/// - DSLs and syntax extensions

// As a macro:
macro_rules! max_macro {
    ($x:expr) => { $x };
    ($x:expr, $($rest:expr),+) => {{
        let temp = max_macro!($($rest),+);
        if $x > temp { $x } else { temp }
    }};
}

// As a generic function (better for simple cases):
#[allow(dead_code)]
fn max_function<T: Ord>(first: T, rest: &[T]) -> T
where
    T: Copy,
{
    let mut max = first;
    for &item in rest {
        if item > max {
            max = item;
        }
    }
    max
}

#[allow(dead_code)]
fn demonstrate_macro_vs_function() {
    // Macro: Good for variable args at compile time
    let m1 = max_macro!(5, 2, 8, 1, 9);
    println!("Max (macro): {m1}");

    // Function: Better type checking, easier debugging
    let m2 = max_function(5, &[2, 8, 1, 9]);
    println!("Max (function): {m2}");

    println!("\nUse macros when you need:");
    println!("  - Variable argument counts");
    println!("  - Compile-time code generation");
    println!("  - DSLs or syntax extensions");
    println!("  - Access to syntax like 'file!()' or 'line!()'");
    println!();
    println!("Use functions/generics when:");
    println!("  - Logic is runtime-based");
    println!("  - You want better error messages");
    println!("  - Code is straightforward");
    println!("  - You need IDE support (autocomplete, refactoring)");
}

// ============================================================================
// Common Macro Patterns from std
// ============================================================================

/// Recreating common std macros for learning.

// vec!-like
macro_rules! my_vec_full {
    () => { Vec::new() };
    ($elem:expr; $n:expr) => {
        vec![$elem; $n]
    };
    ($($x:expr),+ $(,)?) => {{
        let mut v = Vec::new();
        $(v.push($x);)+
        v
    }};
}

// format!-like
macro_rules! my_format {
    ($($arg:tt)*) => {
        format!($($arg)*)
    };
}

// matches!-like (from std)
macro_rules! my_matches {
    ($expr:expr, $pattern:pat) => {
        match $expr {
            $pattern => true,
            _ => false,
        }
    };
}

#[allow(dead_code)]
fn demonstrate_std_patterns() {
    let v = my_vec_full![1, 2, 3];
    println!("Vec: {v:?}");

    let s = my_format!("Hello, {}!", "world");
    println!("{s}");

    let opt = Some(42);
    println!("Matches Some: {}", my_matches!(opt, Some(_)));
}

// ============================================================================
// Advanced: TT Munching
// ============================================================================

/// TT (token tree) munching - processing tokens one at a time.
macro_rules! parse_list {
    // Base case: empty
    () => {
        Vec::<i32>::new()
    };

    // Munch a number
    ($num:literal $(, $($rest:tt)*)?) => {{
        let mut v = Vec::new();
        v.push($num);
        $(v.extend(parse_list!($($rest)*));)?
        v
    }};
}

#[allow(dead_code)]
fn demonstrate_tt_munching() {
    let numbers = parse_list!(1, 2, 3, 4, 5);
    println!("Parsed: {numbers:?}");
}

// ============================================================================
// Interview Patterns
// ============================================================================

/// Common macro patterns you'll see in interviews or production.

// 1. Variadic assertions
macro_rules! assert_all {
    ($($cond:expr),+ $(,)?) => {
        $(assert!($cond);)+
    };
}

// 2. Struct field iteration
macro_rules! impl_display_for_struct {
    ($struct_name:ident { $($field:ident),* $(,)? }) => {
        impl std::fmt::Display for $struct_name {
            fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(f, "{} {{ ", stringify!($struct_name))?;
                $(
                    write!(f, "{}: {:?}, ", stringify!($field), self.$field)?;
                )*
                write!(f, "}}")
            }
        }
    };
}

// 3. Test case generation
macro_rules! test_cases {
    ($test_name:ident: $($name:ident => $input:expr => $expected:expr),+ $(,)?) => {
        $(
            #[test]
            fn $name() {
                assert_eq!($test_name($input), $expected);
            }
        )+
    };
}

#[allow(dead_code)]
fn demonstrate_interview_patterns() {
    let x = 5;
    let y = 10;
    assert_all!(x > 0, y > 0, x < y);
    println!("All assertions passed!");
}
