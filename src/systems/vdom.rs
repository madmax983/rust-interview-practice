//! # Virtual DOM Implementation
//!
//! Implements a minimal UI reconciliation engine by decoupling state from the DOM through tree representations
//! and an efficient diffing algorithm.
//!
//! **Replaces Crates:** `yew`, `dioxus` (core engine), `react` (JavaScript)
//!
//! **Real-world Usage:**
//! - Frontend web frameworks (React, Vue, Yew, Dioxus).
//! - Declarative UI systems (`SwiftUI`, Jetpack Compose, Flutter).
//! - Server-Side Rendering (SSR) systems.
//!
//! **Why build it yourself?**
//! Building a Virtual DOM teaches you tree traversal algorithms and the concept of "reconciliation".
//! You'll learn why manipulating the real DOM is slow, and how a fast in-memory tree diffing algorithm
//! can generate a minimal list of mutations (patches) to apply, providing declarative UI updates efficiently.

// =========================================================================================
// Architecture
// =========================================================================================
//
// Data Structure:
//
//      Old VTree                 New VTree
//     div (id: 1)               div (id: 1)
//     ├── h1 "Title"            ├── h1 "New Title"    <-- Changed (Update Text)
//     └── p "Content"           ├── p "Content"
//                               └── span "Footer"     <-- Added (Insert Node)
//
// Invariants:
// 1. Two elements of different types (e.g., `div` vs `span`) produce different trees. The old is entirely replaced.
// 2. Elements of the same type share the same identity; their attributes and children are diffed.
// 3. Keys (if implemented) uniquely identify elements among siblings to optimize reordering.
//
// Complexity:
// ┌───────────────┬────────┬────────┐
// │ Operation     │ Time   │ Space  │
// ├───────────────┼────────┼────────┤
// │ Diff (Trees)  │ O(N)*  │ O(N)   │
// │ Apply Patch   │ O(P)   │ O(1)   │
// └───────────────┴────────┴────────┘
// * Where N is the number of nodes. O(N) relies on the heuristic invariants above; a true minimum edit distance is O(N^3).
//   P is the number of patches generated.
//
// Design Decisions:
// - VNode is an enum representing either an Element or Text.
// - Diffing returns a list of Patches.
// - We use strings and vectors. A production implementation would use arena allocators or `bumpalo` to avoid allocator overhead during rapid re-renders.

use std::collections::HashMap;

/// Represents a node in the Virtual DOM.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VNode {
    Element(VElement),
    Text(String),
}

/// Represents an HTML element in the Virtual DOM.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VElement {
    pub tag: String,
    pub attributes: HashMap<String, String>,
    pub children: Vec<VNode>,
}

impl VElement {
    pub fn new(tag: impl Into<String>) -> Self {
        Self {
            tag: tag.into(),
            attributes: HashMap::new(),
            children: Vec::new(),
        }
    }

    #[must_use]
    pub fn with_attr(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.attributes.insert(key.into(), value.into());
        self
    }

    #[must_use]
    pub fn with_child(mut self, child: VNode) -> Self {
        self.children.push(child);
        self
    }
}

/// Helper to create an Element node.
pub fn el(tag: impl Into<String>) -> VElement {
    VElement::new(tag)
}

/// Helper to create a Text node.
pub fn text(content: impl Into<String>) -> VNode {
    VNode::Text(content.into())
}

// =========================================================================================
// Reconciliation (Diffing) Algorithm
// =========================================================================================

