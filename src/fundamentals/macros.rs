//! # Macro Patterns
//!
//! Declarative macros (`macro_rules`!) for compile-time metaprogramming.
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

// `_y` demonstrates the `stmt` fragment specifier for gittype practice
#[allow(dead_code, clippy::no_effect_underscore_binding)]
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

// The my_vec! macro deliberately expands to Vec::new()+push to demonstrate macro repetition
#[allow(dead_code, clippy::vec_init_then_push)]
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

    // Generate a struct definition from a field list.
    create_struct!(Point { x: i32, y: i32 });
    let p = Point { x: 1, y: 2 };
    println!("Point: ({}, {})", p.x, p.y);
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
        const fn $name($($param: $ptype),*) -> $rtype $body
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

    // Generate a function (with and without a return type).
    create_function!(add(a: i32, b: i32) -> i32 { a + b });
    create_function!(greet(name: &str) { println!("Hello, {name}!"); });
    println!("add(2, 3) = {}", add(2, 3));
    greet("world");
}

// ============================================================================
// Practical Macro Patterns
// ============================================================================

/// `HashMap` literal macro (like `serde_json::json`!).
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
            const fn as_str(&self) -> &'static str {
                match self {
                    $(
                        $name::$variant => stringify!($variant),
                    )*
                }
            }

            const fn variants() -> &'static [$name] {
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

    // Enum with generated string conversion.
    string_enum!(Direction {
        North,
        East,
        South,
        West,
    });
    println!("First direction: {}", Direction::North.as_str());
    println!("All directions: {:?}", Direction::variants());
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

    // HTML-like DSL
    let markup = html!(div { "hello" });
    println!("HTML: {markup}");
}

// ============================================================================
// Recursive Macros
// ============================================================================

/// Recursive macro for reversing arguments.
///
/// Recurse on the tail first, then `push` the head so it lands at the back.
/// (Using `insert(0, $x)` instead would rebuild the original order, not reverse it.)
macro_rules! reverse {
    // Base case: single element
    ($x:expr) => { vec![$x] };

    // Recursive case: reverse the rest, then append the head at the end.
    ($x:expr, $($rest:expr),+) => {{
        let mut v = reverse!($($rest),+);
        v.push($x);
        v
    }};
}

/// Factorial macro.
///
/// A naive recursive version like `($n:expr) => { $n * factorial!($n - 1) }`
/// never terminates: `$n - 1` is an *expression*, so it never matches the
/// literal `0` base-case arm and the macro expands forever. Instead, expand
/// to a runtime expression that folds the range `1..=n`.
macro_rules! factorial {
    ($n:expr) => {{
        let n: u64 = $n;
        (1..=n).product::<u64>()
    }};
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
    // Tree-building DSL. `tree!` expands to `Node::Leaf` / `Node::Branch`,
    // so a `Node` type must be in scope.
    #[derive(Debug)]
    enum Node {
        Leaf(i32),
        Branch { value: i32, children: Vec<Self> },
    }

    let reversed = reverse!(1, 2, 3, 4, 5);
    println!("Reversed: {reversed:?}");

    // Compile-then-run factorial.
    let f = factorial!(5);
    println!("5! = {f}");

    let leaf = tree!(9);
    let branch = tree!(1 => { tree!(2), tree!(3) });
    println!("Tree leaf: {leaf:?}");
    println!("Tree branch: {branch:?}");
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
        // $crate refers to the crate where the macro is defined, so this
        // resolves no matter which module (or downstream crate) invokes it.
        $crate::fundamentals::macros::demonstrate_basic_macros()
    };
}

