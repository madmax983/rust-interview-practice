//! # Smart Pointer Patterns
//!
//! Smart pointers enable advanced ownership patterns essential for implementing
//! linked lists, trees, and graphs. Master these for data structure problems.

use std::cell::RefCell;
use std::rc::{Rc, Weak};

// ============================================================================
// Box<T> - Heap Allocation
// ============================================================================

/// Basic struct for demonstrating Box
#[allow(dead_code)]
#[derive(Debug)]
struct Point {
    x: i32,
    y: i32,
}

#[allow(dead_code)]
fn demonstrate_box_basics() {
    // Basic heap allocation with Box
    let boxed_point = Box::new(Point { x: 10, y: 20 });
    println!("Boxed point: {boxed_point:?}");

    // Dereferencing Box
    let x_value = (*boxed_point).x; // Explicit dereference
    let y_value = boxed_point.y; // Automatic dereference (deref coercion)
    println!("x: {x_value}, y: {y_value}");

    // Pattern matching on Box
    let boxed_num = Box::new(42);
    match boxed_num {
        box_val if *box_val > 40 => println!("Large number: {box_val}"),
        _ => println!("Small number"),
    }

    // Moving out of Box
    let boxed = Box::new(String::from("hello"));
    let _owned = *boxed; // Moves the String out of the Box
    // boxed is no longer valid here
}

/// Binary tree node using Box for recursive type.
/// Box enables recursive types because it has a known size.
#[allow(dead_code)]
#[derive(Debug)]
struct TreeNode {
    value: i32,
    left: Option<Box<TreeNode>>,
    right: Option<Box<TreeNode>>,
}

impl TreeNode {
    fn new(value: i32) -> Self {
        TreeNode {
            value,
            left: None,
            right: None,
        }
    }

    fn insert(&mut self, value: i32) {
        if value < self.value {
            match &mut self.left {
                Some(node) => node.insert(value),
                None => self.left = Some(Box::new(TreeNode::new(value))),
            }
        } else {
            match &mut self.right {
                Some(node) => node.insert(value),
                None => self.right = Some(Box::new(TreeNode::new(value))),
            }
        }
    }
}

#[allow(dead_code)]
fn demonstrate_box_recursive() {
    // Create a binary search tree using Box
    let mut root = TreeNode::new(10);
    root.insert(5);
    root.insert(15);
    root.insert(3);
    root.insert(7);

    println!("Tree: {root:?}");
}

// ============================================================================
// Rc<T> - Reference Counting
// ============================================================================

#[allow(dead_code)]
fn demonstrate_rc_basics() {
    // Create an Rc
    let data = Rc::new(vec![1, 2, 3, 4, 5]);
    println!("Strong count: {}", Rc::strong_count(&data)); // 1

    // Clone to create another reference (doesn't clone the data!)
    let data2 = Rc::clone(&data);
    println!("Strong count: {}", Rc::strong_count(&data)); // 2

    // Both point to the same data
    println!("data: {data:?}");
    println!("data2: {data2:?}");

    // Can have many owners
    let data3 = Rc::clone(&data);
    let data4 = Rc::clone(&data);
    println!("Strong count: {}", Rc::strong_count(&data)); // 4

    // When all owners drop, data is freed
    drop(data3);
    drop(data4);
    println!("Strong count after drops: {}", Rc::strong_count(&data)); // 2
}

/// Graph node using Rc for multiple parents.
#[allow(dead_code)]
#[derive(Debug)]
struct GraphNode {
    value: i32,
    neighbors: Vec<Rc<GraphNode>>,
}

impl GraphNode {
    fn new(value: i32) -> Rc<Self> {
        Rc::new(GraphNode {
            value,
            neighbors: Vec::new(),
        })
    }
}

