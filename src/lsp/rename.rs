//! UI state for the rename prompt.

use lsp_types::{Position, Uri};

#[derive(Debug, Clone)]
pub struct RenamePrompt {
    pub uri: Uri,
    pub position: Position,
    pub original: String,
    pub input: String,
    pub cursor: usize,
}

impl RenamePrompt {
    pub fn new(uri: Uri, position: Position, original: impl Into<String>) -> Self {
        let original = original.into();
        let cursor = original.len();
        Self {
            input: original.clone(),
            cursor,
            original,
            uri,
            position,
        }
    }

    pub fn insert_char(&mut self, c: char) {
        self.input.insert(self.cursor, c);
        self.cursor += c.len_utf8();
    }

    pub fn backspace(&mut self) {
        if self.cursor == 0 {
            return;
        }
        let new_cursor = self.input[..self.cursor]
            .char_indices()
            .last()
            .map(|(i, _)| i)
            .unwrap_or(0);
        self.input.replace_range(new_cursor..self.cursor, "");
        self.cursor = new_cursor;
    }

    pub fn move_left(&mut self) {
        if self.cursor == 0 {
            return;
        }
        self.cursor = self.input[..self.cursor]
            .char_indices()
            .last()
            .map(|(i, _)| i)
            .unwrap_or(0);
    }
    pub fn move_right(&mut self) {
        if let Some((_, c)) = self.input[self.cursor..].char_indices().next() {
            self.cursor += c.len_utf8();
        }
    }
}
