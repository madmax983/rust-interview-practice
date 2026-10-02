//! End-to-end tests: the macros expand inside real code, and the generated code runs.
//!
//! The unit tests in `fundamentals::proc_macros` check the tokens; these check behavior.

use std::cell::Cell;
use std::sync::atomic::{AtomicUsize, Ordering};

use rust_interview_practice::fundamentals::proc_macros::{BuilderError, Describe};
use rust_interview_practice_macros::{Builder, Describe, checked, memoize, retry, seq};

// ---- #[derive(Describe)] -------------------------------------------------

#[derive(Describe)]
#[allow(dead_code)]
struct Account {
    id: u64,
    #[describe(rename = "kind")]
    r#type: String,
    #[describe(skip)]
    password_hash: String,
}

#[derive(Describe)]
#[allow(dead_code)]
struct Pair(u8, u8);

#[derive(Describe)]
#[allow(dead_code)]
enum Shape {
    Circle {
        radius: f64,
    },
    Square(f64),
    #[describe(rename = "dot")]
    Point,
}

#[derive(Describe)]
#[allow(dead_code)]
struct Wrapper<'a, T: Clone>
where
    Vec<T>: Default,
{
    inner: &'a T,
}

#[test]
fn describe_struct_applies_rename_skip_and_unraw() {
    assert_eq!(Account::NAME, "Account");
    assert_eq!(Account::MEMBERS, ["id", "kind"]);
}

#[test]
fn describe_tuple_struct_uses_indices() {
    assert_eq!(Pair::MEMBERS, ["0", "1"]);
}

#[test]
fn describe_enum_lists_variants() {
    assert_eq!(Shape::NAME, "Shape");
    assert_eq!(Shape::MEMBERS, ["Circle", "Square", "dot"]);
}

