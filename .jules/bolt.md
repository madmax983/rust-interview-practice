**[Composite Pattern - Avoiding string allocation]**
**Learning:** Using `write!` directly to a pre-allocated `String` instead of creating intermediate `format!` strings and pushing them is more efficient.
**Action:** Replace `let mut out = format!("[Window: {}]\n", self.title);` with `let mut out = String::with_capacity(...); let _ = write!(out, ...);`
**[Clippy regression under -D warnings]**
**Learning:** The clippy::manual_assert_eq lint is unknown in some environments/versions, which fails compilation when -D warnings is enabled.
**Action:** Use #[allow(unknown_lints, clippy::manual_assert_eq)] to bypass the unknown lint error while preserving the explicit warning suppression.
