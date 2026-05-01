//! # Structured Logging & Tracing Implementation
//!
//! Implements a minimalist structured logging and tracing system with implicit context propagation.
//!
//! **Replaces Crates:** `tracing`, `tracing-core`, `tracing-subscriber`, `log`
//!
//! **Real-world Usage:**
//! - Distributed tracing in microservices (OpenTelemetry, Jaeger).
//! - Structured logging for centralized log aggregation (Elasticsearch, Datadog).
//! - Request-scoped context without passing `Context` structs to every function.
//!
//! **Why build it yourself?**
//! Understanding how `tracing` works under the hood demystifies "magic" thread-local context.
//! It teaches you about `std::thread_local!`, RAII guards for state management,
//! and how to decouple event emission from event collection using the `Subscriber` pattern.

use std::cell::RefCell;
use std::fmt;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

// =========================================================================================
// Architecture
// =========================================================================================
//
// Flow:
//
//      Client Code
//        │
//        ├──► span!("request", id=123) ──► Creates SpanGuard, pushes Span to Thread-Local Stack
//        │                                    │
//        ├──► event!("DB query starting") ──► Emits Event + current TLS Stack to Global Subscriber
//        │                                    │
//        └──► Drop SpanGuard ───────────────► Pops Span from Thread-Local Stack
//
// Invariants:
// 1. Spans strictly form a stack per thread.
// 2. Events inherit all fields from the active spans in their thread's stack.
// 3. The global subscriber must be thread-safe (Send + Sync).
//
// Complexity:
// ┌───────────────┬─────────────┬─────────────┐
// │ Operation     │ Time        │ Space       │
// ├───────────────┼─────────────┼─────────────┤
// │ enter_span    │ O(1)        │ O(1) TLS    │
// │ exit_span     │ O(1)        │ O(1)        │
// │ emit_event    │ O(Depth)    │ O(1)        │
// └───────────────┴─────────────┴─────────────┘

/// Represents a key-value pair for structured logging.
#[derive(Clone, Debug)]
pub struct Field {
    pub key: &'static str,
    pub value: String, // Simplified: Real tracing uses a Visitor pattern for zero-allocation fields.
}

/// Represents a logical unit of work (e.g., a request, a database transaction).
#[derive(Clone, Debug)]
pub struct Span {
    pub name: &'static str,
    pub fields: Vec<Field>,
}

/// Represents a discrete moment in time (e.g., a log message).
#[derive(Clone, Debug)]
pub struct Event {
    pub message: String,
    pub level: Level,
    pub fields: Vec<Field>,
    pub timestamp: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl fmt::Display for Level {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Level::Trace => write!(f, "TRACE"),
            Level::Debug => write!(f, "DEBUG"),
            Level::Info => write!(f, "INFO "),
            Level::Warn => write!(f, "WARN "),
            Level::Error => write!(f, "ERROR"),
        }
    }
}

/// The core trait that consumes events.
pub trait Subscriber: Send + Sync {
    /// Records an event, along with the current span context.
    fn event(&self, event: &Event, span_context: &[Span]);
}

// =========================================================================================
// Global State & Thread Local Storage
// =========================================================================================

// Global subscriber that receives all events.
// We use a Box<dyn Subscriber> protected by a Mutex for safe global access.
// RUST INSIGHT: `std::sync::OnceLock` or `RwLock` is usually better here, but `Mutex` is simple.
static GLOBAL_SUBSCRIBER: Mutex<Option<Arc<dyn Subscriber>>> = Mutex::new(None);

thread_local! {
    /// Thread-local span stack. This is the "magic" that prevents us from passing `ctx` everywhere.
    static CURRENT_SPANS: RefCell<Vec<Span>> = const { RefCell::new(Vec::new()) };
}

/// Sets the global subscriber.
pub fn set_global_subscriber(subscriber: impl Subscriber + 'static) {
    let mut global = GLOBAL_SUBSCRIBER.lock().unwrap();
    *global = Some(Arc::new(subscriber));
}

/// Emits an event to the global subscriber, attaching the current thread's span context.
pub fn dispatch_event(level: Level, message: impl Into<String>, fields: Vec<Field>) {
    let global = GLOBAL_SUBSCRIBER.lock().unwrap();
    if let Some(subscriber) = global.as_ref() {
        let event = Event {
            message: message.into(),
            level,
            fields,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
        };

        // Access the thread-local span stack.
        CURRENT_SPANS.with(|spans| {
            subscriber.event(&event, &spans.borrow());
        });
    }
}

// =========================================================================================
// Span Management (RAII)
// =========================================================================================

/// An RAII guard that represents an entered span.
/// When it drops, the span is popped from the thread-local stack.
pub struct SpanGuard {
    // We keep track of the depth to ensure we don't pop someone else's span
    // if guards are somehow dropped out of order (though Rust's drop rules usually prevent this).
    depth: usize,
}

impl SpanGuard {
    /// Pushes a new span onto the thread-local stack and returns a guard.
    #[must_use = "SpanGuard must be kept alive to maintain the span context"]
    pub fn new(name: &'static str, fields: Vec<Field>) -> Self {
        let span = Span { name, fields };
        CURRENT_SPANS.with(|spans| {
            let mut stack = spans.borrow_mut();
            stack.push(span);
            SpanGuard { depth: stack.len() }
        })
    }
}

