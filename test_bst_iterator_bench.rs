use std::cell::RefCell;
use std::rc::Rc;
use std::time::Instant;

#[derive(Debug, PartialEq, Eq)]
pub struct TreeNode {
    pub val: i32,
    pub left: Option<Rc<RefCell<Self>>>,
    pub right: Option<Rc<RefCell<Self>>>,
}

impl TreeNode {
    pub fn new(val: i32) -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(Self {
            val,
            left: None,
            right: None,
        }))
    }
}
pub struct BSTIterator {
    stack: Vec<Rc<RefCell<TreeNode>>>,
}

impl BSTIterator {
    pub fn new(root: Option<Rc<RefCell<TreeNode>>>) -> Self {
        let mut iterator = Self { stack: Vec::new() };
        if let Some(node) = root {
            iterator.push_all_left(node);
        }
        iterator
    }

    pub fn next(&mut self) -> i32 {
        let node = self.stack.pop().unwrap();
        let val = node.borrow().val;
        let right_child = node.borrow().right.clone();

        if let Some(right) = right_child {
            self.push_all_left(right);
        }

        val
    }

    pub fn has_next(&self) -> bool {
        !self.stack.is_empty()
    }

    fn push_all_left(&mut self, mut node: Rc<RefCell<TreeNode>>) {
        loop {
            let left_child = {
                let n = node.borrow();
                n.left.clone()
            };
            self.stack.push(node.clone());
            if let Some(left) = left_child {
                node = left;
            } else {
                break;
            }
        }
    }
}

pub struct BSTIteratorNew {
    stack: Vec<Rc<RefCell<TreeNode>>>,
}

impl BSTIteratorNew {
    pub fn new(root: Option<Rc<RefCell<TreeNode>>>) -> Self {
        let mut iterator = Self { stack: Vec::new() };
        if let Some(node) = root {
            iterator.push_all_left(node);
        }
        iterator
    }

    pub fn next(&mut self) -> i32 {
        let node = self.stack.pop().unwrap();
        let val = node.borrow().val;
        let right_child = node.borrow().right.clone();

        if let Some(right) = right_child {
            self.push_all_left(right);
        }

        val
    }

    pub fn has_next(&self) -> bool {
        !self.stack.is_empty()
    }

    fn push_all_left(&mut self, mut node: Rc<RefCell<TreeNode>>) {
        loop {
            self.stack.push(node.clone());
            let left_child = node.borrow().left.clone();
            if let Some(left) = left_child {
                node = left;
            } else {
                break;
            }
        }
    }
}

fn create_tree(depth: i32) -> Rc<RefCell<TreeNode>> {
    let root = TreeNode::new(0);
    let mut current = root.clone();
    let mut i = 1;
    while i < depth {
        let new_node = TreeNode::new(i);
        current.borrow_mut().left = Some(new_node.clone());
        current = new_node;
        i += 1;
    }
    root
}

fn main() {
    let root = create_tree(1000);

    let start = Instant::now();
    for _ in 0..1000 {
        let mut iter = BSTIterator::new(Some(root.clone()));
        while iter.has_next() {
            std::hint::black_box(iter.next());
        }
    }
    println!("Old: {:?}", start.elapsed());

    let start = Instant::now();
    for _ in 0..1000 {
        let mut iter = BSTIteratorNew::new(Some(root.clone()));
        while iter.has_next() {
            std::hint::black_box(iter.next());
        }
    }
    println!("New: {:?}", start.elapsed());
}
