//! # Types and Traits Fundamentals
//!
//! Essential patterns for type definitions, trait usage, and generic programming.
//! These patterns appear constantly in Rust interviews and real-world code.

use std::fmt;

// ============================================================================
// TYPE ALIASES & NEWTYPES
// ============================================================================

/// Type alias: Convenience name for complex types
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
type NodeId = usize;
type Graph = Vec<Vec<NodeId>>;

/// Newtype pattern: Strong typing to prevent mixing up primitives
/// Use this to avoid passing wrong IDs/indices
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UserId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PostId(pub u64);

// Now you can't accidentally pass a PostId where UserId is expected!
pub fn get_user(_id: UserId) -> String {
    String::from("user")
}

// Won't compile: get_user(PostId(42))

// ============================================================================
// GENERIC TYPES
// ============================================================================

/// Generic struct with type parameter
#[derive(Debug)]
pub struct Pair<T> {
    pub first: T,
    pub second: T,
}

impl<T> Pair<T> {
    pub fn new(first: T, second: T) -> Self {
        Self { first, second }
    }
}

/// Generic with multiple type parameters
#[derive(Debug)]
pub struct KeyValue<K, V> {
    pub key: K,
    pub value: V,
}

/// Generic with constraints
impl<T: PartialOrd> Pair<T> {
    pub fn max(&self) -> &T {
        if self.first >= self.second {
            &self.first
        } else {
            &self.second
        }
    }
}

// ============================================================================
// TRAIT DEFINITIONS
// ============================================================================

/// Simple trait definition
pub trait Summary {
    fn summarize(&self) -> String;
}

/// Trait with default implementation
pub trait Displayable {
    fn display(&self) -> String {
        String::from("(default display)")
    }

    fn required_method(&self) -> String; // Must be implemented
}

/// Trait with associated types
pub trait Container {
    type Item;

    fn add(&mut self, item: Self::Item);
    fn get(&self, index: usize) -> Option<&Self::Item>;
}

// ============================================================================
// IMPLEMENTING TRAITS
// ============================================================================

pub struct Article {
    pub title: String,
    pub author: String,
    pub content: String,
}

impl Summary for Article {
    fn summarize(&self) -> String {
        format!("{} by {}", self.title, self.author)
    }
}

impl Displayable for Article {
    // Use default implementation for display()

    fn required_method(&self) -> String {
        format!("Article: {}", self.title)
    }
}

// Implementing standard library traits
impl fmt::Display for Article {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} - {}", self.title, self.author)
    }
}

// ============================================================================
// TRAIT BOUNDS
// ============================================================================

/// Function with trait bound
pub fn print_summary<T: Summary>(item: &T) {
    println!("{}", item.summarize());
}

/// Multiple trait bounds
pub fn process<T: Summary + Displayable>(item: &T) -> String {
    format!("{} | {}", item.summarize(), item.required_method())
}

/// Where clause for complex bounds
pub fn complex_function<T, U>(t: &T, u: &U) -> String
where
    T: Summary + Clone,
    U: Summary + fmt::Display,
{
    format!("{} and {}", t.summarize(), u.summarize())
}

// ============================================================================
// IMPL TRAIT
// ============================================================================

/// Return type using impl Trait
pub fn create_article() -> impl Summary {
    Article {
        title: String::from("Rust Patterns"),
        author: String::from("Ferris"),
        content: String::from("..."),
    }
}

/// Argument using impl Trait
pub fn notify(item: &impl Summary) {
    println!("Breaking news! {}", item.summarize());
}

// ============================================================================
// TRAIT OBJECTS (DYNAMIC DISPATCH)
// ============================================================================

/// Trait object: Box<dyn Trait>
/// Use when you need to store different types implementing the same trait
pub fn create_summaries() -> Vec<Box<dyn Summary>> {
    vec![
        Box::new(Article {
            title: String::from("Article 1"),
            author: String::from("Author 1"),
            content: String::from("..."),
        }),
        // Could add other types implementing Summary here
    ]
}

// ============================================================================
// FROM/INTO TRAITS
// ============================================================================

/// Implementing From (Into comes for free)
#[derive(Debug)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

impl From<(i32, i32)> for Point {
    fn from((x, y): (i32, i32)) -> Self {
        Point { x, y }
    }
}

// Now you can use both:
// - Point::from((1, 2))
// - (1, 2).into() // when type is known

/// Implementing Into explicitly (rare - usually derive from From)
pub struct Coordinate {
    pub x: f64,
    pub y: f64,
}

impl From<Point> for Coordinate {
    fn from(p: Point) -> Self {
        Coordinate {
            x: f64::from(p.x),
            y: f64::from(p.y),
        }
    }
}

// ============================================================================
// DERIVE MACROS
// ============================================================================

/// Common derives for interview code
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Node {
    pub id: usize,
    pub value: i32,
}

/// Derive with custom implementation
#[derive(Debug, Clone)]
pub struct CustomStruct {
    pub data: Vec<i32>,
}

impl PartialEq for CustomStruct {
    fn eq(&self, other: &Self) -> bool {
        // Custom equality: only check if data is equal
        self.data == other.data
    }
}

// ============================================================================
// ASSOCIATED TYPES VS GENERICS
// ============================================================================

