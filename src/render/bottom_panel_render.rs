//! Renderer for the bottom panel — a horizontal area at the bottom of
//! the editor with a tab strip and a single active pane below it.

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::layout::BottomPanel;

pub fn render(frame: &mut Frame<'_>, panel: &BottomPanel, area: Rect, focused: bool) {
    if !panel.open || area.width == 0 || area.height == 0 {
        return;
    }
    let border_color = if focused { Color::White } else { Color::DarkGray };
    let block = Block::default()
        .borders(Borders::ALL)
        .style(Style::default().bg(Color::Black).fg(Color::Gray))
        .border_style(Style::default().fg(border_color));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height == 0 || inner.width == 0 {
        return;
    }

    // Tab strip on row 0 of inner.
    let tab_rect = Rect {
        x: inner.x,
        y: inner.y,
        width: inner.width,
        height: 1,
    };
    let mut spans: Vec<Span> = Vec::new();
    for (i, pane) in panel.panes.iter().enumerate() {
        let label = format!(" {} ", pane.title());
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
        return;
    }
    let pane_rect = Rect {
        x: inner.x,
        y: inner.y + 1,
        width: inner.width,
        height: inner.height - 1,
    };
    if let Some(pane) = panel.panes.get(panel.active) {
        pane.render(frame, pane_rect);
    }
}
