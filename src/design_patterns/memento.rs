// Suppress pedantic and nursery lints for design pattern examples.
#![allow(clippy::pedantic, clippy::nursery, unused)]
//! # Memento Pattern
//!
//! Replaces: **Memento Pattern** (OOP)
//!
//! Real Rust usage: `Clone`, `serde` (for snapshotting state), game engine save systems.
//!
//! ## Why this pattern exists in Rust
//! The Memento pattern captures and externalizes an object's internal state so that the object can be restored
//! to this state later, without violating encapsulation.
//!
//! In traditional OOP, this involves creating a specific `Memento` class that only the `Originator` can access.
//! In Rust, because of structural typing and traits like `Clone` and `Serialize`, the pattern becomes trivial.
//! Instead of complex nested classes, we often just clone the state struct entirely or serialize it.
//!
//! ## Architecture
//!
//! **Approach 1: `Clone`-based Memento**
//! ```text
//! [ Editor (Originator) ] --(clones state)--> [ EditorState (Memento) ]
//! ```
//! The `EditorState` is a distinct struct that implements `Clone`. The `History` (Caretaker) stores a `Vec<EditorState>`.
//!
//! **Approach 2: `Serialize`/`Deserialize`-based Memento**
//! Similar to Approach 1, but the state is converted to a generic byte array or JSON string. Useful when
//! the state is too large to keep in memory (e.g., saving to disk).
//!
//! **Invariants:**
//! - The Memento (state snapshot) should be completely disconnected from the Originator (no references/lifetimes).
//! - The Caretaker (history manager) must not mutate the Memento.
//!
//! ## When to use
//! - Implementing Undo/Redo functionality in applications.
//! - Creating save states for games or checkpoints for long-running processes.
//!
//! ## Anti-patterns
//! - Storing references (`&'a State`) in the Memento. When the Originator mutates, the references become invalid
//!   and the borrow checker will correctly stop you. Mementos must own their data.
//!
//! ## Footer
//! The GoF Memento pattern was designed to work around the lack of simple value-copy semantics for complex objects
//! in languages like Java. In Rust, deriving `Clone` on your state struct gives you a perfectly encapsulated,
//! ownership-safe Memento out of the box.

// ============================================================================
// Approach 1: Clone-based Memento
// ============================================================================

/// The Memento. It holds a snapshot of the Originator's state.
/// COMPILE-TIME WIN: By deriving `Clone`, we get a deep copy automatically.
/// By keeping fields private to the module, we enforce encapsulation.
#[derive(Clone, Debug, PartialEq)]
pub struct EditorState {
    content: String,
    cursor_position: usize,
}

/// The Originator. It produces and consumes Mementos.
pub struct Editor {
    content: String,
    cursor_position: usize,
}

impl Editor {
    #[must_use]
    pub fn new() -> Self {
        Self {
            content: String::new(),
            cursor_position: 0,
        }
    }

    pub fn type_text(&mut self, text: &str) {
        self.content.insert_str(self.cursor_position, text);
        self.cursor_position += text.len();
    }

    pub fn delete_char(&mut self) {
        // `cursor_position` is a byte offset, so we must step back to the
        // previous char boundary rather than assuming one byte per char.
        // Otherwise `String::remove` panics on multibyte chars (e.g. "é").
        if self.cursor_position > 0 {
            if let Some((idx, ch)) = self.content[..self.cursor_position]
                .char_indices()
                .next_back()
            {
                self.content.remove(idx);
                self.cursor_position -= ch.len_utf8();
            }
        }
    }

    /// Creates a Memento.
    // OWNERSHIP INSIGHT: We allocate entirely new memory for the state snapshot.
    // There are no lifetime dependencies between the Editor and the EditorState.
    #[must_use]
    pub fn save(&self) -> EditorState {
        EditorState {
            content: self.content.clone(),
            cursor_position: self.cursor_position,
        }
    }

    /// Restores from a Memento.
    pub fn restore(&mut self, state: EditorState) {
        self.content = state.content;
        self.cursor_position = state.cursor_position;
    }

    #[must_use]
    pub fn content(&self) -> &str {
        &self.content
    }
}

impl Default for Editor {
    fn default() -> Self {
        Self::new()
    }
}

/// The Caretaker. It manages the history of Mementos without modifying them.
#[derive(Default)]
pub struct History {
    undo_stack: Vec<EditorState>,
    redo_stack: Vec<EditorState>,
}

impl History {
    /// Saves the current state of the editor.
    pub fn push_state(&mut self, editor: &Editor) {
        self.undo_stack.push(editor.save());
        // Whenever a new action occurs, the redo history is invalidated.
        self.redo_stack.clear();
    }

    /// Undoes the last action, restoring the editor to the previous state.
    pub fn undo(&mut self, editor: &mut Editor) {
        if let Some(previous_state) = self.undo_stack.pop() {
            // Save current state to redo stack before applying undo
            self.redo_stack.push(editor.save());
            // Restore the previous state
            editor.restore(previous_state);
        }
    }

    /// Redoes a previously undone action.
    pub fn redo(&mut self, editor: &mut Editor) {
        if let Some(next_state) = self.redo_stack.pop() {
            // Save current state to undo stack before applying redo
            self.undo_stack.push(editor.save());
            editor.restore(next_state);
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memento_undo_redo() {
        let mut editor = Editor::new();
        let mut history = History::default();

        // Initial state
        history.push_state(&editor);

        // Type "Hello"
        editor.type_text("Hello");
        history.push_state(&editor);

        // Type " World"
        editor.type_text(" World");
        assert_eq!(editor.content(), "Hello World");

        // Undo " World"
        history.undo(&mut editor);
        assert_eq!(editor.content(), "Hello");

        // Undo "Hello"
        history.undo(&mut editor);
        assert_eq!(editor.content(), "");

        // Redo "Hello"
        history.redo(&mut editor);
        assert_eq!(editor.content(), "Hello");

        // Type something new (invalidates redo)
        history.push_state(&editor);
        editor.type_text(" Rust");
        assert_eq!(editor.content(), "Hello Rust");

        // Try to redo (should do nothing since it was cleared)
        history.redo(&mut editor);
        assert_eq!(editor.content(), "Hello Rust");
    }

    #[test]
    fn test_delete_char_non_ascii_no_panic() {
        // Regression: cursor_position is a byte offset; deleting a multibyte
        // char used to panic ("not a char boundary").
        let mut editor = Editor::new();
        editor.type_text("é");
        editor.delete_char();
        assert_eq!(editor.content(), "");
        assert_eq!(editor.cursor_position, 0);
    }

    #[test]
    fn test_delete_char_mixed_ascii_multibyte() {
        let mut editor = Editor::new();
        editor.type_text("aé");
        editor.delete_char();
        assert_eq!(editor.content(), "a");
        editor.delete_char();
        assert_eq!(editor.content(), "");
    }

    #[test]
    fn test_delete_char_ascii_round_trip() {
        let mut editor = Editor::new();
        editor.type_text("abc");
        editor.delete_char();
        assert_eq!(editor.content(), "ab");
        editor.delete_char();
        editor.delete_char();
        assert_eq!(editor.content(), "");
        // Deleting on empty is a no-op (no underflow / no panic).
        editor.delete_char();
        assert_eq!(editor.content(), "");
    }
}
