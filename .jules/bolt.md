**[Composite Pattern - Avoiding string allocation]**
**Learning:** Using `write!` directly to a pre-allocated `String` instead of creating intermediate `format!` strings and pushing them is more efficient.
**Action:** Replace `let mut out = format!("[Window: {}]\n", self.title);` with `let mut out = String::with_capacity(...); let _ = write!(out, ...);`
**[Lexer Tokenization - Avoiding String allocations]**
**Learning:** Building strings character-by-character using `.push()` inside loops generates amortized O(log N) allocations and unnecessary overhead. When the source string is already in memory, slicing (`&self.input[start_pos..self.pos]`) is much faster and defers allocation to a single `.to_string()` call at the end (or entirely avoids it).
**Action:** Replace `let mut s = String::new(); loop { s.push(c) }` with `let s = &self.input[start_pos..end_pos];` when tokenizing strings.