/// Generic trait: Can implement multiple times with different types
pub trait GenericAdd<T> {
    fn add(&self, other: T) -> Self;
}

/// Associated type: Can only implement once per type
pub trait AssociatedAdd {
    type Output;
    fn add(&self, other: Self) -> Self::Output;
}

// ============================================================================
// TRAIT INHERITANCE
// ============================================================================

/// Supertrait: Shape requires Display
pub trait Shape: fmt::Display {
    fn area(&self) -> f64;
}

pub struct Circle {
    pub radius: f64,
}

impl fmt::Display for Circle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Circle(r={})", self.radius)
    }
}

impl Shape for Circle {
    fn area(&self) -> f64 {
        std::f64::consts::PI * self.radius * self.radius
    }
}

// ============================================================================
// MARKER TRAITS
// ============================================================================

/// Marker trait: No methods, just marks a type
pub trait Serializable {}

pub struct Data {
    pub content: String,
}

impl Serializable for Data {}

pub fn serialize<T: Serializable>(_data: &T) -> String {
    String::from("serialized")
}

// ============================================================================
// COMMON PATTERNS
// ============================================================================

/// Builder pattern with generic types
#[derive(Debug)]
pub struct QueryBuilder<T> {
    items: Vec<T>,
    limit: Option<usize>,
}

impl<T> QueryBuilder<T> {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            limit: None,
        }
    }

    pub fn add(mut self, item: T) -> Self {
        self.items.push(item);
        self
    }

    pub fn limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }

    pub fn build(self) -> Vec<T> {
        if let Some(limit) = self.limit {
            self.items.into_iter().take(limit).collect()
        } else {
            self.items
        }
    }
}

impl<T> Default for QueryBuilder<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// Type state pattern: Use types to enforce state transitions
pub struct Locked;
pub struct Unlocked;

pub struct Door<State = Locked> {
    _state: std::marker::PhantomData<State>,
}

impl Door<Locked> {
    pub fn new() -> Self {
        Door {
            _state: std::marker::PhantomData,
        }
    }

    pub fn unlock(self) -> Door<Unlocked> {
        Door {
            _state: std::marker::PhantomData,
        }
    }
}

impl Door<Unlocked> {
    pub fn lock(self) -> Door<Locked> {
        Door {
            _state: std::marker::PhantomData,
        }
    }

    pub fn open(&self) {
        println!("Door opened!");
    }
}

impl Default for Door<Locked> {
    fn default() -> Self {
        Self::new()
    }
}

// Can't call .open() on a locked door - won't compile!

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_newtype_pattern() {
        let user_id = UserId(42);
        let _user = get_user(user_id);

        // This would not compile:
        // let post_id = PostId(42);
        // let _user = get_user(post_id); // Type error!
    }

    #[test]
    fn test_generic_pair() {
        let pair = Pair::new(5, 10);
        assert_eq!(*pair.max(), 10);

        let str_pair = Pair::new("hello", "world");
        assert_eq!(*str_pair.max(), "world");
    }

    #[test]
    fn test_trait_implementation() {
        let article = Article {
            title: String::from("Test"),
            author: String::from("Author"),
            content: String::from("Content"),
        };

        assert_eq!(article.summarize(), "Test by Author");
        assert!(article.display().contains("default"));
    }

    #[test]
    fn test_impl_trait() {
        let item = create_article();
        assert!(item.summarize().contains("Rust Patterns"));
    }

    #[test]
    fn test_from_into() {
        let point = Point::from((3, 4));
        assert_eq!(point.x, 3);
        assert_eq!(point.y, 4);

        // Using into when type is known
        let point2: Point = (5, 6).into();
        assert_eq!(point2.x, 5);

        // From Point to Coordinate
        let coord: Coordinate = point.into();
        assert!((coord.x - 3.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_builder_pattern() {
        let items = QueryBuilder::new().add(1).add(2).add(3).limit(2).build();

        assert_eq!(items, vec![1, 2]);
    }

    #[test]
    fn test_typestate_pattern() {
        let door = Door::new(); // Locked by default
        let door = door.unlock();
        door.open(); // Only works on unlocked door
        let _door = door.lock();

        // This would not compile:
        // let locked_door = Door::new();
        // locked_door.open(); // Error: no method `open` on Door<Locked>
    }

    #[test]
    fn test_trait_objects() {
        let summaries = create_summaries();
        assert_eq!(summaries.len(), 1);
        assert!(summaries[0].summarize().contains("Article 1"));
    }

    #[test]
    fn test_marker_trait() {
        let data = Data {
            content: String::from("test"),
        };

        let serialized = serialize(&data);
        assert_eq!(serialized, "serialized");
    }

    #[test]
    fn test_shape_trait() {
        let circle = Circle { radius: 2.0 };
        let area = circle.area();
        assert!((area - 12.566_370_614_359_172).abs() < 0.001);

        // Can use Display because Shape requires it
        let display = format!("{circle}");
        assert!(display.contains("Circle"));
    }

    #[test]
    fn test_custom_partial_eq() {
        let s1 = CustomStruct {
            data: vec![1, 2, 3],
        };
        let s2 = CustomStruct {
            data: vec![1, 2, 3],
        };
        let s3 = CustomStruct {
            data: vec![4, 5, 6],
        };

        assert_eq!(s1, s2);
        assert_ne!(s1, s3);
    }
}
