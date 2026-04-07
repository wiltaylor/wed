use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, Borders};

use crate::layout::Sidebar;

pub fn render(frame: &mut Frame<'_>, sidebar: &Sidebar, area: Rect, title: &str) {
    if !sidebar.open || area.width == 0 {
        return;
    }
    let block = Block::default()
        .title(title.to_string())
        .borders(Borders::ALL)
        .style(Style::default().bg(Color::Black).fg(Color::Gray));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if let Some(pane) = sidebar.panes.get(sidebar.active) {
        pane.render(frame, inner);
    }
}
