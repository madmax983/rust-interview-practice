**[Composite Pattern - Avoiding string allocation]**
**Learning:** Using `write!` directly to a pre-allocated `String` instead of creating intermediate `format!` strings and pushing them is more efficient.
**Action:** Replace `let mut out = format!("[Window: {}]\n", self.title);` with `let mut out = String::with_capacity(...); let _ = write!(out, ...);`
**[HTTP Router Optimization - Avoid intermediate allocations]**\n**Learning:** Avoiding  for intermediate arrays and leveraging direct lazy iterators eliminates O(N) heap allocations on critical hot paths like route matching.\n**Action:** Iterate directly over  results rather than calling  into an intermediate vector.
**[HTTP Router Optimization - Avoid intermediate allocations]**
**Learning:** Avoiding `.collect::<Vec<&str>>()` for intermediate arrays and leveraging direct lazy iterators eliminates O(N) heap allocations on critical hot paths like route matching.
**Action:** Iterate directly over `string.split(...)` results rather than calling `.collect()` into an intermediate vector.
