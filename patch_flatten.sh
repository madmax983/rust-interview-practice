#!/bin/bash
sed -i 's|Link: https://leetcode.com/problems/flatten-binary-tree-to-linked-list/|Link: <https://leetcode.com/problems/flatten-binary-tree-to-linked-list/>|g' src/trees/flatten_binary_tree_to_linked_list.rs
sed -i 's/For LeetCode binary tree problems/For `LeetCode` binary tree problems/g' src/trees/flatten_binary_tree_to_linked_list.rs
sed -i 's/pub left: Option<Rc<RefCell<TreeNode>>>/pub left: Option<Rc<RefCell<Self>>>/g' src/trees/flatten_binary_tree_to_linked_list.rs
sed -i 's/pub right: Option<Rc<RefCell<TreeNode>>>/pub right: Option<Rc<RefCell<Self>>>/g' src/trees/flatten_binary_tree_to_linked_list.rs
