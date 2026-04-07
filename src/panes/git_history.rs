//! Tab pane: list commits touching a single file.

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;
use std::path::PathBuf;

use crate::git::CommitInfo;

pub struct GitHistoryPane {
    pub path: PathBuf,
    pub commits: Vec<CommitInfo>,
    pub selected: usize,
}

impl GitHistoryPane {
    pub fn new(path: PathBuf, commits: Vec<CommitInfo>) -> Self {
        Self {
            path,
            commits,
            selected: 0,
        }
    }

    pub fn move_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }
    pub fn move_down(&mut self) {
        if self.selected + 1 < self.commits.len() {
            self.selected += 1;
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => self.move_up(),
            KeyCode::Down | KeyCode::Char('j') => self.move_down(),
            _ => {}
        }
    }

    pub fn render(&self, frame: &mut Frame<'_>, area: Rect) {
        let title = format!(
            " History: {} ",
            self.path
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default()
        );
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray))
            .title(title);
        let inner = block.inner(area);
        frame.render_widget(block, area);
        if self.commits.is_empty() {
            let p = Paragraph::new(Line::from(Span::styled(
                "no commits touch this file",
                Style::default().fg(Color::DarkGray),
            )));
            frame.render_widget(p, inner);
            return;
        }
        let height = inner.height as usize;
        let start = self.selected.saturating_sub(height.saturating_sub(1));
        let lines: Vec<Line> = self
            .commits
            .iter()
            .enumerate()
            .skip(start)
            .take(height)
            .map(|(i, c)| {
                let prefix = format!("{}  {}  {:<16}  ", c.short, c.time, truncate(&c.author, 16));
                let mut spans = vec![
                    Span::styled(prefix, Style::default().fg(Color::Yellow)),
                    Span::raw(c.summary.clone()),
                ];
                if i == self.selected {
                    let st = Style::default()
                        .bg(Color::DarkGray)
                        .add_modifier(Modifier::BOLD);
                    for s in &mut spans {
                        s.style = s.style.patch(st);
                    }
                }
                Line::from(spans)
            })
            .collect();
        frame.render_widget(Paragraph::new(lines), inner);
    }
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() > n {
        s.chars().take(n).collect()
    } else {
        s.to_string()
    }
}