#[allow(dead_code)]
fn demonstrate_rc_graph() {
    // Create nodes
    let node1 = GraphNode::new(1);
    let _node2 = GraphNode::new(2);
    let _node3 = GraphNode::new(3);

    // Note: This example shows the concept, but we can't mutate
    // the neighbors Vec because Rc provides shared immutable access.
    // See Rc<RefCell<T>> pattern below for mutable graphs.

    println!("Node1: {node1:?}");
    println!("Strong count of node1: {}", Rc::strong_count(&node1));
}

/// Weak references to prevent reference cycles.
#[allow(dead_code)]
fn demonstrate_weak_references() {
    let data = Rc::new(vec![1, 2, 3]);
    println!("Strong count: {}", Rc::strong_count(&data)); // 1
    println!("Weak count: {}", Rc::weak_count(&data)); // 0

    // Create weak reference
    let weak = Rc::downgrade(&data);
    println!("Strong count: {}", Rc::strong_count(&data)); // 1
    println!("Weak count: {}", Rc::weak_count(&data)); // 1

    // Upgrade weak to strong (returns Option)
    match weak.upgrade() {
        Some(strong) => {
            println!("Upgraded: {strong:?}");
            println!("Strong count: {}", Rc::strong_count(&strong)); // 2
        }
        None => println!("Data was dropped"),
    }

    // Drop strong references
    drop(data);

    // Weak reference can't be upgraded anymore
    match weak.upgrade() {
        Some(_) => println!("Still alive"),
        None => println!("Data was dropped"), // This prints
    }
}

// ============================================================================
// RefCell<T> - Interior Mutability
// ============================================================================

#[allow(dead_code)]
fn demonstrate_refcell_basics() {
    use std::cell::RefCell;

    // RefCell allows mutation through shared reference
    let data = RefCell::new(vec![1, 2, 3]);

    // Borrow immutably
    {
        let borrowed = data.borrow();
        println!("Data: {borrowed:?}");
        // Can have multiple immutable borrows
        let borrowed2 = data.borrow();
        println!("Data again: {borrowed2:?}");
    } // Borrows dropped here

    // Borrow mutably
    {
        let mut borrowed = data.borrow_mut();
        borrowed.push(4);
        // Cannot have multiple mutable borrows or mix with immutable
        // let borrowed2 = data.borrow(); // Would panic at runtime!
    }

    println!("After mutation: {:?}", data.borrow());

    // try_borrow for safe borrowing
    match data.try_borrow() {
        Ok(borrowed) => println!("Borrowed: {borrowed:?}"),
        Err(_) => println!("Already borrowed mutably"),
    }
}

// ============================================================================
// Rc<RefCell<T>> - The Graph/Tree Pattern
// ============================================================================

/// Tree node with parent reference using Rc<RefCell<T>>.
/// This pattern enables bidirectional navigation and mutation.
#[allow(dead_code)]
#[derive(Debug)]
struct MutableTreeNode {
    value: i32,
    children: Vec<Rc<RefCell<MutableTreeNode>>>,
    parent: Option<Weak<RefCell<MutableTreeNode>>>,
}

impl MutableTreeNode {
    fn new(value: i32) -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(MutableTreeNode {
            value,
            children: Vec::new(),
            parent: None,
        }))
    }

    fn add_child(parent: &Rc<RefCell<Self>>, child: Rc<RefCell<Self>>) {
        // Set parent reference in child
        child.borrow_mut().parent = Some(Rc::downgrade(parent));
        // Add child to parent's children
        parent.borrow_mut().children.push(child);
    }
}

#[allow(dead_code)]
fn demonstrate_rc_refcell_tree() {
    // Create nodes
    let root = MutableTreeNode::new(1);
    let child1 = MutableTreeNode::new(2);
    let child2 = MutableTreeNode::new(3);

    // Build tree
    MutableTreeNode::add_child(&root, Rc::clone(&child1));
    MutableTreeNode::add_child(&root, Rc::clone(&child2));

    // Access and modify
    println!("Root value: {}", root.borrow().value);
    println!("Root has {} children", root.borrow().children.len());

    // Modify through shared reference
    root.borrow_mut().value = 10;
    println!("Modified root value: {}", root.borrow().value);

    // Navigate to parent from child
    if let Some(parent_weak) = &child1.borrow().parent {
        if let Some(parent) = parent_weak.upgrade() {
            println!("Child1's parent value: {}", parent.borrow().value);
        }
    }
}

