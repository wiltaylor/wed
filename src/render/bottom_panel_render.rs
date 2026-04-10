//! Renderer for the bottom panel — a horizontal area at the bottom of
//! the editor with a tab strip and a single active pane below it.

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::layout::BottomPanel;

pub fn render(
    frame: &mut Frame<'_>,
    panel: &BottomPanel,
    area: Rect,
    focused: bool,
) -> Vec<Rect> {
    if !panel.open || area.width == 0 || area.height == 0 {
        return Vec::new();
    }
    let border_color = if focused { Color::White } else { Color::DarkGray };
    let block = Block::default()
        .borders(Borders::ALL)
        .style(Style::default().bg(Color::Black).fg(Color::Gray))
        .border_style(Style::default().fg(border_color));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height == 0 || inner.width == 0 {
        return Vec::new();
    }

    // Tab strip on row 0 of inner.
    let tab_rect = Rect {
        x: inner.x,
        y: inner.y,
        width: inner.width,
        height: 1,
    };
    let mut spans: Vec<Span> = Vec::new();
    let mut tab_rects: Vec<Rect> = Vec::with_capacity(panel.panes.len());
    let mut x = tab_rect.x;
    let max_x = tab_rect.x + tab_rect.width;
    for (i, pane) in panel.panes.iter().enumerate() {
        let title = pane.dynamic_title().unwrap_or_else(|| pane.title().to_string());
        let label = format!(" {} ", title);
        let w = label.chars().count() as u16;
        let r = Rect {
            x,
            y: tab_rect.y,
            width: w.min(max_x.saturating_sub(x)),
            height: 1,
        };
        tab_rects.push(r);
        x = x.saturating_add(w + 1); // +1 for separator space
        let style = if i == panel.active {
            Style::default()
                .bg(Color::White)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Gray)
        };
        spans.push(Span::styled(label, style));
        spans.push(Span::raw(" "));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), tab_rect);

    // Active pane occupies the rows below the tab strip.
    if inner.height < 2 {
        return tab_rects;
    }
    let pane_rect = Rect {
        x: inner.x,
        y: inner.y + 1,
        width: inner.width,
        height: inner.height - 1,
    };
    if let Some(pane) = panel.panes.get(panel.active) {
        pane.render_focused(frame, pane_rect, focused);
    }
    tab_rects
}
