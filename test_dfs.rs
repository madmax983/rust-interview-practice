use std::cell::RefCell;
use std::rc::Rc;

#[derive(Debug, PartialEq, Eq)]
pub struct TreeNode {
    pub val: i32,
    pub left: Option<Rc<RefCell<Self>>>,
    pub right: Option<Rc<RefCell<Self>>>,
}

pub fn lowest_common_ancestor_optimized(
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
fn main() {}
