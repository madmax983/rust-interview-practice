**[Composite Pattern - Avoiding string allocation]**
**Learning:** Using `write!` directly to a pre-allocated `String` instead of creating intermediate `format!` strings and pushing them is more efficient.
**Action:** Replace `let mut out = format!("[Window: {}]\n", self.title);` with `let mut out = String::with_capacity(...); let _ = write!(out, ...);`

**[HTTP Router - Avoiding string allocation]**
**Learning:** Using an iterator directly without `collect::<Vec<_>>()` avoids unnecessary memory allocations during repeated path traversal algorithm execution.
**Action:** Replace `let parts: Vec<&str> = path.split('/').filter(|p| !p.is_empty()).collect();` with `let parts = path.split('/').filter(|p| !p.is_empty());`
