**[Composite Pattern - Avoiding string allocation]**
**Learning:** Using `write!` directly to a pre-allocated `String` instead of creating intermediate `format!` strings and pushing them is more efficient.
**Action:** Replace `let mut out = format!("[Window: {}]\n", self.title);` with `let mut out = String::with_capacity(...); let _ = write!(out, ...);`

**Optimize Vec allocations with with_capacity and extend**
**Learning:** Found multiple instances where `.collect()` and `Vec::new()` with manual push loops resulted in intermediate allocations or capacity growth overhead.
**Action:** Replaced `Vec::new()` with `Vec::with_capacity(n)` and `.collect()` with `.extend()` to prevent intermediate allocations and dynamically sized vector growth.
