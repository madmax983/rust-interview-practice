**[Composite Pattern - Avoiding string allocation]**
**Learning:** Using `write!` directly to a pre-allocated `String` instead of creating intermediate `format!` strings and pushing them is more efficient.
**Action:** Replace `let mut out = format!("[Window: {}]\n", self.title);` with `let mut out = String::with_capacity(...); let _ = write!(out, ...);`
**[Serialization - Avoiding string allocation]**
**Learning:** Using `write!` directly to a pre-allocated `Vec<u8>` instead of `i.to_string().as_bytes()` avoids creating a temporary string.
**Action:** Replace `buf.extend_from_slice(i.to_string().as_bytes());` with `write!(buf, "{}", i).unwrap();`
