**[Composite Pattern - Avoiding string allocation]**
**Learning:** Using `write!` directly to a pre-allocated `String` instead of creating intermediate `format!` strings and pushing them is more efficient.
**Action:** Replace `let mut out = format!("[Window: {}]\n", self.title);` with `let mut out = String::with_capacity(...); let _ = write!(out, ...);`

**[SQL Engine - Eliminating intermediate reallocations]**
**Learning:** Using `Vec::new()` for accumulating tokens causes multiple heap reallocations as the vector grows dynamically during tokenization.
**Action:** Pre-allocate vectors based on conservative estimates (e.g., `Vec::with_capacity(data.len() / 4)`) to minimize heap reallocations when the upper bound is known or predictable.
