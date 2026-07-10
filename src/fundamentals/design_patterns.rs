//! # Design Patterns in Rust
//!
//! Common design patterns adapted for Rust's ownership system and type system.
//! Learn idiomatic Rust patterns for production code.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

// ============================================================================
// Builder Pattern
// ============================================================================

/// Classic builder pattern with setters.
// Fields are the builder's target; only read via derived Debug in this demo
#[allow(dead_code)]
#[derive(Debug, Default)]
struct Server {
    host: String,
    port: u16,
    timeout: u64,
    max_connections: u32,
}

struct ServerBuilder {
    host: String,
    port: u16,
    timeout: u64,
    max_connections: u32,
}

impl ServerBuilder {
    fn new() -> Self {
        Self {
            host: "localhost".to_string(),
            port: 8080,
            timeout: 30,
            max_connections: 100,
        }
    }

    fn host(mut self, host: impl Into<String>) -> Self {
        self.host = host.into();
        self
    }

    const fn port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }

    const fn timeout(mut self, timeout: u64) -> Self {
        self.timeout = timeout;
        self
    }

    const fn max_connections(mut self, max: u32) -> Self {
        self.max_connections = max;
        self
    }

    fn build(self) -> Server {
        Server {
            host: self.host,
            port: self.port,
            timeout: self.timeout,
            max_connections: self.max_connections,
        }
    }
}

#[allow(dead_code)]
fn demonstrate_builder() {
    let server = ServerBuilder::new()
        .host("example.com")
        .port(443)
        .timeout(60)
        .max_connections(1000)
        .build();

    println!("Server: {server:?}");
}

/// Type-state builder - compile-time validation.
/// Different states have different available methods.
#[allow(dead_code)]
struct ConnectionBuilder<State> {
    url: Option<String>,
    state: State,
}

#[allow(dead_code)]
struct Initial;
#[allow(dead_code)]
struct WithUrl;
#[allow(dead_code)]
struct Ready;

impl ConnectionBuilder<Initial> {
    const fn new() -> Self {
        Self {
            url: None,
            state: Initial,
        }
    }

    // Consumes `self` to enforce the type-state transition (can only be called on Initial)
    #[allow(clippy::unused_self)]
    fn url(self, url: impl Into<String>) -> ConnectionBuilder<WithUrl> {
        ConnectionBuilder {
            url: Some(url.into()),
            state: WithUrl,
        }
    }
}

impl ConnectionBuilder<WithUrl> {
    fn connect(self) -> ConnectionBuilder<Ready> {
        println!("Connecting to {:?}", self.url);
        ConnectionBuilder {
            url: self.url,
            state: Ready,
        }
    }
}

impl ConnectionBuilder<Ready> {
    // `&self` requires the Ready state at compile time; the receiver enforces the type-state
    #[allow(clippy::unused_self)]
    fn send(&self, data: &str) {
        println!("Sending: {data}");
    }
}

#[allow(dead_code)]
fn demonstrate_typestate_builder() {
    let conn = ConnectionBuilder::new()
        .url("https://example.com")
        .connect();

    conn.send("hello");

    // This won't compile - must call url() first:
    // ConnectionBuilder::new().connect();

    // This won't compile - can't send before connect:
    // ConnectionBuilder::new().url("test").send("data");
}

// ============================================================================
// Newtype Pattern
// ============================================================================

/// Newtype for type safety - zero runtime cost.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct UserId(u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct ProductId(u64);

impl UserId {
    const fn new(id: u64) -> Self {
        Self(id)
    }

    const fn value(self) -> u64 {
        self.0
    }
}

impl ProductId {
    const fn new(id: u64) -> Self {
        Self(id)
    }

    const fn value(self) -> u64 {
        self.0
    }
}

#[allow(dead_code)]
fn get_user(id: UserId) -> String {
    format!("User {}", id.value())
}

#[allow(dead_code)]
fn get_product(id: ProductId) -> String {
    format!("Product {}", id.value())
}

#[allow(dead_code)]
fn demonstrate_newtype() {
    let user_id = UserId::new(42);
    let _product_id = ProductId::new(99);

    let _ = get_user(user_id);

    // This won't compile - type safety!
    // get_user(product_id);

    // This won't compile either:
    // get_user(42);
}

