**[Composite Pattern - Avoiding string allocation]**
**Learning:** Using `write!` directly to a pre-allocated `String` instead of creating intermediate `format!` strings and pushing them is more efficient.
**Action:** Replace `let mut out = format!("[Window: {}]\n", self.title);` with `let mut out = String::with_capacity(...); let _ = write!(out, ...);`
**[SQL Engine Lexer Allocations]**
**Learning:** In string lexing and tokenization, initializing arrays and strings with `Vec::new()` and `String::new()` results in numerous intermediate heap re-allocations as characters are accumulated or tokens added.
**Action:** Use `Vec::with_capacity` based on the input size (`input.len() / 4` as a heuristic) and `String::with_capacity(N)` for text buffer construction to avoid growing the buffers dynamically during single-token or full-input parsing.
