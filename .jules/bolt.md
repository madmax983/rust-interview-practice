**[Composite Pattern - Avoiding string allocation]**
**Learning:** Using `write!` directly to a pre-allocated `String` instead of creating intermediate `format!` strings and pushing them is more efficient.
**Action:** Replace `let mut out = format!("[Window: {}]\n", self.title);` with `let mut out = String::with_capacity(...); let _ = write!(out, ...);`
**[SQL Engine Lexer - Pre-allocating Capacity]**
**Learning:** Initializing `String` and `Vec` with `.with_capacity()` instead of `.new()` avoids unnecessary intermediate heap reallocations when parsing.
**Action:** Always prefer `String::with_capacity(estimated_size)` over `String::new()` in string parsing loops where the approximate length is known.
