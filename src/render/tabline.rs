use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::app::App;

pub fn render(frame: &mut Frame<'_>, app: &mut App, area: Rect) {
    let active = app.layout.active_tab;
    let mut spans: Vec<Span> = Vec::new();
    app.last_tab_rects.clear();
    if app.layout.tabs.is_empty() {
        spans.push(Span::styled(
            " [no tabs] ",
            Style::default().fg(Color::DarkGray),
        ));
    }
    let mut x = area.x;
    for (i, tab) in app.layout.tabs.iter().enumerate() {
        let label = format!(
            " {} ",
            if tab.name.is_empty() {
                "[scratch]".into()
            } else {
                tab.name.clone()
            }
        );
        let label_w = label.chars().count() as u16;
        let rect = Rect {
            x,
            y: area.y,
            width: label_w.min(area.x + area.width - x),
            height: 1,
        };
        app.last_tab_rects.push(rect);
        x = x.saturating_add(label_w + 1); // +1 for the separator space
        let style = if i == active {
            Style::default()
                .fg(Color::Black)
                .bg(Color::White)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Gray).bg(Color::Black)
        };
        spans.push(Span::styled(label, style));
        spans.push(Span::raw(" "));
    }
    let para = Paragraph::new(Line::from(spans)).style(Style::default().bg(Color::Black));
    frame.render_widget(para, area);
}