/// Newtype with Deref for convenience.
use std::ops::{Deref, DerefMut};

#[derive(Debug)]
struct Email(String);

impl Email {
    fn new(email: String) -> Result<Self, String> {
        if email.contains('@') {
            Ok(Self(email))
        } else {
            Err("Invalid email".to_string())
        }
    }
}

impl Deref for Email {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[allow(dead_code)]
fn demonstrate_newtype_deref() {
    let email = Email::new("alice@example.com".to_string()).unwrap();

    // Can use String methods directly via Deref
    println!("Email length: {}", email.len());
    println!("Lowercase: {}", email.to_lowercase());
}

// ============================================================================
// Type State Pattern
// ============================================================================

/// State machine using types - compile-time enforcement.
#[allow(dead_code)]
struct FileHandle<State> {
    path: String,
    state: State,
}

#[allow(dead_code)]
struct Closed;
#[allow(dead_code)]
struct Open;
#[allow(dead_code)]
struct Reading;

impl FileHandle<Closed> {
    fn new(path: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            state: Closed,
        }
    }

    fn open(self) -> FileHandle<Open> {
        println!("Opening file: {}", self.path);
        FileHandle {
            path: self.path,
            state: Open,
        }
    }
}

impl FileHandle<Open> {
    fn read(self) -> FileHandle<Reading> {
        println!("Reading file: {}", self.path);
        FileHandle {
            path: self.path,
            state: Reading,
        }
    }

    // Demonstrates closing directly from the Open state for gittype practice
    #[allow(dead_code)]
    fn close(self) -> FileHandle<Closed> {
        println!("Closing file: {}", self.path);
        FileHandle {
            path: self.path,
            state: Closed,
        }
    }
}

impl FileHandle<Reading> {
    // `&self` requires the Reading state at compile time; the receiver enforces the type-state
    #[allow(clippy::unused_self)]
    const fn get_data(&self) -> &'static str {
        "file contents"
    }

    fn close(self) -> FileHandle<Closed> {
        println!("Closing file after reading: {}", self.path);
        FileHandle {
            path: self.path,
            state: Closed,
        }
    }
}

#[allow(dead_code)]
fn demonstrate_typestate() {
    let file = FileHandle::new("data.txt").open().read();

    let _data = file.get_data();
    let _closed = file.close();

    // Won't compile - must open before reading:
    // FileHandle::new("test.txt").read();

    // Won't compile - can't get data from closed file:
    // FileHandle::new("test.txt").get_data();
}

// ============================================================================
// RAII Pattern (Drop)
// ============================================================================

/// Resource cleanup with Drop trait.
struct TempFile {
    path: String,
}

impl TempFile {
    fn new(path: impl Into<String>) -> Self {
        let path = path.into();
        println!("Creating temp file: {path}");
        Self { path }
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        println!("Deleting temp file: {}", self.path);
        // In real code: std::fs::remove_file(&self.path)
    }
}

#[allow(dead_code)]
fn demonstrate_raii() {
    {
        let _temp = TempFile::new("/tmp/test.txt");
        println!("Using temp file...");
    } // Drop called here automatically
    println!("Temp file cleaned up");
}

/// Guard pattern for scoped resources.
// Demonstrates the RAII guard pattern (Drop + Deref) for gittype practice
#[allow(dead_code)]
struct MutexGuard<'a, T> {
    data: &'a mut T,
}

impl<T> Drop for MutexGuard<'_, T> {
    fn drop(&mut self) {
        println!("Releasing lock");
    }
}

impl<T> Deref for MutexGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.data
    }
}

impl<T> DerefMut for MutexGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.data
    }
}

// ============================================================================
// Visitor Pattern
// ============================================================================

/// Visitor for traversing data structures.
trait Visitor {
    fn visit_number(&mut self, n: i32);
    fn visit_string(&mut self, s: &str);
}

#[allow(dead_code)]
enum Node {
    Number(i32),
    String(String),
    List(Vec<Self>),
}

impl Node {
    fn accept(&self, visitor: &mut dyn Visitor) {
        match self {
            Self::Number(n) => visitor.visit_number(*n),
            Self::String(s) => visitor.visit_string(s),
            Self::List(nodes) => {
                for node in nodes {
                    node.accept(visitor);
                }
            }
        }
    }
}

