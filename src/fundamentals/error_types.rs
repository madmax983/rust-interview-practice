//! # Error Types and Error Design
//!
//! Comprehensive error handling patterns: custom error types, thiserror for
//! libraries, anyhow for applications, and error design best practices.

use std::error::Error;
use std::fmt;
use std::io;
use std::num::ParseIntError;

// ============================================================================
// Error Fundamentals Recap
// ============================================================================

#[allow(dead_code)]
fn demonstrate_error_basics() {
    // Option - represents presence or absence
    let opt: Option<i32> = Some(42);
    let _value = opt.unwrap_or(0);

    // Result - represents success or error
    let res: Result<i32, String> = Ok(42);
    match res {
        Ok(val) => println!("Success: {val}"),
        Err(e) => println!("Error: {e}"),
    }

    // Error trait - all errors implement this
    fn handle_error(err: &dyn Error) {
        println!("Error: {err}");
        println!("Debug: {err:?}");

        // Error source chain
        if let Some(source) = err.source() {
            println!("Caused by: {source}");
        }
    }

    let err = io::Error::new(io::ErrorKind::NotFound, "file not found");
    handle_error(&err);
}

// ============================================================================
// Custom Error Types (Manual Implementation)
// ============================================================================

/// Custom error enum - the foundation of error design.
#[derive(Debug)]
enum MyError {
    Io(io::Error),
    Parse(ParseIntError),
    Custom(String),
}

// Implement Display for user-facing messages
impl fmt::Display for MyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MyError::Io(err) => write!(f, "I/O error: {err}"),
            MyError::Parse(err) => write!(f, "Parse error: {err}"),
            MyError::Custom(msg) => write!(f, "Error: {msg}"),
        }
    }
}

// Implement Error trait
impl Error for MyError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            MyError::Io(err) => Some(err),
            MyError::Parse(err) => Some(err),
            MyError::Custom(_) => None,
        }
    }
}

// From conversions for ergonomic ? operator usage
impl From<io::Error> for MyError {
    fn from(err: io::Error) -> Self {
        MyError::Io(err)
    }
}

impl From<ParseIntError> for MyError {
    fn from(err: ParseIntError) -> Self {
        MyError::Parse(err)
    }
}

#[allow(dead_code)]
fn use_custom_error() -> Result<i32, MyError> {
    // ? operator automatically converts errors using From trait
    let s = "42";
    let n: i32 = s.parse()?; // ParseIntError -> MyError
    Ok(n)
}

// ============================================================================
// thiserror for Libraries
// ============================================================================

// Note: This section shows patterns. In real code, you'd use:
// #[derive(thiserror::Error, Debug)]

/// Library error pattern (shown without thiserror derive for clarity).
///
/// With thiserror:
/// ```ignore
/// use thiserror::Error;
///
/// #[derive(Error, Debug)]
/// pub enum DatabaseError {
///     #[error("Connection failed: {0}")]
///     ConnectionFailed(String),
///
///     #[error("Query failed: {query}")]
///     QueryFailed { query: String },
///
///     #[error("I/O error")]
///     Io(#[from] std::io::Error),
///
///     #[error(transparent)]
///     Other(#[from] anyhow::Error),
/// }
/// ```
#[derive(Debug)]
pub enum DatabaseError {
    ConnectionFailed(String),
    QueryFailed { query: String },
    Io(io::Error),
    Timeout,
}

impl fmt::Display for DatabaseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DatabaseError::ConnectionFailed(msg) => {
                write!(f, "Connection failed: {msg}")
            }
            DatabaseError::QueryFailed { query } => {
                write!(f, "Query failed: {query}")
            }
            DatabaseError::Io(err) => write!(f, "I/O error: {err}"),
            DatabaseError::Timeout => write!(f, "Operation timed out"),
        }
    }
}

impl Error for DatabaseError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            DatabaseError::Io(err) => Some(err),
            _ => None,
        }
    }
}

impl From<io::Error> for DatabaseError {
    fn from(err: io::Error) -> Self {
        DatabaseError::Io(err)
    }
}

