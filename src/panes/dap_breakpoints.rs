use crate::layout::Pane;
use async_trait::async_trait;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Breakpoint {
    pub path: PathBuf,
    pub line: usize,
    pub enabled: bool,
}

#[derive(Default)]
pub struct DapBreakpointsPane {
    pub breakpoints: Vec<Breakpoint>,
    pub selected: usize,
    pending_jump: Option<(usize, usize)>,
}

impl DapBreakpointsPane {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn set_breakpoints(&mut self, mut bps: Vec<Breakpoint>) {
        bps.sort_by(|a, b| a.path.cmp(&b.path).then(a.line.cmp(&b.line)));
        self.breakpoints = bps;
        if self.selected >= self.breakpoints.len() {
            self.selected = self.breakpoints.len().saturating_sub(1);
        }
    }
}

#[async_trait]
impl Pane for DapBreakpointsPane {
    fn name(&self) -> &str {
        "dap_breakpoints"
    }
    fn title(&self) -> &str {
        "Breakpoints"
    }

    fn render(&self, frame: &mut Frame<'_>, area: Rect) {
        if area.width == 0 || area.height == 0 {
            return;
        }
        if self.breakpoints.is_empty() {
            let p = Paragraph::new(Line::from(Span::styled(
                "no breakpoints",
                Style::default().fg(Color::DarkGray),
            )));
            frame.render_widget(p, area);
            return;
        }
        let mut lines: Vec<Line> = Vec::with_capacity(self.breakpoints.len());
        for (i, bp) in self.breakpoints.iter().enumerate() {
            let marker = if bp.enabled { "●" } else { "○" };
            let prefix = format!(
                "{marker} {}:{}  ",
                bp.path.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default(),
                bp.line + 1
            );
            let mut spans = vec![
                Span::styled(prefix, Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
                Span::raw(bp.path.display().to_string()),
            ];
            let style = if i == self.selected {
                Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            for s in &mut spans {
                s.style = s.style.patch(style);
            }
            lines.push(Line::from(spans));
        }
        let visible = area.height as usize;
        let start = if self.selected >= visible {
            self.selected + 1 - visible
        } else {
            0
        };
        let view: Vec<Line> = lines.into_iter().skip(start).take(visible).collect();
        frame.render_widget(Paragraph::new(view), area);
    }

    fn handle_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                if self.selected > 0 {
                    self.selected -= 1;
                }
                if let Some(b) = self.breakpoints.get(self.selected) {
                    self.pending_jump = Some((b.line, 0));
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.selected + 1 < self.breakpoints.len() {
                    self.selected += 1;
                }
                if let Some(b) = self.breakpoints.get(self.selected) {
                    self.pending_jump = Some((b.line, 0));
                }
            }
            KeyCode::Enter => {
                if let Some(b) = self.breakpoints.get(self.selected) {
                    self.pending_jump = Some((b.line, 0));
                }
            }
            _ => {}
        }
    }

    fn row_count(&self) -> usize {
        self.breakpoints.len()
    }
    fn select_row(&mut self, row: usize) {
        if row < self.breakpoints.len() {
            self.selected = row;
        }
    }
    fn activate_selected(&mut self) {
        if let Some(b) = self.breakpoints.get(self.selected) {
            self.pending_jump = Some((b.line, 0));
        }
    }
    fn take_jump_target(&mut self) -> Option<(usize, usize)> {
        self.pending_jump.take()
    }
    fn as_any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        Some(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sorted_by_line() {
        let mut p = DapBreakpointsPane::new();
        p.set_breakpoints(vec![
            Breakpoint { path: PathBuf::from("a"), line: 5, enabled: true },
            Breakpoint { path: PathBuf::from("a"), line: 2, enabled: true },
            Breakpoint { path: PathBuf::from("a"), line: 9, enabled: false },
        ]);
        let lines: Vec<_> = p.breakpoints.iter().map(|b| b.line).collect();
        assert_eq!(lines, vec![2, 5, 9]);
    }
}