struct SumVisitor {
    sum: i32,
}

impl Visitor for SumVisitor {
    fn visit_number(&mut self, n: i32) {
        self.sum += n;
    }

    fn visit_string(&mut self, _s: &str) {
        // Ignore strings
    }
}

#[allow(dead_code)]
fn demonstrate_visitor() {
    let tree = Node::List(vec![
        Node::Number(1),
        Node::Number(2),
        Node::String("hello".to_string()),
        Node::Number(3),
    ]);

    let mut visitor = SumVisitor { sum: 0 };
    tree.accept(&mut visitor);
    println!("Sum: {}", visitor.sum); // 6
}

// ============================================================================
// Strategy Pattern
// ============================================================================

/// Strategy with trait objects (runtime polymorphism).
trait CompressionStrategy {
    fn compress(&self, data: &[u8]) -> Vec<u8>;
}

struct GzipCompression;
struct ZstdCompression;

impl CompressionStrategy for GzipCompression {
    fn compress(&self, data: &[u8]) -> Vec<u8> {
        println!("Compressing with gzip");
        data.to_vec() // Simplified
    }
}

impl CompressionStrategy for ZstdCompression {
    fn compress(&self, data: &[u8]) -> Vec<u8> {
        println!("Compressing with zstd");
        data.to_vec() // Simplified
    }
}

struct Compressor {
    strategy: Box<dyn CompressionStrategy>,
}

impl Compressor {
    fn new(strategy: Box<dyn CompressionStrategy>) -> Self {
        Self { strategy }
    }

    fn compress(&self, data: &[u8]) -> Vec<u8> {
        self.strategy.compress(data)
    }
}

#[allow(dead_code)]
fn demonstrate_strategy() {
    let data = b"hello world";

    let compressor = Compressor::new(Box::new(GzipCompression));
    let _compressed = compressor.compress(data);

    let compressor = Compressor::new(Box::new(ZstdCompression));
    let _compressed = compressor.compress(data);
}

/// Strategy with generics (compile-time polymorphism).
#[allow(dead_code)]
fn compress_with<S: CompressionStrategy>(strategy: &S, data: &[u8]) -> Vec<u8> {
    strategy.compress(data)
}

// ============================================================================
// Command Pattern
// ============================================================================

/// Command pattern for undo/redo.
///
/// The key idea: a command holds a *shared* handle to the receiver it mutates
/// (`Rc<RefCell<T>>`), not a private copy. `execute()` applies a change to the
/// shared state and `undo()` reverses it, so the effect is observable to anyone
/// else holding a clone of the same `Rc<RefCell<T>>`.
trait Command {
    fn execute(&mut self);
    fn undo(&mut self);
}

/// Shared, mutable receiver observed by both the commands and external code.
type SharedValue = Rc<RefCell<i32>>;

struct AddCommand {
    value: i32,
    target: SharedValue,
}

impl AddCommand {
    const fn new(target: SharedValue, value: i32) -> Self {
        Self { value, target }
    }
}

impl Command for AddCommand {
    fn execute(&mut self) {
        *self.target.borrow_mut() += self.value;
        println!("Added {}, target now: {}", self.value, self.target.borrow());
    }

    fn undo(&mut self) {
        *self.target.borrow_mut() -= self.value;
        println!("Undid add, target now: {}", self.target.borrow());
    }
}

struct CommandHistory {
    done: Vec<Box<dyn Command>>,
    undone: Vec<Box<dyn Command>>,
}

impl CommandHistory {
    fn new() -> Self {
        Self {
            done: Vec::new(),
            undone: Vec::new(),
        }
    }

    fn execute(&mut self, mut command: Box<dyn Command>) {
        command.execute();
        self.done.push(command);
        // A fresh action invalidates the redo stack.
        self.undone.clear();
    }

    fn undo(&mut self) {
        if let Some(mut command) = self.done.pop() {
            command.undo();
            self.undone.push(command);
        }
    }

    fn redo(&mut self) {
        if let Some(mut command) = self.undone.pop() {
            command.execute();
            self.done.push(command);
        }
    }
}

