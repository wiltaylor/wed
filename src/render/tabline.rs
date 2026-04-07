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
    app.last_tab_close_rects.clear();
    if app.layout.tabs.is_empty() {
        spans.push(Span::styled(
            " [no tabs] ",
            Style::default().fg(Color::DarkGray),
        ));
    }
    let mut x = area.x;
    for (i, tab) in app.layout.tabs.iter().enumerate() {
        let name = if tab.name.is_empty() {
            "[scratch]".into()
        } else {
            tab.name.clone()
        };
        // Layout: " {name} x "
        let left = format!(" {name} ");
        let close_glyph = "x";
        let trailing = " ";
        let total_w =
            (left.chars().count() + close_glyph.chars().count() + trailing.chars().count()) as u16;
        let tab_rect = Rect {
            x,
            y: area.y,
            width: total_w.min((area.x + area.width).saturating_sub(x)),
            height: 1,
        };
        app.last_tab_rects.push(tab_rect);
        let close_rect = Rect {
            x: x + left.chars().count() as u16,
            y: area.y,
            width: 1,
            height: 1,
        };
        app.last_tab_close_rects.push(close_rect);
        x = x.saturating_add(total_w + 1); // +1 for the separator space between tabs

        let (bg, fg) = if i == active {
            (Color::White, Color::Black)
        } else {
            (Color::Black, Color::Gray)
        };
        let base = Style::default().bg(bg).fg(fg);
        let active_mod = if i == active {
            base.add_modifier(Modifier::BOLD)
        } else {
            base
        };
        spans.push(Span::styled(left, active_mod));
        spans.push(Span::styled(
            close_glyph.to_string(),
            Style::default().bg(bg).fg(Color::Red),
        ));
        spans.push(Span::styled(trailing.to_string(), base));
        spans.push(Span::raw(" "));
    }
    let para = Paragraph::new(Line::from(spans)).style(Style::default().bg(Color::Black));
    frame.render_widget(para, area);
}
