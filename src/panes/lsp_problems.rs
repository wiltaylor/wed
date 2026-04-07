//! Bottom-panel pane that lists LSP diagnostics for the active buffer.

use crossterm::event::{KeyCode, KeyEvent};
use lsp_types::{Diagnostic, DiagnosticSeverity};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::layout::Pane;

pub struct LspProblemsPane {
    /// (line, col, severity, message) — refreshed each frame from the
    /// host before render via `refresh_from_diagnostics`.
    entries: Vec<(usize, usize, Option<DiagnosticSeverity>, String)>,
    selected: usize,
    /// Set when the user presses Enter on a problem; the host reads it
    /// via `take_jump_target` and applies it to the active buffer's cursor.
    pending_jump: Option<(usize, usize)>,
}

impl Default for LspProblemsPane {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            selected: 0,
            pending_jump: None,
        }
    }
}

impl LspProblemsPane {
    pub fn refresh_from_diagnostics(&mut self, diags: &[Diagnostic]) {
        self.entries = diags
            .iter()
            .map(|d| {
                (
                    d.range.start.line as usize,
                    d.range.start.character as usize,
                    d.severity,
                    d.message
                        .lines()
                        .next()
                        .unwrap_or("")
                        .to_string(),
                )
            })
            .collect();
        // Stable sort by (line, col).
        self.entries.sort_by_key(|(l, c, _, _)| (*l, *c));
        if self.selected >= self.entries.len() {
            self.selected = self.entries.len().saturating_sub(1);
        }
    }
}

impl Pane for LspProblemsPane {
    fn name(&self) -> &str {
        "problems"
    }
    fn title(&self) -> &str {
        "Problems"
    }

    fn refresh_diagnostics(&mut self, diags: &[Diagnostic]) {
        self.refresh_from_diagnostics(diags);
    }

    fn render(&self, frame: &mut Frame<'_>, area: Rect) {
        if area.width == 0 || area.height == 0 {
            return;
        }
        if self.entries.is_empty() {
            let p = Paragraph::new(Line::from(Span::styled(
                "no problems",
                Style::default().fg(Color::DarkGray),
            )));
            frame.render_widget(p, area);
            return;
        }
        let mut lines: Vec<Line> = Vec::with_capacity(self.entries.len());
        for (i, (line, col, sev, msg)) in self.entries.iter().enumerate() {
            let (label, color) = match sev {
                Some(DiagnosticSeverity::ERROR) => ("E", Color::Red),
                Some(DiagnosticSeverity::WARNING) => ("W", Color::Yellow),
                Some(DiagnosticSeverity::INFORMATION) => ("I", Color::Cyan),
                Some(DiagnosticSeverity::HINT) => ("H", Color::Gray),
                _ => ("?", Color::White),
            };
            let prefix = format!("{label} {}:{}  ", line + 1, col + 1);
            let mut spans = vec![
                Span::styled(prefix, Style::default().fg(color).add_modifier(Modifier::BOLD)),
                Span::raw(msg.clone()),
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
        // Scroll so the selected row is visible.
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
                if let Some((line, col, _, _)) = self.entries.get(self.selected) {
                    self.pending_jump = Some((*line, *col));
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.selected + 1 < self.entries.len() {
                    self.selected += 1;
                }
                if let Some((line, col, _, _)) = self.entries.get(self.selected) {
                    self.pending_jump = Some((*line, *col));
                }
            }
            KeyCode::Enter => {
                if let Some((line, col, _, _)) = self.entries.get(self.selected) {
                    self.pending_jump = Some((*line, *col));
                }
            }
            _ => {}
        }
    }

    fn row_count(&self) -> usize {
        self.entries.len()
    }

    fn select_row(&mut self, row: usize) {
        if row < self.entries.len() {
            self.selected = row;
        }
    }

    fn activate_selected(&mut self) {
        if let Some((line, col, _, _)) = self.entries.get(self.selected) {
            self.pending_jump = Some((*line, *col));
        }
    }

    fn take_jump_target(&mut self) -> Option<(usize, usize)> {
        self.pending_jump.take()
    }
}