#[allow(dead_code)]
fn demonstrate_command() {
    // External code holds its own clone of the shared state.
    let value: SharedValue = Rc::new(RefCell::new(0));

    let mut history = CommandHistory::new();
    history.execute(Box::new(AddCommand::new(Rc::clone(&value), 5)));
    history.execute(Box::new(AddCommand::new(Rc::clone(&value), 3)));
    println!("After commands: {}", value.borrow()); // 8

    history.undo();
    println!("After undo: {}", value.borrow()); // 5

    history.redo();
    println!("After redo: {}", value.borrow()); // 8
}

// ============================================================================
// Iterator Pattern
// ============================================================================

/// Custom iterator implementation.
struct Counter {
    count: u32,
    max: u32,
}

impl Counter {
    const fn new(max: u32) -> Self {
        Self { count: 0, max }
    }
}

impl Iterator for Counter {
    type Item = u32;

    fn next(&mut self) -> Option<Self::Item> {
        if self.count < self.max {
            self.count += 1;
            Some(self.count)
        } else {
            None
        }
    }
}

/// `IntoIterator` for owned iteration.
struct Numbers {
    items: Vec<i32>,
}

impl IntoIterator for Numbers {
    type Item = i32;
    type IntoIter = std::vec::IntoIter<i32>;

    fn into_iter(self) -> Self::IntoIter {
        self.items.into_iter()
    }
}

#[allow(dead_code)]
fn demonstrate_iterator() {
    let counter = Counter::new(5);
    for n in counter {
        println!("{n}");
    }

    let numbers = Numbers {
        items: vec![1, 2, 3],
    };
    for n in numbers {
        println!("{n}");
    }

    // Infinite iterator: take a finite prefix
    for n in Fibonacci::new().take(5) {
        println!("fib: {n}");
    }
}

/// Infinite iterator.
struct Fibonacci {
    curr: u64,
    next: u64,
}

impl Fibonacci {
    const fn new() -> Self {
        Self { curr: 0, next: 1 }
    }
}

impl Iterator for Fibonacci {
    type Item = u64;

    fn next(&mut self) -> Option<Self::Item> {
        let new_next = self.curr + self.next;
        self.curr = self.next;
        self.next = new_next;
        Some(self.curr)
    }
}

// ============================================================================
// Observer Pattern
// ============================================================================

/// Observer with callbacks.
type Callback = Box<dyn Fn(&str)>;

struct Observable {
    observers: Vec<Callback>,
}

impl Observable {
    fn new() -> Self {
        Self {
            observers: Vec::new(),
        }
    }

    fn subscribe(&mut self, callback: Callback) {
        self.observers.push(callback);
    }

    fn notify(&self, event: &str) {
        for observer in &self.observers {
            observer(event);
        }
    }
}

#[allow(dead_code)]
fn demonstrate_observer() {
    let mut observable = Observable::new();

    observable.subscribe(Box::new(|event| {
        println!("Observer 1: {event}");
    }));

    observable.subscribe(Box::new(|event| {
        println!("Observer 2: {event}");
    }));

    observable.notify("Something happened");
}

/// Observer with channels (better for async).
#[allow(dead_code)]
fn demonstrate_channel_observer() {
    use std::sync::mpsc;

    let (tx, rx) = mpsc::channel();

    // Observer thread
    let observer = std::thread::spawn(move || {
        for event in rx {
            println!("Received: {event}");
        }
    });

    // Emit events
    tx.send("Event 1".to_string()).unwrap();
    tx.send("Event 2".to_string()).unwrap();
    drop(tx); // Close channel

    observer.join().unwrap();
}

// ============================================================================
// Adapter/Wrapper Pattern
// ============================================================================

/// Adapter to implement foreign traits.
// Demonstrates the adapter/newtype workaround for orphan rules for gittype practice
#[allow(dead_code)]
struct ExternalType {
    value: i32,
}

// Can't implement Display for ExternalType directly (orphan rules)
// Use newtype wrapper:
// Demonstrates the adapter/newtype workaround for orphan rules for gittype practice
#[allow(dead_code)]
struct DisplayAdapter(ExternalType);

impl std::fmt::Display for DisplayAdapter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Value: {}", self.0.value)
    }
}