/// thiserror patterns demonstration (pseudo-code).
#[allow(dead_code)]
fn demonstrate_thiserror_patterns() {
    // Pattern 1: Simple message
    // #[error("Something went wrong")]

    // Pattern 2: With field interpolation
    // #[error("Failed to process {filename}")]
    // FileError { filename: String }

    // Pattern 3: Automatic From with #[from]
    // #[error("I/O error")]
    // Io(#[from] std::io::Error)

    // Pattern 4: Transparent - delegates Display to inner error
    // #[error(transparent)]
    // Other(#[from] SomeError)

    // Pattern 5: Backtrace (nightly)
    // #[error("Operation failed")]
    // Failed {
    //     #[backtrace]
    //     backtrace: std::backtrace::Backtrace,
    // }
}

// ============================================================================
// anyhow for Applications
// ============================================================================

/// anyhow patterns (pseudo-code demonstration).
///
/// anyhow is perfect for applications where you want rich context
/// without defining custom error types.
///
/// ```ignore
/// use anyhow::{Context, Result};
///
/// fn read_config() -> Result<Config> {
///     let contents = std::fs::read_to_string("config.toml")
///         .context("Failed to read config file")?;
///
///     let config: Config = toml::from_str(&contents)
///         .with_context(|| format!("Failed to parse config from {}", "config.toml"))?;
///
///     Ok(config)
/// }
///
/// fn main() -> Result<()> {
///     let config = read_config()?;
///
///     // Error chains provide full context
///     if let Err(err) = dangerous_operation() {
///         eprintln!("Error: {err:?}"); // Shows full chain
///         eprintln!("Root cause: {}", err.root_cause());
///     }
///
///     Ok(())
/// }
/// ```
#[allow(dead_code)]
fn demonstrate_anyhow_patterns() {
    // Pattern 1: .context() - static string
    // result.context("Operation failed")?

    // Pattern 2: .with_context() - closure (lazy evaluation)
    // result.with_context(|| format!("Failed for id {}", id))?

    // Pattern 3: anyhow::bail! - early return with error
    // if invalid {
    //     anyhow::bail!("Invalid input: {}", input);
    // }

    // Pattern 4: anyhow::ensure! - assertion with error
    // anyhow::ensure!(x > 0, "x must be positive");

    // Pattern 5: Downcasting specific errors
    // if let Some(io_err) = err.downcast_ref::<std::io::Error>() {
    //     // Handle specific error type
    // }
}

// ============================================================================
// Error Composition and Variants
// ============================================================================

/// Well-designed error type with multiple variants.
#[derive(Debug)]
pub enum AppError {
    // Network errors
    NetworkTimeout,
    ConnectionRefused { host: String, port: u16 },

    // Validation errors
    InvalidInput { field: String, reason: String },
    MissingField(String),

    // Business logic errors
    InsufficientBalance { required: u64, available: u64 },
    Unauthorized { user_id: u64 },

    // Wrapped errors from dependencies
    Database(DatabaseError),
    Io(io::Error),

    // Internal errors (should not be exposed to users)
    Internal(String),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::NetworkTimeout => write!(f, "Network operation timed out"),
            AppError::ConnectionRefused { host, port } => {
                write!(f, "Connection refused: {host}:{port}")
            }
            AppError::InvalidInput { field, reason } => {
                write!(f, "Invalid input for field '{field}': {reason}")
            }
            AppError::MissingField(field) => {
                write!(f, "Missing required field: {field}")
            }
            AppError::InsufficientBalance {
                required,
                available,
            } => {
                write!(f, "Insufficient balance: need {required}, have {available}")
            }
            AppError::Unauthorized { user_id } => {
                write!(f, "Unauthorized: user {user_id}")
            }
            AppError::Database(err) => write!(f, "Database error: {err}"),
            AppError::Io(err) => write!(f, "I/O error: {err}"),
            AppError::Internal(msg) => write!(f, "Internal error: {msg}"),
        }
    }
}

impl Error for AppError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            AppError::Database(err) => Some(err),
            AppError::Io(err) => Some(err),
            _ => None,
        }
    }
}

impl From<DatabaseError> for AppError {
    fn from(err: DatabaseError) -> Self {
        AppError::Database(err)
    }
}

impl From<io::Error> for AppError {
    fn from(err: io::Error) -> Self {
        AppError::Io(err)
    }
}

// ============================================================================
// Error Metadata and Categories
// ============================================================================

/// Error with metadata for structured error handling.
#[derive(Debug)]
pub struct DetailedError {
    code: ErrorCode,
    message: String,
    context: Vec<(String, String)>,
}

#[derive(Debug, Clone, Copy)]
pub enum ErrorCode {
    ValidationError = 1000,
    AuthenticationError = 2000,
    AuthorizationError = 2001,
    NotFound = 3000,
    Conflict = 3001,
    InternalError = 5000,
}

