// Suppress pedantic and nursery lints for design pattern examples.
#![allow(clippy::pedantic, clippy::nursery, unused)]
//! # Tower-Style Middleware / Service Composition
//!
//! Replaces: **Decorator Pattern** (OOP), **Interceptors** (Java EE), **Middleware** (Express/Koa)
//!
//! Real Rust usage: `tower`, `hyper`, `axum`, `tonic`
//!
//! ## Why this pattern exists in Rust
//! In Rust, asynchronous programming is based on the `Future` trait. The `Service` trait abstracts over "a function from Request to Future<Response>".
//! By designing middleware as types that wrap a `Service` and implement `Service` themselves, we can compose complex behavior (logging, timeouts, auth)
//! statically at compile time without the overhead of dynamic dispatch or inheritance.
//!
//! ## Architecture
//!
//! ```text
//! [ Request ] --> [ LoggingMiddleware ] --> [ AuthMiddleware ] --> [ DatabaseService ]
//!                                                                         |
//! [ Response ] <-- (Log Result) <-------- (Check Auth) <--------- (Process)
//! ```
//!
//! **Invariants:**
//! - Each layer wraps the inner service generic `S`.
//! - The outer layer implements `Service` by delegating to `S::call` and adding logic before/after awaiting the future.
//! - All layers share the same `Request` and `Response` types (or transform them compatibly).
//!
//! ## When to use
//! - When building web servers, RPC clients, or message processors.
//! - When you need cross-cutting concerns (logging, metrics, tracing) separated from business logic.

use std::future::Future;
use std::pin::Pin;

// ============================================================================
// The Service Trait
// ============================================================================

// COMPILE-TIME WIN: By using an associated type `Future`, we allow implementations
// to return their specific future type (zero-cost) or a Boxed future (dynamic dispatch),
// rather than forcing a specific one.
pub trait Service<Request> {
    type Response;
    type Error;

    // The future returned by the service.
    type Future: Future<Output = Result<Self::Response, Self::Error>>;

    // OWNERSHIP INSIGHT: `poll_ready` is typically used for backpressure (flow control),
    // but we'll omit it for this simplified example to focus on composition.
    // fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>>;

    fn call(&mut self, req: Request) -> Self::Future;
}

// ============================================================================
// Concrete Service (The Leaf)
// ============================================================================

#[derive(Clone)]
pub struct EchoService;

impl Service<String> for EchoService {
    type Response = String;
    type Error = String;
    // We use Pin<Box<...>> for simplicity here.
    // In high-perf code, we'd define a custom struct executing the state machine.
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn call(&mut self, req: String) -> Self::Future {
        Box::pin(async move {
            Ok(req) // Echo back
        })
    }
}

// ============================================================================
// Middleware 1: Logging
// ============================================================================

pub struct LoggingMiddleware<S> {
    inner: S,
}

impl<S> LoggingMiddleware<S> {
    pub fn new(inner: S) -> Self {
        LoggingMiddleware { inner }
    }
}

impl<S, Request> Service<Request> for LoggingMiddleware<S>
where
    S: Service<Request> + Send + 'static, // Inner service must be Send for async
    S::Future: Send,
    Request: std::fmt::Debug + Send + 'static,
    S::Response: std::fmt::Debug,
    S::Error: std::fmt::Debug,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn call(&mut self, req: Request) -> Self::Future {
        println!("LOG: Received request: {:?}", req);

        // TRADEOFF: We must move `self.inner` or call it. `call` takes `&mut self`.
        // To await the future, we get it from inner.
        let fut = self.inner.call(req);

        Box::pin(async move {
            let res = fut.await;
            println!("LOG: Response: {:?}", res);
            res
        })
    }
}

// ============================================================================
// Middleware 2: Validation (Blocker)
// ============================================================================

pub struct ValidationMiddleware<S> {
    inner: S,
}

impl<S> ValidationMiddleware<S> {
    pub fn new(inner: S) -> Self {
        ValidationMiddleware { inner }
    }
}

impl<S> Service<String> for ValidationMiddleware<S>
where
    S: Service<String> + Send + 'static,
    S::Future: Send,
    S::Error: From<String>,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn call(&mut self, req: String) -> Self::Future {
        if req.is_empty() {
            // Short-circuit: Return a ready future with error
            return Box::pin(async move { Err("Request cannot be empty".to_string().into()) });
        }

        // Forward to inner
        Box::pin(self.inner.call(req))
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // A minimal async runtime for testing
    fn block_on<F: Future>(future: F) -> F::Output {
        use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

        unsafe fn clone(_: *const ()) -> RawWaker {
            RawWaker::new(std::ptr::null(), &VTABLE)
        }
        unsafe fn wake(_: *const ()) {}
        unsafe fn wake_by_ref(_: *const ()) {}
        unsafe fn drop(_: *const ()) {}

        static VTABLE: RawWakerVTable = RawWakerVTable::new(clone, wake, wake_by_ref, drop);
        let raw_waker = RawWaker::new(std::ptr::null(), &VTABLE);
        let waker = unsafe { Waker::from_raw(raw_waker) };
        let mut cx = Context::from_waker(&waker);

        let mut future = Box::pin(future);
        loop {
            match future.as_mut().poll(&mut cx) {
                Poll::Ready(val) => return val,
                Poll::Pending => continue, // Spin loop (ok for this simple test)
            }
        }
    }

    #[test]
    fn test_service_composition() {
        // Stack: Validation -> Logging -> Echo
        let service = EchoService;
        let service = LoggingMiddleware::new(service);
        let mut service = ValidationMiddleware::new(service);

        let res = block_on(service.call("Hello".to_string()));
        assert_eq!(res.unwrap(), "Hello");
    }

    #[test]
    fn test_validation_failure() {
        let service = EchoService;
        let mut service = ValidationMiddleware::new(service);

        let res = block_on(service.call("".to_string()));
        assert!(res.is_err());
    }
}
