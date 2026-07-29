**[Composite Pattern - Avoiding string allocation]**
**Learning:** Using `write!` directly to a pre-allocated `String` instead of creating intermediate `format!` strings and pushing them is more efficient.
**Action:** Replace `let mut out = format!("[Window: {}]\n", self.title);` with `let mut out = String::with_capacity(...); let _ = write!(out, ...);`
**[Word Ladder BFS Loop Optimization]**
**Learning:** In BFS algorithms that consume a `HashSet` boundary level by level (like bidirectional BFS), iterating over the set by reference (`for word in begin_set`) and subsequently cloning the items into temporary variables creates significant, unnecessary heap allocations. Because the `HashSet` itself is being discarded at the end of the iteration, we can consume it directly by value (`for mut word in begin_set`).
**Action:** When working with collections that are recreated every loop iteration, always look for opportunities to consume the old collection by value to avoid redundant `.clone()` calls on owned data like `Vec` or `String`.
