//! UI state for the code-actions menu.

use lsp_types::CodeActionOrCommand;

#[derive(Default, Debug, Clone)]
pub struct CodeActionsMenu {
    pub actions: Vec<CodeActionOrCommand>,
    pub selected: usize,
}

impl CodeActionsMenu {
    pub fn new(actions: Vec<CodeActionOrCommand>) -> Self {
        Self {
            actions,
            selected: 0,
        }
    }

    pub fn selected_action(&self) -> Option<&CodeActionOrCommand> {
        self.actions.get(self.selected)
    }

    pub fn next(&mut self) {
        if self.actions.is_empty() {
            return;
        }
        self.selected = (self.selected + 1) % self.actions.len();
    }

    pub fn prev(&mut self) {
        if self.actions.is_empty() {
            return;
        }
        self.selected = if self.selected == 0 {
            self.actions.len() - 1
        } else {
            self.selected - 1
        };
    }
}