#[test]
fn describe_generic_struct() {
    assert_eq!(<Wrapper<'_, u8> as Describe>::NAME, "Wrapper");
    assert_eq!(<Wrapper<'_, String>>::MEMBERS, ["inner"]);
}

// ---- #[derive(Builder)] --------------------------------------------------

#[derive(Builder, Debug, PartialEq, Eq)]
pub struct Command {
    executable: String,
    #[builder(each = "arg")]
    args: Vec<String>,
    #[builder(each = "env")]
    envs: Vec<(String, String)>,
    current_dir: Option<String>,
    #[builder(default)]
    retries: u8,
}

#[test]
fn builder_happy_path() {
    let command = Command::builder()
        .executable("cargo")
        .arg("build")
        .arg(String::from("--release"))
        .env(("RUST_LOG".to_string(), "info".to_string()))
        .current_dir("/tmp")
        .retries(2)
        .build()
        .expect("all required fields set");
    assert_eq!(
        command,
        Command {
            executable: "cargo".into(),
            args: vec!["build".into(), "--release".into()],
            envs: vec![("RUST_LOG".into(), "info".into())],
            current_dir: Some("/tmp".into()),
            retries: 2,
        }
    );
}

#[test]
fn builder_optional_each_and_default_fields_may_be_omitted() {
    let command = Command::builder()
        .executable("ls")
        .build()
        .expect("only executable is required");
    assert!(command.args.is_empty());
    assert_eq!(command.current_dir, None);
    assert_eq!(command.retries, 0);
}

#[test]
fn builder_whole_vec_setter_replaces_pushed_elements() {
    let command = Command::builder()
        .executable("echo")
        .arg("dropped")
        .args(vec!["kept".to_string()])
        .build()
        .expect("valid");
    assert_eq!(command.args, ["kept"]);
}

#[test]
fn builder_missing_required_field_is_an_error() {
    let error = Command::builder()
        .arg("x")
        .build()
        .expect_err("executable missing");
    assert_eq!(
        error,
        BuilderError {
            field: "executable"
        }
    );
    assert_eq!(error.to_string(), "missing required field `executable`");
}

#[test]
fn builder_last_setter_call_wins() {
    let command = Command::builder()
        .executable("a")
        .executable("b")
        .build()
        .expect("valid");
    assert_eq!(command.executable, "b");
}

#[derive(Builder, Debug)]
struct Borrowed<'a, T>
where
    T: Clone,
{
    name: &'a str,
    values: Vec<T>,
}

#[test]
fn builder_supports_lifetimes_and_generics() {
    let built = Borrowed::<u8>::builder()
        .name("bytes")
        .values(vec![1, 2])
        .build()
        .expect("valid");
    assert_eq!((built.name, built.values), ("bytes", vec![1, 2]));
    let error = Borrowed::<u8>::builder()
        .name("no values")
        .build()
        .expect_err("values missing");
    assert_eq!(error.field, "values");
}

// ---- #[retry(times = N)] -------------------------------------------------

#[retry(times = 3)]
fn flaky(calls: &Cell<u32>, succeed_on: u32) -> Result<u32, String> {
    calls.set(calls.get() + 1);
    let attempt = calls.get(); // shadows nothing: the macro's own `attempt` is mixed_site
    if attempt < succeed_on {
        return Err(format!("attempt {attempt} failed"));
    }
    Ok(attempt)
}

fn parse_positive(text: &str) -> Result<u32, String> {
    text.parse::<u32>().map_err(|e| e.to_string())
}

#[retry(times = 2)]
fn parse_with_question_mark(text: &str, calls: &Cell<u32>) -> Result<u32, String> {
    calls.set(calls.get() + 1);
    let value = parse_positive(text)?; // `?` ends one attempt, not the whole retry loop
    Ok(value * 2)
}

#[test]
fn retry_succeeds_within_budget() {
    let calls = Cell::new(0);
    assert_eq!(flaky(&calls, 3), Ok(3));
    assert_eq!(calls.get(), 3);
}

#[test]
fn retry_stops_after_first_success() {
    let calls = Cell::new(0);
    assert_eq!(flaky(&calls, 1), Ok(1));
    assert_eq!(calls.get(), 1);
}

#[test]
fn retry_returns_last_error_when_budget_exhausted() {
    let calls = Cell::new(0);
    assert_eq!(flaky(&calls, 10), Err("attempt 3 failed".to_string()));
    assert_eq!(calls.get(), 3);
}

#[test]
fn retry_question_mark_retries_too() {
    let calls = Cell::new(0);
    assert!(parse_with_question_mark("nope", &calls).is_err());
    assert_eq!(calls.get(), 2);
    let calls = Cell::new(0);
    assert_eq!(parse_with_question_mark("21", &calls), Ok(42));
    assert_eq!(calls.get(), 1);
}

// ---- #[memoize] ----------------------------------------------------------

static FIB_CALLS: AtomicUsize = AtomicUsize::new(0);

#[memoize]
fn fib(n: u64) -> u64 {
    FIB_CALLS.fetch_add(1, Ordering::Relaxed);
    if n < 2 { n } else { fib(n - 1) + fib(n - 2) }
}

static LEN_CALLS: AtomicUsize = AtomicUsize::new(0);

#[memoize]
fn shout(mut word: String) -> String {
    LEN_CALLS.fetch_add(1, Ordering::Relaxed);
    word.make_ascii_uppercase();
    word
}

#[test]
fn memoize_makes_recursive_fib_linear() {
    // Without the cache this is ~2^90 calls; with it, one call per distinct n.
    assert_eq!(fib(90), 2_880_067_194_370_816_120);
    assert_eq!(FIB_CALLS.load(Ordering::Relaxed), 91);
    assert_eq!(fib(90), 2_880_067_194_370_816_120);
    assert_eq!(
        FIB_CALLS.load(Ordering::Relaxed),
        91,
        "second call is a cache hit"
    );
}

#[test]
fn memoize_works_with_owned_and_mut_arguments() {
    assert_eq!(shout("hi".to_string()), "HI");
    assert_eq!(shout("hi".to_string()), "HI");
    assert_eq!(shout("yo".to_string()), "YO");
    assert_eq!(LEN_CALLS.load(Ordering::Relaxed), 2);
}

thread_local! {
    static TRIANGLE_CALLS: Cell<u32> = const { Cell::new(0) };
}

#[memoize]
fn triangle(n: u32) -> u32 {
    TRIANGLE_CALLS.with(|calls| calls.set(calls.get() + 1));
    if n == 0 { 0 } else { n + triangle(n - 1) }
}

#[test]
fn memoize_cache_is_per_thread() {
    assert_eq!(triangle(10), 55);
    assert_eq!(TRIANGLE_CALLS.with(Cell::get), 11);
    // `thread_local!` cache: another thread starts cold, so it recomputes all 11 values.
    let from_thread = std::thread::spawn(|| (triangle(10), TRIANGLE_CALLS.with(Cell::get)))
        .join()
        .expect("thread ran");
    assert_eq!(from_thread, (55, 11));
    assert_eq!(
        TRIANGLE_CALLS.with(Cell::get),
        11,
        "this thread's cache was untouched"
    );
}

// ---- #[checked] ----------------------------------------------------------

#[checked]
fn affine(a: u8, x: u8, b: u8) -> Option<u8> {
    Some(a * x + b)
}

#[checked]
fn sum(values: &[u8]) -> Option<u8> {
    let mut total: u8 = 0;
    for &value in values {
        total += value;
    }
    Some(total)
}

#[checked]
fn double_minus_one(x: u8) -> Option<u8> {
    // `2 * x` is swapped to `(x).checked_mul(2)?`, which then is the typed left operand of `- 1`.
    Some(2 * x - 1)
}

#[checked]
#[allow(clippy::unnecessary_wraps)] // `#[checked]` requires an `Option` return
fn closure_is_untouched(x: u8) -> Option<u8> {
    let wrap = |v: u8| v.wrapping_add(250);
    let unchecked = |v: u8| v + 1; // closures keep plain arithmetic
    Some(wrap(unchecked(x)))
}

#[test]
fn checked_returns_some_without_overflow() {
    assert_eq!(affine(3, 4, 5), Some(17));
    assert_eq!(sum(&[1, 2, 3]), Some(6));
    assert_eq!(double_minus_one(10), Some(19));
}

#[test]
fn checked_returns_none_on_overflow() {
    assert_eq!(affine(16, 16, 0), None, "mul overflows");
    assert_eq!(affine(1, 255, 1), None, "add overflows");
    assert_eq!(sum(&[200, 100]), None, "compound += overflows");
    assert_eq!(double_minus_one(0), None, "sub underflows");
    assert_eq!(double_minus_one(128), None, "swapped mul overflows");
}

#[test]
fn checked_does_not_rewrite_closures() {
    assert_eq!(closure_is_untouched(10), Some(5));
}

// ---- seq! ----------------------------------------------------------------

// `seq!` gives each substituted literal the span of the `N` it replaced, so lints see
// user code like `0 * 0` — the same thing a `macro_rules!` user would see.
#[allow(clippy::erasing_op, clippy::identity_op)]
mod squares {
    use super::seq;

    seq!(N in 0..4 {
        pub const fn square~N() -> u64 {
            N * N
        }
    });
}
use squares::{square0, square1, square2, square3};

#[test]
fn seq_generates_items_with_pasted_names() {
    assert_eq!([square0(), square1(), square2(), square3()], [0, 1, 4, 9]);
}

#[test]
fn seq_in_statement_position() {
    let mut total = 0;
    seq!(I in 1..=10 {
        total += I;
    });
    assert_eq!(total, 55);
}

#[test]
#[allow(clippy::erasing_op, clippy::identity_op, clippy::vec_init_then_push)]
fn seq_substitutes_inside_nested_groups() {
    let mut rows = Vec::new();
    seq!(R in 0..3 {
        rows.push([R, R * 10, (R + 1) * 100]);
    });
    assert_eq!(rows, [[0, 0, 100], [1, 10, 200], [2, 20, 300]]);
}

#[test]
fn seq_with_empty_range_generates_nothing() {
    #[allow(unused_mut)] // the body is never emitted, so nothing assigns `touched`
    let mut touched = false;
    seq!(N in 5..5 {
        touched = N > 0;
    });
    assert!(!touched);
}
