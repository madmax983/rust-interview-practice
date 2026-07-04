**[Composite Pattern - Avoiding string allocation]**
**Learning:** Using `write!` directly to a pre-allocated `String` instead of creating intermediate `format!` strings and pushing them is more efficient.
**Action:** Replace `let mut out = format!("[Window: {}]\n", self.title);` with `let mut out = String::with_capacity(...); let _ = write!(out, ...);`

**[Performance Patterns - Avoiding string allocation inside loops]**
**Learning:** Using `write!` directly to a pre-allocated `String` buffer instead of intermediate `.to_string()` bindings combined with `.push_str()` avoids unnecessary intermediate heap allocations on the hot path.
**Action:** Replace `.push_str(&i.to_string())` with `let _ = write!(buffer, "{}", i);`