/// Represents a mutation to apply to the real DOM.
#[derive(Debug, PartialEq, Eq)]
pub enum Patch<'a> {
    /// Completely replace a node with a new one.
    Replace(&'a VNode),
    /// Update the text content of a node.
    UpdateText(&'a str),
    /// Set or update an attribute.
    SetAttribute(&'a str, &'a str),
    /// Remove an attribute.
    RemoveAttribute(&'a str),
    /// Append a new child node.
    AppendChild(&'a VNode),
    /// Remove a child node.
    RemoveChild(usize),
}

/// Recursively diffs two Virtual DOM nodes and returns a list of patches.
///
/// Note: In a real implementation, patches need to be tied to specific DOM node indices or paths.
/// For simplicity, we just collect patches for the current node and its children.
#[must_use]
pub fn diff<'a>(old: &'a VNode, new: &'a VNode) -> Vec<Patch<'a>> {
    let mut patches = Vec::new();
    diff_into(old, new, &mut patches);
    patches
}

/// Recursively diffs two Virtual DOM nodes, pushing patches into the provided vector.
///
/// ⚡ BOLT OPTIMIZATION: Passing `&mut Vec<Patch<'a>>` prevents intermediate O(P) allocations
/// from bubbling up the recursive call tree, acting as a zero-cost abstraction for tree traversals.
pub fn diff_into<'a>(old: &'a VNode, new: &'a VNode, patches: &mut Vec<Patch<'a>>) {
    match (old, new) {
        // If both are text, check if the text changed.
        (VNode::Text(old_txt), VNode::Text(new_txt)) => {
            if old_txt != new_txt {
                patches.push(Patch::UpdateText(new_txt));
            }
        }

        // If both are elements of the same tag, diff attributes and children.
        (VNode::Element(old_el), VNode::Element(new_el)) if old_el.tag == new_el.tag => {
            diff_attributes(old_el, new_el, patches);
            diff_children(old_el, new_el, patches);
        }

        // Different node types entirely (Text vs Element, or different tags). Replace completely.
        _ => {
            patches.push(Patch::Replace(new));
        }
    }
}

fn diff_attributes<'a>(old_el: &'a VElement, new_el: &'a VElement, patches: &mut Vec<Patch<'a>>) {
    // Find new or updated attributes
    for (k, v) in &new_el.attributes {
        if old_el.attributes.get(k) != Some(v) {
            patches.push(Patch::SetAttribute(k, v));
        }
    }

    // Find removed attributes
    for k in old_el.attributes.keys() {
        if !new_el.attributes.contains_key(k) {
            patches.push(Patch::RemoveAttribute(k));
        }
    }
}

fn diff_children<'a>(old_el: &'a VElement, new_el: &'a VElement, patches: &mut Vec<Patch<'a>>) {
    let old_len = old_el.children.len();
    let new_len = new_el.children.len();

    // Diff existing children
    let min_len = std::cmp::min(old_len, new_len);
    for i in 0..min_len {
        // RUST INSIGHT:
        // In a real framework, we'd need a way to associate these `child_patches` with the specific child index `i`.
        // We extend the flat list here for simplicity, but a structured patch tree is required for actual DOM updates.
        diff_into(&old_el.children[i], &new_el.children[i], patches);
    }

    // New children were added
    if new_len > old_len {
        for child in &new_el.children[old_len..] {
            patches.push(Patch::AppendChild(child));
        }
    }

    // Old children were removed
    if old_len > new_len {
        // Notice we remove from the end backwards, or just specify the index to remove.
        for i in (new_len..old_len).rev() {
            patches.push(Patch::RemoveChild(i));
        }
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to canonical crates (`yew`, `dioxus`):
// - `yew` uses components with lifecycles and agents for state management. Its VDOM tracks real `web_sys::Node`s directly.
// - `dioxus` is highly optimized. It doesn't recreate the whole VDOM on every render. It uses static analysis (RSX macros) to separate static and dynamic parts of the tree, only diffing the dynamic parts.
// - `dioxus` uses a Bump allocator (`bumpalo`) to allocate the VTree in a single contiguous block of memory, which is dropped instantly after render, avoiding `String`/`Vec` allocation overheads.
//
// What's missing vs. production:
// - Event listeners (onClick, onInput).
// - Component abstraction and state hooks (`useState`, `useEffect`).
// - A keyed diffing algorithm (React's `key` prop) to efficiently handle lists that reorder, insert in the middle, or remove from the middle (currently O(N) shifts).
// - Path/Index mapping in `Patch` to know exactly *which* deep descendant DOM node a patch applies to.
//
// Suggested Next Steps:
// - Implement a `key` property on `VElement` and upgrade `diff_children` to use a HashMap for keyed diffing.
// - Create a `PatchTree` instead of a flat `Vec<Patch>` to correctly apply updates recursively.
// - Integrate with `web-sys` to compile to WebAssembly and actually render to the browser DOM.

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================================
    // Benchmarking Note
    // =========================================================================================
    // To benchmark the Virtual DOM diffing performance:
    // ```rust
    // use std::time::Instant;
    // use std::hint::black_box;
    //
    // fn bench_diffing() {
    //     let old_tree = VNode::Element(el("div").with_child(text("Hello")).with_child(text("World")));
    //     let new_tree = VNode::Element(el("div").with_child(text("Hello")).with_child(text("Rust")));
    //
    //     let start = Instant::now();
    //     for _ in 0..10_000 {
    //         black_box(diff(black_box(&old_tree), black_box(&new_tree)));
    //     }
    //     println!("Diffing 10k times took: {:?}", start.elapsed());
    // }
    // ```

    #[test]
    fn test_diff_text_nodes() {
        let old = text("Hello");
        let new = text("World");

        let patches = diff(&old, &new);
        assert_eq!(patches.len(), 1);
        assert_eq!(patches[0], Patch::UpdateText("World"));

        // No change
        let same = text("Hello");
        assert!(diff(&old, &same).is_empty());
    }

    #[test]
    fn test_diff_different_types() {
        let old = text("Hello");
        let new = VNode::Element(el("div"));

        let patches = diff(&old, &new);
        assert_eq!(patches.len(), 1);
        assert_eq!(patches[0], Patch::Replace(&new));
    }

    #[test]
    fn test_diff_attributes() {
        let old = VNode::Element(
            el("div")
                .with_attr("id", "app")
                .with_attr("class", "container"),
        );
        let new = VNode::Element(
            el("div")
                .with_attr("id", "app") // Unchanged
                .with_attr("class", "wrapper") // Updated
                .with_attr("data-test", "true"), // Added
                                                 // "class" removed (implicitly, if it wasn't here, but we updated it)
        );

        let old_rem = VNode::Element(el("div").with_attr("removed", "true"));
        let new_rem = VNode::Element(el("div"));

        let patches = diff(&old_rem, &new_rem);
        assert!(patches.contains(&Patch::RemoveAttribute("removed")));

        let patches = diff(&old, &new);
        assert!(patches.contains(&Patch::SetAttribute("class", "wrapper")));
        assert!(patches.contains(&Patch::SetAttribute("data-test", "true")));
    }

    #[test]
    fn test_diff_children() {
        let old = VNode::Element(
            el("ul")
                .with_child(text("Item 1"))
                .with_child(text("Item 2")),
        );
        let new = VNode::Element(
            el("ul")
                .with_child(text("Item 1 (Updated)"))
                .with_child(text("Item 2"))
                .with_child(text("Item 3")),
        );

        let patches = diff(&old, &new);

        // 1 text update, 1 append
        assert_eq!(patches.len(), 2);
        assert!(patches.contains(&Patch::UpdateText("Item 1 (Updated)")));
        assert!(patches.contains(&Patch::AppendChild(&new.to_element().children[2])));
    }

    // Helper for tests
    impl VNode {
        fn to_element(&self) -> &VElement {
            match self {
                VNode::Element(e) => e,
                _ => panic!("Not an element"),
            }
        }
    }
}
