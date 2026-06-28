**[Composite Pattern - Avoiding string allocation]**
**Learning:** Using `write!` directly to a pre-allocated `String` instead of creating intermediate `format!` strings and pushing them is more efficient.
**Action:** Replace `let mut out = format!("[Window: {}]\n", self.title);` with `let mut out = String::with_capacity(...); let _ = write!(out, ...);`

**[JWT Decode - Avoiding string splitting allocation]**
**Learning:** Using `.split('.').collect::<Vec<&str>>()` to parse a known number of segments (like the 3 segments of a JWT) introduces an unnecessary heap allocation for the intermediate `Vec`.
**Action:** Replace `split().collect()` with a mutable iterator (`let mut parts = token.split('.');`) and manually consume segments with `.next()` for zero-allocation parsing when the number of parts is bounded and known.
