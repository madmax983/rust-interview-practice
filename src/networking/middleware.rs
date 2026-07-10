//! # Middleware & Service Architecture
//!
//! Implements a composable middleware chain using the Service and Layer patterns.
//! It allows wrapping request/response handlers with reusable logic such as logging,
//! timeouts, authorization, and retries.
//!
//! **Replaces Crates:** `tower`, `tower-service`, `actix-service`
//!
//! **Real-world Usage:**
//! - Web Frameworks (Axum, Actix-Web, Tonic) for intercepting HTTP/gRPC requests.
//! - Network clients to automatically retry transient failures.
//! - Rate limiting and load shedding in highly available microservices.
//!
//! **Why build it yourself?**
//! The `tower` crate is notoriously difficult to understand due to deeply nested
//! generics and complex type bounds. By building it from scratch, you demystify how
//! decorators wrap inner services and how type-safe middleware chains are constructed
//! without dynamic dispatch (unless explicitly desired).

// =========================================================================================
// Architecture
// =========================================================================================
//
// Flow:
//
//   [Request] ──► LoggerLayer ──► TimeoutLayer ──► BaseHandler
//                                                      │
//   [Response] ◄── LoggerLayer ◄── TimeoutLayer ◄──────┘
//
// Invariants:
// 1. A `Service` receives a Request and returns a Result<Response, Error>.
// 2. A `Layer` takes an inner `Service` and returns a new wrapped `Service`.
// 3. Middleware must not drop requests silently unless returning an explicit Error/Response.
//
// Complexity:
// ┌───────────────┬─────────────┬─────────────┐
// │ Operation     │ Time        │ Space       │
// ├───────────────┼─────────────┼─────────────┤
// │ Compose Chain │ O(L)        │ O(L)        │
// ├───────────────┼─────────────┼─────────────┤
// │ Call Chain    │ O(1)*       │ O(1)*       │
// └───────────────┴─────────────┴─────────────┘
// L is the number of layers. * Overheads only; actual cost depends on the services.
//
// Design Decisions:
// - **Synchronous API**: We use a synchronous `call` method for educational clarity.
//   - *Tradeoff*: Real middleware (like `tower`) uses `poll_ready` and returns `Future`s
//     for backpressure and async execution.
//   - *Alternative*: Returning `Box<dyn Future>` or using `#![feature(async_fn_in_trait)]`.
// - **Generic Generics**: We make `Service` generic over the `Request` type to allow
//   the same service struct to handle multiple request types.
// - **Composition Order**: In our `ServiceBuilder`, layers added last wrap the previous layers.
//   In `tower::ServiceBuilder`, layers added first wrap the subsequent layers (outermost first).
//   We chose bottom-up composition for simplicity in demonstrating generic stack growth.

use std::time::Instant;

/// A `Service` takes a request and returns a response or an error.
pub trait Service<Req> {
    type Res;
    type Err;

    fn call(&self, req: Req) -> Result<Self::Res, Self::Err>;
}

/// A `Layer` decorates an inner service, adding behavior before or after the inner call.
pub trait Layer<S> {
    type Service;

    fn layer(&self, inner: S) -> Self::Service;
}

// =========================================================================================
// Implementations
// =========================================================================================

/// A utility to chain two layers together.
pub struct Stack<Inner, Outer> {
    inner: Inner,
    outer: Outer,
}

impl<Inner, Outer> Stack<Inner, Outer> {
    pub const fn new(inner: Inner, outer: Outer) -> Self {
        Self { inner, outer }
    }
}

impl<S, Inner, Outer> Layer<S> for Stack<Inner, Outer>
where
    Inner: Layer<S>,
    Outer: Layer<Inner::Service>,
{
    type Service = Outer::Service;

    fn layer(&self, inner: S) -> Self::Service {
        let inner_svc = self.inner.layer(inner);
        self.outer.layer(inner_svc)
    }
}

// -------------------------------------------------------------------------
// Example 1: Logger Middleware
// -------------------------------------------------------------------------

/// A layer that adds logging around a service call.
#[derive(Clone, Default)]
pub struct LoggerLayer;

impl<S> Layer<S> for LoggerLayer {
    type Service = LoggerService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        LoggerService { inner }
    }
}

