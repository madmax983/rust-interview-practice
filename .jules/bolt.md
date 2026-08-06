**[Composite Pattern - Avoiding string allocation]**
**Learning:** Using `write!` directly to a pre-allocated `String` instead of creating intermediate `format!` strings and pushing them is more efficient.
**Action:** Replace `let mut out = format!("[Window: {}]\n", self.title);` with `let mut out = String::with_capacity(...); let _ = write!(out, ...);`
**[Lowest Common Ancestor Rc Cloning Optimization]**
**Learning:** Calling `.clone()` on an `Rc` pointer during recursive tree traversals adds noticeable overhead because it must fetch and increment/decrement the atomic-like (even if single threaded, it does a load/add/store) reference counter at each level of depth, which gets expensive when traversing deep trees. By creating an inner helper `fn dfs` that passes the `Rc` pointers as borrowed references (`&Rc<RefCell<TreeNode>>`), we can entirely bypass this overhead.
**Action:** When working on tree problems that receive `Option<Rc<RefCell<TreeNode>>>`, try to use an inner helper closure/function that borrows the `Rc` parameters as references.
