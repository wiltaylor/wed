pub mod command_line_ui;
pub mod editor_view;
pub mod highlight_render;
pub mod popup;
pub mod sidebar_render;
pub mod statusline;
pub mod tabline;

use ratatui::Frame;

use crate::app::App;

pub fn render(_frame: &mut Frame<'_>, _app: &App) {
    // top-level render entry (stub)
}
