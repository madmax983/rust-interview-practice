// Suppress pedantic and nursery lints for design pattern examples.
#![allow(clippy::pedantic, clippy::nursery, unused)]
//! # Command / Undo Pattern with Owned Values
//!
//! Replaces: **Command Pattern** (OOP), **Action/Redux Pattern** (Flux)
//!
//! Real Rust usage: `xi-editor` (Rope science), `druid` (Undo history), `bevy` (ECS Commands)
//!
//! ## Why this pattern exists in Rust
//! In garbage collected languages, the Command pattern often relies on shared mutable state. In Rust, we prefer commands that
//! *take ownership* of the state required to perform (and undo) the action. This ensures that a command in the undo stack
//! contains everything it needs to reverse itself, without worrying about dangling references or concurrent modification.
//!
//! ## Architecture
//!
//! ```text
//! [ Invoker (Editor) ]
//!       |
//!       +--> owns --> [ History (Vec<Box<dyn Command>>) ]
//!                            |
//!                            +--> [ InsertCommand { pos, text } ]
//!                            |
//!                            +--> [ DeleteCommand { pos, deleted_text } ]
//! ```
//!
//! **Invariants:**
//! - Each command object encapsulates the logic to `execute` and `undo`.
//! - `undo` restores the state exactly to what it was before `execute`.
//! - The history owns the command objects.
//!
//! ## When to use
//! - When implementing undo/redo functionality.
//! - When you need to queue operations for later execution.
//! - When creating a transactional system where operations can be rolled back.

// ============================================================================
// The Command Interface
// ============================================================================

/// A trait representing an executable and reversible action.
///
/// **OWNERSHIP INSIGHT:**
/// We use `&mut Target` to allow the command to mutate the state of the application.
/// The command itself (`self`) is immutable during execution in this design,
/// but could be mutable if it needed to update its own internal state.
pub trait Command {
    /// Applies the command to the target.
    fn execute(&mut self, target: &mut String);

    /// Reverses the command on the target.
    fn undo(&mut self, target: &mut String);
}

// ============================================================================
// Concrete Commands
// ============================================================================

/// Command to insert text at a specific position.
pub struct InsertText {
    text: String,
    position: usize,
}

impl InsertText {
    pub fn new(text: impl Into<String>, position: usize) -> Self {
        InsertText {
            text: text.into(),
            position,
        }
    }
}

impl Command for InsertText {
    fn execute(&mut self, target: &mut String) {
        if self.position <= target.len() {
            target.insert_str(self.position, &self.text);
        } else {
            // In a real app, handle error or clamp. Here we append.
            // Update position so undo/redo works correctly.
            self.position = target.len();
            target.push_str(&self.text);
        }
    }

    fn undo(&mut self, target: &mut String) {
        // To undo an insertion, we delete the inserted text.
        // We know the position and the length.
        let start = self.position;
        let end = start + self.text.len();

        if end <= target.len() {
            target.replace_range(start..end, "");
        }
    }
}

/// Command to delete text at a specific position.
///
/// **OWNERSHIP INSIGHT:**
/// This command *captures* the deleted text during execution so it can restore it during undo.
/// This makes the command self-contained.
pub struct DeleteText {
    range: std::ops::Range<usize>,
    // State to restore on undo
    deleted_text: Option<String>,
}

impl DeleteText {
    pub fn new(start: usize, end: usize) -> Self {
        DeleteText {
            range: start..end,
            deleted_text: None,
        }
    }
}

impl Command for DeleteText {
    fn execute(&mut self, target: &mut String) {
        // Clamp range
        let start = self.range.start.min(target.len());
        let end = self.range.end.min(target.len());

        if start < end {
            // Capture the text before deleting
            self.deleted_text = Some(target[start..end].to_string());
            target.replace_range(start..end, "");
        }
    }

    fn undo(&mut self, target: &mut String) {
        if let Some(text) = &self.deleted_text {
            let start = self.range.start.min(target.len());
            target.insert_str(start, text);
        }
    }
}

