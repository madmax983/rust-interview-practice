**[Composite Pattern - Avoiding string allocation]**
**Learning:** Using `write!` directly to a pre-allocated `String` instead of creating intermediate `format!` strings and pushing them is more efficient.
**Action:** Replace `let mut out = format!("[Window: {}]\n", self.title);` with `let mut out = String::with_capacity(...); let _ = write!(out, ...);`
**[RESP Serialization - Avoiding `to_string().as_bytes()` allocations]**
**Learning:** When serializing integers or lengths to a byte buffer (`Vec<u8>`), calling `.to_string().as_bytes()` forces an unnecessary heap allocation of an intermediate `String`.
**Action:** Replace `buf.extend_from_slice(val.to_string().as_bytes());` with `let _ = write!(buf, "{}", val);` (after bringing `std::io::Write` into scope) to format directly into the `Vec<u8>` buffer without temporary allocations.
