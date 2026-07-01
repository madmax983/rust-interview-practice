**[Composite Pattern - Avoiding string allocation]**
**Learning:** Using `write!` directly to a pre-allocated `String` instead of creating intermediate `format!` strings and pushing them is more efficient.
**Action:** Replace `let mut out = format!("[Window: {}]\n", self.title);` with `let mut out = String::with_capacity(...); let _ = write!(out, ...);`
**[SQL Engine Lexer]**
**Learning:** Pre-allocating `String::with_capacity()` in tight loops when lexing string literals, identifiers, and numbers avoids frequent intermediate heap allocations.
**Action:** Always pre-allocate `String`s and `Vec`s with expected capacities when scanning tokens.
