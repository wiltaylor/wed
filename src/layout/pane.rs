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
    /// Override to return a dynamic title string (e.g. with status indicators).
    /// When `Some`, used in place of `title()` for the tab label.
    fn dynamic_title(&self) -> Option<String> {
        None
    }
    fn icon(&self) -> &str {
        ""
    }

    fn render(&self, _frame: &mut Frame<'_>, _area: Rect) {}
    /// Render with knowledge of whether the bottom panel currently has
    /// keyboard focus. Default delegates to `render`.
    fn render_focused(&self, frame: &mut Frame<'_>, area: Rect, _focused: bool) {
        self.render(frame, area);
    }
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
    /// Push a git status snapshot (absolute path → status). Default no-op.
    fn refresh_git_status(
        &mut self,
        _map: &std::collections::HashMap<std::path::PathBuf, crate::git::FileGitStatus>,
    ) {
    }
    /// Push the current list of staged files (paths relative to repo root)
    /// along with whether each is a staged deletion. Default no-op.
    fn refresh_staged(&mut self, _staged: &[(String, bool)]) {}
    /// If the pane wants to perform a git commit, return the message and clear it.
    fn take_commit_request(&mut self) -> Option<String> {
        None
    }
    /// Return the filesystem path the pane displays at row `row`, if any.
    /// Used by mouse handling to resolve a right-click target.
    fn path_at_row(&self, _row: usize) -> Option<std::path::PathBuf> {
        None
    }
    /// How many rows the pane currently displays. Used for mouse hit-testing.
    fn row_count(&self) -> usize {
        0
    }
    /// Move the pane's selection to a specific row index.
    fn select_row(&mut self, _row: usize) {}
    /// Activate the currently selected row (e.g. expand directory or open file).
    fn activate_selected(&mut self) {}

    /// Optional downcast hook so the host can access concrete pane state.
    /// Default returns `None`.
    fn as_any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        None
    }

    async fn on_event(&mut self) {}
}