/// Graph with cycles using Rc<RefCell<T>>.
#[allow(dead_code)]
#[derive(Debug)]
struct MutableGraphNode {
    value: i32,
    neighbors: RefCell<Vec<Rc<MutableGraphNode>>>,
}

impl MutableGraphNode {
    fn new(value: i32) -> Rc<Self> {
        Rc::new(MutableGraphNode {
            value,
            neighbors: RefCell::new(Vec::new()),
        })
    }

    fn add_neighbor(&self, neighbor: Rc<MutableGraphNode>) {
        self.neighbors.borrow_mut().push(neighbor);
    }
}

#[allow(dead_code)]
fn demonstrate_rc_refcell_graph() {
    // Create nodes
    let node1 = MutableGraphNode::new(1);
    let node2 = MutableGraphNode::new(2);
    let node3 = MutableGraphNode::new(3);

    // Build graph with cycles
    node1.add_neighbor(Rc::clone(&node2));
    node2.add_neighbor(Rc::clone(&node3));
    node3.add_neighbor(Rc::clone(&node1)); // Cycle!

    println!("Node1 value: {}", node1.value);
    println!("Node1 neighbors: {}", node1.neighbors.borrow().len());
}

// ============================================================================
// Cow<T> - Clone-on-Write
// ============================================================================

use std::borrow::Cow;

#[allow(dead_code)]
fn demonstrate_cow_basics() {
    // Borrowed variant - no allocation
    let borrowed: Cow<str> = Cow::Borrowed("hello");
    println!("Borrowed: {borrowed}");

    // Owned variant - allocated
    let owned: Cow<str> = Cow::Owned(String::from("world"));
    println!("Owned: {owned}");

    // Clone-on-write: only clones if we need to modify
    let mut cow: Cow<str> = Cow::Borrowed("hello");
    println!("Original: {cow}");

    // to_mut() clones only if borrowed
    let s = cow.to_mut(); // Clones here because it was borrowed
    s.push_str(" world");
    println!("Modified: {cow}"); // Now owned

    // If already owned, no additional clone
    let s = cow.to_mut(); // No clone, already owned
    s.push('!');
    println!("Modified again: {cow}");
}

/// Function that conditionally modifies a string.
/// Uses Cow to avoid cloning if no modification is needed.
#[allow(dead_code)]
fn remove_spaces(input: &str) -> Cow<str> {
    if input.contains(' ') {
        // Need to modify - return owned
        Cow::Owned(input.replace(' ', ""))
    } else {
        // No modification needed - return borrowed
        Cow::Borrowed(input)
    }
}

#[allow(dead_code)]
fn demonstrate_cow_optimization() {
    let s1 = "hello world";
    let result1 = remove_spaces(s1); // Returns Owned (modified)
    println!("Result1: {result1}");

    let s2 = "helloworld";
    let result2 = remove_spaces(s2); // Returns Borrowed (no modification)
    println!("Result2: {result2}");

    // Check which variant
    match result1 {
        Cow::Borrowed(_) => println!("Result1 is borrowed"),
        Cow::Owned(_) => println!("Result1 is owned"),
    }

    match result2 {
        Cow::Borrowed(_) => println!("Result2 is borrowed"),
        Cow::Owned(_) => println!("Result2 is owned"),
    }
}

// ============================================================================
// Comparison and When to Use Each
// ============================================================================

