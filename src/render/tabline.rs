use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::app::App;

pub fn render(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let active = app.layout.active_tab;
    let mut spans: Vec<Span> = Vec::new();
    if app.layout.tabs.is_empty() {
        spans.push(Span::styled(
            " [no tabs] ",
            Style::default().fg(Color::DarkGray),
        ));
    }
    for (i, tab) in app.layout.tabs.iter().enumerate() {
        let label = format!(" {} ", if tab.name.is_empty() { "[scratch]".into() } else { tab.name.clone() });
        let style = if i == active {
            Style::default().fg(Color::Black).bg(Color::White).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Gray).bg(Color::Black)
        };
        spans.push(Span::styled(label, style));
        spans.push(Span::raw(" "));
    }
    let para = Paragraph::new(Line::from(spans)).style(Style::default().bg(Color::Black));
    frame.render_widget(para, area);
}
