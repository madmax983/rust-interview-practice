cat << 'INNER_EOF' > modify_composite_2.patch
--- src/design_patterns/composite.rs
+++ src/design_patterns/composite.rs
@@ -147,7 +147,7 @@
         // ⚡ BOLT OPTIMIZATION: Use `String::with_capacity` and `write!` to avoid
         // intermediate `format!` allocations and reallocations when appending strings.
         let mut out = String::with_capacity(32 + self.children.len() * 32); // Heuristic
-        let _ = write!(out, "[Window: {}]\n", self.title);
+        let _ = write!(&mut out, "[Window: {}]\n", self.title);
         for child in &self.children {
             out.push_str("  ");
             out.push_str(&child.render());
INNER_EOF
patch src/design_patterns/composite.rs < modify_composite_2.patch
cargo test --manifest-path Cargo.toml --all-targets --all-features
