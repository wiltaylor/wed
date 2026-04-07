//! UI state for the rename prompt.

use lsp_types::{Position, Uri};

#[derive(Debug, Clone)]
pub struct RenamePrompt {
    pub uri: Uri,
    pub position: Position,
    pub original: String,
    pub new_name: String,
}

impl RenamePrompt {
    pub fn new(uri: Uri, position: Position, original: impl Into<String>) -> Self {
        let original = original.into();
        Self {
            new_name: original.clone(),
            original,
            uri,
            position,
        }
    }

    pub fn push(&mut self, c: char) {
        self.new_name.push(c);
    }

    pub fn pop(&mut self) {
        self.new_name.pop();
    }
}