impl DetailedError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        DetailedError {
            code,
            message: message.into(),
            context: Vec::new(),
        }
    }

    pub fn with_context(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.context.push((key.into(), value.into()));
        self
    }

    pub fn code(&self) -> ErrorCode {
        self.code
    }
}

impl fmt::Display for DetailedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.code as u32, self.message)?;
        for (key, value) in &self.context {
            write!(f, ", {key}={value}")?;
        }
        Ok(())
    }
}

impl Error for DetailedError {}

#[allow(dead_code)]
fn use_detailed_error() -> Result<(), DetailedError> {
    Err(
        DetailedError::new(ErrorCode::ValidationError, "Invalid email")
            .with_context("field", "email")
            .with_context("value", "not-an-email"),
    )
}

// ============================================================================
// Error Propagation Patterns
// ============================================================================

#[allow(dead_code)]
fn demonstrate_error_propagation() -> Result<i32, AppError> {
    // Pattern 1: ? operator - automatic conversion via From
    let _result = some_fallible_operation()?;

    // Pattern 2: map_err - transform error type
    let _result2 = other_operation().map_err(|e| AppError::Internal(e.to_string()))?;

    // Pattern 3: Wrapping with context
    let _result3 = another_operation().map_err(|_| AppError::MissingField("config".to_string()))?;

    Ok(42)
}

fn some_fallible_operation() -> Result<i32, AppError> {
    Ok(42)
}

fn other_operation() -> Result<i32, String> {
    Ok(42)
}

fn another_operation() -> Result<i32, ()> {
    Ok(42)
}

// ============================================================================
// Boxing Errors for Type Erasure
// ============================================================================

/// Using Box<dyn Error> when you don't want to define custom types.
#[allow(dead_code)]
fn boxed_error_example() -> Result<i32, Box<dyn Error>> {
    let s = "42";
    let n: i32 = s.parse()?; // Works with any error type
    Ok(n)
}

/// Box<dyn Error + Send + Sync> for thread-safe errors.
#[allow(dead_code)]
fn thread_safe_error() -> Result<i32, Box<dyn Error + Send + Sync>> {
    let s = "42";
    let n: i32 = s.parse()?;
    Ok(n)
}

// ============================================================================
// Library vs Application Error Design
// ============================================================================

/// Library error design principles.
///
/// Libraries should:
/// - Define specific error types for their domain
/// - Implement std::error::Error
/// - Provide From conversions for common error sources
/// - Document error variants in API docs
/// - Never use panic! in public APIs (use Result)
///
/// Use thiserror for ergonomic error definitions.
#[allow(dead_code)]
mod library_pattern {
    use std::error::Error;
    use std::fmt;

    #[derive(Debug)]
    pub enum LibraryError {
        InvalidConfiguration(String),
        OperationFailed,
    }

    impl fmt::Display for LibraryError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                LibraryError::InvalidConfiguration(msg) => {
                    write!(f, "Invalid configuration: {msg}")
                }
                LibraryError::OperationFailed => {
                    write!(f, "Operation failed")
                }
            }
        }
    }

    impl Error for LibraryError {}

    pub fn library_function() -> Result<(), LibraryError> {
        Ok(())
    }
}

/// Application error design principles.
///
/// Applications should:
/// - Focus on user-friendly error messages
/// - Provide rich context for debugging
/// - Log errors with full context
/// - Convert library errors to app errors at boundaries
/// - Use anyhow for flexibility
///
/// Use anyhow for rapid development and rich context.
#[allow(dead_code)]
mod application_pattern {
    // In real code:
    // use anyhow::{Context, Result};
    //
    // fn app_function() -> Result<()> {
    //     library_pattern::library_function()
    //         .context("Failed to initialize library")?;
    //     Ok(())
    // }
}

// ============================================================================
// Error Recovery Strategies
// ============================================================================

