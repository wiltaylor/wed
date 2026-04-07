use async_trait::async_trait;
use crossterm::event::{KeyEvent, MouseEvent};
use ratatui::layout::Rect;
use ratatui::Frame;

use crate::app::AppEvent;

#[async_trait]
pub trait Pane: Send + Sync {
    fn name(&self) -> &str;
    fn title(&self) -> &str {
        self.name()
    }
    fn icon(&self) -> &str {
        ""
    }

    fn render(&self, _frame: &mut Frame<'_>, _area: Rect) {}
    fn handle_key(&mut self, _key: KeyEvent) {}
    fn handle_mouse(&mut self, _mouse: MouseEvent) {}
    fn update(&mut self, _event: &AppEvent) {}
    /// If the pane has produced a path it wants the host to open
    /// (e.g. file browser activation), return and clear it.
    fn take_opened_path(&mut self) -> Option<std::path::PathBuf> {
        None
    }
    /// If the pane has produced a `(row, col)` jump target it wants the
    /// host to apply to the active buffer's cursor, return and clear it.
    fn take_jump_target(&mut self) -> Option<(usize, usize)> {
        None
    }
    /// Hook called by the bottom panel each frame to push the current
    /// buffer's LSP diagnostics into a problems-style pane. Default no-op.
    fn refresh_diagnostics(&mut self, _diags: &[lsp_types::Diagnostic]) {}
    /// How many rows the pane currently displays. Used for mouse hit-testing.
    fn row_count(&self) -> usize {
        0
    }
    /// Move the pane's selection to a specific row index.
    fn select_row(&mut self, _row: usize) {}
    /// Activate the currently selected row (e.g. expand directory or open file).
    fn activate_selected(&mut self) {}

    async fn on_event(&mut self) {}
}
