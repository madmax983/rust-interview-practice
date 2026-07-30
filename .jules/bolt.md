**[Composite Pattern - Avoiding string allocation]**
**Learning:** Using `write!` directly to a pre-allocated `String` instead of creating intermediate `format!` strings and pushing them is more efficient.
**Action:** Replace `let mut out = format!("[Window: {}]\n", self.title);` with `let mut out = String::with_capacity(...); let _ = write!(out, ...);`
**[Avoid unnecessary clones when consuming HashSet]**
**Learning:** Consuming a HashSet directly yields owned values, avoiding the need to iterate by reference and clone, which saves heap allocations.
**Action:** Use `for mut item in set` rather than `for item in set { let mut modified = item.clone(); }`.
