**[Composite Pattern - Avoiding string allocation]**
**Learning:** Using `write!` directly to a pre-allocated `String` instead of creating intermediate `format!` strings and pushing them is more efficient.
**Action:** Replace `let mut out = format!("[Window: {}]\n", self.title);` with `let mut out = String::with_capacity(...); let _ = write!(out, ...);`
**[Embedded SQL Engine Lexer Zero-Cost Optimization]
**Learning:** Found string allocations via character-by-character `push` loops in the lexer for `String::new()` in `read_string_literal`, `read_identifier_or_keyword`, and `read_integer_literal`.
**Action:** Replaced these with `&self.input[start_pos..self.pos]` string slicing by tracking `start_pos = self.pos` before the loop. This leverages Rust's string slices and avoids intermediate heap allocations, while keeping logic exact.