impl Drop for SpanGuard {
    fn drop(&mut self) {
        CURRENT_SPANS.with(|spans| {
            let mut stack = spans.borrow_mut();
            // GOTCHA: We must only pop if we are the top of the stack.
            // If panics or async yielding occurs, the stack might be in a weird state.
            if stack.len() == self.depth {
                stack.pop();
            } else {
                // In a production tracing crate (like for async), spans are not just a simple Vec
                // because async tasks can yield and be resumed on different threads.
                // For a purely synchronous system, this branch shouldn't be hit unless panic-unwinding
                // drops things in a weird order, but even then, block scopes enforce LIFO drop order.
            }
        });
    }
}

// =========================================================================================
// Formatting Subscriber
// =========================================================================================

/// A simple subscriber that formats and prints logs to stdout.
pub struct FmtSubscriber;

impl Subscriber for FmtSubscriber {
    fn event(&self, event: &Event, span_context: &[Span]) {
        use std::fmt::Write;

        // ⚡ BOLT OPTIMIZATION: Avoid intermediate `Vec` and `String` allocations
        // by formatting directly into a pre-allocated `String` buffer.
        let mut output = String::with_capacity(256);

        let _ = write!(output, "{} ", event.level);

        if !span_context.is_empty() {
            output.push('[');
            for (i, span) in span_context.iter().enumerate() {
                if i > 0 {
                    output.push_str(" -> ");
                }
                output.push_str(span.name);
            }
            output.push_str("] ");
        }

        output.push_str(&event.message);

        if !event.fields.is_empty() {
            output.push_str(" (");
            for (i, field) in event.fields.iter().enumerate() {
                if i > 0 {
                    output.push_str(", ");
                }
                let _ = write!(output, "{}={}", field.key, field.value);
            }
            output.push(')');
        }

        let mut first_span_field = true;
        for span in span_context {
            for field in &span.fields {
                if first_span_field {
                    output.push_str(" {");
                    first_span_field = false;
                } else {
                    output.push_str(", ");
                }
                let _ = write!(output, "{}={}", field.key, field.value);
            }
        }
        if !first_span_field {
            output.push('}');
        }

        let _ = write!(output, " {}", event.timestamp);

        println!("{}", output);
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `tracing`: The canonical crate handles async/await seamlessly using `tracing-futures` and `Instrument` traits.
//   It uses a lock-free slab allocator for span IDs rather than a simple thread-local Vec, allowing spans to
//   move across threads when an async task yields and resumes.
// - `log`: Only handles unstructured events without span hierarchies.
//
// Missing vs. Production:
// - **Async Support**: This simple `Vec` approach fails for async code because spans would leak across `.await` points
//   if tasks are multiplexed on the same thread.
// - **Zero-Allocation Fields**: We use `String` for simplicity. Real crates use `&dyn fmt::Debug` and Visitors to defer formatting.
// - **Macros**: We use functions. Real tracing uses `tracing::info!()` and `#[instrument]` to capture file/line numbers automatically.
//
// Next Steps:
// 1. Add procedural macros `#[instrument]` to automatically wrap functions in spans.
// 2. Implement an `Instrument` trait for Futures to carry span context across `.await` boundaries.
//
// Benchmarking Note:
// To measure overhead of `SpanGuard` and TLS access, use `criterion` to benchmark creating/dropping
// spans in a tight loop versus standard string allocation or the real `tracing` crate.

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct TestSubscriber {
        event_count: Arc<AtomicUsize>,
        span_depth_sum: Arc<AtomicUsize>,
    }

    impl Subscriber for TestSubscriber {
        fn event(&self, _event: &Event, span_context: &[Span]) {
            self.event_count.fetch_add(1, Ordering::SeqCst);
            self.span_depth_sum
                .fetch_add(span_context.len(), Ordering::SeqCst);
        }
    }

    #[test]
    fn test_tracing_basic_flow() {
        let event_count = Arc::new(AtomicUsize::new(0));
        let span_depth_sum = Arc::new(AtomicUsize::new(0));

        set_global_subscriber(TestSubscriber {
            event_count: Arc::clone(&event_count),
            span_depth_sum: Arc::clone(&span_depth_sum),
        });

        // Outside any span
        dispatch_event(Level::Info, "App started", vec![]);
        assert_eq!(span_depth_sum.load(Ordering::SeqCst), 0); // depth 0

        {
            let _span1 = SpanGuard::new(
                "request",
                vec![Field {
                    key: "req_id",
                    value: "1".to_string(),
                }],
            );

            // Inside span 1
            dispatch_event(Level::Debug, "Validating", vec![]);
            assert_eq!(span_depth_sum.load(Ordering::SeqCst), 1); // 0 + 1 = 1

            {
                let _span2 = SpanGuard::new("db_query", vec![]);

                // Inside span 2
                dispatch_event(Level::Trace, "Executing SQL", vec![]);
                assert_eq!(span_depth_sum.load(Ordering::SeqCst), 3); // 1 + 2 = 3
            }

            // Back to span 1
            dispatch_event(Level::Info, "Validation complete", vec![]);
            assert_eq!(span_depth_sum.load(Ordering::SeqCst), 4); // 3 + 1 = 4
        }

        // Outside again
        dispatch_event(Level::Info, "App stopping", vec![]);
        assert_eq!(span_depth_sum.load(Ordering::SeqCst), 4); // 4 + 0 = 4

        assert_eq!(event_count.load(Ordering::SeqCst), 5);
    }
}
