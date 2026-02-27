//! # Design Patterns in Rust
//!
//! Common design patterns adapted for Rust's ownership system and type system.
//! Learn idiomatic Rust patterns for production code.

use std::collections::HashMap;

// ============================================================================
// Builder Pattern
// ============================================================================

/// Classic builder pattern with setters.
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
        ServerBuilder {
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

    fn port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }

    fn timeout(mut self, timeout: u64) -> Self {
        self.timeout = timeout;
        self
    }

    fn max_connections(mut self, max: u32) -> Self {
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
    fn new() -> Self {
        ConnectionBuilder {
            url: None,
            state: Initial,
        }
    }

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
    fn new(id: u64) -> Self {
        UserId(id)
    }

    fn value(&self) -> u64 {
        self.0
    }
}

impl ProductId {
    fn new(id: u64) -> Self {
        ProductId(id)
    }

    fn value(&self) -> u64 {
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
use std::ops::Deref;

#[derive(Debug)]
struct Email(String);

impl Email {
    fn new(email: String) -> Result<Self, String> {
        if email.contains('@') {
            Ok(Email(email))
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
        FileHandle {
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

    fn close(self) -> FileHandle<Closed> {
        println!("Closing file: {}", self.path);
        FileHandle {
            path: self.path,
            state: Closed,
        }
    }
}

impl FileHandle<Reading> {
    fn get_data(&self) -> &str {
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
        TempFile { path }
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
struct MutexGuard<'a, T> {
    data: &'a mut T,
}

impl<'a, T> Drop for MutexGuard<'a, T> {
    fn drop(&mut self) {
        println!("Releasing lock");
    }
}

impl<'a, T> Deref for MutexGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
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
    List(Vec<Node>),
}

impl Node {
    fn accept(&self, visitor: &mut dyn Visitor) {
        match self {
            Node::Number(n) => visitor.visit_number(*n),
            Node::String(s) => visitor.visit_string(s),
            Node::List(nodes) => {
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
        Compressor { strategy }
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
trait Command {
    fn execute(&mut self);
    fn undo(&mut self);
}

struct AddCommand {
    value: i32,
    target: i32,
}

impl Command for AddCommand {
    fn execute(&mut self) {
        self.target += self.value;
        println!("Added {}, target now: {}", self.value, self.target);
    }

    fn undo(&mut self) {
        self.target -= self.value;
        println!("Undid add, target now: {}", self.target);
    }
}

struct CommandHistory {
    commands: Vec<Box<dyn Command>>,
}

impl CommandHistory {
    fn new() -> Self {
        CommandHistory {
            commands: Vec::new(),
        }
    }

    fn execute(&mut self, mut command: Box<dyn Command>) {
        command.execute();
        self.commands.push(command);
    }

    fn undo(&mut self) {
        if let Some(mut command) = self.commands.pop() {
            command.undo();
        }
    }
}

#[allow(dead_code)]
fn demonstrate_command() {
    let mut history = CommandHistory::new();

    history.execute(Box::new(AddCommand {
        value: 5,
        target: 0,
    }));
    history.execute(Box::new(AddCommand {
        value: 3,
        target: 5,
    }));

    history.undo();
    history.undo();
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
    fn new(max: u32) -> Self {
        Counter { count: 0, max }
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

/// IntoIterator for owned iteration.
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
}

/// Infinite iterator.
struct Fibonacci {
    curr: u64,
    next: u64,
}

impl Fibonacci {
    fn new() -> Self {
        Fibonacci { curr: 0, next: 1 }
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
        Observable {
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
struct ExternalType {
    value: i32,
}

// Can't implement Display for ExternalType directly (orphan rules)
// Use newtype wrapper:
struct DisplayAdapter(ExternalType);

impl std::fmt::Display for DisplayAdapter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Value: {}", self.0.value)
    }
}

/// Transparent wrapper with Deref.
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
        PluginManager {
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
    fn name(&self) -> &str {
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