/// The service produced by `LoggerLayer`.
#[derive(Clone)]
pub struct LoggerService<S> {
    inner: S,
}

impl<S, Req> Service<Req> for LoggerService<S>
where
    S: Service<Req>,
    Req: std::fmt::Debug,
    S::Res: std::fmt::Debug,
    S::Err: std::fmt::Debug,
{
    type Res = S::Res;
    type Err = S::Err;

    fn call(&self, req: Req) -> Result<Self::Res, Self::Err> {
        // RUST INSIGHT: We can do pre-processing before calling inner.
        // We require Req to be Debug to print it.
        println!("--> Request: {req:?}");
        let start = Instant::now();

        // Call the inner service
        let result = self.inner.call(req);

        // Post-processing
        let duration = start.elapsed();
        match &result {
            Ok(res) => println!("<-- Response: {res:?} (took {duration:?})"),
            Err(err) => println!("<-- Error: {err:?} (took {duration:?})"),
        }

        result
    }
}

// -------------------------------------------------------------------------
// Example 2: Map Response Middleware
// -------------------------------------------------------------------------

/// A layer that maps the response type of the inner service using a closure.
pub struct MapResponseLayer<F> {
    f: F,
}

impl<F> MapResponseLayer<F> {
    pub const fn new(f: F) -> Self {
        Self { f }
    }
}

impl<S, F> Layer<S> for MapResponseLayer<F>
where
    F: Clone,
{
    type Service = MapResponseService<S, F>;

    fn layer(&self, inner: S) -> Self::Service {
        MapResponseService {
            inner,
            f: self.f.clone(),
        }
    }
}

pub struct MapResponseService<S, F> {
    inner: S,
    f: F,
}

impl<S, Req, F, NewRes> Service<Req> for MapResponseService<S, F>
where
    S: Service<Req>,
    F: Fn(S::Res) -> NewRes,
{
    type Res = NewRes;
    type Err = S::Err;

    fn call(&self, req: Req) -> Result<Self::Res, Self::Err> {
        let res = self.inner.call(req)?;
        // GOTCHA: We must clone or pass `self.f` by reference carefully.
        // Since `call` takes `&self`, we just invoke `(self.f)(res)`.
        Ok((self.f)(res))
    }
}

// -------------------------------------------------------------------------
// Service Builder
// -------------------------------------------------------------------------

/// A builder to compose layers and wrap a service.
pub struct ServiceBuilder<L> {
    layer: L,
}

/// An identity layer that does nothing, used as the starting point for `ServiceBuilder`.
#[derive(Default, Clone)]
pub struct IdentityLayer;

impl<S> Layer<S> for IdentityLayer {
    type Service = S;

    fn layer(&self, inner: S) -> Self::Service {
        inner
    }
}

impl Default for ServiceBuilder<IdentityLayer> {
    fn default() -> Self {
        Self::new()
    }
}

impl ServiceBuilder<IdentityLayer> {
    #[must_use] 
    pub const fn new() -> Self {
        Self {
            layer: IdentityLayer,
        }
    }
}

impl<L> ServiceBuilder<L> {
    /// Adds a new layer to the stack.
    pub fn layer<NewLayer>(self, layer: NewLayer) -> ServiceBuilder<Stack<L, NewLayer>> {
        ServiceBuilder {
            layer: Stack::new(self.layer, layer),
        }
    }

    /// Wraps the given service with the composed layers.
    pub fn service<S>(&self, service: S) -> L::Service
    where
        L: Layer<S>,
    {
        self.layer.layer(service)
    }
}

// -------------------------------------------------------------------------
// Dynamic Dispatch Service (BoxService)
// -------------------------------------------------------------------------

/// Type alias for a dynamically dispatched Service.
pub type BoxService<Req, Res, Err> =
    Box<dyn Service<Req, Res = Res, Err = Err> + Send + Sync + 'static>;

/// Allows a boxed trait object to implement `Service`.
impl<Req, Res, Err> Service<Req> for BoxService<Req, Res, Err> {
    type Res = Res;
    type Err = Err;

    fn call(&self, req: Req) -> Result<Self::Res, Self::Err> {
        (**self).call(req)
    }
}

/// A layer that boxes the inner service, erasing its concrete type.
pub struct BoxLayer<Req> {
    _phantom: std::marker::PhantomData<fn(Req) -> ()>,
}

