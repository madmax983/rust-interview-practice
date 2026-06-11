**[Composite Pattern - Avoiding string allocation]**
**Learning:** Using `write!` directly to a pre-allocated `String` instead of creating intermediate `format!` strings and pushing them is more efficient.
**Action:** Replace `let mut out = format!("[Window: {}]\n", self.title);` with `let mut out = String::with_capacity(...); let _ = write!(out, ...);`

**[RESP Serialization Optimization]**
**Learning:** Avoid intermediate `String` allocations via `.to_string().as_bytes()` when serializing numbers into a `Vec<u8>`. Use `std::io::Write::write_fmt(buf, format_args!("{}", num))` instead.
**Action:** Replaced `buf.extend_from_slice(i.to_string().as_bytes())` with `std::io::Write::write_fmt(buf, format_args!("{}", i)).unwrap()` in the RESP serializer.