#[allow(dead_code)]
fn demonstrate_error_recovery() {
    // Strategy 1: Retry with backoff
    fn retry_operation() -> Result<i32, AppError> {
        for attempt in 1..=3 {
            match attempt_operation() {
                Ok(val) => return Ok(val),
                Err(e) if attempt < 3 => {
                    eprintln!("Attempt {attempt} failed: {e}, retrying...");
                    std::thread::sleep(std::time::Duration::from_millis(100 * attempt as u64));
                }
                Err(e) => return Err(e),
            }
        }
        unreachable!()
    }

    // Strategy 2: Fallback to default
    fn operation_with_fallback() -> i32 {
        attempt_operation().unwrap_or(42)
    }

    // Strategy 3: Graceful degradation
    fn operation_with_degradation() -> Result<i32, AppError> {
        match attempt_operation() {
            Ok(val) => Ok(val),
            Err(AppError::NetworkTimeout) => {
                // Use cached value
                Ok(0)
            }
            Err(e) => Err(e),
        }
    }

    let _ = retry_operation();
    let _ = operation_with_fallback();
    let _ = operation_with_degradation();
}

fn attempt_operation() -> Result<i32, AppError> {
    Ok(42)
}

// ============================================================================
// When to Panic vs Return Error
// ============================================================================

#[allow(dead_code)]
fn demonstrate_panic_vs_error() {
    // Use Result when:
    // - Error is expected and recoverable
    // - Caller should decide how to handle
    // - Error is part of the API contract
    fn divide(a: i32, b: i32) -> Result<i32, String> {
        if b == 0 {
            Err("Division by zero".to_string())
        } else {
            Ok(a / b)
        }
    }

    // Use panic! when:
    // - Error represents a bug (invariant violation)
    // - Error is unrecoverable
    // - During development/testing
    fn get_element(vec: &[i32], index: usize) -> i32 {
        if index >= vec.len() {
            panic!("Index {index} out of bounds");
        }
        vec[index]
    }

    // expect() for documenting why panic is acceptable
    let config = std::env::var("CONFIG_PATH").expect("CONFIG_PATH must be set");

    let _ = divide(10, 2);
    let _ = get_element(&[1, 2, 3], 0);
    let _ = config;
}

// ============================================================================
// Practical Error Patterns
// ============================================================================

/// Pattern 1: Error context chain.
#[allow(dead_code)]
fn read_user_file(user_id: u64) -> Result<String, AppError> {
    let path = format!("/users/{user_id}/profile.txt");
    std::fs::read_to_string(&path)
        .map_err(|e| AppError::Internal(format!("Failed to read profile for user {user_id}: {e}")))
}

/// Pattern 2: Multiple error sources with match.
#[allow(dead_code)]
fn process_request() -> Result<(), AppError> {
    match validate_request() {
        Ok(()) => {}
        Err(ValidationError::Missing(field)) => {
            return Err(AppError::MissingField(field));
        }
        Err(ValidationError::Invalid(field, reason)) => {
            return Err(AppError::InvalidInput { field, reason });
        }
    }
    Ok(())
}

#[derive(Debug)]
enum ValidationError {
    Missing(String),
    Invalid(String, String),
}

fn validate_request() -> Result<(), ValidationError> {
    Ok(())
}

/// Pattern 3: Logging errors before returning.
#[allow(dead_code)]
fn logged_operation() -> Result<i32, AppError> {
    match risky_operation() {
        Ok(val) => Ok(val),
        Err(e) => {
            eprintln!("Operation failed: {e:?}");
            // Log with full context including source chain
            let mut source = e.source();
            while let Some(err) = source {
                eprintln!("  Caused by: {err}");
                source = err.source();
            }
            Err(e)
        }
    }
}

fn risky_operation() -> Result<i32, AppError> {
    Ok(42)
}

// ============================================================================
// Interview Patterns
// ============================================================================

/// Common error handling interview patterns.
#[allow(dead_code)]
fn interview_patterns() {
    // Pattern 1: Early return with ?
    fn process() -> Result<i32, String> {
        let a = step1()?;
        let b = step2()?;
        Ok(a + b)
    }

    fn step1() -> Result<i32, String> {
        Ok(1)
    }

    fn step2() -> Result<i32, String> {
        Ok(2)
    }

    // Pattern 2: Collecting Results
    fn process_all(items: Vec<i32>) -> Result<Vec<i32>, String> {
        items.iter().map(|&x| validate(x)).collect()
    }

    fn validate(x: i32) -> Result<i32, String> {
        if x > 0 {
            Ok(x)
        } else {
            Err("negative".to_string())
        }
    }

    // Pattern 3: Option to Result conversion
    fn get_config(key: &str) -> Result<String, String> {
        std::env::var(key).map_err(|_| format!("Missing config: {key}"))
    }

    let _ = process();
    let _ = process_all(vec![1, 2, 3]);
    let _ = get_config("HOME");
}