#[allow(dead_code)]
fn demonstrate_comparison() {
    println!("=== Smart Pointer Comparison ===");
    println!();

    println!("Box<T>:");
    println!("  - Use for: Heap allocation, recursive types");
    println!("  - Ownership: Single owner");
    println!("  - Mutability: Through ownership");
    println!("  - Example: Binary tree, linked list");
    println!();

    println!("Rc<T>:");
    println!("  - Use for: Multiple owners (immutable)");
    println!("  - Ownership: Reference counted");
    println!("  - Mutability: Immutable shared access");
    println!("  - Example: DAG nodes, shared config");
    println!();

    println!("RefCell<T>:");
    println!("  - Use for: Interior mutability");
    println!("  - Ownership: Single owner");
    println!("  - Mutability: Runtime borrow checking");
    println!("  - Example: Mock objects, caches");
    println!();

    println!("Rc<RefCell<T>>:");
    println!("  - Use for: Multiple owners (mutable)");
    println!("  - Ownership: Reference counted");
    println!("  - Mutability: Mutable shared access");
    println!("  - Example: Graphs, trees with parent refs");
    println!();

    println!("Cow<T>:");
    println!("  - Use for: Conditional cloning");
    println!("  - Ownership: Borrowed or owned");
    println!("  - Mutability: Clones on write");
    println!("  - Example: String processing, config");
    println!();

    println!("Arc<T> (from concurrency.rs):");
    println!("  - Use for: Multiple owners across threads");
    println!("  - Ownership: Atomic reference counted");
    println!("  - Mutability: Immutable (use Arc<Mutex<T>> for mutable)");
    println!("  - Example: Thread-safe shared state");
}

// ============================================================================
// Practical Interview Patterns
// ============================================================================

/// Singly linked list using Box.
#[allow(dead_code)]
#[derive(Debug)]
struct LinkedList {
    head: Option<Box<ListNode>>,
}

#[allow(dead_code)]
#[derive(Debug)]
struct ListNode {
    value: i32,
    next: Option<Box<ListNode>>,
}

impl LinkedList {
    fn new() -> Self {
        LinkedList { head: None }
    }

    fn push_front(&mut self, value: i32) {
        let new_node = Box::new(ListNode {
            value,
            next: self.head.take(),
        });
        self.head = Some(new_node);
    }

    fn pop_front(&mut self) -> Option<i32> {
        self.head.take().map(|node| {
            self.head = node.next;
            node.value
        })
    }
}

#[allow(dead_code)]
fn demonstrate_linked_list() {
    let mut list = LinkedList::new();
    list.push_front(1);
    list.push_front(2);
    list.push_front(3);

    println!("List: {list:?}");

    while let Some(value) = list.pop_front() {
        println!("Popped: {value}");
    }
}

/// LRU Cache key-value node using Rc<RefCell<T>>.
#[allow(dead_code)]
#[derive(Debug)]
struct LRUNode {
    key: i32,
    value: i32,
    prev: Option<Weak<RefCell<LRUNode>>>,
    next: Option<Rc<RefCell<LRUNode>>>,
}

impl LRUNode {
    fn new(key: i32, value: i32) -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(LRUNode {
            key,
            value,
            prev: None,
            next: None,
        }))
    }
}

#[allow(dead_code)]
fn demonstrate_lru_node() {
    // Create doubly linked list nodes
    let node1 = LRUNode::new(1, 100);
    let node2 = LRUNode::new(2, 200);

    // Link them
    node1.borrow_mut().next = Some(Rc::clone(&node2));
    node2.borrow_mut().prev = Some(Rc::downgrade(&node1));

    println!("Node1: {:?}", node1.borrow().value);
    println!("Node2: {:?}", node2.borrow().value);

    // Navigate
    if let Some(next) = &node1.borrow().next {
        println!("Node1's next value: {}", next.borrow().value);
    }

    if let Some(prev_weak) = &node2.borrow().prev {
        if let Some(prev) = prev_weak.upgrade() {
            println!("Node2's prev value: {}", prev.borrow().value);
        }
    }
}