/// Transparent wrapper with Deref.
// Demonstrates a transparent Deref wrapper for gittype practice
#[allow(dead_code)]
#[repr(transparent)]
struct Wrapper<T>(T);

impl<T> Deref for Wrapper<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

// ============================================================================
// Practical Pattern: Plugin System
// ============================================================================

trait Plugin {
    fn name(&self) -> &str;
    fn execute(&self, input: &str) -> String;
}

struct PluginManager {
    plugins: HashMap<String, Box<dyn Plugin>>,
}

impl PluginManager {
    fn new() -> Self {
        Self {
            plugins: HashMap::new(),
        }
    }

    fn register(&mut self, plugin: Box<dyn Plugin>) {
        self.plugins.insert(plugin.name().to_string(), plugin);
    }

    fn execute(&self, name: &str, input: &str) -> Option<String> {
        self.plugins.get(name).map(|p| p.execute(input))
    }
}

struct UppercasePlugin;

impl Plugin for UppercasePlugin {
    fn name(&self) -> &'static str {
        "uppercase"
    }

    fn execute(&self, input: &str) -> String {
        input.to_uppercase()
    }
}

#[allow(dead_code)]
fn demonstrate_plugin_system() {
    let mut manager = PluginManager::new();
    manager.register(Box::new(UppercasePlugin));

    if let Some(result) = manager.execute("uppercase", "hello") {
        println!("Result: {result}");
    }
}

// ============================================================================
// Best Practices Summary
// ============================================================================

/// Pattern selection guide:
///
/// **Builder**: Complex object construction with many optional parameters
/// **Newtype**: Type safety, trait implementation for foreign types
/// **Type State**: Compile-time state validation, preventing invalid states
/// **RAII**: Automatic resource cleanup (files, locks, connections)
/// **Visitor**: Traversing heterogeneous data structures
/// **Strategy**: Swappable algorithms, dependency injection
/// **Command**: Undo/redo, deferred execution, transaction logs
/// **Iterator**: Custom iteration logic, lazy evaluation
/// **Observer**: Event systems, pub/sub patterns
/// **Adapter**: Trait implementation for external types
///
/// Choose based on:
/// - Compile-time vs runtime polymorphism needs
/// - Performance requirements
/// - API ergonomics
/// - Type safety guarantees needed
#[allow(dead_code)]
const PATTERN_GUIDE: &str = "See module docs";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_execute_undo_redo_affect_shared_state() {
        // Shared receiver observed by both the commands and this test.
        let value: SharedValue = Rc::new(RefCell::new(0));

        let mut history = CommandHistory::new();
        history.execute(Box::new(AddCommand::new(Rc::clone(&value), 5)));
        history.execute(Box::new(AddCommand::new(Rc::clone(&value), 3)));

        // Commands mutated the shared state, visible externally.
        assert_eq!(*value.borrow(), 8);

        // Undo reverses the most recent command.
        history.undo();
        assert_eq!(*value.borrow(), 5);

        history.undo();
        assert_eq!(*value.borrow(), 0);

        // Redo re-applies undone commands in order.
        history.redo();
        assert_eq!(*value.borrow(), 5);

        history.redo();
        assert_eq!(*value.borrow(), 8);
    }

    #[test]
    fn command_new_action_clears_redo_stack() {
        let value: SharedValue = Rc::new(RefCell::new(0));
        let mut history = CommandHistory::new();

        history.execute(Box::new(AddCommand::new(Rc::clone(&value), 5)));
        history.undo();
        assert_eq!(*value.borrow(), 0);

        // A fresh action invalidates the redo stack; redo becomes a no-op.
        history.execute(Box::new(AddCommand::new(Rc::clone(&value), 2)));
        assert_eq!(*value.borrow(), 2);
        history.redo();
        assert_eq!(*value.borrow(), 2);
    }

    #[test]
    fn mutex_guard_derefs_and_mutates_through_deref_mut() {
        let mut data = 10i32;
        {
            let mut guard = MutexGuard { data: &mut data };
            // Read through Deref.
            assert_eq!(*guard, 10);
            // Mutate through DerefMut.
            *guard += 5;
            assert_eq!(*guard, 15);
        }
        // The mutation is reflected in the underlying value.
        assert_eq!(data, 15);
    }
}
