**[Composite Pattern - Avoiding string allocation]**
**Learning:** Using `write!` directly to a pre-allocated `String` instead of creating intermediate `format!` strings and pushing them is more efficient.
**Action:** Replace `let mut out = format!("[Window: {}]\n", self.title);` with `let mut out = String::with_capacity(...); let _ = write!(out, ...);`
**[Optimizing Lowest Common Ancestor DFS Clones]**
**Learning:** In recursive tree traversals taking `Option<Rc<RefCell<TreeNode>>>`, cloning the `Rc` for every left/right child call creates significant $O(N)$ overhead from ref-count churn and new `Option` allocations. We can easily eliminate this by passing `&Option<Rc<RefCell<TreeNode>>>` to an internal `dfs` helper function.
**Action:** When a public API strictly requires an `Option<Rc<RefCell<T>>>` signature, don't cascade the cloning behavior into the internal logic. Immediately wrap the logic in a helper function that takes references, only cloning `Rc` when exactly needed (e.g. at the target nodes).
**[Fixing clippy::ref_option]**
**Learning:** `&Option<T>` triggers a pedantic clippy lint (`clippy::ref_option`), because it is more idiomatic to pass `Option<&T>` in Rust. This caused a CI failure when attempting to optimize the lowest common ancestor DFS function.
**Action:** Always prefer `Option<&T>` over `&Option<T>` for function parameters to align with idiomatic Rust and satisfy the `clippy::ref_option` lint. Use `.as_ref()` on the Option when passing it in.
