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

pub fn lowest_common_ancestor_optimized(
    root: Option<Rc<RefCell<TreeNode>>>,
    p: Option<Rc<RefCell<TreeNode>>>,
    q: Option<Rc<RefCell<TreeNode>>>,
) -> Option<Rc<RefCell<TreeNode>>> {
    let node = root?;
    let p_unwrapped = p.as_ref().unwrap();
    let q_unwrapped = q.as_ref().unwrap();

    if Rc::ptr_eq(&node, p_unwrapped) || Rc::ptr_eq(&node, q_unwrapped) {
        return Some(node);
    }

    let left = lowest_common_ancestor_optimized(node.borrow().left.clone(), p.clone(), q.clone());
    let right = lowest_common_ancestor_optimized(node.borrow().right.clone(), p.clone(), q.clone());

    if left.is_some() && right.is_some() {
        return Some(node);
    }

    left.or(right)
}

pub fn lowest_common_ancestor_optimized_new(
    root: Option<Rc<RefCell<TreeNode>>>,
    p: Option<Rc<RefCell<TreeNode>>>,
    q: Option<Rc<RefCell<TreeNode>>>,
) -> Option<Rc<RefCell<TreeNode>>> {
    let p_ref = p.as_ref().unwrap();
    let q_ref = q.as_ref().unwrap();

    fn dfs(
        node: Option<Rc<RefCell<TreeNode>>>,
        p_ref: &Rc<RefCell<TreeNode>>,
        q_ref: &Rc<RefCell<TreeNode>>,
    ) -> Option<Rc<RefCell<TreeNode>>> {
        let node = node?;

        if Rc::ptr_eq(&node, p_ref) || Rc::ptr_eq(&node, q_ref) {
            return Some(node);
        }

        let left = dfs(node.borrow().left.clone(), p_ref, q_ref);
        let right = dfs(node.borrow().right.clone(), p_ref, q_ref);

        if left.is_some() && right.is_some() {
            return Some(node);
        }

        left.or(right)
    }

    dfs(root, p_ref, q_ref)
}

fn create_tree(depth: i32) -> (Rc<RefCell<TreeNode>>, Rc<RefCell<TreeNode>>, Rc<RefCell<TreeNode>>) {
    let root = TreeNode::new(0);
    let mut current = root.clone();
    let mut i = 1;
    while i < depth {
        let new_node = TreeNode::new(i);
        current.borrow_mut().left = Some(new_node.clone());
        current = new_node;
        i += 1;
    }
    let p = current.clone();

    let mut current = root.clone();
    let mut i = 1;
    while i < depth {
        let new_node = TreeNode::new(-i);
        current.borrow_mut().right = Some(new_node.clone());
        current = new_node;
        i += 1;
    }
    let q = current.clone();

    (root, p, q)
}

fn main() {
    let (root, p, q) = create_tree(1000);

    let start = Instant::now();
    for _ in 0..1000 {
        std::hint::black_box(lowest_common_ancestor_optimized(Some(root.clone()), Some(p.clone()), Some(q.clone())));
    }
    println!("Old: {:?}", start.elapsed());

    let start = Instant::now();
    for _ in 0..1000 {
        std::hint::black_box(lowest_common_ancestor_optimized_new(Some(root.clone()), Some(p.clone()), Some(q.clone())));
    }
    println!("New: {:?}", start.elapsed());
}
