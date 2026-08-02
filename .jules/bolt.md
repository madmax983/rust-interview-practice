**[Composite Pattern - Avoiding string allocation]**
**Learning:** Using `write!` directly to a pre-allocated `String` instead of creating intermediate `format!` strings and pushing them is more efficient.
**Action:** Replace `let mut out = format!("[Window: {}]\n", self.title);` with `let mut out = String::with_capacity(...); let _ = write!(out, ...);`
**[Word Ladder BFS - Consuming HashSet]**
**Learning:** In BFS using HashSets for layers, instead of iterating over `&word` and cloning (`word.clone()`), consuming the set `for mut word in set` provides owned values directly.
**Action:** Replace `for word in begin_set { let mut current_word_bytes = word.clone(); }` with `for mut current_word_bytes in begin_set { }` to eliminate `Vec<u8>` allocation per word in hot loop.
