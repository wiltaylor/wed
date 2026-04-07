use crate::layout::Pane;
use async_trait::async_trait;
use std::path::PathBuf;

/// One diagnostic row, decoupled from any particular LSP type so we don't
/// fight the integration agent for the shape of `lsp::DiagnosticStore`.
#[derive(Debug, Clone)]
pub struct DiagnosticEntry {
    pub path: PathBuf,
    pub line: usize,
    pub col: usize,
    pub severity: u8,
    pub message: String,
}

#[derive(Default)]
pub struct DiagnosticsPane {
    pub entries: Vec<DiagnosticEntry>,
    pub selected: usize,
}

impl DiagnosticsPane {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn set_entries(&mut self, entries: Vec<DiagnosticEntry>) {
        self.entries = entries;
        if self.selected >= self.entries.len() {
            self.selected = 0;
        }
    }
    pub fn move_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }
    pub fn move_down(&mut self) {
        if self.selected + 1 < self.entries.len() {
            self.selected += 1;
        }
    }
}

#[async_trait]
impl Pane for DiagnosticsPane {
    fn name(&self) -> &str {
        "diagnostics"
    }
}
