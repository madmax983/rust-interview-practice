with open("src/design_patterns/composite.rs", "r") as f:
    code = f.read()

search = """impl UiComponent for WindowComponent {
    fn render(&self) -> String {
        let mut out = format!("[Window: {}]\\n", self.title);
        for child in &self.children {
            out.push_str("  ");
            out.push_str(&child.render());
            out.push('\\n');
        }
        out
    }
}"""

replace = """impl UiComponent for WindowComponent {
    fn render(&self) -> String {
        use std::fmt::Write;
        // ⚡ BOLT OPTIMIZATION: Use `String::with_capacity` and `write!` to avoid
        // intermediate `format!` allocations and reallocations when appending strings.
        let mut out = String::with_capacity(32 + self.children.len() * 32); // Heuristic
        let _ = write!(out, "[Window: {}]\\n", self.title);
        for child in &self.children {
            out.push_str("  ");
            out.push_str(&child.render());
            out.push('\\n');
        }
        out
    }
}"""

if search in code:
    with open("src/design_patterns/composite.rs", "w") as f:
        f.write(code.replace(search, replace))
    print("Replaced successfully!")
else:
    print("Search text not found!")