// ============================================================================
// The Invoker (Editor)
// ============================================================================

pub struct TextEditor {
    buffer: String,
    // History owns the command objects as trait objects.
    history: Vec<Box<dyn Command>>,
    // Points to the position *after* the last executed command.
    history_index: usize,
}

impl Default for TextEditor {
    fn default() -> Self {
        Self::new()
    }
}

impl TextEditor {
    pub fn new() -> Self {
        TextEditor {
            buffer: String::new(),
            history: Vec::new(),
            history_index: 0,
        }
    }

    pub fn get_text(&self) -> &str {
        &self.buffer
    }

    /// Executes a new command and adds it to history.
    pub fn execute(&mut self, mut command: Box<dyn Command>) {
        // If we are in the middle of the history (after undo),
        // we must discard the "future" history before adding a new command.
        if self.history_index < self.history.len() {
            self.history.truncate(self.history_index);
        }

        command.execute(&mut self.buffer);
        self.history.push(command);
        self.history_index += 1;
    }

    /// Undoes the last executed command.
    pub fn undo(&mut self) {
        if self.history_index > 0 {
            self.history_index -= 1;
            // Get mutable access to the command to call undo
            if let Some(cmd) = self.history.get_mut(self.history_index) {
                cmd.undo(&mut self.buffer);
            }
        }
    }

    /// Redoes the previously undone command.
    pub fn redo(&mut self) {
        if self.history_index < self.history.len() {
            if let Some(cmd) = self.history.get_mut(self.history_index) {
                cmd.execute(&mut self.buffer);
            }
            self.history_index += 1;
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
    fn test_insert_undo_redo() {
        let mut editor = TextEditor::new();

        // 1. Execute
        editor.execute(Box::new(InsertText::new("Hello", 0)));
        assert_eq!(editor.get_text(), "Hello");

        editor.execute(Box::new(InsertText::new(" World", 5)));
        assert_eq!(editor.get_text(), "Hello World");

        // 2. Undo " World"
        editor.undo();
        assert_eq!(editor.get_text(), "Hello");

        // 3. Undo "Hello"
        editor.undo();
        assert_eq!(editor.get_text(), "");

        // 4. Redo "Hello"
        editor.redo();
        assert_eq!(editor.get_text(), "Hello");

        // 5. Redo " World"
        editor.redo();
        assert_eq!(editor.get_text(), "Hello World");
    }

    #[test]
    fn test_delete_undo() {
        let mut editor = TextEditor::new();
        editor.execute(Box::new(InsertText::new("Rust is awesome", 0)));

        // Delete "awesome" (index 8 to 15)
        editor.execute(Box::new(DeleteText::new(8, 15)));
        assert_eq!(editor.get_text(), "Rust is ");

        // Undo delete
        editor.undo();
        assert_eq!(editor.get_text(), "Rust is awesome");
    }

    #[test]
    fn test_overwrite_history() {
        let mut editor = TextEditor::new();
        editor.execute(Box::new(InsertText::new("A", 0)));
        editor.execute(Box::new(InsertText::new("B", 1))); // "AB"

        editor.undo(); // "A", index at 1 -> 0? No, index was 2, now 1. History has 2 items.
        // Wait: history_index points to *next* slot.
        // After A: index 1.
        // After B: index 2.
        // Undo: index becomes 1. Undo cmd at index 1 (B). Text is "A".

        assert_eq!(editor.get_text(), "A");

        // New branch
        editor.execute(Box::new(InsertText::new("C", 1))); // "AC"

        // Redo should not be possible (history truncated)
        editor.redo();
        assert_eq!(editor.get_text(), "AC");

        // Check history only has A and C
        editor.undo(); // Undo C
        assert_eq!(editor.get_text(), "A");

        editor.undo(); // Undo A
        assert_eq!(editor.get_text(), "");
    }
}
