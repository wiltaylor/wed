use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use crate::app::App;

/// Render a popup widget (e.g. completion / hover) above the editor area.
/// Currently a no-op stub; once `App` exposes popup state this will draw it.
pub fn render(_frame: &mut Frame<'_>, _app: &App, _area: Rect) {
    // TODO: pull popup state from App when Agent owning ui-state wires it.
}

/// Helper to draw a bordered popup with given text inside `area`.
pub fn draw_popup(frame: &mut Frame<'_>, area: Rect, title: &str, text: &str) {
    frame.render_widget(Clear, area);
    let block = Block::default()
        .title(title.to_string())
        .borders(Borders::ALL)
        .style(Style::default().bg(Color::Black).fg(Color::White));
    let para = Paragraph::new(text.to_string()).block(block);
    frame.render_widget(para, area);
}