impl<Req> Default for BoxLayer<Req> {
    fn default() -> Self {
        Self::new()
    }
}

impl<Req> Clone for BoxLayer<Req> {
    fn clone(&self) -> Self {
        Self::new()
    }
}

impl<Req> BoxLayer<Req> {
    #[must_use] 
    pub fn new() -> Self {
        Self {
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<S, Req> Layer<S> for BoxLayer<Req>
where
    S: Service<Req> + Send + Sync + 'static,
{
    type Service = BoxService<Req, S::Res, S::Err>;

    fn layer(&self, inner: S) -> Self::Service {
        // PRODUCTION NOTE: Using `Box<dyn Service>` enables storing multiple differently-typed
        // services in a single collection (like a HashMap for routing), at the cost of
        // dynamic dispatch overhead.
        Box::new(inner)
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `tower`: The standard Rust middleware crate. Tower is significantly more complex because it
//   handles asynchronous execution (returning `Future`s), backpressure (`poll_ready`), and cloneability
//   requirements.
// - `actix-service`: Similar concepts, but specifically tailored to Actix's actor/async model.
//
// Missing vs. Production:
// - **Asynchronous Execution**: Real middleware needs to await database calls or network I/O.
// - **Backpressure**: `tower` has `poll_ready` to prevent overwhelming services.
// - **Infallible**: We force all services to return `Result`. Sometimes services cannot fail.
//
// Next Steps:
// 1. Add asynchronous support using `async fn` in traits or returning boxed futures.
// 2. Implement a `TimeoutLayer` that spawns a timer.
//
// Benchmarking Note:
// Use `criterion` to benchmark the overhead of nesting services.
// You can use `std::hint::black_box` to ensure the optimizer doesn't completely inline
// and erase the cost of `call` passing through layers. The dynamic dispatch of
// `BoxService` should show measurable overhead compared to statically composed layers.

#[cfg(test)]
mod tests {
    use super::*;

    /// A simple backend service that repeats a string.
    #[derive(Clone)]
    struct EchoService;

    impl Service<String> for EchoService {
        type Res = String;
        type Err = ();

        fn call(&self, req: String) -> Result<Self::Res, Self::Err> {
            Ok(format!("ECHO: {}", req))
        }
    }

    #[test]
    fn test_basic_service() {
        let svc = EchoService;
        let res = svc.call("hello".to_string()).unwrap();
        assert_eq!(res, "ECHO: hello");
    }

    #[test]
    fn test_logger_middleware() {
        let svc = EchoService;
        // Wrapping manually
        let logged_svc = LoggerLayer.layer(svc);

        let res = logged_svc.call("hello logger".to_string()).unwrap();
        assert_eq!(res, "ECHO: hello logger");
    }

    #[test]
    fn test_map_response_middleware() {
        let svc = EchoService;
        let map_layer = MapResponseLayer::new(|res: String| res.len());
        let mapped_svc = map_layer.layer(svc);

        let res = mapped_svc.call("hello".to_string()).unwrap();
        // "ECHO: hello" is 11 chars
        assert_eq!(res, 11);
    }

    #[test]
    fn test_service_builder_composition() {
        let builder = ServiceBuilder::new()
            .layer(LoggerLayer)
            .layer(MapResponseLayer::new(|res: String| res.to_uppercase()));

        let svc = builder.service(EchoService);

        let res = svc.call("rust".to_string()).unwrap();
        // The MapResponseLayer runs *after* EchoService, so it uppercase the output
        assert_eq!(res, "ECHO: RUST");
    }

    #[test]
    fn test_boxed_service() {
        let svc: BoxService<String, String, ()> = Box::new(EchoService);
        let res = svc.call("boxed".to_string()).unwrap();
        assert_eq!(res, "ECHO: boxed");

        // Use BoxLayer in builder
        let builder = ServiceBuilder::new()
            .layer(LoggerLayer)
            .layer(BoxLayer::<String>::new());

        let dyn_svc: BoxService<String, String, ()> = builder.service(EchoService);
        let res2 = dyn_svc.call("dyn".to_string()).unwrap();
        assert_eq!(res2, "ECHO: dyn");
    }
}
