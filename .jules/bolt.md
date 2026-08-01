**[Composite Pattern - Avoiding string allocation]**
**Learning:** Using `write!` directly to a pre-allocated `String` instead of creating intermediate `format!` strings and pushing them is more efficient.
**Action:** Replace `let mut out = format!("[Window: {}]\n", self.title);` with `let mut out = String::with_capacity(...); let _ = write!(out, ...);`

**[SQL Engine WHERE Clause Allocation]**
**Learning:** `Value` variants contained `String`, so `evaluate_expr` allocating by `.clone()` caused an allocation per string in the `WHERE` clause.
**Action:** Changed `evaluate_expr` to return `&'a Value`, binding to the lifetime of `&'a Row`.
