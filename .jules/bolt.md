**[Composite Pattern - Avoiding string allocation]**
**Learning:** Using `write!` directly to a pre-allocated `String` instead of creating intermediate `format!` strings and pushing them is more efficient.
**Action:** Replace `let mut out = format!("[Window: {}]\n", self.title);` with `let mut out = String::with_capacity(...); let _ = write!(out, ...);`
**[Word Ladder BFS - Removing unnecessary heap allocations]**
**Learning:** Iterating by reference over a set and cloning each element (`for word in set { let mut cloned = word.clone(); }`) causes significant heap allocation overhead inside a hot BFS path, especially for collections of strings or vectors like `HashSet<Vec<u8>>`.
**Action:** When you intend to mutate the items, consume the collection directly (`for mut word in set`) so that the items are moved rather than borrowed and cloned, eliminating allocations.
