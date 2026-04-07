//! Floating context menu (e.g. right-click in the file browser).

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

#[derive(Debug, Clone)]
pub struct MenuItem {
    pub label: String,
    pub command: String,
    pub args: Vec<String>,
}

#[derive(Debug)]
pub struct ContextMenu {
    pub items: Vec<MenuItem>,
    pub selected: usize,
    pub anchor_x: u16,
    pub anchor_y: u16,
}

impl ContextMenu {
    pub fn new(items: Vec<MenuItem>, anchor_x: u16, anchor_y: u16) -> Self {
        Self {
            items,
            selected: 0,
            anchor_x,
            anchor_y,
        }
    }

    pub fn move_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }
    pub fn move_down(&mut self) {
        if self.selected + 1 < self.items.len() {
            self.selected += 1;
        }
    }

    pub fn rect(&self, screen: Rect) -> Rect {
        let inner_w = self
            .items
            .iter()
            .map(|i| i.label.chars().count())
            .max()
            .unwrap_or(8) as u16;
        let w = (inner_w + 4).min(screen.width.saturating_sub(2)).max(10);
        let h = (self.items.len() as u16 + 2).min(screen.height.saturating_sub(2)).max(3);
        let mut x = self.anchor_x;
        let mut y = self.anchor_y;
        if x + w > screen.x + screen.width {
            x = (screen.x + screen.width).saturating_sub(w);
        }
        if y + h > screen.y + screen.height {
            y = (screen.y + screen.height).saturating_sub(h);
        }
        Rect {
            x,
            y,
            width: w,
            height: h,
        }
    }

    pub fn render(&self, frame: &mut Frame<'_>, area: Rect) {
        let lines: Vec<Line> = self
            .items
            .iter()
            .enumerate()
            .map(|(i, it)| {
                let mut style = Style::default().fg(Color::Gray);
                if i == self.selected {
                    style = style
                        .bg(Color::DarkGray)
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD);
                }
                Line::from(Span::styled(format!(" {} ", it.label), style))
            })
            .collect();
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray))
            .style(Style::default().bg(Color::Black));
        frame.render_widget(Clear, area);
        frame.render_widget(Paragraph::new(lines).block(block), area);
    }
}
