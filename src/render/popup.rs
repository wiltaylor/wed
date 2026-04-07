use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::app::App;

pub fn render(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let Some(picker) = app.picker.as_ref() else {
        return;
    };
    if area.width < 10 || area.height < 5 {
        return;
    }
    let w = area.width.saturating_sub(4).min(80);
    let h = area.height.saturating_sub(4).min(20);
    let x = area.x + (area.width - w) / 2;
    let y = area.y + (area.height - h) / 2;
    let popup_area = Rect {
        x,
        y,
        width: w,
        height: h,
    };
    frame.render_widget(Clear, popup_area);
    let block = Block::default()
        .title(" Files ")
        .borders(Borders::ALL)
        .style(Style::default().bg(Color::Black).fg(Color::White));
    let inner = block.inner(popup_area);
    frame.render_widget(block, popup_area);
    if inner.height < 2 {
        return;
    }
    // Query line at top
    let query_line = Line::from(vec![
        Span::styled("> ", Style::default().fg(Color::Yellow)),
        Span::raw(app.picker_query.clone()),
    ]);
    let query_rect = Rect {
        x: inner.x,
        y: inner.y,
        width: inner.width,
        height: 1,
    };
    frame.render_widget(Paragraph::new(query_line), query_rect);

    // Match list below
    let list_rect = Rect {
        x: inner.x,
        y: inner.y + 1,
        width: inner.width,
        height: inner.height - 1,
    };
    let cap = list_rect.height as usize;
    let start = picker.selected.saturating_sub(cap.saturating_sub(1));
    let lines: Vec<Line> = picker
        .matches
        .iter()
        .enumerate()
        .skip(start)
        .take(cap)
        .map(|(i, (idx, _))| {
            let label = picker
                .items
                .get(*idx)
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_default();
            let mut style = Style::default().fg(Color::Gray);
            if i == picker.selected {
                style = style.bg(Color::DarkGray).add_modifier(Modifier::BOLD);
            }
            Line::from(Span::styled(label, style))
        })
        .collect();
    frame.render_widget(Paragraph::new(lines), list_rect);
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
