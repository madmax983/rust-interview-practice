**[Composite Pattern - Avoiding string allocation]**
**Learning:** Using `write!` directly to a pre-allocated `String` instead of creating intermediate `format!` strings and pushing them is more efficient.
**Action:** Replace `let mut out = format!("[Window: {}]\n", self.title);` with `let mut out = String::with_capacity(...); let _ = write!(out, ...);`

**[Avoid Intermediate Vec Allocations During Iteration]**
**Learning:** `clippy` did not catch this, but removing `.collect::<Vec<_>>()` when a chain of iterator methods is only used to iterate over elements (`for part in parts`) avoids unnecessary heap allocations and speeds up hot paths like HTTP routing.
**Action:** When seeing `.collect::<Vec<_>>()` assigned to a variable that is immediately consumed by a `for` loop, remove the `collect` and assignment and iterate over the `Iterator` directly.