#[allow(dead_code)]
fn demonstrate_hygiene() {
    let temp = 5;
    let result = hygienic_macro!(10); // Uses internal temp, not outer
    println!("temp: {temp}, result: {result}");

    // Invoke an item through the `$crate` path.
    use_crate_item!();
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

// Use case for cargo expand:
// ```bash
// cargo install cargo-expand
// cargo expand --lib fundamentals::macros
// ```
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

// Example: This could be a macro OR a generic function.
// Use macro when you need:
// - Compile-time evaluation
// - Variable number of arguments
// - Code generation based on types
// - DSLs and syntax extensions

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
fn max_function<T: Ord + Copy>(first: T, rest: &[T]) -> T {
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

// Recreating common std macros for learning.

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

// my_vec_full! deliberately expands to Vec::new()+push to demonstrate std-macro recreation
#[allow(dead_code, clippy::vec_init_then_push)]
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

// parse_list! deliberately expands to Vec::new()+push to demonstrate TT munching
#[allow(dead_code, clippy::vec_init_then_push)]
fn demonstrate_tt_munching() {
    let numbers = parse_list!(1, 2, 3, 4, 5);
    println!("Parsed: {numbers:?}");
}

// ============================================================================
// Interview Patterns
// ============================================================================

// Common macro patterns you'll see in interviews or production.

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
    // Generate a Display impl from a field list.
    #[derive(Debug)]
    struct Pair {
        a: i32,
        b: i32,
    }
    impl_display_for_struct!(Pair { a, b });

    let x = 5;
    let y = 10;
    assert_all!(x > 0, y > 0, x < y);
    println!("All assertions passed!");

    println!("Pair display: {}", Pair { a: 1, b: 2 });
}

// `test_cases!` generates `#[test]` functions from a table of inputs/outputs.
#[allow(dead_code)]
const fn square_for_demo(x: i32) -> i32 {
    x * x
}

test_cases!(square_for_demo:
    demo_square_of_two => 2 => 4,
    demo_square_of_three => 3 => 9,
);

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    // Recursive / computation macros
    #[test]
    fn reverse_reverses_order() {
        assert_eq!(reverse!(1, 2, 3), vec![3, 2, 1]);
        assert_eq!(reverse!(1, 2, 3, 4, 5), vec![5, 4, 3, 2, 1]);
        // Single-element base case.
        assert_eq!(reverse!(42), vec![42]);
    }

    #[test]
    fn factorial_computes_and_terminates() {
        assert_eq!(factorial!(0), 1);
        assert_eq!(factorial!(1), 1);
        assert_eq!(factorial!(5), 120);
        assert_eq!(factorial!(10), 3_628_800);
    }

    // `create_struct!` generates a struct definition.
    #[test]
    fn create_struct_defines_fields() {
        create_struct!(Point { x: i32, y: i32 });

        let p = Point { x: 3, y: 4 };
        assert_eq!(p.x, 3);
        assert_eq!(p.y, 4);
    }

    // `create_function!` generates functions, with and without a return type.
    #[test]
    fn create_function_generates_functions() {
        create_function!(add(a: i32, b: i32) -> i32 { a + b });
        create_function!(store(value: i32, into: &mut i32) { *into = value; });

        assert_eq!(add(2, 3), 5);

        let mut slot = 0;
        store(9, &mut slot);
        assert_eq!(slot, 9);
    }

    // `string_enum!` generates an enum plus `as_str`/`variants` helpers.
    #[test]
    fn string_enum_generates_conversions() {
        string_enum!(Color { Red, Green, Blue });

        assert_eq!(Color::Red.as_str(), "Red");
        assert_eq!(Color::Green.as_str(), "Green");
        assert_eq!(Color::variants(), &[Color::Red, Color::Green, Color::Blue]);
        assert_eq!(Color::variants().len(), 3);
    }

    // `html!` builds a nested HTML string.
    #[test]
    fn html_builds_markup() {
        assert_eq!(html!(div { "hello" }), "<div>hello</div>");
        assert_eq!(html!(div { span { "x" } }), "<div><span>x</span></div>");
    }

    // `tree!` builds a recursive node structure.
    #[test]
    fn tree_builds_nodes() {
        #[derive(Debug, PartialEq)]
        enum Node {
            Leaf(i32),
            Branch { value: i32, children: Vec<Node> },
        }

        let leaf = tree!(7);
        assert_eq!(leaf, Node::Leaf(7));

        let branch = tree!(1 => { tree!(2), tree!(3) });
        assert_eq!(
            branch,
            Node::Branch {
                value: 1,
                children: vec![Node::Leaf(2), Node::Leaf(3)],
            }
        );
    }

    // `use_crate_item!` exercises the `$crate` path (just runs without panicking).
    #[test]
    fn use_crate_item_runs() {
        use_crate_item!();
    }

    // `impl_display_for_struct!` generates a `Display` implementation.
    #[test]
    fn impl_display_for_struct_formats() {
        struct Pair {
            a: i32,
            b: i32,
        }
        impl_display_for_struct!(Pair { a, b });

        let pair = Pair { a: 1, b: 2 };
        assert_eq!(format!("{pair}"), "Pair { a: 1, b: 2, }");
    }

    // `test_cases!` generates `#[test]` functions from a table.
    fn square(x: i32) -> i32 {
        x * x
    }

    test_cases!(square:
        square_of_two => 2 => 4,
        square_of_three => 3 => 9,
        square_of_negative => -4 => 16,
    );
}
